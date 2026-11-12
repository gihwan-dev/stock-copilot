mod lexer;
mod money;
mod rules;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use cargo_metadata::MetadataCommand;
use proc_macro2::{Literal, TokenStream, TokenTree};

use crate::check_support::{Finding, read_text, report, string_value, workspace_root};
use money::{MoneyNames, money_rules};
use rules::{ASSEMBLY_MACROS, Recognition, Rule, recognize};

pub(crate) const STORE_DIRECTORY: &str = "crates/adapters/store-sqlite/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    Store,
    Elsewhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Initializer {
    Constant,
    Other,
}

fn place_of(path: &str) -> Place {
    if path.starts_with(STORE_DIRECTORY) {
        Place::Store
    } else {
        Place::Elsewhere
    }
}

fn ident_text(tree: Option<&TokenTree>) -> Option<String> {
    match tree {
        Some(TokenTree::Ident(ident)) => Some(ident.to_string()),
        Some(TokenTree::Group(_) | TokenTree::Punct(_) | TokenTree::Literal(_)) | None => None,
    }
}

fn punct_char(tree: Option<&TokenTree>) -> Option<char> {
    match tree {
        Some(TokenTree::Punct(punct)) => Some(punct.as_char()),
        Some(TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Literal(_)) | None => None,
    }
}

fn is_literal(tree: Option<&TokenTree>) -> bool {
    match tree {
        Some(TokenTree::Literal(_)) => true,
        Some(TokenTree::Group(_) | TokenTree::Ident(_) | TokenTree::Punct(_)) | None => false,
    }
}

fn constant_initializer(trees: &[TokenTree], index: usize) -> Option<usize> {
    let name = index.checked_add(1)?;
    let colon = index.checked_add(2)?;
    let declares = ident_text(trees.get(index)).as_deref() == Some("const")
        && ident_text(trees.get(name)).is_some_and(|text| text != "fn")
        && punct_char(trees.get(colon)) == Some(':');
    if !declares {
        return None;
    }
    let equals = (colon..trees.len())
        .find(|candidate| {
            matches_char(trees, *candidate, '=') || matches_char(trees, *candidate, ';')
        })
        .filter(|position| matches_char(trees, *position, '='))?;
    let value = equals.checked_add(1)?;
    let end = equals.checked_add(2)?;
    (is_literal(trees.get(value)) && matches_char(trees, end, ';')).then_some(value)
}

fn matches_char(trees: &[TokenTree], index: usize, symbol: char) -> bool {
    punct_char(trees.get(index)) == Some(symbol)
}

fn initializer_at(constants: &BTreeSet<usize>, index: usize) -> Initializer {
    if constants.contains(&index) {
        Initializer::Constant
    } else {
        Initializer::Other
    }
}

struct Scan<'a> {
    path: &'a str,
    place: Place,
    names: &'a MoneyNames,
    findings: Vec<Finding<Rule>>,
}

impl Scan<'_> {
    fn literal(&mut self, literal: &Literal, initializer: Initializer) {
        let Some(value) = string_value(literal) else {
            return;
        };
        let line = literal.span().start().line;
        match self.place {
            Place::Store => {
                for rule in money_rules(&value, self.names) {
                    self.findings.push(Finding::at_line(self.path, line, rule));
                }
                if recognize(&value) == Recognition::Sql && initializer == Initializer::Other {
                    self.findings
                        .push(Finding::at_line(self.path, line, Rule::NotConstant));
                }
            }
            Place::Elsewhere => {
                if recognize(&value) == Recognition::Sql {
                    self.findings
                        .push(Finding::at_line(self.path, line, Rule::OutsideStore));
                }
            }
        }
    }

    fn assembly(&mut self, trees: &[TokenTree], index: usize) {
        let Some(TokenTree::Ident(ident)) = trees.get(index) else {
            return;
        };
        let name = ident.to_string();
        let invoked = index
            .checked_add(1)
            .is_some_and(|next| matches_char(trees, next, '!'));
        if self.place == Place::Store && invoked && ASSEMBLY_MACROS.contains(&name.as_str()) {
            let line = ident.span().start().line;
            self.findings
                .push(Finding::at_line(self.path, line, Rule::Assembly(name)));
        }
    }

    fn stream(&mut self, stream: TokenStream) {
        let trees: Vec<TokenTree> = stream.into_iter().collect();
        let constants: BTreeSet<usize> = (0..trees.len())
            .filter_map(|index| constant_initializer(&trees, index))
            .collect();
        for (index, tree) in trees.iter().enumerate() {
            match tree {
                TokenTree::Group(group) => self.stream(group.stream()),
                TokenTree::Literal(literal) => {
                    self.literal(literal, initializer_at(&constants, index));
                }
                TokenTree::Ident(_) => self.assembly(&trees, index),
                TokenTree::Punct(_) => {}
            }
        }
    }
}

pub(crate) fn check_file(path: &str, text: &str, names: &MoneyNames) -> Vec<Finding<Rule>> {
    let place = place_of(path);
    if Path::new(path)
        .extension()
        .is_some_and(|extension| extension == "sql")
    {
        return match place {
            Place::Store => money_rules(text, names)
                .into_iter()
                .map(|rule| Finding::whole_file(path, rule))
                .collect(),
            Place::Elsewhere => vec![Finding::whole_file(path, Rule::OutsideStore)],
        };
    }
    let Ok(stream) = text.parse::<TokenStream>() else {
        return vec![Finding::whole_file(path, Rule::Unparsable)];
    };
    let mut scan = Scan {
        path,
        place,
        names,
        findings: Vec::new(),
    };
    scan.stream(stream);
    scan.findings
}

fn collect_files(directory: &Path, files: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    let listing =
        fs::read_dir(directory).map_err(|error| format!("{}: {error}", directory.display()))?;
    for listed in listing {
        let entry = listed.map_err(|error| format!("{}: {error}", directory.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let checked = path
            .extension()
            .is_some_and(|extension| extension == "rs" || extension == "sql");
        if file_type.is_dir() {
            collect_files(&path, files)?;
        } else if !file_type.is_symlink() && checked {
            files.push(path);
        }
    }
    Ok(())
}

pub(crate) fn check_repository(
    root: &Path,
    names: &MoneyNames,
) -> Result<BTreeSet<Finding<Rule>>, String> {
    let metadata = MetadataCommand::new()
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .other_options(vec![String::from("--locked")])
        .exec()
        .map_err(|error| format!("cargo metadata: {error}"))?;
    let mut files = Vec::new();
    for package in metadata.workspace_packages() {
        if let Some(directory) = package.manifest_path.as_std_path().parent() {
            collect_files(directory, &mut files)?;
        }
    }
    let mut findings = BTreeSet::new();
    for file in files {
        let relative = file
            .strip_prefix(root)
            .ok()
            .and_then(Path::to_str)
            .map(String::from)
            .ok_or_else(|| format!("{}: 저장소 밖의 경로", file.display()))?;
        findings.extend(check_file(&relative, &read_text(&file)?, names));
    }
    Ok(findings)
}

pub(crate) fn command() -> ExitCode {
    let outcome = workspace_root().and_then(|root| {
        let names = MoneyNames::load(&root)?;
        check_repository(&root, &names)
    });
    report("sql-check", outcome)
}
