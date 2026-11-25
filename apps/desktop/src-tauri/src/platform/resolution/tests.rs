use std::sync::atomic::{AtomicUsize, Ordering};

use application::PortError;
use application::resolve_estimates::ResolutionDataIssue;
use domain::{
    CalendarDate, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime, Currency,
    EstimateOutcome, EstimateTask, KnownMarket, MarketCalendar, MarketSession, Nanosecond,
    NumericInput, OffsetSeconds, Ohlcv, OutcomeState, OutcomeUnavailable, Price, PriceAdjustment,
    Quantity, QuoteInstrument, SessionData, SessionInformation, SpanSeconds, TimeInterval,
    TimeSpan, Timestamp, UnixSeconds,
};

use super::{ResolutionData, ResolutionSource};
use crate::ipc::state::BoxFuture;

const DAY: i64 = 86_400;
const KST: i32 = 32_400;
const SESSION_SECONDS: u64 = 23_400;

fn kst(utc: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(utc),
        Nanosecond::new(0),
        OffsetSeconds::new(KST),
    )
    .expect("시각")
}

fn opening(day: i64) -> Timestamp {
    kst(day.checked_mul(DAY).expect("초"))
}

fn row(day: i64) -> MarketSession {
    let start = opening(day);
    let end = start
        .checked_add(
            TimeSpan::try_new(SpanSeconds::new(SESSION_SECONDS), Nanosecond::new(0)).expect("길이"),
        )
        .expect("끝");
    MarketSession::try_new(
        start.offset_calendar_date(),
        SessionInformation::Kr {
            pre_market: None,
            regular_market: Some(
                SessionData::try_new(TimeInterval::try_new(start, end).expect("구간"), None)
                    .expect("장"),
            ),
            after_market: None,
        },
    )
    .expect("행")
}

fn day_of(date: CalendarDate) -> i64 {
    (0_i64..200)
        .find(|day| opening(*day).offset_calendar_date() == date)
        .expect("날짜")
}

fn price(text: &str) -> Price {
    Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("가격")
}

fn bar(
    quote: &QuoteInstrument,
    at: Timestamp,
    period: CandlePeriod,
    adjustment: PriceAdjustment,
    close: &str,
) -> Candle {
    let level = price(close);
    Candle::try_new(
        quote.saved_state().instrument_id,
        CandleTime::try_new(at, period).expect("봉 시각"),
        adjustment,
        Ohlcv::try_new(
            level,
            level,
            level,
            price(close),
            Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
        )
        .expect("값"),
        CandleObservation::new(at, CandleStatus::Finalized),
    )
    .expect("봉")
}

struct FakeToss {
    raw_close: &'static str,
    fail_candles: bool,
    calendar_calls: AtomicUsize,
}

impl FakeToss {
    fn new(raw_close: &'static str) -> Self {
        Self {
            raw_close,
            fail_candles: false,
            calendar_calls: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.calendar_calls.load(Ordering::SeqCst)
    }
}

fn unavailable() -> PortError {
    PortError::new(
        application::ReadEndpoint::Calendar,
        application::HttpStatus::NotReceived,
        application::PortErrorKind::Unavailable {
            cause: application::UnavailableCause::Network,
            retry: application::RetryDelay::Unknown,
        },
    )
}

impl ResolutionSource for FakeToss {
    fn candles<'a>(
        &'a self,
        instrument: &'a QuoteInstrument,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> BoxFuture<'a, Result<Vec<Candle>, PortError>> {
        let close = match adjustment {
            PriceAdjustment::Raw => self.raw_close,
            PriceAdjustment::Adjusted(_) => "100",
        };
        let failing = self.fail_candles;
        Box::pin(async move {
            if failing {
                return Err(unavailable());
            }
            let bars = match period {
                CandlePeriod::Daily => (0_i64..200)
                    .map(|day| {
                        kst(day
                            .checked_mul(DAY)
                            .and_then(|seconds| seconds.checked_sub(i64::from(KST)))
                            .expect("자정"))
                    })
                    .filter(|at| *at >= window.start() && *at < window.end())
                    .map(|at| bar(instrument, at, period, adjustment, close))
                    .collect(),
                CandlePeriod::Minute => {
                    let first = *window.start().utc_seconds().as_i64();
                    let last = *window.end().utc_seconds().as_i64();
                    (first.checked_add(60).expect("분")..=last)
                        .filter(|seconds| seconds.rem_euclid(60) == first.rem_euclid(60))
                        .map(|seconds| bar(instrument, kst(seconds), period, adjustment, "100"))
                        .collect()
                }
            };
            Ok(bars)
        })
    }

