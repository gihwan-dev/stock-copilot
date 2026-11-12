use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, ExprMethodCall, Macro, Token};

use super::{Findings, Rule, command_list_names};

pub(crate) const EXPECTED_BUILD_SCRIPT_FILE: &str = "harness/capability-check/build.rs.txt";
const BUILD_SCRIPT: &str = "build.rs";
const SOURCE_DIRECTORY: &str = "src";
const COLLECT_MACRO: &str = "collect_commands";
const HANDLER_MACRO: &str = "generate_handler";
const HANDLER_METHOD: &str = "invoke_handler";

#[derive(Debug, Default)]
struct Registrations {
    collected: Vec<Vec<String>>,
    hand_written: usize,
    unparsable: usize,
}

fn last_segment(path: &syn::Path) -> Option<String> {
    path.segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn is_specta_handler(argument: &Expr) -> bool {
    if let Expr::MethodCall(call) = argument {
        call.method == HANDLER_METHOD && call.args.is_empty()
    } else {
        false
    }
}

impl<'ast> Visit<'ast> for Registrations {
    fn visit_macro(&mut self, i: &'ast Macro) {
        match last_segment(&i.path).as_deref() {
            Some(COLLECT_MACRO) => {
                let parser = Punctuated::<syn::Path, Token![,]>::parse_terminated;
                match i.parse_body_with(parser) {
                    Ok(paths) => self
                        .collected
                        .push(paths.iter().filter_map(last_segment).collect()),
                    Err(_) => self.unparsable = self.unparsable.saturating_add(1),
                }
            }
            Some(HANDLER_MACRO) => self.hand_written = self.hand_written.saturating_add(1),
            Some(_) | None => {}
        }
        visit::visit_macro(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &'ast ExprMethodCall) {
        if i.method == HANDLER_METHOD && !i.args.is_empty() && !i.args.iter().all(is_specta_handler)
        {
            self.hand_written = self.hand_written.saturating_add(1);
        }
        visit::visit_expr_method_call(self, i);
    }
}

fn rust_files(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(listing) = fs::read_dir(directory) else {
        return;
    };
    let mut entries: Vec<PathBuf> = listing
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            rust_files(&entry, found);
        } else if entry.extension().is_some_and(|extension| extension == "rs") {
            found.push(entry);
        }
    }
}

fn registrations(directory: &Path, findings: &mut Findings) -> Registrations {
    let mut files = Vec::new();
    rust_files(&directory.join(SOURCE_DIRECTORY), &mut files);
    let mut found = Registrations::default();
    for file in &files {
        let parsed = fs::read_to_string(file)
            .ok()
            .and_then(|text| syn::parse_file(&text).ok());
        if let Some(syntax) = parsed {
            found.visit_file(&syntax);
        } else {
            let relative = file.strip_prefix(directory).map_or_else(
                |_| file.display().to_string(),
                |path| path.display().to_string(),
            );
            findings.add(&relative, Rule::Unparsable);
        }
    }
    found
}

pub(super) fn registration_findings(directory: &Path, findings: &mut Findings) {
    let listed: BTreeSet<String> = command_list_names(directory).into_iter().collect();
    let found = registrations(directory, findings);
    if found.hand_written > 0 {
        findings.add(SOURCE_DIRECTORY, Rule::HandWrittenHandler);
    }
    if found.unparsable > 0 {
        findings.add(SOURCE_DIRECTORY, Rule::Unparsable);
    }
    match found.collected.as_slice() {
        [] if listed.is_empty() => {}
        [only] => {
            let registered: BTreeSet<String> = only.iter().cloned().collect();
            if registered.len() != only.len() || registered != listed {
                findings.add(SOURCE_DIRECTORY, Rule::RegistrationMismatch);
            }
        }
        [] | [_, _, ..] => findings.add(SOURCE_DIRECTORY, Rule::RegistrationCount),
    }
}

pub(super) fn build_body_findings(directory: &Path, expected: &str, findings: &mut Findings) {
    let actual = fs::read_to_string(directory.join(BUILD_SCRIPT)).ok();
    if actual.as_deref() != Some(expected) {
        findings.add(BUILD_SCRIPT, Rule::BuildBody);
    }
}
