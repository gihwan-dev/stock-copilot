use crate::ScorecardError;
use crate::scorecard::{ScorecardBin, ScorecardDisplay, ScorecardPoint};
use crate::{CalibrationBin, CalibrationOutcome, CalibrationTable, RatioQuotient};

#[derive(Clone, Copy)]
struct Integer(u128);

impl Integer {
    fn add(self, other: Self) -> Result<Self, ScorecardError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(ScorecardError::Calculation)
    }
    fn sub(self, other: Self) -> Result<Self, ScorecardError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(ScorecardError::Calculation)
    }
    fn mul(self, other: Self) -> Result<Self, ScorecardError> {
        self.0
            .checked_mul(other.0)
            .map(Self)
            .ok_or(ScorecardError::Calculation)
    }
    fn div(self, other: Self) -> Result<Self, ScorecardError> {
        self.0
            .checked_div(other.0)
            .map(Self)
            .ok_or(ScorecardError::Calculation)
    }
}
fn display(numerator: Integer, denominator: Integer) -> Result<ScorecardDisplay, ScorecardError> {
    let scaled = numerator.mul(Integer(10_000))?;
    let rounded = scaled
        .add(Integer(denominator.0.wrapping_div(2)))?
        .div(denominator)?;
    Ok(ScorecardDisplay::new(format!(
        "{}.{:04}",
        rounded.0.wrapping_div(10_000),
        rounded.0.wrapping_rem(10_000)
    )))
}
fn percent(fact: &crate::CalibrationFact) -> Integer {
    Integer(
        crate::probability::scorecard_ai_percent(fact.estimate().predicted_probability()).as_u128(),
    )
}
fn squared_error(
    predicted: Integer,
    outcome: CalibrationOutcome,
) -> Result<Integer, ScorecardError> {
    let error = match outcome {
        CalibrationOutcome::Hit => Integer(100).sub(predicted)?,
        CalibrationOutcome::Miss => predicted,
    };
    error.mul(error)
}
pub(crate) fn brier(table: &CalibrationTable) -> Result<ScorecardDisplay, ScorecardError> {
    let terms = table
        .selected_facts()
        .iter()
        .map(|fact| squared_error(percent(fact), fact.outcome()))
        .collect();
    brier_result(
        terms,
        Integer(
            rust_decimal::Decimal::from(table.judgment_count().as_usize())
                .mantissa()
                .unsigned_abs(),
        ),
    )
}
fn brier_result(
    terms: Result<Vec<Integer>, ScorecardError>,
    count: Integer,
) -> Result<ScorecardDisplay, ScorecardError> {
    let sum = terms?.into_iter().try_fold(Integer(0), Integer::add)?;
    let denominator = count.mul(Integer(10_000))?;
    display(sum, denominator)
}
fn mean_result(
    terms: Result<Vec<Integer>, ScorecardError>,
    count: Integer,
) -> Result<ScorecardDisplay, ScorecardError> {
    let sum = terms?.into_iter().try_fold(Integer(0), Integer::add)?;
    let denominator = count.mul(Integer(100))?;
    display(sum, denominator)
}
#[derive(Clone, Copy)]
struct Scale(u32);
fn ratio_parts(numerator: Integer, scale: Scale) -> Result<ScorecardDisplay, ScorecardError> {
    let denominator = 10_u128
        .checked_pow(scale.0)
        .map(Integer)
        .ok_or(ScorecardError::Calculation)?;
    display(numerator, denominator)
}
pub(crate) fn reliability(table: &CalibrationTable) -> Result<Vec<ScorecardBin>, ScorecardError> {
    [
        CalibrationBin::Below50,
        CalibrationBin::From50To60,
        CalibrationBin::From60To70,
        CalibrationBin::AtLeast70,
    ]
    .into_iter()
    .map(|bin| bin_result(table, bin, table.bin(bin).hit_rate()))
    .collect()
}
fn bin_result(
    table: &CalibrationTable,
    bin: CalibrationBin,
    input: Result<Option<RatioQuotient>, crate::MoneyError>,
) -> Result<ScorecardBin, ScorecardError> {
    match input.map_err(calculation_error)? {
        None => Ok(ScorecardBin::NoData(bin)),
        Some(rate) => {
            let value = match rate {
                RatioQuotient::Exact(value) => value.as_decimal(),
                RatioQuotient::Approximate(value) => value.as_decimal(),
            };
            let hits = ratio_parts(
                Integer(value.mantissa().unsigned_abs()),
                Scale(value.scale()),
            );
            let count = table.bin(bin).judgment_count().clone();
            let terms = table
                .selected_facts()
                .iter()
                .filter(|fact| {
                    CalibrationBin::for_probability(
                        fact.estimate().predicted_probability().into_probability(),
                    ) == bin
                })
                .map(|fact| Ok(percent(fact)))
                .collect();
            let mean = mean_result(
                terms,
                Integer(
                    rust_decimal::Decimal::from(count.as_usize())
                        .mantissa()
                        .unsigned_abs(),
                ),
            );
            observed_point(bin, count, mean, hits)
        }
    }
}
fn observed_point(
    bin: CalibrationBin,
    count: crate::CalibrationCount,
    mean: Result<ScorecardDisplay, ScorecardError>,
    hits: Result<ScorecardDisplay, ScorecardError>,
) -> Result<ScorecardBin, ScorecardError> {
    Ok(ScorecardBin::Observed(ScorecardPoint::new(
        bin, count, mean?, hits?,
    )))
}

