use crate::{
    Amount, CheckResult, Currency, Money, NoBuyQuantity, Quantity, RiskAppetite, SignalOperation,
};
use rust_decimal::Decimal;

pub(crate) fn floor_target(
    total: Money,
    appetite: RiskAppetite,
    operation: SignalOperation,
) -> Result<Money, NoBuyQuantity> {
    if total.currency() != Currency::Krw || total.amount().as_decimal().is_sign_negative() {
        return Err(NoBuyQuantity::Indeterminate);
    }
    let percent = match appetite {
        RiskAppetite::Conservative => 1_i128,
        RiskAppetite::Neutral => 2,
        RiskAppetite::Aggressive => 3,
    };
    let divisor = match operation {
        SignalOperation::Conservative => 200_i128,
        SignalOperation::EligibleBeforeE91 => 100,
    };
    let value = total.amount().as_decimal();
    let numerator = value.mantissa().wrapping_mul(percent);
    10_i128
        .checked_pow(value.scale())
        .and_then(|scale| scale.checked_mul(divisor))
        .and_then(|denominator| numerator.checked_div(denominator))
        .map(|units| {
            Money::new(
                Amount::new(Decimal::from_i128_with_scale(units, 0)),
                Currency::Krw,
            )
        })
        .ok_or(NoBuyQuantity::Indeterminate)
}

pub(crate) fn maximum_quantity<F>(upper: Quantity, mut fits: F) -> Result<Quantity, NoBuyQuantity>
where
    F: FnMut(Quantity) -> Result<CheckResult, NoBuyQuantity>,
{
    let mut low = 0_i128;
    let ceiling = upper.as_decimal().mantissa();
    let mut high = ceiling.min(1);
    for _step in 0_u32..96 {
        if high == 0 {
            break;
        }
        let fit = Quantity::try_new(Decimal::from_i128_with_scale(high, 0))
            .or(Err(NoBuyQuantity::Indeterminate))
            .and_then(&mut fits)?;
        match fit {
            CheckResult::Failed => break,
            CheckResult::Unknown => return Err(NoBuyQuantity::Indeterminate),
            CheckResult::Passed => low = high,
        }
        if high == ceiling {
            break;
        }
        high = high.wrapping_mul(2).min(ceiling);
    }
    for _step in 0_u32..96 {
        if low == high {
            break;
        }
        let middle = low.wrapping_add(high).wrapping_add(1).wrapping_div(2);
        let fit = Quantity::try_new(Decimal::from_i128_with_scale(middle, 0))
            .or(Err(NoBuyQuantity::Indeterminate))
            .and_then(&mut fits)?;
        match fit {
            CheckResult::Passed => low = middle,
            CheckResult::Unknown => return Err(NoBuyQuantity::Indeterminate),
            CheckResult::Failed => {
                high = middle.wrapping_sub(1);
            }
        }
    }
    Quantity::try_new(Decimal::from_i128_with_scale(low, 0)).or(Err(NoBuyQuantity::Indeterminate))
}

