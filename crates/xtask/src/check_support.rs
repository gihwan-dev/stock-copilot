use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cargo_metadata::MetadataCommand;
use proc_macro2::{Literal, TokenStream, TokenTree};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Location {
    WholeFile,
    Line(usize),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Finding<R> {
    pub(crate) path: String,
    pub(crate) location: Location,
    pub(crate) rule: R,
}

impl<R> Finding<R> {
    pub(crate) fn whole_file(path: &str, rule: R) -> Self {
        Self {
            path: String::from(path),
            location: Location::WholeFile,
            rule,
        }
    }

    pub(crate) fn at_line(path: &str, line: usize, rule: R) -> Self {
        Self {
            path: String::from(path),
            location: Location::Line(line),
            rule,
        }
    }
}

impl<R: fmt::Display> fmt::Display for Finding<R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.location {
            Location::WholeFile => write!(f, "{}: {}", self.path, self.rule),
            Location::Line(line) => write!(f, "{}:{line}: {}", self.path, self.rule),
        }
    }
}

pub(crate) fn workspace_root() -> Result<PathBuf, String> {
    MetadataCommand::new()
        .no_deps()
        .other_options(vec![String::from("--locked")])
        .exec()
        .map(|metadata| metadata.workspace_root.into_std_path_buf())
        .map_err(|error| format!("cargo metadata: {error}"))
}

pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))
}

pub(crate) fn report<R: fmt::Display>(
    name: &str,
    outcome: Result<BTreeSet<Finding<R>>, String>,
) -> ExitCode {
    match print_outcome(name, outcome) {
        Ok(code) => code,
        Err(_) => ExitCode::FAILURE,
    }
}

fn print_outcome<R: fmt::Display>(
    name: &str,
    outcome: Result<BTreeSet<Finding<R>>, String>,
) -> io::Result<ExitCode> {
    match outcome {
        Ok(findings) if findings.is_empty() => {
            writeln!(io::stdout().lock(), "{name}: 위반 없음")?;
            Ok(ExitCode::SUCCESS)
        }
        Ok(findings) => {
            let mut output = io::stderr().lock();
            for finding in &findings {
                writeln!(output, "{finding}")?;
            }
            writeln!(output, "{name}: 위반 {}건", findings.len())?;
            Ok(ExitCode::FAILURE)
        }
        Err(failure) => {
            writeln!(io::stderr().lock(), "{name}: {failure}")?;
            Ok(ExitCode::FAILURE)
        }
    }
}

pub(crate) fn string_value(literal: &Literal) -> Option<String> {
    let stream = TokenStream::from(TokenTree::Literal(literal.clone()));
    if let Ok(text) = syn::parse2::<syn::LitStr>(stream.clone()) {
        return Some(text.value());
    }
    if let Ok(bytes) = syn::parse2::<syn::LitByteStr>(stream.clone()) {
        return Some(String::from_utf8_lossy(&bytes.value()).into_owned());
    }
    syn::parse2::<syn::LitCStr>(stream)
        .ok()
        .map(|text| text.value().to_string_lossy().into_owned())
}
