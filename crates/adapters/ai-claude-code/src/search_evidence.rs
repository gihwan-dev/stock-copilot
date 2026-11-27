use application::{SearchObservation, SourceDateKind};
use domain::{CalendarDate, CalendarYear, DayNumber, MonthNumber, NormalizedUrl, UntrustedText};
use serde_json::Value;

fn calendar_date(value: &str) -> Option<CalendarDate> {
    let (year, rest) = value.split_once('-')?;
    let (month, day) = rest.split_once('-')?;
    if (year.len(), month.len(), day.len()) != (4, 2, 2)
        || ![year, month, day]
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    CalendarDate::from_parts(
        CalendarYear::new(year.parse().ok()?),
        MonthNumber::new(month.parse().ok()?),
        DayNumber::new(day.parse().ok()?),
    )
    .ok()
}

fn source_date(value: &str) -> Option<CalendarDate> {
    if let Some(date) = calendar_date(value) {
        return Some(date);
    }
    let (month_name, rest) = value.split_once(' ')?;
    let (day, year) = rest.split_once(", ")?;
    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let month = months
        .iter()
        .position(|name| *name == month_name)?
        .checked_add(1)?;
    if day.is_empty()
        || day.len() > 2
        || year.len() != 4
        || !day
            .bytes()
            .chain(year.bytes())
            .all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    CalendarDate::from_parts(
        CalendarYear::new(year.parse().ok()?),
        MonthNumber::new(u8::try_from(month).ok()?),
        DayNumber::new(day.parse().ok()?),
    )
    .ok()
}

fn dated_source(record: &Value, title: &str) -> Option<(SourceDateKind, CalendarDate)> {
    for (field, kind) in [
        ("published_at", SourceDateKind::Published),
        ("observed_at", SourceDateKind::Observed),
        ("page_age", SourceDateKind::Updated),
    ] {
        if let Some(value) = record.get(field).filter(|value| !value.is_null()) {
            return Some((kind, source_date(value.as_str()?)?));
        }
    }
    for (marker, kind) in [
        ("as of ", SourceDateKind::Observed),
        ("published ", SourceDateKind::Published),
        ("관측일 ", SourceDateKind::Observed),
        ("보도일 ", SourceDateKind::Published),
    ] {
        if let Some((_, rest)) = title.to_ascii_lowercase().split_once(marker) {
            let date = rest
                .split(|character: char| !character.is_ascii_digit() && character != '-')
                .next()?;
            return Some((kind, calendar_date(date)?));
        }
    }
    None
}

fn observation(record: &Value, query: &UntrustedText) -> Option<SearchObservation> {
    if let Some(kind) = record.get("type")
        && kind.as_str() != Some("web_search_result")
    {
        return None;
    }
    let title = record.get("title")?.as_str()?.trim();
    if title.is_empty() || title.chars().count() > 400 {
        return None;
    }
    let url = NormalizedUrl::try_new(record.get("url")?.as_str()?).ok()?;
    let (date_kind, date) = dated_source(record, title)?;
    Some(SearchObservation {
        query: query.clone(),
        url,
        date,
        date_kind,
        title: UntrustedText::try_new(title).ok()?,
    })
}

pub(super) fn extract(value: &Value, query: &UntrustedText, out: &mut Vec<SearchObservation>) {
    if out.len() >= 20 {
        return;
    }
    match value {
        Value::Array(items) => {
            for item in items.iter().take(40) {
                extract(item, query, out);
            }
        }
        Value::Object(item) if item.get("type").and_then(Value::as_str) == Some("text") => {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                links(text, query, out);
            }
        }
        Value::Object(_) => {
            if let Some(item) = observation(value, query) {
                out.push(item);
            }
        }
        Value::String(text) => links(text, query, out),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

fn links(text: &str, query: &UntrustedText, out: &mut Vec<SearchObservation>) {
    for line in text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("Links:"))
    {
        let Ok(Value::Array(rows)) = serde_json::from_str(line.trim()) else {
            continue;
        };
        out.extend(
            rows.iter()
                .filter_map(|row| observation(row, query))
                .take(20_usize.saturating_sub(out.len())),
        );
    }
}

#[cfg(test)]
mod tests {
    use crate::{ClaudeEventDecoder, IsolationExpectation};
    use application::{RunKind, SourceDateKind};
    use serde_json::json;
    use std::collections::BTreeSet;

