use std::collections::BTreeSet;

use crate::{WebSearchRejectionReason, WebSearchSensitiveContext};

use super::normalize::{PreparedQuery, RuleMatch, ScalarChar, TextBuf, is_hanja_numeral};

fn is_url_or_domain(folded: &TextBuf) -> RuleMatch {
    let text = folded.as_str();
    let scheme_hit = [
        "://",
        "http:",
        "https:",
        "ftp:",
        "file:",
        "data:",
        "javascript:",
        "mailto:",
        "ws:",
        "wss:",
        "www.",
        "[.]",
        "(.)",
        "[dot]",
        "(dot)",
        " dot ",
    ]
    .iter()
    .any(|needle| text.contains(needle));
    if scheme_hit {
        return RuleMatch::Matched;
    }
    let hit = text
        .split(|ch: char| ch.is_whitespace() || ",;()[]{}<>\"'|!#".contains(ch))
        .filter(|tok| !tok.is_empty())
        .any(|tok| is_url_token(&TextBuf::new(tok)) == RuleMatch::Matched);
    if hit {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn has_ascii_host_port(raw_host: &TextBuf) -> RuleMatch {
    let Some((host, port)) = raw_host.as_str().split_once(':') else {
        return RuleMatch::NoMatch;
    };
    let host_ok = !host.is_empty()
        && host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.')
        && host.bytes().any(|byte| byte.is_ascii_alphabetic());
    let port_ok = !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit());
    if host_ok && port_ok {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn is_url_token(token: &TextBuf) -> RuleMatch {
    let raw = token.as_str();
    let trimmed = raw.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '/');
    let has_email_or_query = raw.contains('@')
        || raw.contains("::")
        || trimmed.starts_with('/')
        || raw.contains('\\')
        || raw.split('/').count() >= 3_usize
        || raw.contains('?')
        || trimmed
            .split_once('=')
            .is_some_and(|(left, right)| !left.is_empty() && !right.is_empty());
    if has_email_or_query {
        return RuleMatch::Matched;
    }
    if trimmed.is_empty() {
        return RuleMatch::NoMatch;
    }
    let has_path = trimmed.contains('/');
    let raw_host = trimmed.split_once('/').map_or(trimmed, |(host, _)| host);
    if raw_host == "localhost" || has_ascii_host_port(&TextBuf::new(raw_host)) == RuleMatch::Matched
    {
        return RuleMatch::Matched;
    }
    let host_part = raw_host
        .split_once(':')
        .map_or(raw_host, |(host, _)| host)
        .trim_matches(|ch: char| !ch.is_alphanumeric());
    let labels: Vec<&str> = host_part.split('.').collect();
    if labels.len() < 2_usize || labels.iter().any(|part| part.is_empty()) {
        return RuleMatch::NoMatch;
    }
    let all_ascii_labels = labels.iter().all(|part| {
        part.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    });
    if !all_ascii_labels {
        return RuleMatch::NoMatch;
    }
    let ipv4 = labels.len() == 4_usize
        && labels
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()));
    let tld_like = labels.last().is_some_and(|tld| {
        tld.len() >= 2_usize && tld.bytes().all(|byte| byte.is_ascii_alphabetic())
    });
    if ipv4 || tld_like || labels.len() >= 3_usize || has_path {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn is_allowed_year(text: &TextBuf) -> RuleMatch {
    let raw = text.as_str();
    if raw.len() == 4_usize
        && raw.bytes().all(|byte| byte.is_ascii_digit())
        && raw.starts_with("20")
    {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn is_quarter_word(text: &TextBuf) -> RuleMatch {
    if ["1분기", "2분기", "3분기", "4분기", "q1", "q2", "q3", "q4"].contains(&text.as_str())
    {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn is_single_year_quarter_token(text: &TextBuf) -> RuleMatch {
    let tok = text.as_str();
    if let Some(year) = tok.strip_suffix('년') {
        return is_allowed_year(&TextBuf::new(year));
    }
    if let Some(year) = tok.strip_prefix("fy-").or_else(|| tok.strip_prefix("fy")) {
        return is_allowed_year(&TextBuf::new(year));
    }
    for suffix in ["1분기", "2분기", "3분기", "4분기", "상반기", "하반기"] {
        if let Some(prefix) = tok.strip_suffix(suffix)
            && let Some(year) = prefix.strip_suffix('년')
        {
            return is_allowed_year(&TextBuf::new(year));
        }
    }
    for suffix in ["-q1", "-q2", "-q3", "-q4", "q1", "q2", "q3", "q4"] {
        if let Some(year) = tok.strip_suffix(suffix) {
            return is_allowed_year(&TextBuf::new(year));
        }
    }
    for prefix in ["q1-", "q2-", "q3-", "q4-"] {
        if let Some(year) = tok.strip_prefix(prefix) {
            return is_allowed_year(&TextBuf::new(year));
        }
    }
    is_quarter_word(text)
}

fn is_two_token_year_quarter(first: &TextBuf, second: &TextBuf) -> RuleMatch {
    let left = first.as_str();
    let right = second.as_str();
    let left_is_year = is_allowed_year(first) == RuleMatch::Matched;
    let left_is_kor_year = left
        .strip_suffix('년')
        .is_some_and(|year| is_allowed_year(&TextBuf::new(year)) == RuleMatch::Matched);
    let right_is_quarter = is_quarter_word(second) == RuleMatch::Matched;
    let right_is_half = right == "상반기" || right == "하반기";
    let left_is_q_or_fy = is_quarter_word(first) == RuleMatch::Matched || left == "fy";
    let right_is_year = is_allowed_year(second) == RuleMatch::Matched;
    if (left_is_year && right_is_quarter)
        || (left_is_kor_year && (right_is_quarter || right_is_half))
        || (left_is_q_or_fy && right_is_year)
    {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn mask_year_quarter_tokens(folded: &TextBuf) -> TextBuf {
    let tokens: Vec<&str> = folded.as_str().split_whitespace().collect();
    let mut kept: Vec<&str> = Vec::with_capacity(tokens.len());
    let mut idx = 0_usize;
    let mut masked_count = 0_usize;
    while let Some(&current) = tokens.get(idx) {
        let cur_buf = TextBuf::new(current);
        let pair_hit = masked_count < 2_usize
            && tokens
                .get(idx.saturating_add(1_usize))
                .is_some_and(|&next| {
                    is_two_token_year_quarter(&cur_buf, &TextBuf::new(next)) == RuleMatch::Matched
                });
        if pair_hit {
            masked_count = masked_count.saturating_add(1_usize);
            idx = idx.saturating_add(2_usize);
        } else if masked_count < 2_usize
            && is_single_year_quarter_token(&cur_buf) == RuleMatch::Matched
        {
            masked_count = masked_count.saturating_add(1_usize);
            idx = idx.saturating_add(1_usize);
        } else {
            kept.push(current);
            idx = idx.saturating_add(1_usize);
        }
    }
    TextBuf::new(&kept.join(" "))
}

fn has_remaining_multi_digits(masked: &TextBuf) -> RuleMatch {
    let digit_count = masked.as_str().bytes().filter(u8::is_ascii_digit).count();
    if digit_count >= 2_usize {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScriptKind {
    Ascii,
    Hangul,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SingleScript {
    None,
    Char(ScriptKind, ScalarChar),
}

fn classify_single_char(token: &TextBuf) -> SingleScript {
    let mut chars = token.as_str().chars();
    match (chars.next(), chars.next()) {
        (Some(first), None) if first.is_ascii_alphabetic() => {
            SingleScript::Char(ScriptKind::Ascii, ScalarChar::new(first))
        }
        (Some(first), None) if ('가'..='힣').contains(&first) => {
            SingleScript::Char(ScriptKind::Hangul, ScalarChar::new(first))
        }
        _ => SingleScript::None,
    }
}

fn collapse_spaced_single_chars(folded: &TextBuf) -> TextBuf {
    let tokens: Vec<&str> = folded
        .as_str()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect();
    let mut rebuilt: Vec<String> = Vec::with_capacity(tokens.len());
    let mut idx = 0_usize;
    while let Some(&token) = tokens.get(idx) {
        let (kind, first_ch) = match classify_single_char(&TextBuf::new(token)) {
            SingleScript::Char(kind, ch) => (kind, *ch.as_char()),
            SingleScript::None => {
                rebuilt.push(token.to_owned());
                idx = idx.saturating_add(1_usize);
                continue;
            }
        };
        let mut merged = String::from(first_ch);
        let mut next_idx = idx.saturating_add(1_usize);
        while let Some(&next_tok) = tokens.get(next_idx) {
            match classify_single_char(&TextBuf::new(next_tok)) {
                SingleScript::Char(next_kind, ch) if next_kind == kind => {
                    merged.push(*ch.as_char());
                    next_idx = next_idx.saturating_add(1_usize);
                }
                SingleScript::Char(_, _) | SingleScript::None => break,
            }
        }
        rebuilt.push(merged);
        idx = next_idx;
    }
    TextBuf::new(&rebuilt.join(" "))
}

fn token_has_sino_number(token: &TextBuf) -> RuleMatch {
    let sino_digit = |ch: char| "영공일이삼사오육륙칠팔구".contains(ch);
    let sino_place = |ch: char| "십백천만억조".contains(ch);
    let chars: Vec<char> = token.as_str().chars().collect();
    if chars.len() == 2_usize && chars.iter().copied().all(sino_digit) {
        return RuleMatch::Matched;
    }
    let mut run_len = 0_usize;
    let mut run_has_place = false;
    let mut digit_run_len = 0_usize;
    for ch in chars {
        if sino_place(ch) {
            run_len = run_len.saturating_add(1_usize);
            run_has_place = true;
            digit_run_len = 0_usize;
        } else if sino_digit(ch) {
            run_len = run_len.saturating_add(1_usize);
            digit_run_len = digit_run_len.saturating_add(1_usize);
        } else {
            run_len = 0_usize;
            run_has_place = false;
            digit_run_len = 0_usize;
        }
        if (run_len >= 2_usize && run_has_place) || digit_run_len >= 3_usize {
            return RuleMatch::Matched;
        }
    }
    RuleMatch::NoMatch
}

fn has_korean_sino_number(text: &TextBuf) -> RuleMatch {
    let hit = text
        .as_str()
        .split_whitespace()
        .any(|tok| token_has_sino_number(&TextBuf::new(tok)) == RuleMatch::Matched);
    if hit {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn has_native_korean_number(text: &TextBuf) -> RuleMatch {
    let raw = text.as_str();
    let multi_native = "스물 스무 서른 마흔 쉰 예순 일흔 여든 아흔 열하나 열한 열둘 열두 열셋 열세 열넷 열네 열다섯 열여섯 열일곱 열여덟 열아홉";
    if multi_native.split(' ').any(|word| raw.contains(word)) {
        return RuleMatch::Matched;
    }
    let single_native = [
        "하나", "둘", "셋", "넷", "다섯", "여섯", "일곱", "여덟", "아홉", "열", "한", "두", "세",
        "네", "석", "넉",
    ];
    let unit_words = [
        "개", "주", "원", "좌", "만", "천", "백", "억", "조", "달러", "불", "엔",
    ];
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut native_count = 0_usize;
    for (idx, &tok) in tokens.iter().enumerate() {
        let native_plus_unit = single_native.iter().any(|num| {
            tok.strip_prefix(num)
                .is_some_and(|rest| unit_words.contains(&rest))
        });
        if native_plus_unit {
            return RuleMatch::Matched;
        }
        if single_native.contains(&tok) {
            native_count = native_count.saturating_add(1_usize);
            let next_is_unit = tokens
                .get(idx.saturating_add(1_usize))
                .is_some_and(|next| unit_words.contains(next));
            if native_count >= 2_usize || next_is_unit {
                return RuleMatch::Matched;
            }
        }
    }
    RuleMatch::NoMatch
}

fn has_hanja_number(text: &TextBuf) -> RuleMatch {
    let count = text
        .as_str()
        .chars()
        .filter(|&ch| is_hanja_numeral(ScalarChar::new(ch)) == RuleMatch::Matched)
        .count();
    if count >= 2_usize {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn has_english_number_words(text: &TextBuf) -> RuleMatch {
    let multi_en = "ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty thirty forty fifty sixty seventy eighty ninety hundred thousand million billion trillion dozen";
    let single_en = "zero one two three four five six seven eight nine";
    let mut single_count = 0_usize;
    for word in text
        .as_str()
        .split(|ch: char| !ch.is_ascii_alphabetic())
        .filter(|part| !part.is_empty())
    {
        if multi_en.split(' ').any(|item| item == word) {
            return RuleMatch::Matched;
        }
        if single_en.split(' ').any(|item| item == word) {
            single_count = single_count.saturating_add(1_usize);
            if single_count >= 2_usize {
                return RuleMatch::Matched;
            }
        }
    }
    RuleMatch::NoMatch
}

fn strip_intra_token_punct(folded: &TextBuf) -> TextBuf {
    let cleaned: Vec<String> = folded
        .as_str()
        .split_whitespace()
        .map(|tok| {
            tok.chars()
                .filter(|ch| ch.is_alphanumeric())
                .collect::<String>()
        })
        .filter(|tok| !tok.is_empty())
        .collect();
    TextBuf::new(&cleaned.join(" "))
}

fn has_money_quantity_or_account_term(
    folded: &TextBuf,
    collapsed: &TextBuf,
    punct_stripped: &TextBuf,
) -> RuleMatch {
    if has_currency_or_korean_finance_keyword(folded) == RuleMatch::Matched
        || has_currency_or_korean_finance_keyword(collapsed) == RuleMatch::Matched
        || has_english_finance_keyword(folded) == RuleMatch::Matched
        || has_english_finance_keyword(collapsed) == RuleMatch::Matched
        || has_english_finance_keyword(punct_stripped) == RuleMatch::Matched
        || has_korean_unit_pattern(folded) == RuleMatch::Matched
    {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn has_currency_or_korean_finance_keyword(text: &TextBuf) -> RuleMatch {
    let raw = text.as_str();
    let has_symbol = raw.chars().any(|ch| "₩$€£¥¢₿".contains(ch));
    let korean_keywords = "계좌 보유 잔고 잔액 예수금 평단 매입가 수량 평가액 평가금액 평가손익 수익금 매수금 매도금 매수가 매도가 투자금 원금 시드머니 포트폴리오 달러 유로 파운드 위안 엔화 만원 천원 백원 억원 조원 만주 천주 백주 주민번호 주민등록 전화번호 휴대폰 연락처 이메일 예금주 실명 본명 비밀번호 토큰";
    if has_symbol || korean_keywords.split(' ').any(|word| raw.contains(word)) {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn has_english_finance_keyword(text: &TextBuf) -> RuleMatch {
    let keywords = "krw usd eur jpy gbp cny hkd won dollar dollars cent cents euro euros yen pound pounds yuan cash money amount amounts balance balances deposit deposits budget costbasis avgprice share shares qty quantity quantities holding holdings position positions portfolio lot lots account accounts acct iban swift ssn rrn passport email phone mobile address userid username password token secret apikey";
    let hit = text
        .as_str()
        .split(|ch: char| !ch.is_ascii_alphabetic())
        .filter(|part| !part.is_empty())
        .any(|word| keywords.split(' ').any(|entry| entry == word));
    if hit {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn has_korean_unit_pattern(text: &TextBuf) -> RuleMatch {
    let standalone_units = ["원", "주", "좌", "불", "엔", "만", "억", "조", "천", "백"];
    let numeral_chars = "영공일이삼사오육륙칠팔구십백천만억조한두세네열셋넷섯곱덟홉";
    for token in text
        .as_str()
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
    {
        if standalone_units.contains(&token) {
            return RuleMatch::Matched;
        }
        let chars: Vec<char> = token.chars().collect();
        for [prev, unit] in chars.array_windows::<2_usize>() {
            let prev_is_numeral = prev.is_ascii_digit() || numeral_chars.contains(*prev);
            if "원주좌불엔".contains(*unit) && prev_is_numeral {
                return RuleMatch::Matched;
            }
        }
    }
    RuleMatch::NoMatch
}

fn matches_sensitive_decimal(
    value: &TextBuf,
    unmasked_digits: &TextBuf,
    masked_digits: &TextBuf,
) -> RuleMatch {
    let digits_only: String = value
        .as_str()
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let trimmed = digits_only.trim_start_matches('0');
    if trimmed.is_empty() {
        return RuleMatch::NoMatch;
    }
    let in_masked = masked_digits.as_str().contains(trimmed)
        || (digits_only.len() >= 2_usize && masked_digits.as_str().contains(&digits_only));
    let in_unmasked = trimmed.len() >= 4_usize
        && (unmasked_digits.as_str().contains(trimmed)
            || unmasked_digits.as_str().contains(&digits_only));
    if in_masked || in_unmasked {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn matches_sensitive_accounts(
    prepared: &PreparedQuery,
    context: &WebSearchSensitiveContext,
) -> RuleMatch {
    let unmasked = prepared.unmasked_digits().as_str();
    for account in context.account_numbers() {
        let raw = account.as_str();
        let all_digits: String = raw.chars().filter(char::is_ascii_digit).collect();
        let last_four: String = all_digits
            .chars()
            .skip(all_digits.len().saturating_sub(4_usize))
            .collect();
        if unmasked.contains(&all_digits) || unmasked.contains(&last_four) {
            return RuleMatch::Matched;
        }
        if raw
            .split(|ch: char| !ch.is_ascii_digit())
            .any(|segment| segment.len() >= 3_usize && unmasked.contains(segment))
        {
            return RuleMatch::Matched;
        }
    }
    for masked_account in context.masked_accounts() {
        if unmasked.contains(&masked_account.to_string()) {
            return RuleMatch::Matched;
        }
    }
    RuleMatch::NoMatch
}

fn matches_sensitive_decimal_text(
    raw: &TextBuf,
    normalized: &TextBuf,
    unmasked: &TextBuf,
    masked_digits: &TextBuf,
) -> RuleMatch {
    let whole = raw
        .as_str()
        .split_once('.')
        .map_or(raw.as_str(), |(prefix, _)| prefix);
    if matches_sensitive_decimal(raw, unmasked, masked_digits) == RuleMatch::Matched
        || matches_sensitive_decimal(normalized, unmasked, masked_digits) == RuleMatch::Matched
        || matches_sensitive_decimal(&TextBuf::new(whole), unmasked, masked_digits)
            == RuleMatch::Matched
    {
        RuleMatch::Matched
    } else {
        RuleMatch::NoMatch
    }
}

fn matches_sensitive_quantity(
    qty: &crate::Quantity,
    unmasked: &TextBuf,
    masked_digits: &TextBuf,
) -> RuleMatch {
    let dec = qty.as_decimal();
    matches_sensitive_decimal_text(
        &TextBuf::new(&dec.to_string()),
        &TextBuf::new(&dec.normalize().to_string()),
        unmasked,
        masked_digits,
    )
}

fn matches_sensitive_amount(
    amount: &crate::Amount,
    unmasked: &TextBuf,
    masked_digits: &TextBuf,
) -> RuleMatch {
    let dec = amount.as_decimal();
    matches_sensitive_decimal_text(
        &TextBuf::new(&dec.to_string()),
        &TextBuf::new(&dec.normalize().to_string()),
        unmasked,
        masked_digits,
    )
}

fn matches_sensitive_context(
    prepared: &PreparedQuery,
    masked: &TextBuf,
    context: &WebSearchSensitiveContext,
) -> RuleMatch {
    match matches_sensitive_accounts(prepared, context) {
        RuleMatch::Matched => return RuleMatch::Matched,
        RuleMatch::NoMatch => {}
    }
    let unmasked = prepared.unmasked_digits();
    let masked_digits_raw: String = masked
        .as_str()
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let masked_digits = TextBuf::new(&masked_digits_raw);
    for qty in context.holding_quantities() {
        if matches_sensitive_quantity(qty, unmasked, &masked_digits) == RuleMatch::Matched {
            return RuleMatch::Matched;
        }
    }
    for amount in context.holding_amounts() {
        if matches_sensitive_amount(amount, unmasked, &masked_digits) == RuleMatch::Matched {
            return RuleMatch::Matched;
        }
    }
    let compact = prepared.compact_alnum().as_str();
    let folded = prepared.folded().as_str();
    for name in context.personal_names() {
        let lower = name.as_str().to_lowercase();
        let compact_name: String = lower.chars().filter(|ch| ch.is_alphanumeric()).collect();
        if folded.contains(&lower) || compact.contains(&compact_name) {
            return RuleMatch::Matched;
        }
    }
    RuleMatch::NoMatch
}

pub(super) fn evaluate_rules(
    prepared: &PreparedQuery,
    context: &WebSearchSensitiveContext,
    reasons: &mut BTreeSet<WebSearchRejectionReason>,
) {
    let folded = prepared.folded();
    let collapsed = collapse_spaced_single_chars(folded);
    let punct_stripped = strip_intra_token_punct(folded);
    match is_url_or_domain(folded) {
        RuleMatch::Matched => {
            reasons.insert(WebSearchRejectionReason::UrlOrDomainShape);
        }
        RuleMatch::NoMatch => {}
    }
    let masked = mask_year_quarter_tokens(folded);
    match has_remaining_multi_digits(&masked) {
        RuleMatch::Matched => {
            reasons.insert(WebSearchRejectionReason::MultiDigitNumber);
        }
        RuleMatch::NoMatch => {}
    }
    let spelled_hit = has_korean_sino_number(folded) == RuleMatch::Matched
        || has_korean_sino_number(&collapsed) == RuleMatch::Matched
        || has_native_korean_number(folded) == RuleMatch::Matched
        || has_native_korean_number(&collapsed) == RuleMatch::Matched
        || has_native_korean_number(&punct_stripped) == RuleMatch::Matched
        || has_hanja_number(folded) == RuleMatch::Matched
        || has_english_number_words(folded) == RuleMatch::Matched
        || has_english_number_words(&collapsed) == RuleMatch::Matched
        || has_english_number_words(&punct_stripped) == RuleMatch::Matched;
    if spelled_hit {
        reasons.insert(WebSearchRejectionReason::SpelledOutNumber);
    }
    match has_money_quantity_or_account_term(folded, &collapsed, &punct_stripped) {
        RuleMatch::Matched => {
            reasons.insert(WebSearchRejectionReason::MoneyQuantityOrAccountTerm);
        }
        RuleMatch::NoMatch => {}
    }
    match matches_sensitive_context(prepared, &masked, context) {
        RuleMatch::Matched => {
            reasons.insert(WebSearchRejectionReason::SensitiveContextMatch);
        }
        RuleMatch::NoMatch => {}
    }
}
