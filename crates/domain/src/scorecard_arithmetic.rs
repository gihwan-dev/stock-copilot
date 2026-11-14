use crate::ScorecardError;
use crate::scorecard::{ScorecardBin, ScorecardDisplay, ScorecardPoint};
use crate::{CalibrationBin, CalibrationOutcome, CalibrationTable, RatioQuotient};

#[derive(Clone, Copy)]
struct Integer(u128);

fn display(numerator: Integer, denominator: Integer) -> Result<ScorecardDisplay, ScorecardError> {
    let scaled = numerator.0.wrapping_mul(10_000);
    let rounded = scaled
        .wrapping_add(denominator.0.wrapping_div(2))
        .checked_div(denominator.0)
        .ok_or(ScorecardError::Calculation)?;
    Ok(ScorecardDisplay::new(format!(
        "{}.{:04}",
        rounded.wrapping_div(10_000),
        rounded.wrapping_rem(10_000)
    )))
}
fn percent(fact: &crate::CalibrationFact) -> Integer {
    Integer(
        fact.estimate()
            .predicted_probability()
            .into_probability()
            .as_decimal()
            .mantissa()
            .unsigned_abs(),
    )
}
fn squared_error(predicted: Integer, outcome: CalibrationOutcome) -> Integer {
    let error = match outcome {
        CalibrationOutcome::Hit => 100_u128.wrapping_sub(predicted.0),
        CalibrationOutcome::Miss => predicted.0,
    };
    Integer(error.wrapping_mul(error))
}
pub(crate) fn brier(table: &CalibrationTable) -> Result<ScorecardDisplay, ScorecardError> {
    let sum = table.selected_facts().iter().fold(0_u128, |total, fact| {
        let predicted = percent(fact).0;
        total.wrapping_add(squared_error(Integer(predicted), fact.outcome()).0)
    });
    display(
        Integer(sum),
        Integer(
            rust_decimal::Decimal::from(table.judgment_count().as_usize())
                .mantissa()
                .unsigned_abs()
                .wrapping_mul(10_000),
        ),
    )
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
            let hits = display(
                Integer(value.mantissa().unsigned_abs()),
                Integer(10_u128.wrapping_pow(value.scale())),
            );
            let count = table.bin(bin).judgment_count().clone();
            let sum = table
                .selected_facts()
                .iter()
                .filter(|fact| {
                    CalibrationBin::for_probability(
                        fact.estimate().predicted_probability().into_probability(),
                    ) == bin
                })
                .fold(0_u128, |total, fact| total.wrapping_add(percent(fact).0));
            let mean = display(
                Integer(sum),
                Integer(
                    rust_decimal::Decimal::from(count.as_usize())
                        .mantissa()
                        .unsigned_abs()
                        .wrapping_mul(100),
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
                    .0
                    .wrapping_mul(u128::from(count));
                let opposite = squared_error(Integer(100_u128.wrapping_sub(predicted)), outcome)
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
