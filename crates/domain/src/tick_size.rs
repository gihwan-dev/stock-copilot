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
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(1_u64..1_000_000, 1_u64..10_000, 0_u32..=8, 0_u32..=8),
                assert_different_scale_reference,
            )
            .expect("다른 스케일 정수 기준");
    }
    #[test]
    fn integer_reference_proves_tick_bounds_and_direction_for_4096_cases() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(1_u64..1_000_000, 1_u64..10_000, 0_u32..=8),
                assert_same_scale_reference,
            )
            .expect("독립 정수 격자");
    }
    fn reference_price(coefficient: u64, scale: u32) -> Price {
        Price::try_new(Money::new(
            Amount::new(rust_decimal::Decimal::from_i128_with_scale(
                i128::from(coefficient),
                scale,
            )),
            Currency::Usd,
        ))
        .expect("가격")
    }
    fn reference_scaled(coefficient: u64, common: u32, scale: u32) -> u64 {
        let places = common.checked_sub(scale).expect("공통 스케일");
        coefficient
            .checked_mul(10_u64.checked_pow(places).expect("스케일 배수"))
            .expect("정수 배수")
    }
    fn reference_bounds(numerator: u64, denominator: u64) -> (u64, u64, u64) {
        let residue = numerator.checked_rem(denominator).expect("양의 단위");
        let lower = numerator.checked_sub(residue).expect("하한");
        let upper = if residue == 0 {
            numerator
        } else {
            lower.checked_add(denominator).expect("상한")
        };
        (residue, lower, upper)
    }
    fn reference_rounded(candidate: Price, tick: TickSize, direction: TickRounding) -> Money {
        round_fixed(candidate, tick, direction).expect("정확 격자")
    }
    fn assert_different_scale_reference(
        (coefficient, unit, price_scale, tick_scale): (u64, u64, u32, u32),
    ) -> proptest::test_runner::TestCaseResult {
        let common = price_scale.max(tick_scale);
        let numerator = reference_scaled(coefficient, common, price_scale);
        let denominator = reference_scaled(unit, common, tick_scale);
        let (residue, lower, upper) = reference_bounds(numerator, denominator);
        let candidate = reference_price(coefficient, price_scale);
        let tick = TickSize::from_response(reference_price(unit, tick_scale));
        proptest::prop_assert_eq!(
            align_fixed(candidate, tick),
            Ok(if residue == 0 {
                TickAlignment::OnTick
            } else {
                TickAlignment::OffTick
            })
        );
        for (direction, expected) in [(TickRounding::Down, lower), (TickRounding::Up, upper)] {
            let result = reference_rounded(candidate, tick, direction);
            proptest::prop_assert_eq!(
                result.amount().as_decimal(),
                rust_decimal::Decimal::from_i128_with_scale(i128::from(expected), common)
            );
            proptest::prop_assert!(lower <= numerator && numerator <= upper);
            proptest::prop_assert!(
                numerator.abs_diff(lower) < denominator && upper.abs_diff(numerator) < denominator
            );
        }
        Ok(())
    }
    fn assert_same_scale_reference(
        (coefficient, unit, scale): (u64, u64, u32),
    ) -> proptest::test_runner::TestCaseResult {
        let candidate = reference_price(coefficient, scale);
        let tick = TickSize::from_response(reference_price(unit, scale));
        let (_, down, up) = reference_bounds(coefficient, unit);
        for (direction, expected) in [(TickRounding::Down, down), (TickRounding::Up, up)] {
            let result = reference_rounded(candidate, tick, direction);
            proptest::prop_assert_eq!(
                result.amount().as_decimal(),
                rust_decimal::Decimal::from_i128_with_scale(i128::from(expected), scale)
            );
            proptest::prop_assert_eq!(expected.checked_rem(unit), Some(0));
            proptest::prop_assert!(down <= coefficient && coefficient <= up);
            proptest::prop_assert!(
                coefficient.abs_diff(down) < unit && up.abs_diff(coefficient) < unit
            );
        }
        Ok(())
    }
    #[test]
    fn deterministic_tick_references_cover_aligned_and_unaligned_prices() {
        for case in [(100, 25, 2), (123, 5, 2)] {
            assert_same_scale_reference(case).expect("같은 스케일");
        }
        for case in [(100, 25, 2, 2), (100, 10, 1, 2), (123, 5, 2, 1)] {
            assert_different_scale_reference(case).expect("다른 스케일");
        }
        let tick = TickSize::from_response(price("0.25", Currency::Usd));
        assert_eq!(
            align_fixed(price("1.00", Currency::Usd), tick),
            Ok(TickAlignment::OnTick)
        );
        assert_eq!(
            round_fixed(price("1.00", Currency::Usd), tick, TickRounding::Up),
            Ok(price("1.00", Currency::Usd).as_money())
        );
        assert_eq!(
            round_fixed(price("1.23", Currency::Usd), tick, TickRounding::Down),
            Ok(price("1.00", Currency::Usd).as_money())
        );
        assert_eq!(
            round_fixed(price("1.23", Currency::Usd), tick, TickRounding::Up),
            Ok(price("1.25", Currency::Usd).as_money())
        );
    }
}
