use crate::{
    CalendarDate, CandlePeriod, CandleStatus, IndicatorCandleStatus, Market, MarketIndicatorCandle,
    MarketSession, OffsetSeconds, TimeSpan,
};

const KST: OffsetSeconds = OffsetSeconds::new(32_400);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorDayAge {
    Passed,
    Current(CalendarDate),
    Intraday,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorDayError {
    InvalidTime,
    MissingSession,
    SessionMismatch,
}

pub fn indicator_day_age(
    candle: &MarketIndicatorCandle,
) -> Result<IndicatorDayAge, IndicatorDayError> {
    if candle.period() != CandlePeriod::Daily {
        return Ok(IndicatorDayAge::Intraday);
    }
    let date = candle
        .start
        .date_at_offset(KST)
        .or(Err(IndicatorDayError::InvalidTime))?;
    let day_end = date
        .start_at_offset(KST)
        .ok()
        .and_then(|start| start.checked_add(TimeSpan::DAY).ok())
        .ok_or(IndicatorDayError::InvalidTime)?;
    if day_end <= candle.observation().collected_at() {
        Ok(IndicatorDayAge::Passed)
    } else {
        Ok(IndicatorDayAge::Current(date))
    }
}

pub fn settle_indicator_day(
    mut candle: MarketIndicatorCandle,
    session: Option<&MarketSession>,
) -> Result<MarketIndicatorCandle, IndicatorDayError> {
    match indicator_day_age(&candle)? {
        IndicatorDayAge::Intraday => Ok(candle),
        IndicatorDayAge::Passed => {
            candle.status = IndicatorCandleStatus::Finalized;
            Ok(candle)
        }
        IndicatorDayAge::Current(date) => {
            let today = session.ok_or(IndicatorDayError::MissingSession)?;
            if today.market() != Market::Kr || today.date() != date {
                return Err(IndicatorDayError::SessionMismatch);
            }
            let status =
                crate::regular_session_candle_status(today, candle.observation().collected_at())
                    .or(Err(IndicatorDayError::SessionMismatch))?;
            candle.status = match status {
                CandleStatus::Finalized => IndicatorCandleStatus::Finalized,
                CandleStatus::InProgress => IndicatorCandleStatus::InProgress,
            };
            Ok(candle)
        }
    }
}

#[cfg(test)]
mod tests;
