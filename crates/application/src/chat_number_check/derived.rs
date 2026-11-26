use std::collections::BTreeSet;

use super::{AnswerNumber, NumberRelevance, NumberToken};
use domain::UntrustedText;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Fixed(i128);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Factor(i128);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Cursor(usize);

#[derive(Debug, Clone)]
struct Chars(Vec<Glyph>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Glyph(char);

const SCALE: Factor = Factor::new(1_000_000_i128);
const TEN: Factor = Factor::new(10_i128);
const HUNDRED: Factor = Factor::new(100_i128);
const ONE: Factor = Factor::new(1_i128);
const SCALE_DIGITS: Cursor = Cursor::new(6_usize);
const MAX_PAIR_BASE: Cursor = Cursor::new(300_usize);
const BEFORE_WINDOW: Cursor = Cursor::new(12_usize);
const NEAR_WINDOW: Cursor = Cursor::new(3_usize);
const AFTER_WINDOW: Cursor = Cursor::new(8_usize);

impl Factor {
    const fn new(value: i128) -> Self {
        Self(value)
    }
}

impl Cursor {
    const fn new(value: usize) -> Self {
        Self(value)
    }

    const fn forward(self, step: Self) -> Self {
        Self(self.0.saturating_add(step.0))
    }

    const fn back(self, step: Self) -> Self {
        Self(self.0.saturating_sub(step.0))
    }

    const fn next(self) -> Self {
        Self(self.0.saturating_add(1_usize))
    }
}

impl Glyph {
    const fn new(value: char) -> Self {
        Self(value)
    }

    const fn is_digit(self) -> Reach {
        if self.0.is_ascii_digit() {
            Reach::Reachable
        } else {
            Reach::Unreachable
        }
    }
}

impl Chars {
    fn new(text: &UntrustedText) -> Self {
        Self(text.as_untrusted_str().chars().map(Glyph::new).collect())
    }

    fn at(&self, cursor: Cursor) -> Option<Glyph> {
        self.0.get(cursor.0).copied()
    }

    fn end(&self) -> Cursor {
        Cursor::new(self.0.len())
    }

    fn window(&self, start: Cursor, end: Cursor) -> Window {
        Window(match self.0.get(start.0..end.min(self.end()).0) {
            Some(glyphs) => glyphs.iter().map(|glyph| glyph.0).collect(),
            None => String::new(),
        })
    }
}

impl Fixed {
    const fn new(units: i128) -> Self {
        Self(units)
    }

    fn from_answer(number: &AnswerNumber) -> Option<Self> {
        let (whole, fraction) = number.0.split_once('.').unwrap_or((number.0.as_str(), ""));
        let whole_units = whole.parse::<i128>().ok()?.checked_mul(SCALE.0)?;
        let mut padded: String = fraction.chars().take(SCALE_DIGITS.0).collect();
        while padded.chars().count() < SCALE_DIGITS.0 {
            padded.push('0');
        }
        let fraction_units = padded.parse::<i128>().ok()?;
        whole_units.checked_add(fraction_units).map(Self::new)
    }

    fn plus(self, other: Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Self::new)
    }

    fn gap(self, other: Self) -> Option<Self> {
        if self >= other {
            self.0.checked_sub(other.0).map(Self::new)
        } else {
            other.0.checked_sub(self.0).map(Self::new)
        }
    }

    fn times(self, other: Self) -> Option<Self> {
        self.0
            .checked_mul(other.0)?
            .checked_div(SCALE.0)
            .map(Self::new)
    }

    fn over(self, other: Self) -> Option<Self> {
        if other.0 == 0_i128 {
            return None;
        }
        self.0
            .checked_mul(SCALE.0)?
            .checked_div(other.0)
            .map(Self::new)
    }

    fn scaled_up(self, factor: Factor) -> Option<Self> {
        self.0.checked_mul(factor.0).map(Self::new)
    }

    fn scaled_down(self, factor: Factor) -> Option<Self> {
        self.0.checked_div(factor.0).map(Self::new)
    }

    fn halved(self) -> Self {
        match self.0.checked_div(2_i128) {
            Some(half) => Self::new(half),
            None => self,
        }
    }

    const fn lower(self, tolerance: Self) -> Self {
        Self::new(self.0.saturating_sub(tolerance.0))
    }

    const fn upper(self, tolerance: Self) -> Self {
        Self::new(self.0.saturating_add(tolerance.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wording {
    Approximate,
    Stated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subject {
    Fact,
    Suggestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    Reachable,
    Unreachable,
}

#[derive(Debug, Clone)]
pub(super) struct Claim {
    pub(super) number: AnswerNumber,
    value: Fixed,
    tolerance: Fixed,
    unit: Factor,
    wording: Wording,
    subject: Subject,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Grounding {
    Derived,
    Ungrounded,
}

struct Window(String);

impl Window {
    fn approximate_before(&self) -> Wording {
        let trimmed = self.0.trim_end();
        if ["약", "대략", "~", "최대"]
            .iter()
            .any(|marker| trimmed.ends_with(marker))
        {
            Wording::Approximate
        } else {
            Wording::Stated
        }
    }

    fn approximate_after(&self) -> Wording {
        if ["가량", "정도", "안팎", "내외", "남짓", "쯤", "여 "]
            .iter()
            .any(|marker| self.0.contains(marker))
        {
            Wording::Approximate
        } else {
            Wording::Stated
        }
    }

    fn subject(&self) -> Subject {
        if [
            "현재가",
            "보유",
            "평단",
            "평균단가",
            "매입가",
            "수익률",
            "주가",
            "종가",
            "평가금액",
            "평가손익",
            "시세",
        ]
        .iter()
        .any(|cue| self.0.contains(cue))
        {
            Subject::Fact
        } else {
            Subject::Suggestion
        }
    }

    fn unit(&self) -> Factor {
        match self.0.trim_start().chars().next() {
            Some('만') => Factor::new(10_000_i128),
            Some('억') => Factor::new(100_000_000_i128),
            Some('천') => Factor::new(1_000_i128),
            _ => ONE,
        }
    }
}

fn display_tolerance(token: &NumberToken, wording: Wording) -> Fixed {
    let raw: String = token.0.chars().filter(|ch| *ch != ',').collect();
    let place = match raw.split_once('.') {
        Some((_whole, fraction)) => {
            let digits = fraction.chars().count().min(SCALE_DIGITS.0);
            (0..digits).fold(Fixed::new(SCALE.0), |acc, _digit| {
                acc.scaled_down(TEN).unwrap_or(Fixed::new(1_i128))
            })
        }
        None if wording == Wording::Approximate => {
            let zeros = raw.chars().rev().take_while(|ch| *ch == '0').count();
            let kept = zeros.min(raw.chars().count().saturating_sub(1_usize));
            (0..kept).fold(Fixed::new(SCALE.0), |acc, _zero| {
                acc.scaled_up(TEN).unwrap_or(acc)
            })
        }
        None => Fixed::new(SCALE.0),
    };
    place.halved()
}

pub(super) fn claims_in(text: &UntrustedText) -> Vec<Claim> {
    let chars = Chars::new(text);
    let mut claims = Vec::new();
    let mut cursor = Cursor::new(0_usize);
    while let Some(glyph) = chars.at(cursor) {
        if glyph.is_digit() == Reach::Unreachable {
            cursor = cursor.next();
            continue;
        }
        let end = number_end(&chars, cursor);
        let token = NumberToken(chars.window(cursor, end).0);
        let number = AnswerNumber::from_token(&token);
        if number.relevance() == NumberRelevance::Checked
            && let Some(claim) = build_claim(&chars, (cursor, end), &token, number)
        {
            claims.push(claim);
        }
        cursor = end.max(cursor.next());
    }
    claims
}

fn number_end(chars: &Chars, start: Cursor) -> Cursor {
    let mut end = start;
    let mut seen_dot = Wording::Stated;
    while let Some(Glyph(ch)) = chars.at(end) {
        let next_is_digit = chars
            .at(end.next())
            .is_some_and(|next| next.is_digit() == Reach::Reachable);
        let joins = ch.is_ascii_digit()
            || ((ch == ',' || ch == '.') && next_is_digit && seen_dot == Wording::Stated);
        if !joins {
            break;
        }
        if ch == '.' {
            seen_dot = Wording::Approximate;
        }
        end = end.next();
    }
    end
}

fn build_claim(
    chars: &Chars,
    (start, end): (Cursor, Cursor),
    token: &NumberToken,
    number: AnswerNumber,
) -> Option<Claim> {
    let before = chars.window(start.back(BEFORE_WINDOW), start);
    let near_before = chars.window(start.back(NEAR_WINDOW), start);
    let after = chars.window(end, end.forward(AFTER_WINDOW));
    let wording = match (near_before.approximate_before(), after.approximate_after()) {
        (Wording::Stated, Wording::Stated) => Wording::Stated,
        (Wording::Approximate, _) | (_, Wording::Approximate) => Wording::Approximate,
    };
    let value = Fixed::from_answer(&number)?;
    Some(Claim {
        tolerance: display_tolerance(token, wording),
        number,
        value,
        unit: after.unit(),
        wording,
        subject: before.subject(),
    })
}

fn push_pairs(derived: &mut Vec<Fixed>, base: &[Fixed]) {
    let percent = Fixed::new(HUNDRED.0.saturating_mul(SCALE.0));
    for (left, right) in base
        .iter()
        .flat_map(|left| base.iter().map(move |right| (*left, *right)))
        .filter(|(left, right)| left != right)
    {
        push_pair(derived, left, right, percent);
    }
}

fn push_pair(derived: &mut Vec<Fixed>, left: Fixed, right: Fixed, percent: Fixed) {
    derived.extend(left.plus(right));
    derived.extend(left.gap(right));
    derived.extend(left.times(right));
    derived.extend(left.over(right));
    derived.extend(left.over(right).and_then(|ratio| ratio.times(percent)));
    derived.extend(
        left.gap(right)
            .and_then(|gap| gap.over(right))
            .and_then(|ratio| ratio.times(percent)),
    );
}

pub(super) struct DerivationBook {
    derived: Vec<Fixed>,
}

impl DerivationBook {
    pub(super) fn new(known: &BTreeSet<AnswerNumber>) -> Self {
        let base: Vec<Fixed> = known.iter().filter_map(Fixed::from_answer).collect();
        let mut derived = Vec::new();
        for value in &base {
            derived.push(*value);
            derived.extend(value.scaled_up(HUNDRED));
            derived.extend(value.scaled_down(HUNDRED));
        }
        if Cursor::new(base.len()) <= MAX_PAIR_BASE {
            push_pairs(&mut derived, &base);
        }
        derived.sort_unstable();
        derived.dedup();
        Self { derived }
    }

    pub(super) fn classify(&self, claim: &Claim) -> Grounding {
        let scaled = (
            claim.value.scaled_up(claim.unit).unwrap_or(claim.value),
            claim
                .tolerance
                .scaled_up(claim.unit)
                .unwrap_or(claim.tolerance),
        );
        let reach = match (
            self.near(claim.value, claim.tolerance),
            self.near(scaled.0, scaled.1),
        ) {
            (Reach::Unreachable, Reach::Unreachable) => Reach::Unreachable,
            (Reach::Reachable, _) | (_, Reach::Reachable) => Reach::Reachable,
        };
        match (reach, claim.subject, claim.wording) {
            (Reach::Reachable, _, _)
            | (Reach::Unreachable, Subject::Suggestion, Wording::Approximate) => Grounding::Derived,
            (Reach::Unreachable, Subject::Fact, _)
            | (Reach::Unreachable, Subject::Suggestion, Wording::Stated) => Grounding::Ungrounded,
        }
    }

    fn near(&self, value: Fixed, tolerance: Fixed) -> Reach {
        let low = value.lower(tolerance);
        let high = value.upper(tolerance);
        let from = self.derived.partition_point(|candidate| *candidate < low);
        match self.derived.get(from) {
            Some(candidate) if *candidate <= high => Reach::Reachable,
            Some(_) | None => Reach::Unreachable,
        }
    }
}
