use crate::{Amount, Currency, Money};
use rust_decimal::RoundingStrategy;

pub(crate) fn ceil_fee(value: Money) -> Money {
    let places = match value.currency() {
        Currency::Krw => 0,
        Currency::Usd => 2,
    };
    Money::new(
        Amount::new(
            value
                .amount()
                .as_decimal()
                .round_dp_with_strategy(places, RoundingStrategy::ToPositiveInfinity),
        ),
        value.currency(),
    )
}