#[cfg(test)]
mod tests {
    use super::{floor_target, maximum_quantity};
    use crate::{Amount, Currency, Money, RiskAppetite, SignalOperation};
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    #[test]
    fn target_is_always_floored_against_independent_rational() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(0_i64..1_000_000_000, 0_u32..=6, 0_usize..3),
                |(coefficient, scale, risk)| {
                    let appetite = [
                        RiskAppetite::Conservative,
                        RiskAppetite::Neutral,
                        RiskAppetite::Aggressive,
                    ][risk];
                    for (operation, denominator) in [
                        (SignalOperation::EligibleBeforeE91, 100_i128),
                        (SignalOperation::Conservative, 200_i128),
                    ] {
                        let total = Money::new(
                            Amount::new(Decimal::from_i128_with_scale(
                                i128::from(coefficient),
                                scale,
                            )),
                            Currency::Krw,
                        );
                        let actual = floor_target(total, appetite, operation).expect("목표");
                        let numerator = i128::from(coefficient)
                            .checked_mul(
                                i128::try_from(risk.checked_add(1).expect("비중")).expect("비중"),
                            )
                            .expect("곱");
                        let divisor = denominator.checked_mul(10_i128.pow(scale)).expect("분모");
                        let expected = numerator.checked_div(divisor).expect("몫");
                        proptest::prop_assert_eq!(
                            actual.amount().as_decimal(),
                            Decimal::from(expected)
                        );
                        proptest::prop_assert!(expected * divisor <= numerator);
                        proptest::prop_assert!((expected + 1) * divisor > numerator);
                    }
                    Ok(())
                },
            )
            .expect("내림 기준");
    }

    fn fits_reference(trial: crate::Quantity, budget: u64, price: u64) -> crate::CheckResult {
        if trial.as_decimal() <= Decimal::from(budget.checked_div(price).expect("몫")) {
            crate::CheckResult::Passed
        } else {
            crate::CheckResult::Failed
        }
    }

    #[test]
    fn bounded_search_returns_largest_whole_quantity() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_u64..1_000_000, 1_u64..10000), |(budget, price)| {
                let result = maximum_quantity(
                    crate::Quantity::try_new(Decimal::MAX).expect("상한"),
                    |trial| Ok(fits_reference(trial, budget, price)),
                )
                .expect("탐색");
                proptest::prop_assert_eq!(
                    result.as_decimal(),
                    Decimal::from(budget.checked_div(price).expect("몫"))
                );
                Ok(())
            })
            .expect("최대 수량");
    }
    #[test]
    fn zero_unknown_and_invalid_target_are_closed() {
        let zero = crate::Quantity::try_new(Decimal::ZERO).expect("수량");
        let ten = crate::Quantity::try_new(Decimal::TEN).expect("수량");
        let passes = |_| Ok(crate::CheckResult::Passed);
        assert_eq!(maximum_quantity(ten, passes), Ok(ten));
        assert_eq!(maximum_quantity(zero, passes), Ok(zero));
        assert_eq!(
            maximum_quantity(ten, |_| Ok(crate::CheckResult::Unknown)),
            Err(crate::NoBuyQuantity::Indeterminate)
        );
        assert_eq!(
            maximum_quantity(ten, |trial| Ok(
                if trial.as_decimal() == Decimal::from(3_i32) {
                    crate::CheckResult::Unknown
                } else if trial.as_decimal() <= Decimal::from(2_i32) {
                    crate::CheckResult::Passed
                } else {
                    crate::CheckResult::Failed
                }
            )),
            Err(crate::NoBuyQuantity::Indeterminate)
        );
        for total in [
            Money::new(Amount::new(Decimal::ONE), Currency::Usd),
            Money::new(Amount::new(Decimal::NEGATIVE_ONE), Currency::Krw),
        ] {
            assert_eq!(
                floor_target(total, RiskAppetite::Neutral, SignalOperation::Conservative),
                Err(crate::NoBuyQuantity::Indeterminate)
            );
        }
    }
    #[test]
    fn evaluator_failures_propagate_in_both_search_phases() {
        let ten = crate::Quantity::try_new(Decimal::TEN).expect("수량");
        assert_eq!(
            maximum_quantity(ten, |_| Err(crate::NoBuyQuantity::InvalidPriceOrFx)),
            Err(crate::NoBuyQuantity::InvalidPriceOrFx)
        );
        assert_eq!(
            maximum_quantity(ten, |trial| {
                if trial.as_decimal() == Decimal::from(3_i32) {
                    Err(crate::NoBuyQuantity::InvalidPriceOrFx)
                } else {
                    Ok(fits_reference(trial, 2, 1))
                }
            }),
            Err(crate::NoBuyQuantity::InvalidPriceOrFx)
        );
    }
}
