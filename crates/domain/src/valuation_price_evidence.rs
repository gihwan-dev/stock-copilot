use crate::{
    CalendarDate, Currency, Money, Price, SnapshotCalendar, SnapshotError, SnapshotPriceBasis,
    Timestamp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationPriceEvidence {
    SessionClose {
        price: Price,
        date: CalendarDate,
    },
    Intraday {
        price: Price,
        observed_at: Timestamp,
    },
    HolidayCarry {
        price: Price,
        date: CalendarDate,
    },
    AwaitingSession {
        price: Price,
        date: CalendarDate,
    },
}
impl ValuationPriceEvidence {
    pub fn closing_price(
        self,
        calendar: SnapshotCalendar,
        currency: Currency,
        at: Timestamp,
    ) -> Result<Price, SnapshotError> {
        let (price, date, basis) = match self {
            Self::SessionClose { price, date } => (price, date, SnapshotPriceBasis::CurrentSession),
            Self::HolidayCarry { price, date } => (price, date, SnapshotPriceBasis::HolidayCarry),
            Self::AwaitingSession { price, date } => {
                (price, date, SnapshotPriceBasis::AwaitingSession)
            }
            Self::Intraday { .. } => return Err(SnapshotError::InvalidCandle),
        };
        if price.as_money().currency() != currency || calendar.price(currency, at)? != (date, basis)
        {
            return Err(SnapshotError::InvalidCandle);
        }
        Ok(price)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationAmountPresence {
    Missing,
    Zero(Money),
    NonZero(Money),
}
#[must_use]
pub fn valuation_amount_presence(value: Option<Money>) -> ValuationAmountPresence {
    match value {
        None => ValuationAmountPresence::Missing,
        Some(amount) if amount.amount().as_decimal().is_zero() => {
            ValuationAmountPresence::Zero(amount)
        }
        Some(amount) => ValuationAmountPresence::NonZero(amount),
    }
}

#[cfg(test)]
mod tests {
    use crate::valuation_snapshot::tests::{at, calendar, date, money};
    use crate::{
        Currency, Price, SnapshotError, ValuationAmountPresence, ValuationPriceEvidence,
        valuation_amount_presence,
    };
    #[test]
    fn closing_evidence_requires_exact_basis_date_currency_and_completed_session() {
        let price = Price::try_new(money("123", Currency::Krw)).expect("가격");
        let close = ValuationPriceEvidence::SessionClose {
            price,
            date: date(3),
        };
        assert_eq!(
            close.closing_price(calendar(3), Currency::Krw, at(3, 2)),
            Ok(price)
        );
        let awaiting = ValuationPriceEvidence::AwaitingSession {
            price,
            date: date(2),
        };
        assert_eq!(
            awaiting.closing_price(calendar(3), Currency::Krw, at(3, 0)),
            Ok(price)
        );
        for evidence in [
            close,
            ValuationPriceEvidence::HolidayCarry {
                price,
                date: date(2),
            },
            ValuationPriceEvidence::Intraday {
                price,
                observed_at: at(3, 0),
            },
            ValuationPriceEvidence::AwaitingSession {
                price,
                date: date(3),
            },
        ] {
            assert_eq!(
                evidence.closing_price(calendar(3), Currency::Krw, at(3, 0)),
                Err(SnapshotError::InvalidCandle)
            );
        }
        assert_eq!(
            close.closing_price(calendar(3), Currency::Usd, at(3, 2)),
            Err(SnapshotError::InvalidCandle)
        );
        assert_eq!(
            awaiting.closing_price(calendar(3), Currency::Krw, at(1, 0)),
            Err(SnapshotError::MissingPrice)
        );
    }
    #[test]
    fn zero_price_is_invalid_but_explicit_zero_valuation_is_present() {
        for currency in [Currency::Krw, Currency::Usd] {
            let zero = money("0.00", currency);
            assert_eq!(
                Price::try_new(zero),
                Err(crate::NumericError::NonPositivePrice)
            );
            assert_eq!(
                valuation_amount_presence(Some(zero)),
                ValuationAmountPresence::Zero(zero)
            );
            for value in ["-1.23", "4.56"] {
                let amount = money(value, currency);
                assert_eq!(
                    valuation_amount_presence(Some(amount)),
                    ValuationAmountPresence::NonZero(amount)
                );
            }
        }
        assert_eq!(
            valuation_amount_presence(None),
            ValuationAmountPresence::Missing
        );
    }
}