    fn calendar_on(
        &self,
        market: KnownMarket,
        date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>> {
        let _previous = self.calendar_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            assert_eq!(market, KnownMarket::Kr);
            let day = day_of(date);
            MarketCalendar::try_new(
                row(day.checked_sub(1).expect("전날")),
                row(day),
                row(day.checked_add(1).expect("다음날")),
            )
            .map_err(|_error| unavailable())
        })
    }
}

fn task() -> EstimateTask {
    testkit_ports::estimate_fixtures::sample_cost_eligible_task()
        .expect("표본 과제")
        .task()
        .clone()
}

#[test]
fn after_the_horizon_the_gathered_inputs_judge_the_estimate_and_calendars_are_cached() {
    let data = ResolutionData::new(FakeToss::new("100"));
    let task = task();
    let now = opening(75);
    let inputs = tauri::async_runtime::block_on(data.gather(&task, now)).expect("판정 입력");
    let state = EstimateOutcome::evaluate_with_complete_data(
        &task,
        &inputs.calendars,
        &inputs.adjustment,
        &inputs.candles,
        now,
    )
    .state();
    assert!(
        state != OutcomeState::Ongoing
            && state != OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
            && state != OutcomeState::Unavailable(OutcomeUnavailable::MissingCandles),
        "{state:?}"
    );
    let first = data.source.calls();
    let _again = tauri::async_runtime::block_on(data.gather(&task, now)).expect("다시");
    assert_eq!(
        data.source.calls(),
        first,
        "같은 날짜 일정은 다시 묻지 않는다"
    );
}

#[test]
fn before_the_horizon_the_estimate_stays_open() {
    let data = ResolutionData::new(FakeToss::new("100"));
    let task = task();
    let now = opening(61);
    let inputs = tauri::async_runtime::block_on(data.gather(&task, now)).expect("판정 입력");
    let state = EstimateOutcome::evaluate_with_complete_data(
        &task,
        &inputs.calendars,
        &inputs.adjustment,
        &inputs.candles,
        now,
    )
    .state();
    assert_eq!(state, OutcomeState::Ongoing);
}

#[test]
fn adjusted_prices_that_differ_from_raw_prices_are_not_judged_without_corporate_actions() {
    let data = ResolutionData::new(FakeToss::new("50"));
    let error = tauri::async_runtime::block_on(data.gather(&task(), opening(75)))
        .expect_err("기업 행위 자료 없음");
    assert_eq!(
        error,
        ResolutionDataIssue::Incomplete(OutcomeUnavailable::Adjustment)
    );
    assert_eq!(data.source.calls(), 0, "일정을 묻기 전에 멈춘다");
}

#[test]
fn a_failed_toss_read_is_reported_as_a_read_error_for_a_later_retry() {
    let mut toss = FakeToss::new("100");
    toss.fail_candles = true;
    let data = ResolutionData::new(toss);
    let error =
        tauri::async_runtime::block_on(data.gather(&task(), opening(75))).expect_err("읽기 실패");
    assert!(matches_read(&error), "{error:?}");
}

const fn matches_read(error: &ResolutionDataIssue) -> bool {
    match error {
        ResolutionDataIssue::Read(_) => true,
        ResolutionDataIssue::Incomplete(_)
        | ResolutionDataIssue::Storage(_)
        | ResolutionDataIssue::Calibration(_) => false,
    }
}
