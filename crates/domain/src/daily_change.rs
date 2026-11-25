use rust_decimal::Decimal;

use crate::exact_arithmetic::divide;
use crate::{Candle, DisplayDirection, MoneyError, Price, RatioQuotient, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyChange {
    rate: RatioQuotient,
    direction: DisplayDirection,
}

impl DailyChange {
    #[must_use]
    pub const fn rate(&self) -> RatioQuotient {
        self.rate
    }

    #[must_use]
    pub const fn direction(&self) -> DisplayDirection {
        self.direction
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DailyChangeError {
    CurrencyMismatch,
    NonPositiveBase,
    Arithmetic(MoneyError),
}

#[must_use]
pub fn previous_session_close(daily: &[Candle], at: Timestamp) -> Option<Price> {
    daily
        .iter()
        .filter(|candle| {
            at.date_at_offset(candle.time().timestamp().offset_seconds())
                .is_ok_and(|today| candle.time().date() < today)
        })
        .max_by_key(|candle| candle.time().timestamp())
        .map(|candle| candle.values().close())
}

pub fn daily_change(
    previous_close: &Price,
    current: &Price,
) -> Result<DailyChange, DailyChangeError> {
    let base = previous_close.as_money();
    let latest = current.as_money();
    if base.currency() != latest.currency() {
        return Err(DailyChangeError::CurrencyMismatch);
    }
    if base.amount().as_decimal() <= Decimal::ZERO {
        return Err(DailyChangeError::NonPositiveBase);
    }
    let moved = latest
        .checked_sub(&base)
        .map_err(DailyChangeError::Arithmetic)?;
    let rate = divide(moved.amount(), base.amount()).map_err(DailyChangeError::Arithmetic)?;
    let direction = match moved.amount().as_decimal().cmp(&Decimal::ZERO) {
        std::cmp::Ordering::Greater => DisplayDirection::Up,
        std::cmp::Ordering::Less => DisplayDirection::Down,
        std::cmp::Ordering::Equal => DisplayDirection::Flat,
    };
    Ok(DailyChange { rate, direction })
}

#[cfg(test)]
mod tests;
