use crate::{Margin, NoThresholds, ThresholdDisplay};
use rust_decimal::{Decimal, RoundingStrategy};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ThresholdNumber(Decimal);
impl ThresholdNumber {
    pub(crate) fn new(value: Decimal) -> Self {
        Self(value)
    }
    pub(crate) fn percent(self) -> Result<ThresholdDisplay, NoThresholds> {
        let percent = self
            .as_decimal()
            .checked_mul(Decimal::from(100_i32))
            .ok_or(NoThresholds::NumericalDomain)?;
        let rounded = percent.round_dp_with_strategy(1, RoundingStrategy::MidpointAwayFromZero);
        Ok(ThresholdDisplay::new(format!("≈{rounded:.1}%")))
    }
    pub(crate) fn as_decimal(&self) -> Decimal {
        self.0
    }
}
impl std::fmt::Debug for ThresholdNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rounded = self
            .0
            .round_dp_with_strategy(4, RoundingStrategy::MidpointAwayFromZero);
        write!(f, "≈{rounded:.4}")
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ThresholdValues {
    pub(crate) breakeven: ThresholdNumber,
    pub(crate) buy: ThresholdNumber,
    pub(crate) sell: ThresholdNumber,
}

pub(crate) fn calculate(
    base: ThresholdNumber,
    target: ThresholdNumber,
    stop: ThresholdNumber,
    cost: ThresholdNumber,
    margin: Margin,
) -> Result<ThresholdValues, NoThresholds> {
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
    let values = base.0.checked_add(delta).and_then(|breakeven| {
        breakeven
            .checked_add(margin.ratio().as_decimal())
            .zip(base.0.checked_sub(margin.ratio().as_decimal()))
            .map(|(buy, sell)| ThresholdValues {
                breakeven: ThresholdNumber(breakeven),
                buy: ThresholdNumber(buy),
                sell: ThresholdNumber(sell.max(Decimal::new(5, 2))),
            })
    });
    values.ok_or(NoThresholds::NumericalDomain)
}
