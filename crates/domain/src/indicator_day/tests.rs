use super::{IndicatorDayAge, IndicatorDayError, indicator_day_age, settle_indicator_day};
use crate::{
    CalendarDate, CalendarYear, CandlePeriod, DayNumber, IndicatorCandleRow, IndicatorCandleStatus,
    MarketIndicatorCandle, MarketSession, MasterCode, MonthNumber, Nanosecond, NumericInput,
    OffsetSeconds, SessionData, SessionInformation, SpanSeconds, TimeInterval, TimeSpan, Timestamp,
};

fn june(day: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(2026),
        MonthNumber::new(6),
        DayNumber::new(day),
    )
    .expect("날짜")
}

fn kst(day: u8, minutes: u64) -> Timestamp {
    june(day)
        .start_at_offset(OffsetSeconds::new(32_400))
        .expect("자정")
        .checked_add(
            TimeSpan::try_new(
                SpanSeconds::new(minutes.saturating_mul(60)),
                Nanosecond::new(0),
            )
            .expect("간격"),
        )
        .expect("시각")
}

fn number(text: &str) -> NumericInput {
    NumericInput::try_new(text).expect("수치")
}

fn daily(day: u8, collected: Timestamp, period: CandlePeriod) -> MarketIndicatorCandle {
    MarketIndicatorCandle::from_response(
        MasterCode::try_new("KOSPI").expect("지수"),
        period,
        &IndicatorCandleRow {
            timestamp: kst(day, 540),
            open: number("2800"),
            high: number("2820"),
            low: number("2790"),
            close: number("2810"),
            volume: number("10"),
        },
        collected,
    )
    .expect("일봉")
}

fn open_day(day: u8) -> MarketSession {
    MarketSession::try_new(
        june(day),
        SessionInformation::Kr {
            pre_market: None,
            regular_market: Some(
                SessionData::try_new(
                    TimeInterval::try_new(kst(day, 540), kst(day, 930)).expect("정규장"),
                    None,
                )
                .expect("세션"),
            ),
            after_market: None,
        },
    )
    .expect("장")
}

fn settled(
    candle: MarketIndicatorCandle,
    session: Option<&MarketSession>,
) -> Result<IndicatorCandleStatus, IndicatorDayError> {
    settle_indicator_day(candle, session).map(|value| value.status())
}

#[test]
fn a_day_old_index_candle_is_final_without_a_session() {
    let candle = daily(10, kst(11, 0), CandlePeriod::Daily);
    assert_eq!(indicator_day_age(&candle), Ok(IndicatorDayAge::Passed));
    assert_eq!(settled(candle, None), Ok(IndicatorCandleStatus::Finalized));
}

#[test]
fn todays_index_candle_follows_the_regular_session() {
    let during = daily(11, kst(11, 600), CandlePeriod::Daily);
    assert_eq!(
        indicator_day_age(&during),
        Ok(IndicatorDayAge::Current(june(11)))
    );
    assert_eq!(
        settled(during.clone(), Some(&open_day(11))),
        Ok(IndicatorCandleStatus::InProgress)
    );
    let after = daily(11, kst(11, 960), CandlePeriod::Daily);
    assert_eq!(
        settled(after, Some(&open_day(11))),
        Ok(IndicatorCandleStatus::Finalized)
    );
    assert_eq!(
        settled(during.clone(), None),
        Err(IndicatorDayError::MissingSession)
    );
    assert_eq!(
        settled(during, Some(&open_day(10))),
        Err(IndicatorDayError::SessionMismatch)
    );
}

#[test]
fn minute_index_candles_keep_their_own_finality() {
    let minute = daily(11, kst(11, 600), CandlePeriod::Minute);
    let status = minute.status();
    assert_eq!(indicator_day_age(&minute), Ok(IndicatorDayAge::Intraday));
    assert_eq!(settled(minute, None), Ok(status));
}
