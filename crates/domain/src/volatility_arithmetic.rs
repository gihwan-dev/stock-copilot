use crate::{Candle, VolatilityError};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ApproximateRoot(Decimal);
impl ApproximateRoot {
    #[must_use]
    pub(crate) const fn as_decimal(&self) -> Decimal {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DailyVolatility(Decimal);
impl DailyVolatility {
    pub(crate) const fn as_decimal(&self) -> Decimal {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AnnualVolatility(Decimal);
impl AnnualVolatility {
    pub(crate) const fn as_decimal(&self) -> Decimal {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
struct Statistic(Decimal);

#[derive(Debug, Clone, Copy)]
struct IterationLimit(u16);

pub(crate) fn calculate(candles: &[Candle]) -> Result<DailyVolatility, VolatilityError> {
    let returns = candles
        .iter()
        .zip(candles.iter().skip(1))
        .map(|(previous, current)| {
            current
                .values()
                .close()
                .as_money()
                .amount()
                .as_decimal()
                .checked_div(previous.values().close().as_money().amount().as_decimal())
                .and_then(|quotient| quotient.checked_sub(Decimal::ONE))
                .map(Statistic)
        })
        .collect::<Option<Vec<_>>>();
    let variance = returns
        .and_then(|values| sample_variance(&values))
        .ok_or(VolatilityError::Arithmetic)?;
    square_root(crate::Ratio::new(variance.0)).map(|value| DailyVolatility(value.as_decimal()))
}

pub(crate) fn annualize(daily: DailyVolatility) -> Result<AnnualVolatility, VolatilityError> {
    let variance = daily
        .0
        .checked_mul(daily.0)
        .and_then(|value| value.checked_mul(Decimal::from(252_u32)))
        .map(Statistic)
        .ok_or(VolatilityError::Arithmetic)?;
    root(variance).map(|value| AnnualVolatility(value.as_decimal()))
}

pub(crate) fn square_root(value: crate::Ratio) -> Result<ApproximateRoot, VolatilityError> {
    root(Statistic(value.as_decimal()))
}

fn sample_variance(values: &[Statistic]) -> Option<Statistic> {
    let count = Decimal::from(values.len());
    let denominator = Decimal::from(values.iter().skip(1).count());
    let mean = values
        .iter()
        .try_fold(Decimal::ZERO, |sum, value| sum.checked_add(value.0))
        .and_then(|sum| sum.checked_div(count));
    mean.and_then(|average| {
        let squared = values.iter().try_fold(Decimal::ZERO, |sum, value| {
            let square = value
                .0
                .checked_sub(average)
                .and_then(|difference| difference.checked_mul(difference))?;
            sum.checked_add(square)
        })?;
        squared.checked_div(denominator)
    })
    .map(Statistic)
}

fn root(value: Statistic) -> Result<ApproximateRoot, VolatilityError> {
    if value.0 < Decimal::ZERO {
        return Err(VolatilityError::Arithmetic);
    }
    if value.0 == Decimal::ZERO {
        return Ok(ApproximateRoot(Decimal::ZERO));
    }
    iterate_root(
        value,
        Statistic(value.0.max(Decimal::ONE)),
        IterationLimit(256),
    )
}

fn root_step(value: Statistic, estimate: Statistic) -> Option<Statistic> {
    let half = estimate.0.checked_div(Decimal::TWO);
    let other_half = value
        .0
        .checked_div(estimate.0)
        .and_then(|quotient| quotient.checked_div(Decimal::TWO));
    half.zip(other_half)
        .and_then(|(left, right)| left.checked_add(right))
        .map(Statistic)
}

fn iterate_root(
    value: Statistic,
    initial: Statistic,
    limit: IterationLimit,
) -> Result<ApproximateRoot, VolatilityError> {
    let mut estimate = initial;
    for _iteration in 0..limit.0 {
        let step = root_step(value, estimate)
            .and_then(|next| {
                let tolerance = next.0.max(Decimal::ONE).checked_mul(Decimal::new(1, 24));
                let difference = next.0.checked_sub(estimate.0).map(|delta| delta.abs());
                tolerance
                    .zip(difference)
                    .map(|(threshold, delta)| (next, delta, threshold))
            })
            .ok_or(VolatilityError::Arithmetic)?;
        let (next, difference, tolerance) = step;
        if difference <= tolerance {
            return Ok(ApproximateRoot(next.0));
        }
        estimate = next;
    }
    Err(VolatilityError::NonConvergence)
}

#[cfg(test)]
mod tests {
    use super::{
        ApproximateRoot, DailyVolatility, IterationLimit, Statistic, annualize, calculate,
        iterate_root, root, sample_variance,
    };
    use crate::{NumericInput, Ratio, VolatilityError};
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;
    #[test]
    fn root_matches_independent_square_bounds_and_known_constants() {
        assert_eq!(
            root(Statistic(Decimal::NEGATIVE_ONE)),
            Err(VolatilityError::Arithmetic)
        );
        assert_eq!(
            root(Statistic(Decimal::ZERO)),
            Ok(ApproximateRoot(Decimal::ZERO))
        );
        let expected =
            Ratio::parse(&NumericInput::try_new("1.4142135623730950488016887242").expect("길이"))
                .expect("알려진 루트2")
                .as_decimal();
        let actual = root(Statistic(Decimal::TWO)).expect("루트2");
        assert!(
            actual.0.checked_sub(expected).expect("차이").abs() < Decimal::new(1, 24),
            "독립 상수"
        );
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_i64..=1_000_000_000, 0_u32..=28),
                |(coefficient, scale)| {
                    let input = Decimal::new(coefficient, scale);
                    let result = root(Statistic(input)).expect("수렴").0;
                    let epsilon = result
                        .max(Decimal::ONE)
                        .checked_mul(Decimal::new(1, 22))
                        .expect("허용 오차");
                    let lower = result
                        .checked_sub(epsilon)
                        .expect("하한")
                        .max(Decimal::ZERO);
                    let upper = result.checked_add(epsilon).expect("상한");
                    proptest::prop_assert!(lower.checked_mul(lower).expect("하한 제곱") <= input);
                    proptest::prop_assert!(upper.checked_mul(upper).expect("상한 제곱") >= input);
                    proptest::prop_assert_eq!(root(Statistic(input)).expect("반복").0, result);
                    Ok(())
                },
            )
            .expect("독립 제곱 구간과 결정성");
        for input in [Decimal::MAX, Decimal::new(1, 28), Decimal::ONE] {
            assert!(root(Statistic(input)).is_ok(), "극단값 수렴");
        }
        assert_eq!(calculate(&[]), Err(VolatilityError::Arithmetic));
    }
    #[test]
    fn bounded_newton_reports_exhaustion_and_checked_arithmetic_failures() {
        assert_eq!(
            iterate_root(
                Statistic(Decimal::TWO),
                Statistic(Decimal::ONE),
                IterationLimit(0)
            ),
            Err(VolatilityError::NonConvergence)
        );
        assert_eq!(
            iterate_root(
                Statistic(Decimal::MAX),
                Statistic(Decimal::new(1, 28)),
                IterationLimit(256)
            ),
            Err(VolatilityError::Arithmetic)
        );
        assert_eq!(
            iterate_root(
                Statistic(Decimal::ONE),
                Statistic(Decimal::ZERO),
                IterationLimit(256)
            ),
            Err(VolatilityError::Arithmetic)
        );
    }

    #[test]
    fn variance_and_annualization_reject_checked_overflow_and_undefined_samples() {
        assert_eq!(sample_variance(&[]).map(|value| value.0), None);
        assert_eq!(
            sample_variance(&[Statistic(Decimal::ONE)]).map(|value| value.0),
            None
        );
        for values in [
            vec![Statistic(Decimal::MAX), Statistic(Decimal::MAX)],
            vec![
                Statistic(Decimal::MAX),
                Statistic(Decimal::MIN),
                Statistic(Decimal::MIN),
            ],
            vec![Statistic(Decimal::MAX), Statistic(Decimal::ZERO)],
            vec![
                Statistic(Decimal::from(400_000_000_000_000_u64)),
                Statistic(Decimal::ZERO),
                Statistic(Decimal::from(400_000_000_000_000_u64)),
                Statistic(Decimal::ZERO),
            ],
        ] {
            assert!(
                sample_variance(&values).is_none(),
                "합·편차·제곱·제곱합 넘침"
            );
        }
        let daily = calculate(&crate::candle::tests::series(&[
            "1",
            "200000000000000",
            "1",
        ]))
        .expect("일간은 유효");
        assert_eq!(annualize(daily), Err(VolatilityError::Arithmetic));
        assert_eq!(
            annualize(DailyVolatility(Decimal::MAX)),
            Err(VolatilityError::Arithmetic)
        );
        assert_eq!(
            calculate(&crate::candle::tests::series(&[
                "1",
                "79228162514264337593543950335",
                "1",
                "79228162514264337593543950335"
            ])),
            Err(VolatilityError::Arithmetic)
        );
    }
    #[test]
    fn newton_tolerance_accepts_the_exact_boundary_within_one_iteration() {
        let input =
            Ratio::parse(&NumericInput::try_new("1.000000000000000000000002").expect("길이"))
                .expect("입력")
                .as_decimal();
        let expected =
            Ratio::parse(&NumericInput::try_new("1.000000000000000000000001").expect("길이"))
                .expect("기대값")
                .as_decimal();
        assert_eq!(
            iterate_root(Statistic(input), Statistic(Decimal::ONE), IterationLimit(1))
                .expect("허용 오차 경계")
                .as_decimal(),
            expected
        );
    }
    fn pairwise_variance(changes: &[Decimal]) -> Decimal {
        let mut pairs = Decimal::ZERO;
        for (index, left) in changes.iter().enumerate() {
            for right in changes.iter().skip(index.checked_add(1).expect("다음")) {
                let difference = left.checked_sub(*right).expect("차이");
                pairs = pairs
                    .checked_add(difference.checked_mul(difference).expect("제곱"))
                    .expect("합");
            }
        }
        let count = Decimal::from(changes.len());
        pairs
            .checked_div(
                count
                    .checked_mul(count.checked_sub(Decimal::ONE).expect("n-1"))
                    .expect("분모"),
            )
            .expect("독립 분산")
    }
    #[test]
    fn generated_statistics_match_independent_pairwise_variance() {
        use crate::Volatility;
        use crate::candle::tests::series;
        TestRunner::new(Config::with_cases(4096))
            .run(
                &proptest::collection::vec(1_u32..=10_000, 3..17),
                |prices| {
                    let texts = prices.iter().map(u32::to_string).collect::<Vec<_>>();
                    let refs = texts.iter().map(String::as_str).collect::<Vec<_>>();
                    let candles = series(&refs);
                    let result = Volatility::calculate(&candles).expect("통계");
                    let changes = prices
                        .iter()
                        .zip(prices.iter().skip(1))
                        .map(|(old, new)| {
                            Decimal::from(*new)
                                .checked_div(Decimal::from(*old))
                                .expect("독립 수익률")
                                .checked_sub(Decimal::ONE)
                                .expect("수익률")
                        })
                        .collect::<Vec<_>>();
                    let expected = pairwise_variance(&changes);
                    for (sigma, factor) in [
                        (result.daily().as_decimal(), Decimal::ONE),
                        (
                            result.annualized().expect("연율").as_decimal(),
                            Decimal::from(252_u32),
                        ),
                    ] {
                        let expected_scaled = expected.checked_mul(factor).expect("연율 분산");
                        let actual = sigma.checked_mul(sigma).expect("실제 분산");
                        proptest::prop_assert!(
                            actual.checked_sub(expected_scaled).expect("오차").abs()
                                <= expected_scaled
                                    .max(Decimal::ONE)
                                    .checked_mul(Decimal::new(1, 20))
                                    .expect("상대 오차")
                        );
                    }
                    proptest::prop_assert_eq!(
                        Volatility::calculate(&candles).expect("반복"),
                        result
                    );
                    Ok(())
                },
            )
            .expect("쌍별 차이 분산과 결정성");
    }
}
