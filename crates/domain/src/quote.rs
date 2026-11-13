use crate::{Currency, Price, Symbol, Timestamp};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quote {
    symbol: Symbol,
    currency: Currency,
    last_price: Option<Price>,
    timestamp: Option<Timestamp>,
    collected_at: Timestamp,
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
        if timestamp.is_some_and(|value| value > collected_at) {
            return Err(QuoteError::FutureTimestamp);
        }
        Ok(Self {
            symbol,
            currency,
            last_price,
            timestamp,
            collected_at,
        })
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
}
