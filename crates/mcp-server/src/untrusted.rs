use domain::UntrustedText;
use serde_json::{Map, Value};

pub const UNTRUSTED_TEXT_FIELD: &str = "untrusted_text";
pub const DEFAULT_UNTRUSTED_TEXT_CHARS: usize = 4_000_usize;
pub const MAX_UNTRUSTED_TEXT_CHARS: usize = 20_000_usize;
const MAX_FLATTEN_ROUNDS: usize = 16_usize;

const LINK_SCHEMES: [&str; 8] = [
    "http:",
    "https:",
    "ftp:",
    "file:",
    "data:",
    "javascript:",
    "vbscript:",
    "mailto:",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UntrustedTextLimitError {
    OutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UntrustedTextLimit(usize);

impl UntrustedTextLimit {
    pub const fn try_new(chars: usize) -> Result<Self, UntrustedTextLimitError> {
        if chars == 0_usize || chars > MAX_UNTRUSTED_TEXT_CHARS {
            return Err(UntrustedTextLimitError::OutOfRange);
        }
        Ok(Self(chars))
    }

    #[must_use]
    pub const fn chars(self) -> usize {
        self.0
    }
}

impl Default for UntrustedTextLimit {
    fn default() -> Self {
        Self(DEFAULT_UNTRUSTED_TEXT_CHARS)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truncation {
    Complete,
    Truncated,
}

#[derive(Clone, PartialEq, Eq)]
pub struct NormalizedUntrustedText {
    text: String,
    truncation: Truncation,
}

impl std::fmt::Debug for NormalizedUntrustedText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NormalizedUntrustedText")
            .field("chars", &self.text.chars().count())
            .field("truncation", &self.truncation)
            .finish()
    }
}

impl NormalizedUntrustedText {
    #[must_use]
    pub fn from_untrusted(source: &UntrustedText, limit: UntrustedTextLimit) -> Self {
        Self::from_raw(source.as_untrusted_str(), limit)
    }

    #[must_use]
    pub fn from_raw(raw: &str, limit: UntrustedTextLimit) -> Self {
        let mut current = strip_invisible(raw);
        for _round in 0_usize..MAX_FLATTEN_ROUNDS {
            let next = flatten_markdown_links(&strip_html_tags(&current));
            if next == current {
                break;
            }
            current = next;
        }
        let plain = drop_bare_links(&current);
        let total = plain.chars().count();
        if total <= limit.chars() {
            return Self {
                text: plain,
                truncation: Truncation::Complete,
            };
        }
        let clipped: String = plain.chars().take(limit.chars()).collect();
        Self {
            text: clipped.trim_end().to_owned(),
            truncation: Truncation::Truncated,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn truncation(&self) -> Truncation {
        self.truncation
    }

    #[must_use]
    pub fn into_value(self) -> Value {
        Value::String(self.text)
    }

    #[must_use]
    pub fn into_field_object(self) -> Value {
        let mut object = Map::new();
        object.insert(String::from(UNTRUSTED_TEXT_FIELD), Value::String(self.text));
        Value::Object(object)
    }
}

#[must_use]
pub fn untrusted_text_value(source: &UntrustedText, limit: UntrustedTextLimit) -> Value {
    NormalizedUntrustedText::from_untrusted(source, limit).into_value()
}

const REMOVED_RANGES: [(char, char); 19] = [
    ('\u{00AD}', '\u{00AD}'),
    ('\u{034F}', '\u{034F}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{115F}', '\u{115F}'),
    ('\u{1160}', '\u{1160}'),
    ('\u{17B4}', '\u{17B4}'),
    ('\u{17B5}', '\u{17B5}'),
    ('\u{180B}', '\u{180F}'),
    ('\u{200B}', '\u{200F}'),
    ('\u{2028}', '\u{202E}'),
    ('\u{2060}', '\u{206F}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{FE00}', '\u{FE0F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFA0}', '\u{FFA0}'),
    ('\u{FFF9}', '\u{FFFB}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0000}', '\u{E0FFF}'),
];

#[must_use]
pub fn is_removed_char(ch: char) -> bool {
    if ch == '\n' {
        return false;
    }
    ch.is_control()
        || REMOVED_RANGES
            .iter()
            .any(|(low, high)| (*low..=*high).contains(&ch))
}

fn strip_invisible(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            '\r' | '\t' | '\u{000B}' | '\u{000C}' | '\u{0085}' => ' ',
            other => other,
        })
        .filter(|ch| !is_removed_char(*ch))
        .collect()
}

fn strip_html_tags(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0_usize;
    while let Some(&ch) = chars.get(index) {
        if ch == '<' && looks_like_tag_start(chars.get(index.saturating_add(1_usize)).copied()) {
            let close = chars
                .iter()
                .skip(index)
                .position(|candidate| *candidate == '>');
            if let Some(offset) = close {
                out.push(' ');
                index = index.saturating_add(offset).saturating_add(1_usize);
                continue;
            }
        }
        out.push(ch);
        index = index.saturating_add(1_usize);
    }
    out
}

const fn looks_like_tag_start(next: Option<char>) -> bool {
    match next {
        Some(ch) => ch.is_ascii_alphabetic() || ch == '/' || ch == '!' || ch == '?',
        None => false,
    }
}

fn flatten_markdown_links(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0_usize;
    while let Some(&ch) = chars.get(index) {
        let label_start = match (ch, chars.get(index.saturating_add(1_usize))) {
            ('!', Some('[')) => Some(index.saturating_add(2_usize)),
            ('[', _) => Some(index.saturating_add(1_usize)),
            _ => None,
        };
        if let Some(start) = label_start
            && let Some((label, next)) = parse_link(&chars, start)
        {
            out.push_str(&label);
            index = next;
            continue;
        }
        out.push(ch);
        index = index.saturating_add(1_usize);
    }
    out
}

fn parse_link(chars: &[char], label_start: usize) -> Option<(String, usize)> {
    let label_len = chars
        .iter()
        .skip(label_start)
        .position(|candidate| *candidate == ']' || *candidate == '\n')?;
    let label_end = label_start.saturating_add(label_len);
    if chars.get(label_end) != Some(&']') {
        return None;
    }
    let after = label_end.saturating_add(1_usize);
    let target_close = match chars.get(after) {
        Some('(') => ')',
        Some('[') => ']',
        _ => return None,
    };
    let target_len = chars
        .iter()
        .skip(after.saturating_add(1_usize))
        .position(|candidate| *candidate == target_close || *candidate == '\n')?;
    let target_end = after.saturating_add(1_usize).saturating_add(target_len);
    if chars.get(target_end) != Some(&target_close) {
        return None;
    }
    let label: String = chars.get(label_start..label_end)?.iter().collect();
    Some((label, target_end.saturating_add(1_usize)))
}

fn is_link_token(token: &str) -> bool {
    let lower = token.to_lowercase();
    let trimmed = lower.trim_start_matches(|ch: char| !ch.is_alphanumeric());
    if lower.contains("://") || trimmed.starts_with("www.") {
        return true;
    }
    LINK_SCHEMES
        .iter()
        .any(|scheme| trimmed.starts_with(scheme) && trimmed.len() > scheme.len())
}

fn drop_bare_links(text: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in text.split('\n') {
        let words: Vec<&str> = line
            .split_whitespace()
            .filter(|token| !is_link_token(token))
            .collect();
        let joined = words.join(" ");
        let previous_blank = lines.last().is_some_and(String::is_empty);
        if joined.is_empty() && (lines.is_empty() || previous_blank) {
            continue;
        }
        lines.push(joined);
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}
