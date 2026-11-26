use std::collections::BTreeSet;

use domain::{InstrumentId, UntrustedText};

const GROUP_PREFIXES: [&str; 9] = ["SK", "LG", "롯데", "한화", "현대", "삼성", "CJ", "GS", "DB"];

const PARTICLES: [&str; 22] = [
    "에서", "에게", "으로", "까지", "부터", "보다", "처럼", "이라", "라고", "은", "는", "이", "가",
    "을", "를", "의", "에", "와", "과", "도", "로", "만",
];

const SECURITIES_CONTEXT: [&str; 10] = [
    "주가",
    "급등",
    "급락",
    "공시",
    "실적",
    "특징주",
    "상한가",
    "하한가",
    "목표가",
    "시총",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchNameKind {
    Official,
    Alias,
    English,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchNameAmbiguity {
    Distinct,
    CommonNoun,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchDictionaryEntry {
    instrument: InstrumentId,
    name: String,
    kind: MatchNameKind,
    ambiguity: MatchNameAmbiguity,
}

impl MatchDictionaryEntry {
    #[must_use]
    pub fn new(
        instrument: InstrumentId,
        name: &str,
        kind: MatchNameKind,
        ambiguity: MatchNameAmbiguity,
    ) -> Option<Self> {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.chars().count() > 64 {
            return None;
        }
        Some(Self {
            instrument,
            name: trimmed.to_owned(),
            kind,
            ambiguity,
        })
    }

    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    #[must_use]
    pub const fn kind(&self) -> MatchNameKind {
        self.kind
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TitleMatch {
    matched: Vec<InstrumentId>,
    ambiguous: Vec<InstrumentId>,
    group_mentions: Vec<String>,
}

impl TitleMatch {
    #[must_use]
    pub fn matched(&self) -> &[InstrumentId] {
        &self.matched
    }

    #[must_use]
    pub fn ambiguous(&self) -> &[InstrumentId] {
        &self.ambiguous
    }

    #[must_use]
    pub fn group_mentions(&self) -> &[String] {
        &self.group_mentions
    }

    #[must_use]
    pub fn needs_fast_model(&self) -> bool {
        !self.ambiguous.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Matched,
    Ambiguous,
    GroupMention,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockMatcher {
    entries: Vec<MatchDictionaryEntry>,
}

fn is_ascii_name(name: &str) -> bool {
    name.chars()
        .all(|character| character.is_ascii_alphanumeric() || " &.-".contains(character))
}

fn is_korean_leading_boundary(character: char) -> bool {
    character.is_whitespace() || "([{<'\"‘“「『·&,/…)]}>’”」』:|".contains(character)
}

fn is_trailing_boundary(character: char) -> bool {
    character.is_whitespace()
        || character.is_ascii_digit()
        || (character.is_ascii_punctuation() && character != '-')
        || ")]}>'\"’”」』·…".contains(character)
}

fn trailing_is_particle_then_boundary(rest: &str) -> bool {
    PARTICLES.iter().any(|particle| {
        rest.strip_prefix(particle)
            .is_some_and(|after| after.chars().next().is_none_or(is_trailing_boundary))
    })
}

fn find_all(haystack: &str, needle: &str) -> Vec<usize> {
    let mut found = Vec::new();
    let mut from = 0_usize;
    while let Some(rest) = haystack.get(from..) {
        let Some(offset) = rest.find(needle) else {
            break;
        };
        let Some(start) = from.checked_add(offset) else {
            break;
        };
        found.push(start);
        let step = rest
            .get(offset..)
            .and_then(|tail| tail.chars().next())
            .map_or(1_usize, char::len_utf8);
        let Some(next) = start.checked_add(step) else {
            break;
        };
        from = next;
    }
    found
}

fn has_securities_context(title: &str) -> bool {
    SECURITIES_CONTEXT.iter().any(|word| title.contains(word))
}

impl StockMatcher {
    #[must_use]
    pub fn new(mut entries: Vec<MatchDictionaryEntry>) -> Self {
        entries.sort_by(|left, right| {
            right
                .name
                .chars()
                .count()
                .cmp(&left.name.chars().count())
                .then_with(|| left.name.cmp(&right.name))
        });
        Self { entries }
    }

    fn judge(entry: &MatchDictionaryEntry, title: &str, start: usize, end: usize) -> Verdict {
        let before = title.get(..start).and_then(|head| head.chars().next_back());
        let rest = title.get(end..).unwrap_or("");
        let after = rest.chars().next();
        let ascii = is_ascii_name(&entry.name);
        let leading_ok = if ascii {
            before.is_none_or(|character| !character.is_ascii_alphanumeric())
        } else {
            before.is_none_or(is_korean_leading_boundary)
        };
        if !leading_ok {
            return Verdict::Rejected;
        }
        if GROUP_PREFIXES.contains(&entry.name.as_str()) && after.is_some_and(char::is_alphabetic) {
            return Verdict::GroupMention;
        }
        let clean_end = if ascii {
            after.is_none_or(|character| !character.is_alphanumeric())
        } else {
            after.is_none_or(is_trailing_boundary)
        };
        let trailing_ok = clean_end || trailing_is_particle_then_boundary(rest);
        if !trailing_ok {
            return Verdict::Rejected;
        }
        match entry.ambiguity {
            MatchNameAmbiguity::Distinct => Verdict::Matched,
            MatchNameAmbiguity::CommonNoun => {
                let at_start = title
                    .get(..start)
                    .is_some_and(|head| head.trim().is_empty());
                let comma_after = after == Some(',');
                if at_start || comma_after || has_securities_context(title) {
                    Verdict::Matched
                } else {
                    Verdict::Ambiguous
                }
            }
        }
    }

    #[must_use]
    pub fn match_title(&self, title: &UntrustedText) -> TitleMatch {
        let text = title.as_untrusted_str();
        let mut collector = MatchCollector::default();
        for entry in &self.entries {
            for start in find_all(text, &entry.name) {
                collector.consider(entry, text, start);
            }
        }
        collector.finish()
    }
}

#[derive(Debug, Default)]
struct MatchCollector {
    taken: Vec<(usize, usize)>,
    matched: BTreeSet<InstrumentId>,
    ambiguous: BTreeSet<InstrumentId>,
    groups: BTreeSet<String>,
}

impl MatchCollector {
    fn consider(&mut self, entry: &MatchDictionaryEntry, text: &str, start: usize) {
        let Some(end) = start.checked_add(entry.name.len()) else {
            return;
        };
        if self
            .taken
            .iter()
            .any(|(left, right)| start < *right && *left < end)
        {
            return;
        }
        match StockMatcher::judge(entry, text, start, end) {
            Verdict::Matched => {
                self.matched.insert(entry.instrument.clone());
            }
            Verdict::Ambiguous => {
                self.ambiguous.insert(entry.instrument.clone());
            }
            Verdict::GroupMention => {
                self.groups.insert(entry.name.clone());
            }
            Verdict::Rejected => return,
        }
        self.taken.push((start, end));
    }

    fn finish(self) -> TitleMatch {
        let matched = self.matched;
        let pending = self
            .ambiguous
            .into_iter()
            .filter(|instrument| !matched.contains(instrument))
            .collect();
        TitleMatch {
            matched: matched.into_iter().collect(),
            ambiguous: pending,
            group_mentions: self.groups.into_iter().collect(),
        }
    }
}
