use crate::{Currency, Price, Symbol, Timestamp};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    symbol: Symbol,
    currency: Currency,
    last_price: Option<Price>,
    timestamp: Option<Timestamp>,
    collected_at: Timestamp,
    clock_alignment: ClockAlignment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteError {
    CurrencyMismatch,
    FutureTimestamp,
}

impl Quote {
    pub fn try_new(
        symbol: Symbol,
        currency: Currency,
        last_price: Option<Price>,
        timestamp: Option<Timestamp>,
        collected_at: Timestamp,
    ) -> Result<Self, QuoteError> {
        if last_price.is_some_and(|price| price.as_money().currency() != currency) {
            return Err(QuoteError::CurrencyMismatch);
        }
        let clock_alignment = timestamp
            .map_or(Ok(ClockAlignment::Aligned), |value| {
                assess_time(value, collected_at, TimestampAllowance::Observation)
            })
            .map_err(|TimeAssessmentError::TooFarAhead| QuoteError::FutureTimestamp)?;
        Ok(Self {
            symbol,
            currency,
            last_price,
            timestamp,
            collected_at,
            clock_alignment,
        })
    }
    #[must_use]
    pub const fn clock_alignment(&self) -> ClockAlignment {
        self.clock_alignment
    }
    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
    #[must_use]
    pub const fn last_price(&self) -> Option<Price> {
        self.last_price
    }
    #[must_use]
    pub const fn timestamp(&self) -> Option<Timestamp> {
        self.timestamp
    }
    #[must_use]
    pub const fn collected_at(&self) -> Timestamp {
        self.collected_at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockAlignment {
    Aligned,
    AheadWithinTolerance,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TimestampAllowance {
    Observation,
    MinuteClose,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TimeAssessmentError {
    TooFarAhead,
}

pub(crate) fn assess_time(
    data: Timestamp,
    collected: Timestamp,
    allowance: TimestampAllowance,
) -> Result<ClockAlignment, TimeAssessmentError> {
    let ordinary_seconds = match allowance {
        TimestampAllowance::Observation => 0_i64,
        TimestampAllowance::MinuteClose => 60_i64,
    };
    let seconds = data
        .utc_seconds()
        .as_i64()
        .checked_sub(*collected.utc_seconds().as_i64());
    let position = seconds.map(|value| (value, *data.nanosecond().as_u32()));
    if position.is_some_and(|value| value <= (ordinary_seconds, *collected.nanosecond().as_u32())) {
        return Ok(ClockAlignment::Aligned);
    }
    let tolerated = ordinary_seconds
        .checked_add(2)
        .map(|limit| (limit, *collected.nanosecond().as_u32()));
    if position
        .zip(tolerated)
        .is_some_and(|(value, limit)| value <= limit)
    {
        return Ok(ClockAlignment::AheadWithinTolerance);
    }
    Err(TimeAssessmentError::TooFarAhead)
}

#[cfg(test)]
mod tests {
    use super::{Quote, QuoteError};
    use crate::candle::tests::{at, price};
    use crate::{Currency, Symbol};
    #[test]
    fn quote_preserves_nullable_time_missing_price_currency_and_collection_time() {
        let symbol = Symbol::try_new("AAPL").expect("종목");
        for last in [None, Some(price("123", Currency::Usd))] {
            for stamp in [None, Some(at(0)), Some(at(1))] {
                let value = Quote::try_new(symbol.clone(), Currency::Usd, last, stamp, at(1))
                    .expect("시세");
                assert_eq!(value.symbol(), &symbol);
                assert_eq!(value.currency(), Currency::Usd);
                assert_eq!(value.last_price(), last);
                assert_eq!(value.timestamp(), stamp);
                assert_eq!(value.collected_at(), at(1));
            }
        }
        assert_eq!(
            Quote::try_new(
                symbol.clone(),
                Currency::Krw,
                Some(price("1", Currency::Usd)),
                None,
                at(1)
            ),
            Err(QuoteError::CurrencyMismatch)
        );
        assert_eq!(
            Quote::try_new(symbol, Currency::Usd, None, Some(at(2)), at(1)),
            Err(QuoteError::FutureTimestamp)
        );
    }
    #[test]
    fn small_clock_skew_is_preserved_but_two_seconds_is_the_limit() {
        use crate::{ClockAlignment, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
        for (ahead, nanos, expected) in [
            (0, 0, Ok(ClockAlignment::Aligned)),
            (0, 1, Ok(ClockAlignment::AheadWithinTolerance)),
            (1, 999_999_999, Ok(ClockAlignment::AheadWithinTolerance)),
            (2, 0, Ok(ClockAlignment::AheadWithinTolerance)),
            (2, 1, Err(QuoteError::FutureTimestamp)),
        ] {
            let stamp = Timestamp::from_parts(
                UnixSeconds::new(at(0).utc_seconds().as_i64().checked_add(ahead).expect("초")),
                Nanosecond::new(nanos),
                OffsetSeconds::new(0),
            )
            .expect("시각");
            let result = Quote::try_new(
                Symbol::try_new("AAPL").expect("심볼"),
                Currency::Usd,
                None,
                Some(stamp),
                at(0),
            )
            .map(|quote| quote.clock_alignment());
            assert_eq!(result, expected);
        }
    }
}
