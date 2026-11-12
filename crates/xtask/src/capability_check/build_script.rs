use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use proc_macro2::{Delimiter, TokenStream, TokenTree};

use super::{COMMAND_LIST_FILE, Findings, Rule, valid_command};
use crate::check_support::string_value;

const BUILD_SCRIPT: &str = "build.rs";
const MANIFEST_TYPE: &str = "AppManifest";
const COMMANDS_METHOD: &str = "commands";

#[derive(Debug, Default)]
struct Contents {
    idents: BTreeSet<String>,
    strings: Vec<String>,
    array_arguments: usize,
}

fn contains_bracket(stream: TokenStream) -> bool {
    stream.into_iter().any(|tree| match tree {
        TokenTree::Group(group) => {
            group.delimiter() == Delimiter::Bracket || contains_bracket(group.stream())
        }
        TokenTree::Ident(_) | TokenTree::Punct(_) | TokenTree::Literal(_) => false,
    })
}

impl Contents {
    fn ident(&mut self, name: String, arguments: Option<&TokenTree>) {
        if name == COMMANDS_METHOD
            && let Some(TokenTree::Group(group)) = arguments
            && group.delimiter() == Delimiter::Parenthesis
            && contains_bracket(group.stream())
        {
            self.array_arguments = self.array_arguments.saturating_add(1);
        }
        self.idents.insert(name);
    }

    fn collect(&mut self, stream: TokenStream) {
        let trees: Vec<TokenTree> = stream.into_iter().collect();
        for (index, tree) in trees.iter().enumerate() {
            match tree {
                TokenTree::Group(group) => self.collect(group.stream()),
                TokenTree::Ident(ident) => {
                    let arguments = index.checked_add(1).and_then(|next| trees.get(next));
                    self.ident(ident.to_string(), arguments);
                }
                TokenTree::Literal(literal) => self.strings.extend(string_value(literal)),
                TokenTree::Punct(_) => {}
            }
        }
    }
}

pub(super) fn build_script_findings(
    directory: &Path,
    commands: &[String],
    findings: &mut Findings,
) {
    let Ok(text) = fs::read_to_string(directory.join(BUILD_SCRIPT)) else {
        if !commands.is_empty() {
            findings.add(BUILD_SCRIPT, Rule::BuildNotReadingList);
        }
        return;
    };
    let Ok(stream) = text.parse::<TokenStream>() else {
        findings.add(BUILD_SCRIPT, Rule::Unparsable);
        return;
    };
    let mut contents = Contents::default();
    contents.collect(stream);
    let hand_written = contents.strings.iter().any(|value| valid_command(value));
    if hand_written || contents.array_arguments > 0 {
        findings.add(BUILD_SCRIPT, Rule::BuildHandWrittenCommands);
    }
    let reads_list = contents
        .strings
        .iter()
        .any(|value| value == COMMAND_LIST_FILE)
        && contents.idents.contains(MANIFEST_TYPE)
        && contents.idents.contains(COMMANDS_METHOD);
    if !commands.is_empty() && !reads_list {
        findings.add(BUILD_SCRIPT, Rule::BuildNotReadingList);
    }
}
