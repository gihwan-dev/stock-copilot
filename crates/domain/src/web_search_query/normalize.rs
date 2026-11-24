use std::collections::BTreeSet;

use crate::{WebSearchCandidateQuery, WebSearchRejectionReason};

pub(super) struct TextBuf(String);

impl TextBuf {
    pub(super) fn new(value: &str) -> Self {
        Self(value.to_owned())
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

pub(super) struct PreparedQuery {
    folded: TextBuf,
    compact_alnum: TextBuf,
    unmasked_digits: TextBuf,
}

impl PreparedQuery {
    pub(super) fn folded(&self) -> &TextBuf {
        &self.folded
    }

    pub(super) fn compact_alnum(&self) -> &TextBuf {
        &self.compact_alnum
    }

    pub(super) fn unmasked_digits(&self) -> &TextBuf {
        &self.unmasked_digits
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum RuleMatch {
    NoMatch,
    Matched,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct ScalarChar(char);

impl ScalarChar {
    pub(super) const fn new(value: char) -> Self {
        Self(value)
    }

    pub(super) const fn as_char(&self) -> &char {
        &self.0
    }
}

pub(super) fn is_hanja_numeral(scalar: ScalarChar) -> RuleMatch {
    match *scalar.as_char() {
        '零' | '〇' | '一' | '二' | '三' | '四' | '五' | '六' | '七' | '八' | '九' | '十'
        | '百' | '千' | '萬' | '億' | '兆' | '壹' | '貳' | '參' => RuleMatch::Matched,
        _ => RuleMatch::NoMatch,
    }
}

fn is_control_or_invisible(scalar: ScalarChar) -> RuleMatch {
    let ch = *scalar.as_char();
    if ch.is_control() {
        return RuleMatch::Matched;
    }
    match ch {
        '\u{00AD}'
        | '\u{034F}'
        | '\u{061C}'
        | '\u{115F}'
        | '\u{1160}'
        | '\u{17B4}'
        | '\u{17B5}'
        | '\u{180E}'
        | '\u{200B}'..='\u{200F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{206F}'
        | '\u{3164}'
        | '\u{FE00}'..='\u{FE0F}'
        | '\u{FEFF}'
        | '\u{FFA0}'
        | '\u{E0000}'..='\u{E007F}'
        | '\u{0300}'..='\u{036F}'
        | '\u{1AB0}'..='\u{1AFF}'
        | '\u{1DC0}'..='\u{1DFF}'
        | '\u{20D0}'..='\u{20FF}'
        | '\u{FE20}'..='\u{FE2F}' => RuleMatch::Matched,
        _ => RuleMatch::NoMatch,
    }
}

fn is_obfuscated_char(scalar: ScalarChar) -> RuleMatch {
    let ch = *scalar.as_char();
    let allowed_ascii = (' '..='~').contains(&ch);
    let allowed_hangul = ('가'..='힣').contains(&ch);
    let currency_symbol = "₩$€£¥¢₿".contains(ch);
    let hanja_numeral = match is_hanja_numeral(scalar) {
        RuleMatch::Matched => true,
        RuleMatch::NoMatch => false,
    };
    if allowed_ascii || allowed_hangul || currency_symbol || hanja_numeral {
        RuleMatch::NoMatch
    } else {
        RuleMatch::Matched
    }
}

const CONFUSABLE_MAP: &[(ScalarChar, ScalarChar)] = &[
    (ScalarChar::new('。'), ScalarChar::new('.')),
    (ScalarChar::new('｡'), ScalarChar::new('.')),
    (ScalarChar::new('а'), ScalarChar::new('a')),
    (ScalarChar::new('А'), ScalarChar::new('a')),
    (ScalarChar::new('α'), ScalarChar::new('a')),
    (ScalarChar::new('Α'), ScalarChar::new('a')),
    (ScalarChar::new('е'), ScalarChar::new('e')),
    (ScalarChar::new('Е'), ScalarChar::new('e')),
    (ScalarChar::new('ε'), ScalarChar::new('e')),
    (ScalarChar::new('Ε'), ScalarChar::new('e')),
    (ScalarChar::new('о'), ScalarChar::new('o')),
    (ScalarChar::new('О'), ScalarChar::new('o')),
    (ScalarChar::new('ο'), ScalarChar::new('o')),
    (ScalarChar::new('Ο'), ScalarChar::new('o')),
    (ScalarChar::new('р'), ScalarChar::new('p')),
    (ScalarChar::new('Р'), ScalarChar::new('p')),
    (ScalarChar::new('ρ'), ScalarChar::new('p')),
    (ScalarChar::new('Ρ'), ScalarChar::new('p')),
    (ScalarChar::new('с'), ScalarChar::new('c')),
    (ScalarChar::new('С'), ScalarChar::new('c')),
    (ScalarChar::new('х'), ScalarChar::new('x')),
    (ScalarChar::new('Х'), ScalarChar::new('x')),
    (ScalarChar::new('χ'), ScalarChar::new('x')),
    (ScalarChar::new('Χ'), ScalarChar::new('x')),
    (ScalarChar::new('у'), ScalarChar::new('y')),
    (ScalarChar::new('У'), ScalarChar::new('y')),
    (ScalarChar::new('і'), ScalarChar::new('i')),
    (ScalarChar::new('І'), ScalarChar::new('i')),
    (ScalarChar::new('ι'), ScalarChar::new('i')),
    (ScalarChar::new('Ι'), ScalarChar::new('i')),
    (ScalarChar::new('к'), ScalarChar::new('k')),
    (ScalarChar::new('К'), ScalarChar::new('k')),
    (ScalarChar::new('κ'), ScalarChar::new('k')),
    (ScalarChar::new('Κ'), ScalarChar::new('k')),
    (ScalarChar::new('м'), ScalarChar::new('m')),
    (ScalarChar::new('М'), ScalarChar::new('m')),
    (ScalarChar::new('μ'), ScalarChar::new('m')),
    (ScalarChar::new('Μ'), ScalarChar::new('m')),
    (ScalarChar::new('т'), ScalarChar::new('t')),
    (ScalarChar::new('Т'), ScalarChar::new('t')),
    (ScalarChar::new('τ'), ScalarChar::new('t')),
    (ScalarChar::new('Τ'), ScalarChar::new('t')),
    (ScalarChar::new('н'), ScalarChar::new('h')),
    (ScalarChar::new('Н'), ScalarChar::new('h')),
    (ScalarChar::new('η'), ScalarChar::new('h')),
    (ScalarChar::new('Η'), ScalarChar::new('h')),
    (ScalarChar::new('в'), ScalarChar::new('b')),
    (ScalarChar::new('В'), ScalarChar::new('b')),
    (ScalarChar::new('β'), ScalarChar::new('b')),
    (ScalarChar::new('Β'), ScalarChar::new('b')),
    (ScalarChar::new('①'), ScalarChar::new('1')),
    (ScalarChar::new('⑴'), ScalarChar::new('1')),
    (ScalarChar::new('⒈'), ScalarChar::new('1')),
    (ScalarChar::new('¹'), ScalarChar::new('1')),
    (ScalarChar::new('₁'), ScalarChar::new('1')),
    (ScalarChar::new('١'), ScalarChar::new('1')),
    (ScalarChar::new('②'), ScalarChar::new('2')),
    (ScalarChar::new('⑵'), ScalarChar::new('2')),
    (ScalarChar::new('⒉'), ScalarChar::new('2')),
    (ScalarChar::new('²'), ScalarChar::new('2')),
    (ScalarChar::new('₂'), ScalarChar::new('2')),
    (ScalarChar::new('٢'), ScalarChar::new('2')),
    (ScalarChar::new('③'), ScalarChar::new('3')),
    (ScalarChar::new('⑶'), ScalarChar::new('3')),
    (ScalarChar::new('⒊'), ScalarChar::new('3')),
    (ScalarChar::new('³'), ScalarChar::new('3')),
    (ScalarChar::new('₃'), ScalarChar::new('3')),
    (ScalarChar::new('٣'), ScalarChar::new('3')),
    (ScalarChar::new('④'), ScalarChar::new('4')),
    (ScalarChar::new('⑷'), ScalarChar::new('4')),
    (ScalarChar::new('⒋'), ScalarChar::new('4')),
    (ScalarChar::new('⁴'), ScalarChar::new('4')),
    (ScalarChar::new('₄'), ScalarChar::new('4')),
    (ScalarChar::new('٤'), ScalarChar::new('4')),
    (ScalarChar::new('⑤'), ScalarChar::new('5')),
    (ScalarChar::new('⑸'), ScalarChar::new('5')),
    (ScalarChar::new('⒌'), ScalarChar::new('5')),
    (ScalarChar::new('⁵'), ScalarChar::new('5')),
    (ScalarChar::new('₅'), ScalarChar::new('5')),
    (ScalarChar::new('٥'), ScalarChar::new('5')),
    (ScalarChar::new('⑥'), ScalarChar::new('6')),
    (ScalarChar::new('⑹'), ScalarChar::new('6')),
    (ScalarChar::new('⒍'), ScalarChar::new('6')),
    (ScalarChar::new('⁶'), ScalarChar::new('6')),
    (ScalarChar::new('₆'), ScalarChar::new('6')),
    (ScalarChar::new('٦'), ScalarChar::new('6')),
    (ScalarChar::new('⑦'), ScalarChar::new('7')),
    (ScalarChar::new('⑺'), ScalarChar::new('7')),
    (ScalarChar::new('⒎'), ScalarChar::new('7')),
    (ScalarChar::new('⁷'), ScalarChar::new('7')),
    (ScalarChar::new('₇'), ScalarChar::new('7')),
    (ScalarChar::new('٧'), ScalarChar::new('7')),
    (ScalarChar::new('⑧'), ScalarChar::new('8')),
    (ScalarChar::new('⑻'), ScalarChar::new('8')),
    (ScalarChar::new('⒏'), ScalarChar::new('8')),
    (ScalarChar::new('⁸'), ScalarChar::new('8')),
    (ScalarChar::new('₈'), ScalarChar::new('8')),
    (ScalarChar::new('٨'), ScalarChar::new('8')),
    (ScalarChar::new('⑨'), ScalarChar::new('9')),
    (ScalarChar::new('⑼'), ScalarChar::new('9')),
    (ScalarChar::new('⒐'), ScalarChar::new('9')),
    (ScalarChar::new('⁹'), ScalarChar::new('9')),
    (ScalarChar::new('₉'), ScalarChar::new('9')),
    (ScalarChar::new('٩'), ScalarChar::new('9')),
    (ScalarChar::new('⓪'), ScalarChar::new('0')),
    (ScalarChar::new('⁰'), ScalarChar::new('0')),
    (ScalarChar::new('₀'), ScalarChar::new('0')),
    (ScalarChar::new('٠'), ScalarChar::new('0')),
];

fn fold_confusable_char(scalar: ScalarChar) -> ScalarChar {
    let ch = *scalar.as_char();
    let code = u32::from(ch);
    if let Some(mapped) = (0xFF01_u32..=0xFF5E_u32)
        .contains(&code)
        .then(|| char::from_u32(code.saturating_sub(0xFEE0_u32)))
        .flatten()
    {
        return ScalarChar::new(mapped.to_ascii_lowercase());
    }
    if let Some(mapped) = CONFUSABLE_MAP
        .iter()
        .find_map(|&(from, to)| (from == scalar).then_some(to))
    {
        return mapped;
    }
    if ch.is_whitespace() {
        ScalarChar::new(' ')
    } else {
        ScalarChar::new(ch.to_ascii_lowercase())
    }
}

fn decode_percent_and_detect(text: &TextBuf) -> (TextBuf, RuleMatch) {
    let chars: Vec<char> = text.as_str().chars().collect();
    let mut out = String::with_capacity(text.as_str().len());
    let mut idx = 0_usize;
    let mut found = RuleMatch::NoMatch;
    while let Some(&ch) = chars.get(idx) {
        let next_two = chars
            .get(idx.saturating_add(1_usize))
            .zip(chars.get(idx.saturating_add(2_usize)));
        if ch == '%'
            && let Some((&hi, &lo)) = next_two
            && let (Some(high_nibble), Some(low_nibble)) =
                (hi.to_digit(16_u32), lo.to_digit(16_u32))
        {
            found = RuleMatch::Matched;
            let combined = (high_nibble << 4_u32) | low_nibble;
            if let Some(decoded_char) =
                char::from_u32(combined).filter(|decoded| (' '..='~').contains(decoded))
            {
                out.push(decoded_char.to_ascii_lowercase());
            }
            idx = idx.saturating_add(3_usize);
        } else {
            out.push(ch);
            idx = idx.saturating_add(1_usize);
        }
    }
    (TextBuf::new(&out), found)
}

fn has_encoding_marker(folded: &TextBuf, raw: &WebSearchCandidateQuery) -> RuleMatch {
    let text = folded.as_str();
    let has_entity = text.contains("&#") && text.contains(';');
    let has_escape = text.contains("\\u") || text.contains("\\x");
    let has_punycode = text.contains("xn--");
    let has_radix_prefix = text.split_whitespace().any(|raw_tok| {
        let tok = raw_tok.trim_matches(|ch: char| !ch.is_ascii_alphanumeric());
        tok.strip_prefix("0x").is_some_and(|rest| {
            rest.len() >= 2_usize && rest.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) || tok.strip_prefix("0b").is_some_and(|rest| {
            rest.len() >= 4_usize && rest.bytes().all(|byte| byte == b'0' || byte == b'1')
        })
    });
    let has_base64 = raw.as_str().split_whitespace().any(|raw_tok| {
        let tok =
            raw_tok.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && !"+/=_-".contains(ch));
        let base64_chars = tok
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"+/=_-".contains(&byte));
        let has_upper = tok.bytes().any(|byte| byte.is_ascii_uppercase());
        let has_lower = tok.bytes().any(|byte| byte.is_ascii_lowercase());
        let has_digit = tok.bytes().any(|byte| byte.is_ascii_digit());
        let padded = tok.ends_with('=') && tok.len() >= 8_usize;
        base64_chars && ((tok.len() >= 16_usize && has_upper && has_lower && has_digit) || padded)
    });
    if has_entity || has_escape || has_punycode || has_radix_prefix || has_base64 {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

pub(super) fn prepare_query(
    candidate: &WebSearchCandidateQuery,
    reasons: &mut BTreeSet<WebSearchRejectionReason>,
) -> PreparedQuery {
    let raw = candidate.as_str();
    if raw.trim().is_empty() {
        reasons.insert(WebSearchRejectionReason::EmptyQuery);
    }
    if raw.len() > 600_usize || raw.chars().count() > 200_usize {
        reasons.insert(WebSearchRejectionReason::QueryTooLong);
    }
    let mut initial_folded = String::with_capacity(raw.len());
    for ch in raw.chars() {
        let scalar = ScalarChar::new(ch);
        match is_control_or_invisible(scalar) {
            RuleMatch::Matched => {
                reasons.insert(WebSearchRejectionReason::ControlOrInvisibleCharacter);
                continue;
            }
            RuleMatch::NoMatch => {}
        }
        match is_obfuscated_char(scalar) {
            RuleMatch::Matched => {
                reasons.insert(WebSearchRejectionReason::EncodedOrObfuscatedText);
            }
            RuleMatch::NoMatch => {}
        }
        initial_folded.push(*fold_confusable_char(scalar).as_char());
    }
    let (decoded, percent_hit) = decode_percent_and_detect(&TextBuf::new(&initial_folded));
    match (percent_hit, has_encoding_marker(&decoded, candidate)) {
        (RuleMatch::Matched, _) | (_, RuleMatch::Matched) => {
            reasons.insert(WebSearchRejectionReason::EncodedOrObfuscatedText);
        }
        (RuleMatch::NoMatch, RuleMatch::NoMatch) => {}
    }
    let mut compact = String::with_capacity(decoded.as_str().len());
    let mut digits = String::new();
    for ch in decoded.as_str().chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        }
        if ch.is_alphanumeric() {
            compact.push(ch);
        }
    }
    PreparedQuery {
        folded: decoded,
        compact_alnum: TextBuf::new(&compact),
        unmasked_digits: TextBuf::new(&digits),
    }
}
