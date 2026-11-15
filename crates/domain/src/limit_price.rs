use crate::{
    BestQuote, OrderSide, Orderbook, Price, PriceLimitCheck, PriceLimits, QuoteInstrument,
    TickRounding, TickTable, Timestamp,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitPriceError {
    DifferentInstrument,
    UnknownTimestamp,
    StaleFact,
    TickUnavailable,
    NoQuote,
    NoLiquidity,
    Tick(crate::TickSizeError),
    OutsideLimits,
}
#[derive(Clone, PartialEq, Eq)]
pub struct CalculatedLimitPrice {
    instrument: QuoteInstrument,
    side: OrderSide,
    price: Price,
}
impl std::fmt::Debug for CalculatedLimitPrice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CalculatedLimitPrice(..)")
    }
}
impl CalculatedLimitPrice {
    #[must_use]
    pub const fn price(&self) -> Price {
        self.price
    }
    #[must_use]
    pub const fn side(&self) -> OrderSide {
        self.side
    }
    #[must_use]
    pub const fn instrument(&self) -> &QuoteInstrument {
        &self.instrument
    }
}
pub fn calculate_limit_price(
    book: &Orderbook,
    tick_table: Option<&TickTable>,
    limits: &PriceLimits,
    side: OrderSide,
    now: Timestamp,
) -> Result<CalculatedLimitPrice, LimitPriceError> {
    let table = tick_table.ok_or(LimitPriceError::TickUnavailable)?;
    if book.instrument() != table.instrument() || book.instrument() != limits.instrument() {
        return Err(LimitPriceError::DifferentInstrument);
    }
    let timestamp = book.timestamp().ok_or(LimitPriceError::UnknownTimestamp)?;
    for time in [
        timestamp,
        book.collected_at(),
        limits.timestamp(),
        limits.collected_at(),
    ] {
        let age = now
            .utc_seconds()
            .as_i64()
            .checked_sub(*time.utc_seconds().as_i64());
        if time > now
            || age.is_none_or(|seconds| {
                seconds > 5
                    || (seconds == 5 && time.nanosecond().as_u32() < now.nanosecond().as_u32())
            })
        {
            return Err(LimitPriceError::StaleFact);
        }
    }
    let (quote, rounding) = match side {
        OrderSide::Buy => (book.best_ask(), TickRounding::Up),
        OrderSide::Sell => (book.best_bid(), TickRounding::Down),
    };
    let level = match quote {
        BestQuote::NoQuote => return Err(LimitPriceError::NoQuote),
        BestQuote::Available(value) => value,
    };
    if level.volume().as_decimal().is_zero() {
        return Err(LimitPriceError::NoLiquidity);
    }
    let price = crate::round_to_tick(table, book.instrument(), level.price(), rounding)
        .map_err(LimitPriceError::Tick)?;
    match limits.check(book.instrument(), price) {
        Ok(PriceLimitCheck::Within | PriceLimitCheck::Unlimited) => Ok(CalculatedLimitPrice {
            instrument: book.instrument().clone(),
            side,
            price,
        }),
        Ok(PriceLimitCheck::BelowLower | PriceLimitCheck::AboveUpper) | Err(_) => {
            Err(LimitPriceError::OutsideLimits)
        }
    }
}
