use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::git_base::{git, output};
use crate::lint_file::targets::{Coverage, Targets};
use crate::workspace_sources::{SourceFailure, io_failure};

use super::Row;
use super::script_comments::comments;

pub(crate) const SCRIPT_RULES_FILE: &str = "harness/disable-control/typescript.toml";

const SCRIPT_RULES_KEYS: [&str; 3] = ["example-bundles", "extensions", "regex-keywords"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Directive {
    DisableNextLine,
    DisableLine,
    Disable,
    Enable,
    InlineConfig,
    TsIgnore,
    TsExpectError,
    TsNocheck,
}

pub(crate) const DIRECTIVES: [Directive; 8] = [
    Directive::DisableNextLine,
    Directive::DisableLine,
    Directive::Disable,
    Directive::Enable,
    Directive::InlineConfig,
    Directive::TsIgnore,
    Directive::TsExpectError,
    Directive::TsNocheck,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Described {
    RuleAndReason,
    ReasonOnly,
}

impl Directive {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::DisableNextLine => "eslint-disable-next-line",
            Self::DisableLine => "eslint-disable-line",
            Self::Disable => "eslint-disable",
            Self::Enable => "eslint-enable",
            Self::InlineConfig => "eslint",
            Self::TsIgnore => "@ts-ignore",
            Self::TsExpectError => "@ts-expect-error",
            Self::TsNocheck => "@ts-nocheck",
        }
    }

    const fn unlinted(self) -> &'static str {
        match self {
            Self::DisableNextLine => "eslint-disable-next-line (ESLint 밖)",
            Self::DisableLine => "eslint-disable-line (ESLint 밖)",
            Self::Disable => "eslint-disable (ESLint 밖)",
            Self::Enable => "eslint-enable (ESLint 밖)",
            Self::InlineConfig => "eslint (ESLint 밖)",
            Self::TsIgnore => "@ts-ignore (ESLint 밖)",
            Self::TsExpectError => "@ts-expect-error (ESLint 밖)",
            Self::TsNocheck => "@ts-nocheck (ESLint 밖)",
        }
    }

    pub(crate) const fn kind(self, coverage: Coverage) -> &'static str {
        match coverage {
            Coverage::Linted => self.name(),
            Coverage::Unlinted => self.unlinted(),
        }
    }

    const fn described(self) -> Described {
        match self {
            Self::DisableNextLine
            | Self::DisableLine
            | Self::Disable
            | Self::Enable
            | Self::InlineConfig => Described::RuleAndReason,
            Self::TsIgnore | Self::TsExpectError | Self::TsNocheck => Described::ReasonOnly,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Found {
    pub(crate) line: usize,
    pub(crate) directive: Directive,
    pub(crate) rule: String,
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptRules {
    pub(crate) extensions: BTreeSet<String>,
    pub(crate) example_bundles: BTreeSet<String>,
    pub(crate) regex_keywords: BTreeSet<String>,
}

fn names(table: &toml::Table, key: &str) -> Result<BTreeSet<String>, SourceFailure> {
    table
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| SourceFailure(format!("{SCRIPT_RULES_FILE}: {key} 목록이 없다")))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .filter(|name| !name.trim().is_empty())
                .map(String::from)
                .ok_or_else(|| {
                    SourceFailure(format!("{SCRIPT_RULES_FILE}: {key}의 원소는 문자열이다"))
                })
        })
        .collect()
}

impl ScriptRules {
    pub(crate) fn parse(table: &toml::Table) -> Result<Self, SourceFailure> {
        if let Some(unknown) = table
            .keys()
            .find(|key| !SCRIPT_RULES_KEYS.contains(&key.as_str()))
        {
            return Err(SourceFailure(format!(
                "{SCRIPT_RULES_FILE}: 모르는 키 {unknown}"
            )));
        }
        Ok(Self {
            extensions: names(table, "extensions")?,
            example_bundles: names(table, "example-bundles")?,
            regex_keywords: names(table, "regex-keywords")?,
        })
    }

