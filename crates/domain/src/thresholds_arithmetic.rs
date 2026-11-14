use crate::{NoThresholds, ThresholdDisplay, ThresholdKind};
use rust_decimal::{Decimal, RoundingStrategy};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ThresholdNumber(Decimal);
impl ThresholdNumber {
    pub(crate) fn new(value: Decimal) -> Self {
        Self(value)
    }
    pub(crate) fn percent(self, kind: ThresholdKind) -> Result<ThresholdDisplay, NoThresholds> {
        let percent = self
            .as_decimal()
            .checked_mul(Decimal::from(100_i32))
            .ok_or(NoThresholds::NumericalDomain)?;
        let strategy = match kind {
            ThresholdKind::Buy | ThresholdKind::Breakeven => RoundingStrategy::ToPositiveInfinity,
            ThresholdKind::Sell => RoundingStrategy::ToNegativeInfinity,
        };
        let rounded = percent.round_dp_with_strategy(1, strategy);
        Ok(ThresholdDisplay::new(format!("≈{rounded:.1}%")))
    }
    pub(crate) fn as_decimal(&self) -> Decimal {
        self.0
    }
}
impl std::fmt::Debug for ThresholdNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ThresholdNumber(..)")
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThresholdValues {
    pub(crate) breakeven: ThresholdNumber,
    pub(crate) buy: ThresholdNumber,
    pub(crate) sell: ThresholdNumber,
}

pub(crate) fn cost_delta(
    target: ThresholdNumber,
    stop: ThresholdNumber,
    cost: ThresholdNumber,
) -> Result<ThresholdNumber, NoThresholds> {
    let sum = target
        .0
        .checked_add(stop.0)
        .ok_or(NoThresholds::NumericalDomain)?;
    let delta = cost
        .0
        .checked_div(sum)
        .ok_or(NoThresholds::NumericalDomain)?;
    let scaled_cost = cost
        .0
        .checked_mul(Decimal::from(20_i32))
        .ok_or(NoThresholds::NumericalDomain)?;
    if scaled_cost > sum {
        return Err(NoThresholds::ExcessiveCost);
    }
    Ok(ThresholdNumber(delta))
}
pub(crate) fn calculate_values(
    base: ThresholdNumber,
    delta: ThresholdNumber,
    margins: crate::thresholds::SideMargins,
) -> Result<ThresholdValues, NoThresholds> {
    let values = base.0.checked_add(delta.0).and_then(|breakeven| {
        breakeven
            .checked_add(margins.buy.ratio().as_decimal())
            .zip(base.0.checked_sub(margins.sell.ratio().as_decimal()))
            .map(|(buy, sell)| ThresholdValues {
                breakeven: ThresholdNumber(breakeven),
                buy: ThresholdNumber(buy),
                sell: ThresholdNumber(sell.max(Decimal::new(5, 2))),
            })
    });
    values.ok_or(NoThresholds::NumericalDomain)
}

#[cfg(test)]
pub(crate) mod tests {
    pub(crate) fn document_rounding(value: super::ThresholdNumber) -> crate::ThresholdDisplay {
        let percent = value
            .as_decimal()
            .checked_mul(rust_decimal::Decimal::from(100_i32))
            .expect("표시 범위");
        crate::ThresholdDisplay::new(format!(
            "≈{:.1}%",
            percent.round_dp_with_strategy(1, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
        ))
    }
}
