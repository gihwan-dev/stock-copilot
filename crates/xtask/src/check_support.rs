use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cargo_metadata::MetadataCommand;
use proc_macro2::Literal;

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

fn unescape(body: &str) -> String {
    let mut text = String::new();
    let mut rest = body.chars().peekable();
    while let Some(next) = rest.next() {
        if next != '\\' {
            text.push(next);
            continue;
        }
        match rest.next() {
            Some('n') => text.push('\n'),
            Some('r') => text.push('\r'),
            Some('t') => text.push('\t'),
            Some('0') => text.push('\0'),
            Some('\n') => {
                while rest
                    .next_if(|candidate| candidate.is_whitespace())
                    .is_some()
                {}
            }
            Some(other) => text.push(other),
            None => {}
        }
    }
    text
}

fn raw_body(rest: &str) -> Option<String> {
    rest.trim_start_matches('#')
        .strip_prefix('"')
        .map(|body| body.trim_end_matches('#'))
        .and_then(|body| body.strip_suffix('"'))
        .map(String::from)
}

pub(crate) fn string_value(literal: &Literal) -> Option<String> {
    let source = literal.to_string();
    let unprefixed = ["b", "c"]
        .iter()
        .find_map(|prefix| {
            source
                .strip_prefix(prefix)
                .filter(|rest| rest.starts_with('"') || rest.starts_with('r'))
        })
        .unwrap_or(source.as_str());
    if let Some(rest) = unprefixed.strip_prefix('r') {
        return raw_body(rest);
    }
    unprefixed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .map(unescape)
}