    pub(crate) fn load(root: &Path) -> Result<Self, SourceFailure> {
        let path = root.join(SCRIPT_RULES_FILE);
        let text = fs::read_to_string(&path).map_err(|error| io_failure(&path, &error))?;
        let table: toml::Table = text.parse().map_err(|error: toml::de::Error| {
            SourceFailure(format!("{SCRIPT_RULES_FILE}: {error}"))
        })?;
        Self::parse(&table)
    }

    pub(crate) fn scanned(&self, path: &str) -> Option<()> {
        let extension = Path::new(path).extension()?.to_str()?;
        let bundled = self
            .example_bundles
            .iter()
            .any(|bundle| path.starts_with(bundle.as_str()));
        (self.extensions.contains(extension) && !bundled).then_some(())
    }
}

fn separated(rest: &str) -> (String, String) {
    for (index, _) in rest.match_indices("--") {
        let (Some(before), Some(tail)) = (rest.get(..index), rest.get(index..)) else {
            continue;
        };
        let after = tail.trim_start_matches('-');
        let spaced_before = before.is_empty() || before.ends_with(char::is_whitespace);
        let spaced_after = after.is_empty() || after.starts_with(char::is_whitespace);
        if spaced_before && spaced_after {
            return (String::from(before.trim()), String::from(after.trim()));
        }
    }
    (String::from(rest.trim()), String::new())
}

fn directive(text: &str) -> Option<(Directive, &str)> {
    let body = text.trim_start_matches(|next: char| next.is_whitespace() || next == '*');
    DIRECTIVES.into_iter().find_map(|candidate| {
        body.strip_prefix(candidate.name())
            .filter(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
            .map(|rest| (candidate, rest))
    })
}

pub(crate) fn directives(source: &str, rules: &ScriptRules) -> Vec<Found> {
    comments(source, &rules.regex_keywords)
        .into_iter()
        .filter_map(|comment| {
            let (found, rest) = directive(&comment.text)?;
            let (rule, reason) = match found.described() {
                Described::RuleAndReason => separated(rest),
                Described::ReasonOnly => (String::new(), String::from(rest.trim())),
            };
            Some(Found {
                line: comment.line,
                directive: found,
                rule,
                reason,
            })
        })
        .collect()
}

pub(crate) fn script_rows(
    path: &str,
    source: &str,
    rules: &ScriptRules,
    coverage: &dyn Fn(&str) -> Coverage,
) -> Vec<Row> {
    if rules.scanned(path).is_none() {
        return Vec::new();
    }
    let found = directives(source, rules);
    if found.is_empty() {
        return Vec::new();
    }
    let covered = coverage(path);
    found
        .into_iter()
        .map(|item| Row {
            kind: item.directive.kind(covered),
            path: String::from(path),
            line: item.line,
            rule: item.rule,
            reason: item.reason,
        })
        .collect()
}

pub(crate) fn listed_files(root: &Path) -> Result<Vec<String>, SourceFailure> {
    let mut command = git(root);
    command.args([
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ]);
    let listing = output(command, "ls-files").map_err(SourceFailure)?;
    let mut paths: Vec<String> = listing
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
        .map(|name| String::from_utf8_lossy(name).into_owned())
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub(crate) fn repository_script_rows(
    root: &Path,
    rules: &ScriptRules,
    targets: &Targets,
) -> Result<Vec<Row>, SourceFailure> {
    let coverage = |path: &str| targets.eslint_coverage(root, path);
    let mut found = Vec::new();
    for path in listed_files(root)? {
        let full = root.join(&path);
        if rules.scanned(&path).is_none() || !full.is_file() {
            continue;
        }
        let text = fs::read_to_string(&full).map_err(|error| io_failure(&full, &error))?;
        found.extend(script_rows(&path, &text, rules, &coverage));
    }
    found.sort();
    Ok(found)
}