fn calculation_error(_error: crate::MoneyError) -> ScorecardError {
    ScorecardError::Calculation
}

#[cfg(test)]
mod tests {
    use super::{Integer, calculation_error, display, squared_error};
    use crate::{MoneyError, ScorecardError};
    #[test]
    fn checked_arithmetic_propagates_overflow_underflow_and_invalid_terms() {
        assert_eq!(
            squared_error(Integer(101), crate::CalibrationOutcome::Hit).map(|value| value.0),
            Err(ScorecardError::Calculation)
        );
        assert_eq!(
            squared_error(Integer(u128::MAX), crate::CalibrationOutcome::Miss).map(|value| value.0),
            Err(ScorecardError::Calculation)
        );
        for numerator in [u128::MAX, u128::MAX.wrapping_div(10000)] {
            assert_eq!(
                display(Integer(numerator), Integer(u128::MAX)),
                Err(ScorecardError::Calculation)
            );
        }
        assert_eq!(
            super::ratio_parts(Integer(1), super::Scale(39)),
            Err(ScorecardError::Calculation)
        );
        for calculate in [super::brier_result, super::mean_result] {
            assert_eq!(
                calculate(Err(ScorecardError::Calculation), Integer(1)),
                Err(ScorecardError::Calculation)
            );
            assert_eq!(
                calculate(Ok(vec![Integer(u128::MAX), Integer(1)]), Integer(1)),
                Err(ScorecardError::Calculation)
            );
            assert_eq!(
                calculate(Ok(Vec::new()), Integer(u128::MAX)),
                Err(ScorecardError::Calculation)
            );
        }
    }
    #[test]
    fn bin_and_point_errors_never_emit_partial_metrics() {
        let key = crate::CalibrationKey::new(
            crate::CalibrationModel::new(
                crate::ModelIdentityPart::try_new("p").expect("제공자"),
                crate::ModelIdentityPart::try_new("m").expect("모델"),
                crate::ModelIdentityPart::try_new("v").expect("버전"),
            ),
            crate::Horizon::new(crate::HorizonSelection::FewDays),
        );
        let table = crate::Calibration::try_new(key.clone(), &[])
            .expect("빈 표")
            .for_combination(&key);
        assert_eq!(
            super::bin_result(
                &table,
                crate::CalibrationBin::Below50,
                Err(MoneyError::Overflow)
            ),
            Err(ScorecardError::Calculation)
        );
        let count = table.judgment_count();
        let valid = || display(Integer(0), Integer(1));
        assert_eq!(
            super::observed_point(
                crate::CalibrationBin::Below50,
                count.clone(),
                Err(ScorecardError::Calculation),
                valid()
            ),
            Err(ScorecardError::Calculation)
        );
        assert_eq!(
            super::observed_point(
                crate::CalibrationBin::Below50,
                count,
                valid(),
                Err(ScorecardError::Calculation)
            ),
            Err(ScorecardError::Calculation)
        );
    }
    #[test]
    fn perfect_and_opposite_theoretical_boundaries_for_4096_cases() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(4096))
            .run(&(1_u32..=65536, 0_u8..2), |(count, binary)| {
                let outcome = if binary == 1 {
                    crate::CalibrationOutcome::Hit
                } else {
                    crate::CalibrationOutcome::Miss
                };
                let predicted = u128::from(binary).wrapping_mul(100);
                let denominator = Integer(u128::from(count).wrapping_mul(10000));
                let perfect = squared_error(Integer(predicted), outcome)
                    .expect("제곱")
                    .0
                    .wrapping_mul(u128::from(count));
                let opposite = squared_error(Integer(100_u128.wrapping_sub(predicted)), outcome)
                    .expect("제곱")
                    .0
                    .wrapping_mul(u128::from(count));
                assert_eq!(
                    display(Integer(perfect), denominator)
                        .expect("완벽")
                        .as_str(),
                    "0.0000"
                );
                assert_eq!(
                    display(Integer(opposite), denominator)
                        .expect("최악")
                        .as_str(),
                    "1.0000"
                );
                Ok(())
            })
            .expect("이론적 경계");
    }
    #[test]
    fn zero_denominator_and_error_mapping_are_closed() {
        assert_eq!(
            display(Integer(1), Integer(0)),
            Err(ScorecardError::Calculation)
        );
        assert_eq!(
            calculation_error(MoneyError::Overflow),
            ScorecardError::Calculation
        );
        assert_eq!(
            display(Integer(0), Integer(1)).expect("영점").as_str(),
            "0.0000"
        );
        assert_eq!(
            display(Integer(1), Integer(1)).expect("최악").as_str(),
            "1.0000"
        );
        assert_eq!(
            display(Integer(1), Integer(32)).expect("동률").as_str(),
            "0.0313"
        );
    }
}
