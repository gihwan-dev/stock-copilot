use crate::{Amount, Money, MoneyError, Price};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickSize(Price);
impl TickSize {
    #[must_use]
    pub(crate) const fn from_response(price: Price) -> Self {
        Self(price)
    }
    #[must_use]
    pub const fn price(&self) -> Price {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickAlignment {
    OnTick,
    OffTick,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickRounding {
    Down,
    Up,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickSizeError {
    CurrencyMismatch,
    DifferentInstrument,
    Unavailable,
    BelowFirstTick,
    NumericValue(crate::NumericError),
    Arithmetic(MoneyError),
}

pub(crate) fn align_fixed(price: Price, tick: TickSize) -> Result<TickAlignment, TickSizeError> {
    let remainder = tick_remainder(price, tick)?;
    Ok(if remainder.as_decimal().is_zero() {
        TickAlignment::OnTick
    } else {
        TickAlignment::OffTick
    })
}
pub(crate) fn round_fixed(
    price: Price,
    tick: TickSize,
    direction: TickRounding,
) -> Result<Money, TickSizeError> {
    let remainder = tick_remainder(price, tick)?;
    if remainder.as_decimal().is_zero() {
        return Ok(price.as_money());
    }
    let down = price
        .as_money()
        .amount()
        .checked_sub(&remainder)
        .map_err(TickSizeError::Arithmetic)?;
    let amount = match direction {
        TickRounding::Down => down,
        TickRounding::Up => down
            .checked_add(&tick.price().as_money().amount())
            .map_err(TickSizeError::Arithmetic)?,
    };
    Ok(Money::new(amount, price.as_money().currency()))
}
fn tick_remainder(price: Price, tick: TickSize) -> Result<Amount, TickSizeError> {
    if price.as_money().currency() != tick.price().as_money().currency() {
        return Err(TickSizeError::CurrencyMismatch);
    }
    let value = price.as_money().amount().as_decimal();
    let unit = tick.price().as_money().amount().as_decimal();
    let scale = value.scale().max(unit.scale());
    let remainder = match value.scale().cmp(&unit.scale()) {
        std::cmp::Ordering::Less => {
            let mut remainder = value.mantissa().checked_rem(unit.mantissa());
            for _ in value.scale()..unit.scale() {
                remainder = remainder
                    .and_then(|coefficient| coefficient.checked_mul(10))
                    .and_then(|coefficient| coefficient.checked_rem(unit.mantissa()));
            }
            remainder
        }
        std::cmp::Ordering::Equal | std::cmp::Ordering::Greater => {
            let scaled_divisor = 10_i128
                .checked_pow(value.scale().abs_diff(unit.scale()))
                .and_then(|factor| unit.mantissa().checked_mul(factor));
            match scaled_divisor {
                Some(divisor) => value.mantissa().checked_rem(divisor),
                None => Some(value.mantissa()),
            }
        }
    };
    remainder
        .and_then(|coefficient| {
            rust_decimal::Decimal::try_from_i128_with_scale(coefficient, scale).ok()
        })
        .map(Amount::new)
        .ok_or(TickSizeError::Arithmetic(MoneyError::Overflow))
}

#[cfg(test)]
mod tests {
    use super::{align_fixed, round_fixed};
    use crate::orderbook::tests::price;
    use crate::{
        Amount, Currency, Money, MoneyError, NumericInput, Price, TickAlignment, TickRounding,
        TickSize, TickSizeError,
    };
    use proptest::test_runner::{Config, TestRunner};
    #[test]
    fn explicit_response_units_round_exactly_and_preserve_currency() {
        let tick = TickSize::from_response(price("0.05", Currency::Usd));
        assert_eq!(tick.price(), price("0.05", Currency::Usd));
        for (input, down, up, alignment) in [
            ("1.23", "1.20", "1.25", TickAlignment::OffTick),
            ("1.25", "1.25", "1.25", TickAlignment::OnTick),
        ] {
            let candidate = price(input, Currency::Usd);
            assert_eq!(align_fixed(candidate, tick), Ok(alignment));
            assert_eq!(
                round_fixed(candidate, tick, TickRounding::Down),
                Ok(price(down, Currency::Usd).as_money())
            );
            assert_eq!(
                round_fixed(candidate, tick, TickRounding::Up),
                Ok(price(up, Currency::Usd).as_money())
            );
        }
        let below = price("0.01", Currency::Usd);
        assert_eq!(
            round_fixed(below, tick, TickRounding::Down),
            Ok(
                Money::parse(&NumericInput::try_new("0").expect("입력"), Currency::Usd)
                    .expect("영")
            )
        );
        assert_eq!(
            round_fixed(below, tick, TickRounding::Up),
            Ok(tick.price().as_money())
        );
        assert_eq!(
            align_fixed(price("1", Currency::Krw), tick),
            Err(TickSizeError::CurrencyMismatch)
        );
        assert_eq!(
            round_fixed(price("1", Currency::Krw), tick, TickRounding::Down),
            Err(TickSizeError::CurrencyMismatch)
        );
    }
    #[test]
    fn extreme_scales_and_overflow_never_silently_round() {
        let tiny = TickSize::from_response(price("0.0000000000000000000000000001", Currency::Usd));
        let maximum = price("79228162514264337593543950335", Currency::Usd);
        assert_eq!(align_fixed(maximum, tiny), Ok(TickAlignment::OnTick));
        let huge = TickSize::from_response(maximum);
        assert_eq!(align_fixed(tiny.price(), huge), Ok(TickAlignment::OffTick));
        assert_eq!(
            round_fixed(tiny.price(), huge, TickRounding::Up),
            Ok(maximum.as_money())
        );
        let two = TickSize::from_response(price("2", Currency::Usd));
        assert_eq!(
            round_fixed(maximum, two, TickRounding::Up),
            Err(TickSizeError::Arithmetic(MoneyError::Overflow))
        );
        let awkward =
            TickSize::from_response(price("0.0000000000000000000000000002", Currency::Usd));
        assert_eq!(
            round_fixed(maximum, awkward, TickRounding::Down),
            Ok(maximum.as_money())
        );
    }
    #[test]
    fn unrepresentable_tick_rounding_is_an_explicit_precision_error() {
        let candidate = price("7922816251426433759354395033.5", Currency::Usd);
        let tick = TickSize::from_response(price("0.0000000000000000000000000011", Currency::Usd));
        for direction in [TickRounding::Down, TickRounding::Up] {
            assert_eq!(
                round_fixed(candidate, tick, direction),
                Err(TickSizeError::Arithmetic(MoneyError::PrecisionLoss))
            );
        }
    }
    #[test]
    fn different_scales_match_independent_common_scale_integer_reference() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_u64..1_000_000, 1_u64..10_000, 0_u32..=8, 0_u32..=8),
                |(coefficient, unit, price_scale, tick_scale)| {
                    let common = price_scale.max(tick_scale);
                    let numerator = coefficient * 10_u64.pow(common - price_scale);
                    let denominator = unit * 10_u64.pow(common - tick_scale);
                    let residue = numerator % denominator;
                    let lower = numerator - residue;
                    let upper = if residue == 0 {
                        numerator
                    } else {
                        lower + denominator
                    };
                    let candidate = Price::try_new(Money::new(
                        Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                            i128::from(coefficient),
                            price_scale,
                        )),
                        Currency::Usd,
                    ))
                    .expect("가격");
                    let tick = TickSize::from_response(
                        Price::try_new(Money::new(
                            Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                                i128::from(unit),
                                tick_scale,
                            )),
                            Currency::Usd,
                        ))
                        .expect("단위"),
                    );
                    proptest::prop_assert_eq!(
                        align_fixed(candidate, tick),
                        Ok(if residue == 0 {
                            TickAlignment::OnTick
                        } else {
                            TickAlignment::OffTick
                        })
                    );
                    for (direction, expected) in
                        [(TickRounding::Down, lower), (TickRounding::Up, upper)]
                    {
                        let result = round_fixed(candidate, tick, direction).expect("정확 격자");
                        proptest::prop_assert_eq!(
                            result.amount().as_decimal(),
                            rust_decimal::Decimal::from_i128_with_scale(
                                i128::from(expected),
                                common
                            )
                        );
                        proptest::prop_assert!(lower <= numerator && numerator <= upper);
                        proptest::prop_assert!(
                            numerator - lower < denominator && upper - numerator < denominator
                        );
                    }
                    Ok(())
                },
            )
            .expect("다른 스케일 정수 기준");
    }
    #[test]
    fn integer_reference_proves_tick_bounds_and_direction_for_4096_cases() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_u64..1_000_000, 1_u64..10_000, 0_u32..=8),
                |(coefficient, unit, scale)| {
                    let candidate = Price::try_new(Money::new(
                        Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                            i128::from(coefficient),
                            scale,
                        )),
                        Currency::Usd,
                    ))
                    .expect("가격");
                    let tick = TickSize::from_response(
                        Price::try_new(Money::new(
                            Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                                i128::from(unit),
                                scale,
                            )),
                            Currency::Usd,
                        ))
                        .expect("단위"),
                    );
                    let remainder = coefficient % unit;
                    let down = coefficient - remainder;
                    let up = if remainder == 0 {
                        coefficient
                    } else {
                        down + unit
                    };
                    for (direction, expected) in
                        [(TickRounding::Down, down), (TickRounding::Up, up)]
                    {
                        let result = round_fixed(candidate, tick, direction).expect("격자 가격");
                        proptest::prop_assert_eq!(
                            result.amount().as_decimal(),
                            rust_decimal::Decimal::from_i128_with_scale(
                                i128::from(expected),
                                scale
                            )
                        );
                        proptest::prop_assert_eq!(expected % unit, 0);
                        proptest::prop_assert!(down <= coefficient && coefficient <= up);
                        proptest::prop_assert!(
                            coefficient - down < unit && up - coefficient < unit
                        );
                    }
                    Ok(())
                },
            )
            .expect("독립 정수 격자");
    }
}