    #[test]
    fn market_outlook_uses_dated_rows_from_matched_successful_websearch_results() {
        let executable = testkit::test_executable().expect("실행 파일");
        let fixture = executable
            .ancestors()
            .map(|dir| dir.join("fixtures/ai/claude-collection.jsonl"))
            .find(|path| path.is_file())
            .expect("수집 fixture");
        let fixture_text = std::fs::read_to_string(fixture).expect("수집 fixture");

        let start = || {
            let expectation = IsolationExpectation::new(
                RunKind::Collection,
                BTreeSet::from(["WebSearch".to_owned()]),
                None,
                BTreeSet::from(["cc-plugin-privacy".to_owned()]),
                None,
            );
            let mut decoder = ClaudeEventDecoder::new(expectation);
            for line in fixture_text.lines().take(2) {
                decoder.decode_line(line).expect("격리와 도구 시작");
            }
            assert_eq!(decoder.line_observations().len(), 0_usize);
            decoder
        };
        let report = json!({"title":"Public market close","url":"https://news.example/close","page_age":"April 30, 2025"});
        let links = format!("Results\nLinks: [{report}]");
        for (content, is_error, expected) in [
            (json!([{"type":"text","text":links}]), false, 1),
            (json!([report]), false, 1),
            (json!("found 3 items"), false, 0),
            (json!([report]), true, 0),
            (
                json!([{"title":"2026-09-21 forecast","url":"https://news.example/old"}]),
                false,
                0,
            ),
            (
                json!([{"title":"Public close","url":"https://news.example/close","observed_at":"2025-02-29"}]),
                false,
                0,
            ),
            (
                json!([{"title":"Public close","url":"file:///secret","published_at":"2026-01-01"}]),
                false,
                0,
            ),
        ] {
            let mut decoder = start();
            decoder.decode_line(&json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_web_01","is_error":is_error,"content":content}]}}).to_string()).expect("도구 결과");
            let items = decoder.line_observations();
            assert_eq!(items.len(), expected);
            if let Some(item) = items.first() {
                assert_eq!(item.query.as_untrusted_str(), "KOSPI index today");
                assert_eq!(item.date_kind, SourceDateKind::Updated);
                assert_eq!(item.date.year().as_i32(), &2025_i32);
                assert_eq!(item.date.month().as_u8(), &4);
                assert_eq!(item.date.day().as_u8(), &30);
            }
        }
        let mut decoder = start();
        let _error = decoder.decode_line(&json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"other","content":[report]}]}}).to_string()).expect_err("짝 없는 결과");
        assert_eq!(decoder.line_observations().len(), 0_usize);
    }
}

#[cfg(test)]
mod date_tests {
    use super::{SourceDateKind, observation};
    use domain::UntrustedText;
    use serde_json::json;

    #[test]
    fn evidence_requires_valid_source_title_and_source_date() {
        let query = UntrustedText::try_new("public query").expect("질의");
        let row = json!({"type":"web_search_result","title":"Public index close", "url":"https://public.example/report","published_at":"2024-02-29"});
        for (field, value) in [
            ("type", json!("web_search_tool_result_error")),
            ("title", json!("   ")),
            ("title", json!("x".repeat(401))),
            ("url", json!("https://")),
            ("url", json!("javascript:alert")),
            ("published_at", json!("2025-02-29")),
            ("published_at", json!("2025-13-01")),
            ("published_at", json!("2025-11-31")),
            ("published_at", json!("25-01-01")),
            ("published_at", json!("2025-1-01")),
            ("published_at", json!("2025-01-1")),
            ("published_at", json!("2025-+1-01")),
            ("published_at", json!("0000-01-01")),
            ("published_at", json!("February 29, 2025")),
            ("published_at", json!("September 111, 2025")),
            ("published_at", json!("unknown")),
            ("published_at", json!(null)),
        ] {
            let mut bad = row.clone();
            bad[field] = value;
            assert!(observation(&bad, &query).is_none(), "{bad}");
        }
        for (key, date, kind) in [
            ("published_at", "2024-02-29", SourceDateKind::Published),
            ("observed_at", "2024-02-29", SourceDateKind::Observed),
            ("page_age", "February 29, 2024", SourceDateKind::Updated),
        ] {
            let item = observation(
                &json!({"title":"public report","url":"https://public.example/report",key:date}),
                &query,
            )
            .expect("날짜가 있는 출처");
            assert_eq!(item.date_kind, kind);
            assert_eq!(item.date.year().as_i32(), &2024_i32);
            assert_eq!(item.date.month().as_u8(), &2);
            assert_eq!(item.date.day().as_u8(), &29);
            assert_eq!(item.query, query);
            assert_eq!(item.url.as_str(), "https://public.example/report");
            assert_eq!(item.title.as_untrusted_str(), "public report");
        }
        for (title, kind) in [
            ("Market report as of 2024-02-29", SourceDateKind::Observed),
            ("Report published 2024-02-29", SourceDateKind::Published),
            ("지수 관측일 2024-02-29", SourceDateKind::Observed),
            ("종가 보도일 2024-02-29", SourceDateKind::Published),
        ] {
            let item = observation(
                &json!({"title":title,"url":"https://public.example/report"}),
                &query,
            )
            .expect("제목 속 명시한 날짜");
            assert_eq!(item.date_kind, kind);
        }
    }
}
