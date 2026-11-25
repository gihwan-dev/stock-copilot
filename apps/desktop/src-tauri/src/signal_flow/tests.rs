use super::{next_regular_open, occurrence_id, proposal_id, settled_daily, task_candle_window};
use domain::{InstrumentId, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
use testkit_ports::estimate_fixtures::{
    sample_daily_candles, sample_day, sample_kr_calendars, sample_local,
};

fn at(seconds: i64, nanos: u32) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn shifted(base: Timestamp, seconds: i64) -> Timestamp {
    at(
        base.utc_seconds()
            .as_i64()
            .checked_add(seconds)
            .expect("시각 이동"),
        0,
    )
}

#[test]
fn proposal_and_occurrence_ids_fit_their_formats_and_differ_per_instant() {
    let instrument = InstrumentId::try_new("kr:005930/very-long-identifier-value").expect("ID");
    let first = proposal_id(&instrument, at(1_790_000_000, 1)).expect("제안 ID");
    let second = proposal_id(&instrument, at(1_790_000_000, 2)).expect("제안 ID");
    assert_ne!(first, second);
    assert!(first.as_client_order_id().as_str().len() <= 36);
    assert!(
        first
            .as_client_order_id()
            .as_str()
            .starts_with("p-kr005930")
    );
    let occurrence = occurrence_id(&instrument, at(1_790_000_000, 5)).expect("발생 ID");
    assert!(occurrence.as_str().starts_with("occ-kr005930-1790000000-5"));
}

#[test]
fn next_open_is_today_before_the_session_and_the_next_business_day_after_it() {
    let calendars = sample_kr_calendars(61, 61).expect("달력");
    let calendar = calendars.first().expect("하루");
    let open_today = sample_local(sample_day(61).expect("날")).expect("현지");
    assert_eq!(
        next_regular_open(calendar, shifted(open_today, -60)),
        Some(open_today)
    );
    let open_next = sample_local(sample_day(62).expect("날")).expect("현지");
    assert_eq!(
        next_regular_open(calendar, shifted(open_today, 30_000)),
        Some(open_next)
    );
}

#[test]
fn task_series_excludes_the_creation_day_and_ends_at_creation() {
    let created = sample_local(sample_day(62).expect("날")).expect("현지");
    let window = task_candle_window(created).expect("창");
    assert_eq!(window.end(), created);
    let candles = sample_daily_candles(63).expect("일봉");
    let settled = settled_daily(candles, created);
    assert_eq!(settled.len(), 62);
    assert!(
        settled
            .iter()
            .all(|candle| candle.time().date() < created.offset_calendar_date())
    );
}

#[test]
fn redraft_maps_draft_outcomes_without_sending_a_stale_proposal() {
    use approval_command::ports::RedraftOutcome;
    let kind = |outcome| match super::redraft_outcome(outcome, None) {
        RedraftOutcome::Redrafted(_) => "redrafted",
        RedraftOutcome::OutOfEstimateRange => "out-of-range",
        RedraftOutcome::MarketClosed => "closed",
        RedraftOutcome::NoSignal => "no-signal",
    };
    assert_eq!(
        kind(application::DraftProposalOutcome::NoSignal),
        "no-signal"
    );
    assert_eq!(
        kind(application::DraftProposalOutcome::MarketClosed),
        "closed"
    );
    assert_eq!(
        kind(application::DraftProposalOutcome::ReestimateScheduled {
            deliveries: Vec::new()
        }),
        "out-of-range"
    );
    let missing =
        domain::ProposalId::new(domain::ClientOrderId::try_new("p-missing-1").expect("제안"));
    assert_eq!(
        kind(application::DraftProposalOutcome::Existing {
            proposal_id: missing
        }),
        "no-signal",
        "읽을 수 없는 기존 제안은 보내지 않는다"
    );
}
