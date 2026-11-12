use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::git_base::listed_paths;
use crate::lint_file::targets::{Coverage, Targets};
use crate::workspace_sources::{SourceFailure, io_failure};

use super::Row;

pub(crate) const SCRIPT_RULES_FILE: &str = "harness/disable-control/typescript.toml";

const SCRIPT_RULES_KEYS: [&str; 2] = ["example-bundles", "extensions"];

pub(crate) const COMMENTS_SCRIPT: &str = "harness/disable-control/comments.mjs";

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
    pub(crate) applied: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScriptRules {
    pub(crate) extensions: BTreeSet<String>,
    pub(crate) example_bundles: BTreeSet<String>,
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

fn directive(line: &str) -> Option<(Directive, &str)> {
    let body =
        line.trim_start_matches(|next: char| next.is_whitespace() || next == '*' || next == '/');
    DIRECTIVES.into_iter().find_map(|candidate| {
        let name = candidate.name();
        let head = body.get(..name.len())?;
        let rest = body.get(name.len()..)?;
        (head.eq_ignore_ascii_case(name)
            && (rest.is_empty() || rest.starts_with(char::is_whitespace)))
        .then_some((candidate, rest))
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Comment {
    pub(crate) line: usize,
    pub(crate) end_line: usize,
    pub(crate) text: String,
}

fn json_string(text: &str) -> String {
    let mut quoted = String::from("\"");
    for next in text.chars() {
        match next {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            _ if u32::from(next) < 0x20 => {
                let code = u32::from(next);
                quoted.push_str("\\u00");
                for nibble in [code.checked_shr(4).unwrap_or(0), code & 0xf] {
                    quoted.push(char::from_digit(nibble, 16).unwrap_or('0'));
                }
            }
            _ => quoted.push(next),
        }
    }
    quoted.push('"');
    quoted
}

fn unescaped(text: &str) -> String {
    let mut plain = String::new();
    let mut chars = text.chars();
    while let Some(next) = chars.next() {
        if next != '\\' {
            plain.push(next);
            continue;
        }
        match chars.next() {
            Some('n') => plain.push('\n'),
            Some('r') => plain.push('\r'),
            Some('t') => plain.push('\t'),
            Some(other) => plain.push(other),
            None => {}
        }
    }
    plain
}

fn parsed_line(line: &str) -> Option<(usize, Comment)> {
    let mut fields = line.splitn(4, '\t');
    let index = fields.next()?.parse().ok()?;
    let start = fields.next()?.parse().ok()?;
    let end = fields.next()?.parse().ok()?;
    Some((
        index,
        Comment {
            line: start,
            end_line: end,
            text: unescaped(fields.next()?),
        },
    ))
}

pub(crate) fn comments_of(
    root: &Path,
    files: &[(String, String)],
) -> Result<Vec<Vec<Comment>>, SourceFailure> {
    let mut found: Vec<Vec<Comment>> = files.iter().map(|_| Vec::new()).collect();
    if files.is_empty() {
        return Ok(found);
    }
    let input = format!(
        "[{}]",
        files
            .iter()
            .map(|(name, text)| format!("[{},{}]", json_string(name), json_string(text)))
            .collect::<Vec<String>>()
            .join(",")
    );
    let mut child = Command::new("node")
        .arg(root.join(COMMENTS_SCRIPT))
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| SourceFailure(format!("node {COMMENTS_SCRIPT}: {error}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .map_err(|error| SourceFailure(format!("node {COMMENTS_SCRIPT}: {error}")))?;
    }
    let finished = child
        .wait_with_output()
        .map_err(|error| SourceFailure(format!("node {COMMENTS_SCRIPT}: {error}")))?;
    if !finished.status.success() {
        return Err(SourceFailure(format!(
            "node {COMMENTS_SCRIPT}: {}",
            String::from_utf8_lossy(&finished.stderr).trim()
        )));
    }
    for line in String::from_utf8_lossy(&finished.stdout).lines() {
        let (index, comment) = parsed_line(line)
            .ok_or_else(|| SourceFailure(format!("node {COMMENTS_SCRIPT}: 모르는 출력 {line}")))?;
        found
            .get_mut(index)
            .ok_or_else(|| {
                SourceFailure(format!("node {COMMENTS_SCRIPT}: 없는 파일 번호 {index}"))
            })?
            .push(comment);
    }
    Ok(found)
}

fn applied_line(source: &str, directive: Directive, line: usize, end_line: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let found = match directive {
        Directive::DisableLine => lines.get(line.saturating_sub(1)).copied(),
        Directive::DisableNextLine
        | Directive::Disable
        | Directive::Enable
        | Directive::InlineConfig
        | Directive::TsIgnore
        | Directive::TsExpectError
        | Directive::TsNocheck => lines
            .iter()
            .skip(end_line)
            .find(|text| !text.trim().is_empty())
            .copied(),
    };
    found.map_or_else(String::new, |text| String::from(text.trim()))
}

pub(crate) fn directives(source: &str, comments: &[Comment]) -> Vec<Found> {
    let mut found = Vec::new();
    for comment in comments {
        let lines: Vec<&str> = comment.text.split('\n').collect();
        for (offset, line) in lines.iter().enumerate() {
            let Some((matched, rest)) = directive(line) else {
                continue;
            };
            let following: Vec<&str> = lines
                .iter()
                .skip(offset.saturating_add(1))
                .copied()
                .collect();
            let mut whole = String::from(rest);
            for next in following {
                whole.push('\n');
                whole.push_str(next);
            }
            let (rule, reason) = match matched.described() {
                Described::RuleAndReason => separated(&whole),
                Described::ReasonOnly => (String::new(), String::from(whole.trim())),
            };
            found.push(Found {
                line: comment.line.saturating_add(offset),
                directive: matched,
                rule,
                reason,
                applied: applied_line(source, matched, comment.line, comment.end_line),
            });
        }
    }
    found
}

pub(crate) fn keyed_rows(
    path: &str,
    source: &str,
    comments: &[Comment],
    coverage: &dyn Fn(&str) -> Coverage,
) -> Vec<(Row, String)> {
    let found = directives(source, comments);
    if found.is_empty() {
        return Vec::new();
    }
    let covered = coverage(path);
    found
        .into_iter()
        .map(|item| {
            (
                Row {
                    kind: item.directive.kind(covered),
                    path: String::from(path),
                    line: item.line,
                    rule: item.rule,
                    reason: item.reason,
                },
                item.applied,
            )
        })
        .collect()
}

pub(crate) fn script_rows(
    root: &Path,
    files: &[(String, String)],
    rules: &ScriptRules,
    coverage: &dyn Fn(&str) -> Coverage,
) -> Result<Vec<(Row, String)>, SourceFailure> {
    let scanned: Vec<(String, String)> = files
        .iter()
        .filter(|(path, _)| rules.scanned(path).is_some())
        .cloned()
        .collect();
    let comments = comments_of(root, &scanned)?;
    Ok(scanned
        .iter()
        .zip(comments)
        .flat_map(|((path, text), found)| keyed_rows(path, text, &found, coverage))
        .collect())
}

pub(crate) fn listed_files(root: &Path) -> Result<Vec<String>, SourceFailure> {
    Ok(listed_paths(root)
        .map_err(SourceFailure)?
        .into_iter()
        .collect())
}

pub(crate) fn repository_files(
    root: &Path,
    rules: &ScriptRules,
) -> Result<Vec<(String, String)>, SourceFailure> {
    let mut files = Vec::new();
    for path in listed_files(root)? {
        let full = root.join(&path);
        if rules.scanned(&path).is_none() || !full.is_file() {
            continue;
        }
        let text = fs::read_to_string(&full).map_err(|error| io_failure(&full, &error))?;
        files.push((path, text));
    }
    Ok(files)
}

pub(crate) fn repository_script_rows(
    root: &Path,
    rules: &ScriptRules,
    targets: &Targets,
) -> Result<Vec<Row>, SourceFailure> {
    let coverage = |path: &str| targets.eslint_coverage(root, path);
    let files = repository_files(root, rules)?;
    let mut found: Vec<Row> = script_rows(root, &files, rules, &coverage)?
        .into_iter()
        .map(|(row, _)| row)
        .collect();
    found.sort();
    Ok(found)
}
