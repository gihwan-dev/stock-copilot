use rust_decimal::{Decimal, RoundingStrategy};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct CalibrationNumber(Decimal);
impl CalibrationNumber {
    pub(crate) const ZERO: Self = Self(Decimal::ZERO);
    pub(crate) const ONE: Self = Self(Decimal::ONE);
    pub(crate) const fn new(value: Decimal) -> Self {
        Self(value)
    }
    pub(crate) const fn as_decimal(&self) -> Decimal {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CalibrationSample {
    pub(crate) predicted: CalibrationNumber,
    pub(crate) observed: CalibrationNumber,
}

fn mean(
    values: impl Iterator<Item = CalibrationNumber>,
    count: CalibrationNumber,
) -> Option<CalibrationNumber> {
    let mut total = CalibrationNumber::ZERO;
    for value in values {
        total = sum(total, value)?;
    }
    total.0.checked_div(count.0).map(CalibrationNumber)
}

fn clamp_unit(value: CalibrationNumber) -> CalibrationNumber {
    CalibrationNumber(value.0.clamp(Decimal::ZERO, Decimal::ONE))
}

fn count_of(samples: &[CalibrationSample]) -> Option<CalibrationNumber> {
    let mut count = CalibrationNumber::ZERO;
    for _sample in samples {
        count = sum(count, CalibrationNumber::ONE)?;
    }
    Some(count)
}

pub(crate) fn recalibrated(
    samples: &[CalibrationSample],
    predicted: CalibrationNumber,
    base: CalibrationNumber,
    strength: CalibrationNumber,
) -> Option<CalibrationNumber> {
    if samples.is_empty() {
        return Some(base);
    }
    let count = count_of(samples)?;
    let mean_predicted = mean(samples.iter().map(|sample| sample.predicted), count)?.0;
    let mean_observed = mean(samples.iter().map(|sample| sample.observed), count)?.0;
    let mut spread = Decimal::ZERO;
    let mut covariance = Decimal::ZERO;
    for sample in samples {
        let dx = sample.predicted.0.checked_sub(mean_predicted)?;
        let dy = sample.observed.0.checked_sub(mean_observed)?;
        spread = spread.checked_add(dx.checked_mul(dx)?)?;
        covariance = covariance.checked_add(dx.checked_mul(dy)?)?;
    }
    let slope = if spread.is_zero() {
        Decimal::ZERO
    } else {
        covariance.checked_div(spread)?
    };
    let fitted = clamp_unit(CalibrationNumber(
        mean_observed.checked_add(slope.checked_mul(predicted.0.checked_sub(mean_predicted)?)?)?,
    ))
    .0;
    let weighted = count
        .0
        .checked_mul(fitted)?
        .checked_add(strength.0.checked_mul(base.0)?)?;
    let shrunk = weighted.checked_div(count.0.checked_add(strength.0)?)?;
    Some(CalibrationNumber(
        clamp_unit(CalibrationNumber(shrunk))
            .0
            .round_dp_with_strategy(6, RoundingStrategy::MidpointNearestEven),
    ))
}

pub(crate) fn squared_error(
    probability: CalibrationNumber,
    observed: CalibrationNumber,
) -> Option<CalibrationNumber> {
    let gap = probability.0.checked_sub(observed.0)?;
    gap.checked_mul(gap).map(CalibrationNumber)
}

pub(crate) fn sum(left: CalibrationNumber, right: CalibrationNumber) -> Option<CalibrationNumber> {
    left.0.checked_add(right.0).map(CalibrationNumber)
}

pub(crate) fn average(
    total: CalibrationNumber,
    count: CalibrationNumber,
) -> Option<CalibrationNumber> {
    total.0.checked_div(count.0).map(CalibrationNumber)
}
