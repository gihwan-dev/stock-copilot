use rust_decimal::Decimal;

use super::{DailyChangeError, daily_change, previous_session_close};
use crate::candle::tests::{at, price, series};
use crate::{Currency, DisplayDirection, Nanosecond, RatioQuotient, SpanSeconds, TimeSpan};

fn later(day: i64, seconds: u64) -> crate::Timestamp {
    at(day)
        .checked_add(
            TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)).expect("간격"),
        )
        .expect("시각")
}

#[test]
fn previous_close_is_the_last_daily_close_before_the_quote_date_in_the_candle_offset() {
    let mut candles = series(&["100", "110", "121"]);
    candles.reverse();
    assert_eq!(
        previous_session_close(&candles, later(2, 3_600)),
        Some(price("110", Currency::Usd))
    );
    assert_eq!(
        previous_session_close(&candles, later(3, 60)),
        Some(price("121", Currency::Usd))
    );
    assert_eq!(previous_session_close(&candles, later(0, 60)), None);
    assert_eq!(previous_session_close(&[], later(2, 60)), None);
}

#[test]
fn daily_change_is_the_exact_move_over_the_previous_close_with_its_direction() {
    let up =
        daily_change(&price("110", Currency::Usd), &price("121", Currency::Usd)).expect("상승");
    assert_eq!(up.direction(), DisplayDirection::Up);
    assert_eq!(
        up.rate(),
        RatioQuotient::Exact(crate::Ratio::new(Decimal::new(1, 1)))
    );
    let down =
        daily_change(&price("100", Currency::Usd), &price("97.5", Currency::Usd)).expect("하락");
    assert_eq!(down.direction(), DisplayDirection::Down);
    assert_eq!(
        down.rate(),
        RatioQuotient::Exact(crate::Ratio::new(Decimal::new(-25, 3)))
    );
    let flat =
        daily_change(&price("100", Currency::Usd), &price("100", Currency::Usd)).expect("보합");
    assert_eq!(flat.direction(), DisplayDirection::Flat);
}

#[test]
fn daily_change_refuses_mixed_currencies() {
    assert_eq!(
        daily_change(&price("100", Currency::Usd), &price("100", Currency::Krw)),
        Err(DailyChangeError::CurrencyMismatch)
    );
}
