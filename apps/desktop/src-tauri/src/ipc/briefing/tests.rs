use std::fs;
use std::path::Path;
use std::sync::Arc;

use application::{
    BriefingFailure, BriefingOutcome, BriefingRecord, BriefingSource, RetryDelay, SystemFailure,
    SystemPort, SystemPortError,
};
use domain::{
    BriefingId, BriefingKind, CalendarDate, CalendarYear, DayNumber, InformationSource,
    MonthNumber, Nanosecond, NewsSource, NormalizedUrl, OffsetSeconds, Timestamp, UnixSeconds,
    UntrustedText,
};
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::super::error::IpcErrorKind;
use super::super::state::BoxFuture;
use super::{
    BriefingHistory, BriefingIpcState, BriefingsScope, GetBriefingsRequest, UiBriefingFailure,
    UiBriefingSourceKind, UiBriefingStatus, get_briefings, load_briefings_view, to_briefings_view,
};

struct FixedHistory(Result<Vec<BriefingRecord>, SystemPortError>);

impl BriefingHistory for FixedHistory {
    fn recent(&self) -> BoxFuture<'_, Result<Vec<BriefingRecord>, SystemPortError>> {
        let value = self.0.clone();
        Box::pin(async move { value })
    }
}

fn date(day: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(2026),
        MonthNumber::new(11),
        DayNumber::new(day),
    )
    .expect("date")
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("timestamp")
}

fn text(value: &str) -> UntrustedText {
    UntrustedText::try_new(value).expect("text")
}

const NOV_24_2320_UTC: i64 = 1_795_562_400;

fn completed(day: u8, seconds: i64) -> BriefingRecord {
    BriefingRecord {
        id: BriefingId::new(BriefingKind::Morning, date(day)),
        finished_at: at(seconds),
        outcome: BriefingOutcome::Completed {
            body: text("밤사이 미국장은 올랐습니다 [1]"),
            sources: vec![
                BriefingSource {
                    title: text("연합 기사"),
                    url: Some(NormalizedUrl::try_new("https://example.com/a").expect("url")),
                    origin: InformationSource::News(NewsSource::Yonhap),
                    observed_at: at(seconds),
                },
                BriefingSource {
                    title: text("웹 요약"),
                    url: None,
                    origin: InformationSource::Web,
                    observed_at: at(seconds),
                },
            ],
        },
    }
}

fn failed(day: u8, seconds: i64) -> BriefingRecord {
    BriefingRecord {
        id: BriefingId::new(BriefingKind::ClosingReview, date(day)),
        finished_at: at(seconds),
        outcome: BriefingOutcome::Failed(BriefingFailure::IsolationRejected),
    }
}

#[test]
fn newest_briefing_is_latest_and_the_rest_are_past_in_descending_order() {
    let older = completed(23, NOV_24_2320_UTC - 86_400);
    let newest = completed(25, NOV_24_2320_UTC);
    let middle = failed(24, NOV_24_2320_UTC - 3_600);
    let view = to_briefings_view(&[older, newest, middle]);
    let latest = view.latest.expect("latest");
    assert_eq!(latest.id.date, "2026-11-25");
    assert_eq!(latest.finished_at, "2026-11-25T08:20:00+09:00");
    assert_eq!(latest.status, UiBriefingStatus::Completed);
    assert_eq!(latest.failure, UiBriefingFailure::None);
    let cited = &latest.sources;
    assert_eq!(cited.len(), 2);
    assert_eq!(cited[0].kind, UiBriefingSourceKind::News);
    assert_eq!(cited[0].url.as_deref(), Some("https://example.com/a"));
    assert_eq!(cited[1].kind, UiBriefingSourceKind::Web);
    assert_eq!(cited[1].url, None);
    let past: Vec<&str> = view.past.iter().map(|item| item.id.date.as_str()).collect();
    assert_eq!(past, vec!["2026-11-24", "2026-11-23"]);
    assert_eq!(view.past[0].status, UiBriefingStatus::Failed);
    assert_eq!(view.past[0].failure, UiBriefingFailure::AiUnavailable);
    assert_eq!(view.past[0].body, "");
    let past_cited = &view.past[0].sources;
    assert_eq!(past_cited, &Vec::new());
}

#[test]
fn empty_history_has_no_latest_and_no_past() {
    let view = to_briefings_view(&[]);
    assert_eq!(view.latest, None);
    assert_eq!(view.past, Vec::new());
}

#[test]
fn past_list_is_capped() {
    let records: Vec<BriefingRecord> = (0..40_i64)
        .map(|index| completed(25, NOV_24_2320_UTC - index * 60))
        .collect();
    let view = to_briefings_view(&records);
    assert!(view.latest.is_some());
    assert_eq!(view.past.len(), 30);
}

#[test]
fn storage_failure_and_missing_state_surface_as_ipc_errors() {
    let unavailable = BriefingIpcState::new(Arc::new(FixedHistory(Err(SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    )))));
    let error = tauri::async_runtime::block_on(load_briefings_view(&unavailable))
        .expect_err("storage error");
    assert_ne!(error.kind, IpcErrorKind::Bug);

    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let missing = tauri::async_runtime::block_on(get_briefings(
        app.handle().clone(),
        GetBriefingsRequest {
            scope: BriefingsScope::Recent,
        },
    ))
    .expect_err("no state");
    assert_eq!(missing.kind, IpcErrorKind::NoData);
}

#[test]
fn get_briefings_response_is_recorded_for_the_typescript_fake() {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _managed = tauri::Manager::manage(
        &app,
        BriefingIpcState::new(Arc::new(FixedHistory(Ok(vec![
            failed(24, NOV_24_2320_UTC - 3_600),
            completed(25, NOV_24_2320_UTC),
        ])))),
    );
    let view = tauri::async_runtime::block_on(get_briefings(
        app.handle().clone(),
        GetBriefingsRequest {
            scope: BriefingsScope::Recent,
        },
    ))
    .expect("view");
    let path = Path::new("ipc/responses").join("get_briefings.json");
    let mut actual = serde_json::to_string_pretty(&view).expect("serialize");
    actual.push('\n');
    let recorded = fs::read_to_string(&path).expect("read recorded response");
    assert_eq!(recorded, actual, "{} differs", path.display());
}
