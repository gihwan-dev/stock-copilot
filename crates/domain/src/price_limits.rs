mod history;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceLimitsInput {
    pub instrument: QuoteInstrument,
    pub currency: Currency,
    pub lower: Option<Price>,
    pub upper: Option<Price>,
    pub timestamp: Timestamp,
    pub collected_at: Timestamp,
}
use crate::{ClockAlignment, Currency, KnownMarket, Price, QuoteInstrument, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceLimitRange {
    Limited { lower: Price, upper: Price },
    Unlimited,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceLimitCheck {
    Within,
    BelowLower,
    AboveUpper,
    Unlimited,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceLimitsError {
    CurrencyMismatch,
    DifferentInstrument,
    MissingLimits,
    UnexpectedLimits,
    ReversedLimits,
    FutureTimestamp,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriceLimits {
    instrument: QuoteInstrument,
    range: PriceLimitRange,
    timestamp: Timestamp,
    collected_at: Timestamp,
    clock_alignment: ClockAlignment,
}
impl PriceLimits {
    pub fn check_adjusted_move(
        instrument: &crate::MasterInstrument,
        date: crate::CalendarDate,
        previous: Price,
        current: Price,
    ) -> Result<PriceLimitCheck, PriceLimitsError> {
        history::check_adjusted_move(instrument, date, previous, current)
    }

    pub fn try_new(input: PriceLimitsInput) -> Result<Self, PriceLimitsError> {
        let PriceLimitsInput {
            instrument,
            currency,
            lower,
            upper,
            timestamp,
            collected_at,
        } = input;
        if instrument.currency() != currency
            || lower
                .into_iter()
                .chain(upper)
                .any(|candidate| candidate.as_money().currency() != currency)
        {
            return Err(PriceLimitsError::CurrencyMismatch);
        }
        let range = match (instrument.market(), lower, upper) {
            (KnownMarket::Kr, Some(lower_price), Some(upper_price)) => {
                if lower_price.as_money().amount().as_decimal()
                    > upper_price.as_money().amount().as_decimal()
                {
                    return Err(PriceLimitsError::ReversedLimits);
                }
                PriceLimitRange::Limited {
                    lower: lower_price,
                    upper: upper_price,
                }
            }
            (KnownMarket::Kr, None, _) | (KnownMarket::Kr, Some(_), None) => {
                return Err(PriceLimitsError::MissingLimits);
            }
            (KnownMarket::Us, None, None) => PriceLimitRange::Unlimited,
            (KnownMarket::Us, Some(_), _) | (KnownMarket::Us, None, Some(_)) => {
                return Err(PriceLimitsError::UnexpectedLimits);
            }
        };
        let clock_alignment = crate::quote::assess_time(
            timestamp,
            collected_at,
            crate::quote::TimestampAllowance::Observation,
        )
        .map_err(|crate::quote::TimeAssessmentError::TooFarAhead| {
            PriceLimitsError::FutureTimestamp
        })?;
        Ok(Self {
            instrument,
            range,
            timestamp,
            collected_at,
            clock_alignment,
        })
    }
    pub fn check(
        &self,
        instrument: &QuoteInstrument,
        price: Price,
    ) -> Result<PriceLimitCheck, PriceLimitsError> {
        if instrument != &self.instrument {
            return Err(PriceLimitsError::DifferentInstrument);
        }
        if price.as_money().currency() != self.instrument.currency() {
            return Err(PriceLimitsError::CurrencyMismatch);
        }
        Ok(match self.range {
            PriceLimitRange::Unlimited => PriceLimitCheck::Unlimited,
            PriceLimitRange::Limited { lower, upper } => {
                let value = price.as_money().amount().as_decimal();
                if value < lower.as_money().amount().as_decimal() {
                    PriceLimitCheck::BelowLower
                } else if value > upper.as_money().amount().as_decimal() {
                    PriceLimitCheck::AboveUpper
                } else {
                    PriceLimitCheck::Within
                }
            }
        })
    }
    #[must_use]
    pub const fn instrument(&self) -> &QuoteInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn range(&self) -> PriceLimitRange {
        self.range
    }
    #[must_use]
    pub const fn timestamp(&self) -> Timestamp {
        self.timestamp
    }
    #[must_use]
    pub const fn collected_at(&self) -> Timestamp {
        self.collected_at
    }
    #[must_use]
    pub const fn clock_alignment(&self) -> ClockAlignment {
        self.clock_alignment
    }
}

#[cfg(test)]
mod tests {
    use crate::orderbook::tests::{at, instrument, price};
    use crate::{
        ClockAlignment, Currency, KnownMarket, PriceLimitCheck, PriceLimitRange, PriceLimits,
        PriceLimitsError, PriceLimitsInput,
    };
    #[test]
    fn korean_limits_are_inclusive_and_keep_identity_and_times() {
        let lower = price("50", Currency::Krw);
        let upper = price("130", Currency::Krw);
        let value = PriceLimits::try_new(PriceLimitsInput {
            instrument: instrument(KnownMarket::Kr),
            currency: Currency::Krw,
            lower: Some(lower),
            upper: Some(upper),
            timestamp: at(0),
            collected_at: at(1),
        })
        .expect("제한");
        assert_eq!(value.instrument(), &instrument(KnownMarket::Kr));
        assert_eq!(value.range(), PriceLimitRange::Limited { lower, upper });
        assert_eq!(value.timestamp(), at(0));
        assert_eq!(value.collected_at(), at(1));
        assert_eq!(value.clock_alignment(), ClockAlignment::Aligned);
        for (candidate, expected) in [
            ("49", PriceLimitCheck::BelowLower),
            ("50", PriceLimitCheck::Within),
            ("80", PriceLimitCheck::Within),
            ("130", PriceLimitCheck::Within),
            ("131", PriceLimitCheck::AboveUpper),
        ] {
            assert_eq!(
                value.check(value.instrument(), price(candidate, Currency::Krw)),
                Ok(expected)
            );
        }
        assert_eq!(
            value.check(value.instrument(), price("80", Currency::Usd)),
            Err(PriceLimitsError::CurrencyMismatch)
        );
    }
    #[test]
    fn market_shapes_currency_order_and_future_time_are_validated() {
        let kr = instrument(KnownMarket::Kr);
        let candidate = price("1", Currency::Krw);
        for (lower, upper) in [
            (None, None),
            (Some(candidate), None),
            (None, Some(candidate)),
        ] {
            assert_eq!(
                PriceLimits::try_new(PriceLimitsInput {
                    instrument: kr.clone(),
                    currency: Currency::Krw,
                    lower,
                    upper,
                    timestamp: at(0),
                    collected_at: at(0)
                }),
                Err(PriceLimitsError::MissingLimits)
            );
        }
        let (lower, upper) = (Some(price("2", Currency::Krw)), Some(candidate));
        assert_eq!(
            PriceLimits::try_new(PriceLimitsInput {
                instrument: kr.clone(),
                currency: Currency::Krw,
                lower,
                upper,
                timestamp: at(0),
                collected_at: at(0)
            }),
            Err(PriceLimitsError::ReversedLimits)
        );
        let same = PriceLimits::try_new(PriceLimitsInput {
            instrument: kr,
            currency: Currency::Krw,
            lower: Some(candidate),
            upper: Some(candidate),
            timestamp: at(0),
            collected_at: at(0),
        })
        .expect("같은 경계");
        assert_eq!(
            same.check(same.instrument(), candidate),
            Ok(PriceLimitCheck::Within)
        );
    }
    #[test]
    fn inconsistent_currencies_us_shapes_and_future_time_are_rejected() {
        let kr = instrument(KnownMarket::Kr);
        let candidate = price("1", Currency::Krw);
        let usd = price("1", Currency::Usd);
        for (lower, upper) in [(Some(usd), Some(candidate)), (Some(candidate), Some(usd))] {
            assert_eq!(
                PriceLimits::try_new(PriceLimitsInput {
                    instrument: kr.clone(),
                    currency: Currency::Krw,
                    lower,
                    upper,
                    timestamp: at(0),
                    collected_at: at(0)
                }),
                Err(PriceLimitsError::CurrencyMismatch)
            );
        }
        assert_eq!(
            PriceLimits::try_new(PriceLimitsInput {
                instrument: kr,
                currency: Currency::Usd,
                lower: Some(candidate),
                upper: Some(candidate),
                timestamp: at(0),
                collected_at: at(0)
            }),
            Err(PriceLimitsError::CurrencyMismatch)
        );
    }
    #[test]
    fn us_shapes_and_future_time_are_rejected() {
        let us = instrument(KnownMarket::Us);
        let usd = price("1", Currency::Usd);
        for (lower, upper) in [(Some(usd), None), (None, Some(usd)), (Some(usd), Some(usd))] {
            assert_eq!(
                PriceLimits::try_new(PriceLimitsInput {
                    instrument: us.clone(),
                    currency: Currency::Usd,
                    lower,
                    upper,
                    timestamp: at(0),
                    collected_at: at(0)
                }),
                Err(PriceLimitsError::UnexpectedLimits)
            );
        }
        let unlimited = PriceLimits::try_new(PriceLimitsInput {
            instrument: us.clone(),
            currency: Currency::Usd,
            lower: None,
            upper: None,
            timestamp: at(2),
            collected_at: at(0),
        })
        .expect("제한 없음");
        assert_eq!(unlimited.range(), PriceLimitRange::Unlimited);
        assert_eq!(
            unlimited.check(unlimited.instrument(), usd),
            Ok(PriceLimitCheck::Unlimited)
        );
        assert_eq!(
            unlimited.clock_alignment(),
            ClockAlignment::AheadWithinTolerance
        );
        assert_eq!(
            PriceLimits::try_new(PriceLimitsInput {
                instrument: us,
                currency: Currency::Usd,
                lower: None,
                upper: None,
                timestamp: at(3),
                collected_at: at(0)
            }),
            Err(PriceLimitsError::FutureTimestamp)
        );
    }

    #[test]
    fn limits_reject_another_instrument_even_in_the_same_currency() {
        let query = instrument(KnownMarket::Kr);
        let candidate = price("100", Currency::Krw);
        let value = PriceLimits::try_new(PriceLimitsInput {
            instrument: query.clone(),
            currency: Currency::Krw,
            lower: Some(price("50", Currency::Krw)),
            upper: Some(price("130", Currency::Krw)),
            timestamp: at(0),
            collected_at: at(0),
        })
        .expect("제한");
        let foreign = crate::orderbook::tests::master(KnownMarket::Kr, "STOCK", "other")
            .quote_instrument()
            .expect("다른 종목");
        assert_eq!(
            value.check(&foreign, candidate),
            Err(PriceLimitsError::DifferentInstrument)
        );
        assert_eq!(value.check(&query, candidate), Ok(PriceLimitCheck::Within));
    }
    #[test]
    fn korean_instrument_with_usd_response_and_both_usd_bounds_is_rejected() {
        let usd = price("1", Currency::Usd);
        assert_eq!(
            PriceLimits::try_new(PriceLimitsInput {
                instrument: instrument(KnownMarket::Kr),
                currency: Currency::Usd,
                lower: Some(usd),
                upper: Some(usd),
                timestamp: at(0),
                collected_at: at(0),
            }),
            Err(PriceLimitsError::CurrencyMismatch)
        );
    }
    #[test]
    fn unlimited_limits_also_require_the_same_us_instrument() {
        let query = instrument(KnownMarket::Us);
        let value = PriceLimits::try_new(PriceLimitsInput {
            instrument: query.clone(),
            currency: Currency::Usd,
            lower: None,
            upper: None,
            timestamp: at(0),
            collected_at: at(0),
        })
        .expect("제한 없음");
        let foreign = crate::orderbook::tests::master(KnownMarket::Us, "STOCK", "other")
            .quote_instrument()
            .expect("다른 종목");
        assert_eq!(
            value.check(&foreign, price("1", Currency::Usd)),
            Err(PriceLimitsError::DifferentInstrument)
        );
        assert_eq!(
            value.check(&query, price("1", Currency::Usd)),
            Ok(PriceLimitCheck::Unlimited)
        );
    }
}
