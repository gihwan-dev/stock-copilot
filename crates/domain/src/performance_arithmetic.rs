use crate::performance::PerformanceError;
use rust_decimal::Decimal;
use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Number(Decimal);
impl Debug for Number {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Number([REDACTED])")
    }
}
impl Number {
    pub(crate) const fn new(value: Decimal) -> Self {
        Self(value)
    }
    pub(crate) const fn as_decimal(&self) -> Decimal {
        self.0
    }
    pub(crate) fn add(self, other: Self) -> Result<Self, PerformanceError> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or(PerformanceError::Arithmetic)
    }
    pub(crate) fn sub(self, other: Self) -> Result<Self, PerformanceError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(PerformanceError::Arithmetic)
    }
    pub(crate) fn mul(self, other: Self) -> Result<Self, PerformanceError> {
        self.0
            .checked_mul(other.0)
            .map(Self)
            .ok_or(PerformanceError::Arithmetic)
    }
    pub(crate) fn div(self, other: Self) -> Result<Self, PerformanceError> {
        self.0
            .checked_div(other.0)
            .map(Self)
            .ok_or(PerformanceError::Arithmetic)
    }
    pub(crate) fn neg(self) -> Self {
        let mut value = self.0;
        value.set_sign_negative(!value.is_sign_negative());
        Self(value)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Days(u32);
impl Days {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }
    pub(crate) const fn as_u32(&self) -> &u32 {
        &self.0
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PerformanceValue(Number);
impl Debug for PerformanceValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("PerformanceValue([REDACTED])")
    }
}
impl Display for PerformanceValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.6}",
            self.0
                .0
                .round_dp_with_strategy(6, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
        )
    }
}
impl PerformanceValue {
    #[must_use]
    pub fn direction(self) -> crate::DisplayDirection {
        match self
            .0
            .0
            .round_dp_with_strategy(6, rust_decimal::RoundingStrategy::MidpointAwayFromZero)
            .cmp(&Decimal::ZERO)
        {
            std::cmp::Ordering::Greater => crate::DisplayDirection::Up,
            std::cmp::Ordering::Less => crate::DisplayDirection::Down,
            std::cmp::Ordering::Equal => crate::DisplayDirection::Flat,
        }
    }
    pub(crate) const fn number(self) -> Number {
        self.0
    }
    pub(crate) const fn from_number(value: Number) -> Self {
        Self(value)
    }
}
pub(crate) fn dietz(
    start: Number,
    change: Number,
    flows: &[(Days, Number)],
    duration: Days,
) -> Result<PerformanceValue, PerformanceError> {
    if duration.0 == 0 {
        return Err(PerformanceError::InvalidPeriod);
    }
    let denominator = flows.iter().try_fold(start, |sum, (day, amount)| {
        let remaining = duration
            .0
            .checked_sub(day.0)
            .ok_or(PerformanceError::InvalidPeriod)?;
        weighted_capital(sum, *amount, Days(remaining), duration)
            .ok_or(PerformanceError::Arithmetic)
    })?;
    if denominator.0 <= Decimal::ZERO {
        return Err(PerformanceError::NonPositiveCapital);
    }
    let scale = flows.iter().fold(start.0.abs(), |largest, (_, amount)| {
        largest.max(amount.0.abs())
    });
    if denominator
        .0
        .checked_div(scale)
        .is_none_or(|relative| relative < Decimal::new(1, 12))
    {
        return Err(PerformanceError::InsufficientPrecision);
    }
    if change.0 < Decimal::ZERO {
        ensure_growth(denominator, change, flows)?;
    }
    change.div(denominator).map(PerformanceValue)
}
fn ensure_growth(
    capital: Number,
    change: Number,
    flows: &[(Days, Number)],
) -> Result<(), PerformanceError> {
    let numerator = capital.add(change)?;
    if numerator.0 < Decimal::ZERO {
        return Err(PerformanceError::NonPositiveCapital);
    }
    let factor = numerator.div(capital)?;
    if factor.0 < Decimal::new(1, 6)
        && (!factor.0.is_zero()
            || !numerator.0.is_zero()
            || flows.iter().any(|(_, amount)| !amount.0.is_zero()))
    {
        return Err(PerformanceError::InsufficientPrecision);
    }
    Ok(())
}
fn factor_product(left: Number, right: Number) -> Result<Number, PerformanceError> {
    let result = left.mul(right)?;
    if result.0.is_zero() && !left.0.is_zero() && !right.0.is_zero() {
        return Err(PerformanceError::InsufficientPrecision);
    }
    Ok(result)
}
fn weighted_capital(
    sum: Number,
    amount: Number,
    remaining: Days,
    duration: Days,
) -> Option<Number> {
    Decimal::from(remaining.0)
        .checked_div(Decimal::from(duration.0))
        .and_then(|weight| amount.0.checked_mul(weight))
        .and_then(|weighted| sum.0.checked_add(weighted))
        .map(Number)
}
pub(crate) fn linked(values: &[PerformanceValue]) -> Result<PerformanceValue, PerformanceError> {
    values
        .iter()
        .try_fold(Number(Decimal::ONE), |total, value| {
            let factor = Number(Decimal::ONE).add(value.0)?;
            if factor.0 < Decimal::ZERO {
                return Err(PerformanceError::NonPositiveCapital);
            }
            factor_product(total, factor)
        })?
        .sub(Number(Decimal::ONE))
        .map(PerformanceValue)
}
fn power(mut base: Number, exponent: Days) -> Result<Number, PerformanceError> {
    let mut result = Number(Decimal::ONE);
    let mut remaining = exponent.0;
    while remaining != 0 {
        if remaining.checked_rem(2) == Some(1) {
            result = result.mul(base)?;
        }
        remaining = remaining.wrapping_div(2);
        if remaining != 0 {
            base = base.mul(base)?;
        }
    }
    Ok(result)
}
fn residual(
    cash: &[(Days, Number)],
    duration: Days,
    daily: Number,
) -> Result<Number, PerformanceError> {
    cash.iter()
        .try_fold(Number(Decimal::ZERO), |sum, (day, amount)| {
            let exponent = duration
                .0
                .checked_sub(day.0)
                .ok_or(PerformanceError::InvalidPeriod)?;
            sum.add(amount.mul(power(daily, Days(exponent))?)?)
        })
}
pub(crate) fn mwr(
    cash: &[(Days, Number)],
    duration: Days,
    output_days: Days,
) -> Result<PerformanceValue, PerformanceError> {
    if duration.0 == 0 || duration.0 > 4096 {
        return Err(PerformanceError::InvalidPeriod);
    }
    if cash
        .iter()
        .filter(|(_, amount)| !amount.0.is_zero())
        .count()
        > 2
    {
        relative_precision(cash)?;
    }
    let values = canonical_cash(cash)?;
    let signs = values
        .iter()
        .zip(values.iter().skip(1))
        .filter(|(left, right)| left.1.0.is_sign_negative() != right.1.0.is_sign_negative())
        .count();
    if signs != 1
        || values
            .first()
            .is_none_or(|(_, value)| value.0 >= Decimal::ZERO)
    {
        return Err(PerformanceError::AmbiguousOrMissingRoot);
    }
    if output_days == duration
        && let [(Days(0), initial), (end, terminal)] = values.as_slice()
        && *end == duration
    {
        return simple_return(initial.neg(), *terminal);
    }
    let prepared = root_cash(&values, duration)?;
    bisect(
        &prepared.values,
        prepared.duration,
        output_days,
        IterationLimit(128),
    )
}
fn canonical_cash(cash: &[(Days, Number)]) -> Result<Vec<(Days, Number)>, PerformanceError> {
    let mut ordered = cash.to_vec();
    ordered.sort_unstable();
    let mut combined = std::collections::BTreeMap::<Days, Number>::new();
    for (day, amount) in ordered {
        match combined.entry(day) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                let _stored = slot.insert(amount);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                let sum = crate::exact_arithmetic::add(
                    crate::Amount::new(slot.get().0),
                    crate::Amount::new(amount.0),
                )
                .map(|total| Number(total.as_decimal()))
                .map_err(|_cause| PerformanceError::Arithmetic)?;
                let _previous = slot.insert(sum);
            }
        }
    }
    Ok(combined
        .into_iter()
        .filter(|(_, value)| !value.0.is_zero())
        .collect())
}
#[derive(Debug)]
struct RootCash {
    values: Vec<(Days, Number)>,
    duration: Days,
}
fn relative_precision(cash: &[(Days, Number)]) -> Result<(), PerformanceError> {
    let maximum = cash.iter().fold(Decimal::ZERO, |largest, (_, amount)| {
        largest.max(amount.0.abs())
    });
    if cash.iter().any(|(_, amount)| {
        !amount.0.is_zero()
            && amount
                .0
                .abs()
                .checked_div(maximum)
                .is_none_or(|relative| relative < Decimal::new(1, 6))
    }) {
        return Err(PerformanceError::InsufficientPrecision);
    }
    Ok(())
}
fn root_cash(values: &[(Days, Number)], requested: Days) -> Result<RootCash, PerformanceError> {
    let first = *values.first().ok_or(PerformanceError::InvalidPeriod)?;
    let last = values.last().copied().unwrap_or(first);
    if last.0 > requested || last.0 <= first.0 {
        return Err(PerformanceError::InvalidPeriod);
    }
    relative_precision(values)?;
    let maximum = values.iter().fold(Decimal::ZERO, |largest, (_, amount)| {
        largest.max(amount.0.abs())
    });
    let normalized = values
        .iter()
        .map(|(day, amount)| {
            let offset = day
                .0
                .checked_sub(first.0.0)
                .ok_or(PerformanceError::InvalidPeriod)?;
            amount
                .div(Number(maximum))
                .map(|value| (Days(offset), value))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RootCash {
        values: normalized,
        duration: Days(last.0.0.wrapping_sub(first.0.0)),
    })
}
#[derive(Clone, Copy)]
struct RootPoint(u128);
impl RootPoint {
    fn number(&self) -> Number {
        let [
            low0,
            low1,
            low2,
            low3,
            mid0,
            mid1,
            mid2,
            mid3,
            high0,
            high1,
            high2,
            high3,
            _,
            _,
            _,
            _,
        ] = self.0.to_le_bytes();
        Number(Decimal::from_parts(
            u32::from_le_bytes([low0, low1, low2, low3]),
            u32::from_le_bytes([mid0, mid1, mid2, mid3]),
            u32::from_le_bytes([high0, high1, high2, high3]),
            false,
            24,
        ))
    }
}
#[derive(Clone, Copy)]
struct IterationLimit(u32);
fn bisect(
    cash: &[(Days, Number)],
    duration: Days,
    output_days: Days,
    limit: IterationLimit,
) -> Result<PerformanceValue, PerformanceError> {
    if duration.0 == 0 || duration.0 > 4096 {
        return Err(PerformanceError::InvalidPeriod);
    }
    let mut low = RootPoint(989_999_999_999_000_000_000_000);
    let mut high = RootPoint(1_010_000_000_001_000_000_000_000);
    if residual(cash, duration, low.number())?.0 < Decimal::ZERO
        || residual(cash, duration, high.number())?.0 > Decimal::ZERO
    {
        return Err(PerformanceError::RootOutsideRange);
    }
    for _ in 0..limit.0 {
        let middle = RootPoint(low.0.wrapping_add(high.0).wrapping_div(2));
        if residual(cash, duration, middle.number())?.0 > Decimal::ZERO {
            low = middle;
        } else {
            high = middle;
        }
        if high.0.wrapping_sub(low.0) <= 1 {
            return certify_return(cash, duration, output_days, middle.number());
        }
    }
    Err(PerformanceError::NonConvergence)
}
#[derive(Clone, Copy)]
struct Checked(Option<Number>);
impl Checked {
    fn add(self, other: Self) -> Self {
        Self(
            self.0
                .zip(other.0)
                .and_then(|(left, right)| left.0.checked_add(right.0))
                .map(Number),
        )
    }
    fn sub(self, other: Self) -> Self {
        Self(
            self.0
                .zip(other.0)
                .and_then(|(left, right)| left.0.checked_sub(right.0))
                .map(Number),
        )
    }
    fn mul(self, other: Self) -> Self {
        Self(
            self.0
                .zip(other.0)
                .and_then(|(left, right)| left.0.checked_mul(right.0))
                .map(Number),
        )
    }
    fn div(self, other: Self) -> Self {
        Self(
            self.0
                .zip(other.0)
                .and_then(|(left, right)| left.0.checked_div(right.0))
                .map(Number),
        )
    }
}
fn constant(value: Number) -> Checked {
    Checked(Some(value))
}
fn certify_return(
    cash: &[(Days, Number)],
    duration: Days,
    output: Days,
    candidate: Number,
) -> Result<PerformanceValue, PerformanceError> {
    let residual_bound = Checked(
        residual(cash, duration, candidate)
            .ok()
            .map(|value| Number(value.0.abs())),
    );
    let largest_power = Checked(
        power(candidate, duration)
            .ok()
            .map(|value| Number(value.0.abs().max(Decimal::ONE))),
    );
    let growth = power(candidate, output).ok();
    let terms = constant(Number(Decimal::from(cash.len())));
    let factor = constant(Number(Decimal::from(duration.0)))
        .mul(constant(Number(Decimal::TWO)))
        .add(constant(Number(Decimal::from(cash.len()))))
        .add(constant(Number(Decimal::from(3_u32))));
    let budget = terms
        .mul(factor)
        .mul(largest_power.mul(constant(Number(Decimal::TWO))))
        .mul(constant(Number(Decimal::new(1, 26))));
    let relative_error = residual_bound
        .add(budget)
        .mul(constant(Number(Decimal::TWO)))
        .div(constant(Number(Decimal::new(1, 6))));
    let rate_error = relative_error
        .mul(constant(Number(Decimal::from(4_u32))))
        .mul(constant(Number(Decimal::from(output.0))))
        .mul(Checked(
            growth.map(|value| Number(value.0.max(Decimal::ONE))),
        ));
    let output_error = constant(Number(Decimal::from(output.0)))
        .mul(constant(Number(Decimal::TWO)))
        .mul(constant(Number(Decimal::new(1, 26))))
        .mul(Checked(
            growth.map(|estimate| Number(estimate.0.max(Decimal::ONE))),
        ));
    let certified_error = rate_error.add(output_error);
    let positive = (candidate.0 > Decimal::ZERO).then_some(());
    let value = Checked(growth).sub(constant(Number(Decimal::ONE)));
    certified_error
        .0
        .zip(value.0)
        .zip(positive)
        .map(|(estimate, _positive)| estimate)
        .filter(|(error, _)| error.0 <= Decimal::new(1, 10))
        .map(|(_, estimate)| PerformanceValue(estimate))
        .ok_or(PerformanceError::InsufficientPrecision)
}
pub(crate) fn annualize(
    value: PerformanceValue,
    days: Days,
) -> Result<PerformanceValue, PerformanceError> {
    if days.0 < 365 || value.0.0 == Decimal::NEGATIVE_ONE {
        return Ok(value);
    }
    mwr(
        &[
            (Days(0), Number(Decimal::NEGATIVE_ONE)),
            (days, value.0.add(Number(Decimal::ONE))?),
        ],
        days,
        Days(365),
    )
}

pub(crate) fn annual_variance(daily: Number) -> Result<Number, PerformanceError> {
    daily
        .0
        .checked_mul(daily.0)
        .and_then(|square| square.checked_mul(Decimal::from(252_u32)))
        .map(Number)
        .ok_or(PerformanceError::Arithmetic)
}
fn simple_return(previous: Number, current: Number) -> Result<PerformanceValue, PerformanceError> {
    current
        .0
        .checked_div(previous.0)
        .and_then(|ratio| ratio.checked_sub(Decimal::ONE))
        .map(|value| PerformanceValue(Number(value)))
        .ok_or(PerformanceError::Arithmetic)
}
pub(crate) fn price_return(
    previous: Number,
    current: Number,
) -> Result<PerformanceValue, PerformanceError> {
    let growth = current.div(previous)?;
    if growth.0 < Decimal::new(1, 6) {
        return Err(PerformanceError::InsufficientPrecision);
    }
    simple_return(previous, current)
}
pub(crate) fn benchmark_weight(native: Number, total: Number) -> Result<Number, PerformanceError> {
    if total.0 <= Decimal::ZERO || native.0 < Decimal::ZERO || native.0 > total.0 {
        return Err(PerformanceError::NonPositiveCapital);
    }
    if native.0.is_zero() {
        return Ok(Number(Decimal::ZERO));
    }
    if native == total {
        return Ok(Number(Decimal::ONE));
    }
    native
        .0
        .checked_div(total.0)
        .filter(|weight| *weight >= Decimal::new(1, 6) && *weight <= Decimal::new(999_999, 6))
        .map(Number)
        .ok_or(PerformanceError::InsufficientPrecision)
}
pub(crate) fn blended_return(
    weight: Number,
    domestic: Number,
    foreign: Number,
) -> Result<PerformanceValue, PerformanceError> {
    let result = weight
        .0
        .checked_mul(domestic.0)
        .zip(
            Decimal::ONE
                .checked_sub(weight.0)
                .and_then(|other| other.checked_mul(foreign.0)),
        )
        .and_then(|(left, right)| left.checked_add(right))
        .map(|value| PerformanceValue(Number(value)))
        .ok_or(PerformanceError::Arithmetic)?;
    let budget = domestic
        .0
        .abs()
        .max(Decimal::ONE)
        .checked_mul(Decimal::new(1, 26))
        .zip(
            foreign
                .0
                .abs()
                .max(Decimal::ONE)
                .checked_mul(Decimal::new(1, 26)),
        )
        .and_then(|(left, right)| left.checked_add(right))
        .and_then(|sum| sum.checked_mul(Decimal::from(100_u32)));
    budget
        .filter(|error| *error <= Decimal::new(1, 10))
        .map(|_error| result)
        .ok_or(PerformanceError::InsufficientPrecision)
}
#[cfg(test)]
mod tests {
    use super::{Days, Number, dietz, linked, mwr};
    use rust_decimal::Decimal;

    #[test]
    fn research_example_one_fixes_all_six_decimal_places() {
        fn number(value: i32) -> Number {
            Number(Decimal::from(value))
        }
        let first = dietz(
            number(10_000_000),
            number(1_000_000),
            &[(Days(180), number(5_000_000))],
            Days(180),
        )
        .expect("전반");
        let second = dietz(number(16_000_000), number(-600_000), &[], Days(183)).expect("후반");
        assert_eq!(
            linked(&[first, second]).expect("연결").to_string(),
            "0.058750"
        );
        assert_eq!(
            dietz(
                number(10_000_000),
                number(400_000),
                &[(Days(180), number(5_000_000))],
                Days(363)
            )
            .expect("디에츠")
            .to_string(),
            "0.031947"
        );
        let cash = [
            (Days(0), number(-10_000_000)),
            (Days(180), number(-5_000_000)),
            (Days(363), number(15_400_000)),
        ];
        assert_eq!(
            mwr(&cash, Days(363), Days(365))
                .expect("연율 기준")
                .to_string(),
            "0.032177"
        );
        assert_eq!(
            mwr(&cash, Days(363), Days(363)).expect("기간").to_string(),
            "0.031997"
        );
        for (horizon, reference) in [
            (Days(365), "0.0321766005132066747734361670"),
            (Days(363), "0.0319974993264703387104445618"),
        ] {
            let expected =
                crate::Ratio::parse(&crate::NumericInput::try_new(reference).expect("입력"))
                    .expect("독립 70자리 기준의 28자리 표현")
                    .as_decimal();
            let actual = mwr(&cash, Days(363), horizon)
                .expect("해")
                .number()
                .as_decimal();
            assert!(actual.checked_sub(expected).expect("차이").abs() < Decimal::new(1, 20));
        }
    }
}

#[cfg(test)]
mod contract_tests {
    use super::{
        Days, Number, PerformanceValue, annualize, bisect, dietz, linked, mwr, power, residual,
    };
    use crate::PerformanceError;
    use proptest::prelude::{prop_assert, proptest};
    use proptest::test_runner::Config;
    use rust_decimal::Decimal;
    fn number(value: i32) -> Number {
        Number::new(Decimal::from(value))
    }
    #[test]
    fn errors_are_closed_and_debug_redacts_values() {
        let zero = number(0);
        let one = number(1);
        let max = Number::new(Decimal::MAX);
        assert_eq!(format!("{one:?}"), "Number([REDACTED])");
        assert_eq!(max.add(max), Err(PerformanceError::Arithmetic));
        assert_eq!(max.neg().sub(max), Err(PerformanceError::Arithmetic));
        assert_eq!(max.mul(number(2)), Err(PerformanceError::Arithmetic));
        assert_eq!(one.div(zero), Err(PerformanceError::Arithmetic));
        assert_eq!(
            dietz(one, one, &[], Days(0)),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            dietz(zero, one, &[], Days(1)),
            Err(PerformanceError::NonPositiveCapital)
        );
        assert_eq!(
            dietz(one, one, &[(Days(2), one)], Days(1)),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            linked(&[PerformanceValue(number(-2))]),
            Err(PerformanceError::NonPositiveCapital)
        );
        assert_eq!(
            mwr(&[], Days(1), Days(1)),
            Err(PerformanceError::AmbiguousOrMissingRoot)
        );
        assert_eq!(
            mwr(
                &[
                    (Days(0), number(-1)),
                    (Days(1), number(2)),
                    (Days(2), number(-1))
                ],
                Days(2),
                Days(2)
            ),
            Err(PerformanceError::AmbiguousOrMissingRoot)
        );
    }
    #[test]
    fn root_failures_are_closed() {
        let one = number(1);
        let cash = [(Days(0), number(-100)), (Days(1), number(100))];
        assert_eq!(
            bisect(&cash, Days(0), Days(1), super::IterationLimit(128)),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            bisect(&cash, Days(4097), Days(1), super::IterationLimit(128)),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            bisect(&cash, Days(1), Days(1), super::IterationLimit(0)),
            Err(PerformanceError::NonConvergence)
        );
        assert_eq!(
            bisect(
                &[(Days(0), number(-1)), (Days(1), number(2))],
                Days(1),
                Days(1),
                super::IterationLimit(128)
            ),
            Err(PerformanceError::RootOutsideRange)
        );
        assert_eq!(
            bisect(
                &[(Days(0), number(-2)), (Days(1), number(1))],
                Days(1),
                Days(1),
                super::IterationLimit(128)
            ),
            Err(PerformanceError::RootOutsideRange)
        );
        assert_eq!(
            residual(&cash, Days(0), one),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(power(one, Days(0)), Ok(one));
        assert_eq!(
            format!("{:?}", PerformanceValue(number(123))),
            "PerformanceValue([REDACTED])"
        );
    }
    #[test]
    fn annualization_uses_actual_days_and_a_fixed_365_day_year() {
        let rate = PerformanceValue(Number::new(Decimal::new(10, 2)));
        assert_eq!(annualize(rate, Days(364)), Ok(rate));
        assert_eq!(
            annualize(rate, Days(365)).expect("365").to_string(),
            "0.100000"
        );
        assert_eq!(
            annualize(rate, Days(366)).expect("윤년").to_string(),
            "0.099714"
        );
    }
    proptest! {
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn no_flow_identity_and_partition_invariance(initial in 100_u32..10000,middle in 100_u32..10000,terminal in 100_u32..10000) {
            let start=Number::new(Decimal::from(initial)); let mid=Number::new(Decimal::from(middle)); let end=Number::new(Decimal::from(terminal));
            let total=dietz(start,end.sub(start).expect("차이"),&[],Days(1000)).expect("단순");
            let parts=[dietz(start,mid.sub(start).expect("차"),&[],Days(400)).expect("첫"),dietz(mid,end.sub(mid).expect("차"),&[],Days(600)).expect("뒤")];
            prop_assert!(linked(&parts).expect("연결").number().sub(total.number()).expect("오차").as_decimal().abs()<Decimal::new(1,20));
            let actual=mwr(&[(Days(0),start.neg()),(Days(1000),end)],Days(1000),Days(1000));
            let result=actual.expect("범위 안 유일근"); { prop_assert!(result.number().sub(total.number()).expect("오차").as_decimal().abs()<Decimal::new(1,16)); }
        }
    }
}

#[cfg(test)]
mod rational_tests {
    use super::{Days, Number, mwr};
    use proptest::prelude::{prop_assert, proptest};
    use proptest::test_runner::Config;
    use rust_decimal::Decimal;
    proptest! {
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn mwr_matches_independent_integer_rational_compounding(increment in 0_u32..11,exponent in 1_u32..7) {
            let initial=1000_u128.checked_pow(exponent).expect("정수");
            let terminal=u128::from(1000_u32.checked_add(increment).expect("기수")).checked_pow(exponent).expect("정수");
            let start=Number::new(Decimal::from(initial)); let end=Number::new(Decimal::from(terminal));
            let solved=mwr(&[(Days(0),start.neg()),(Days(exponent),end)],Days(exponent),Days(exponent)).expect("유일근");
            let scaled=solved.number().as_decimal().checked_mul(Decimal::from(1_000_000_000_000_u64)).expect("표시").round_dp(0).mantissa();
            let actual=u128::try_from(scaled).expect("양수").checked_mul(initial).expect("대조");
            let expected=terminal.checked_sub(initial).expect("이익").checked_mul(1_000_000_000_000).expect("대조");
            prop_assert!(actual.abs_diff(expected)<=initial);
        }
    }
}

#[cfg(test)]
mod cash_oracle_tests {
    use super::{Days, Number, mwr};
    use proptest::prelude::{prop_assert, proptest};
    use proptest::test_runner::Config;
    use rust_decimal::Decimal;
    proptest! {
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn deposit_timing_matches_an_independent_exact_rational_root(increment in -10_i32..11_i32,half_period in 1_u32..5,deposit_units in 2_000_000_u128..100_000_000,scale in 0_u32..29) {
            let total_days=half_period.checked_mul(2).expect("기간");
            let coefficient=u128::try_from(1000_i32.checked_add(increment).expect("기수")).expect("양수");
            let initial=1000_u128.checked_pow(total_days).expect("정수");
            let deposit=deposit_units.checked_mul(1000_u128.checked_pow(half_period).expect("정수")).expect("입금");
            let expected_terminal=coefficient.checked_pow(total_days).expect("정수");
            let terminal=expected_terminal.checked_add(deposit_units.checked_mul(coefficient.checked_pow(half_period).expect("정수")).expect("입금 성장")).expect("종료");
            let num=|value| Number::new(Decimal::from(value));
            let cash=[(Days(0),num(initial).neg()),(Days(half_period),num(deposit).neg()),(Days(total_days),num(terminal))];
            let result=mwr(&cash,Days(total_days),Days(total_days)).expect("단일 근");
            let scaled=result.number().as_decimal().checked_mul(Decimal::from(1_000_000_000_000_u64)).expect("표시").round_dp(0).mantissa();
            let actual=scaled.checked_mul(i128::try_from(initial).expect("계수")).expect("대조");
            let expected=i128::try_from(expected_terminal).expect("계수").checked_sub(i128::try_from(initial).expect("계수")).expect("손익").checked_mul(1_000_000_000_000).expect("대조");
            prop_assert!(actual.abs_diff(expected)<=initial);
            let scaled_cash=cash.iter().map(|(day,amount)| (*day,Number::new(Decimal::try_from_i128_with_scale(amount.as_decimal().mantissa(),scale).expect("정확한 단위 변경")))).collect::<Vec<_>>();
            prop_assert!(mwr(&scaled_cash,Days(total_days),Days(total_days)).expect("단위 불변")==result);
            let mut reverse=cash; reverse.reverse();
            prop_assert!(mwr(&reverse,Days(total_days),Days(total_days)).expect("순서")==result);
        }
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::{Days, Number, PerformanceValue, bisect, dietz, linked, mwr, power, residual};
    use crate::PerformanceError;
    use rust_decimal::Decimal;
    #[test]
    fn approximate_computation_propagates_overflow_at_each_stage() {
        let maximum = Number::new(Decimal::MAX);
        let one = Number::new(Decimal::ONE);
        let two = Number::new(Decimal::TWO);
        for result in [
            dietz(maximum, one, &[(Days(0), maximum)], Days(1)),
            dietz(Number::new(Decimal::new(1, 28)), maximum, &[], Days(1)),
            linked(&[PerformanceValue(maximum)]),
            linked(&[PerformanceValue(maximum.div(two).expect("절반")); 2]),
            mwr(&[(Days(0), maximum), (Days(0), maximum)], Days(1), Days(1)),
            bisect(
                &[(Days(0), maximum), (Days(1), maximum)],
                Days(1),
                Days(1),
                super::IterationLimit(128),
            ),
            bisect(
                &[(Days(0), maximum.neg()), (Days(1), maximum)],
                Days(1),
                Days(u32::MAX),
                super::IterationLimit(128),
            ),
        ] {
            assert_eq!(result, Err(PerformanceError::Arithmetic));
        }
        assert_eq!(
            residual(&[(Days(0), one)], Days(2), maximum),
            Err(PerformanceError::Arithmetic)
        );
        assert_eq!(power(maximum, Days(2)), Err(PerformanceError::Arithmetic));
        assert_eq!(power(maximum, Days(3)), Err(PerformanceError::Arithmetic));
        assert_eq!(
            residual(&[(Days(0), maximum)], Days(1), two),
            Err(PerformanceError::Arithmetic)
        );
        assert_eq!(
            residual(&[(Days(0), maximum), (Days(1), maximum)], Days(1), one),
            Err(PerformanceError::Arithmetic)
        );
    }
}

#[cfg(test)]
mod benchmark_arithmetic_tests {
    use super::{Number, benchmark_weight, blended_return, price_return};
    use crate::PerformanceError;
    use rust_decimal::Decimal;
    #[test]
    fn benchmark_helpers_preserve_approximation_and_fail_closed() {
        let zero = Number::new(Decimal::ZERO);
        let one = Number::new(Decimal::ONE);
        let two = Number::new(Decimal::TWO);
        let maximum = Number::new(Decimal::MAX);
        assert_eq!(
            super::PerformanceValue(one).direction(),
            crate::DisplayDirection::Up
        );
        assert_eq!(
            super::PerformanceValue(one.neg()).direction(),
            crate::DisplayDirection::Down
        );
        assert_eq!(
            super::PerformanceValue(zero).direction(),
            crate::DisplayDirection::Flat
        );
        assert_eq!(price_return(zero, one), Err(PerformanceError::Arithmetic));
        assert_eq!(
            price_return(Number::new(Decimal::new(1, 28)), maximum),
            Err(PerformanceError::Arithmetic)
        );
        assert_eq!(
            benchmark_weight(one, zero),
            Err(PerformanceError::NonPositiveCapital)
        );
        assert_eq!(
            benchmark_weight(one.neg(), one),
            Err(PerformanceError::NonPositiveCapital)
        );
        assert_eq!(
            benchmark_weight(two, one),
            Err(PerformanceError::NonPositiveCapital)
        );
        assert_eq!(
            benchmark_weight(one, two).expect("비중").as_decimal(),
            Decimal::new(5, 1)
        );
        assert_eq!(
            blended_return(two, maximum, maximum),
            Err(PerformanceError::Arithmetic)
        );
        assert_eq!(
            blended_return(Number::new(Decimal::new(5, 1)), maximum, maximum),
            Err(PerformanceError::Arithmetic)
        );
    }
}

#[cfg(test)]
mod root_stage_tests {
    use super::{Days, Number, PerformanceValue, annualize, bisect, power};
    use crate::PerformanceError;
    use rust_decimal::Decimal;
    #[test]
    fn root_midpoint_residual_output_power_and_annualization_errors_are_checked() {
        let maximum = Number::new(Decimal::MAX);
        let coefficient = |mantissa| {
            maximum
                .mul(Number::new(Decimal::new(mantissa, 5)))
                .expect("계수")
        };
        let cash = [
            (Days(0), coefficient(49500_i64).neg()),
            (Days(1), coefficient(98990_i64)),
            (Days(2), coefficient(50511_i64)),
            (Days(2), coefficient(99996_i64).neg()),
        ];
        assert_eq!(
            bisect(&cash, Days(2), Days(2), super::IterationLimit(128)),
            Err(PerformanceError::Arithmetic)
        );
        assert_eq!(
            bisect(
                &[
                    (Days(0), Number::new(Decimal::from(-100_i32))),
                    (Days(1), Number::new(Decimal::from(101_u32)))
                ],
                Days(1),
                Days(100_000),
                super::IterationLimit(128)
            ),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            power(Number::new(Decimal::from(10_000_000_000_u64)), Days(3)),
            Err(PerformanceError::Arithmetic)
        );
        assert_eq!(
            annualize(PerformanceValue(maximum), Days(365)),
            Err(PerformanceError::Arithmetic)
        );
    }
}

#[cfg(test)]
mod closed_form_tests {
    use super::{Days, Number, mwr};
    use crate::PerformanceError;
    use rust_decimal::Decimal;
    #[test]
    fn two_endpoint_cash_flows_have_no_artificial_daily_root_bound() {
        let initial = Number::new(Decimal::from(-100_i32));
        let last = Number::new(Decimal::from(200_u32));
        let equal = Number::new(Decimal::from(100_u32));
        assert_eq!(
            mwr(&[(Days(0), initial), (Days(1), last)], Days(1), Days(1))
                .expect("하루 두 배")
                .to_string(),
            "1.000000"
        );
        assert_eq!(
            mwr(&[(Days(0), initial), (Days(1), equal)], Days(2), Days(2))
                .expect("조기 인출")
                .to_string(),
            "0.000000"
        );
        assert_eq!(
            mwr(&[(Days(1), initial), (Days(2), equal)], Days(2), Days(2))
                .expect("늦은 입금")
                .to_string(),
            "0.000000"
        );
        assert_eq!(
            mwr(&[], Days(0), Days(1)),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            mwr(&[], Days(4097), Days(1)),
            Err(PerformanceError::InvalidPeriod)
        );
    }
}

#[cfg(test)]
mod conditioning_tests {
    use super::{Days, Number, mwr, root_cash};
    use crate::{NumericInput, PerformanceError, Ratio};
    use rust_decimal::Decimal;
    fn number(text: &str) -> Number {
        Number::new(
            Ratio::parse(&NumericInput::try_new(text).expect("입력"))
                .expect("수")
                .as_decimal(),
        )
    }
    #[test]
    fn scaling_cash_units_does_not_create_false_convergence() {
        let ordinary = [
            (Days(0), number("-1")),
            (Days(180), number("-1")),
            (Days(365), number("3")),
        ];
        let tiny = [
            (Days(0), number("-0.0000000000000000000000000001")),
            (Days(180), number("-0.0000000000000000000000000001")),
            (Days(365), number("0.0000000000000000000000000003")),
        ];
        assert_eq!(
            mwr(&ordinary, Days(365), Days(365)),
            mwr(&tiny, Days(365), Days(365))
        );
        assert_eq!(
            mwr(
                &[
                    (Days(0), number("-1")),
                    (Days(1), number("0.0000000000000000000000000001")),
                    (Days(2), number("1"))
                ],
                Days(2),
                Days(2)
            ),
            Err(PerformanceError::InsufficientPrecision)
        );
        let cancellation = [
            (Days(0), number("-1")),
            (Days(1), number("-1000000000000")),
            (Days(1), number("999999999999.9999999999999999")),
            (Days(2), number("1")),
        ];
        assert_eq!(
            mwr(&cancellation, Days(2), Days(2)),
            Err(PerformanceError::InsufficientPrecision)
        );
    }
    #[test]
    fn normalized_endpoint_root_does_not_fail_from_rounding_noise() {
        let cash = [
            (Days(0), number("-1000000")),
            (Days(1), number("-530000")),
            (Days(2), number("1555400")),
        ];
        assert_eq!(
            mwr(&cash, Days(2), Days(2))
                .expect("상한의 실제 근")
                .to_string(),
            "0.020100"
        );
    }
    #[test]
    fn normalization_rejects_invalid_dates_and_zero_scale() {
        let one = Number::new(Decimal::ONE);
        let zero = Number::new(Decimal::ZERO);
        assert_eq!(
            root_cash(&[], Days(1)).expect_err("빈 열"),
            PerformanceError::InvalidPeriod
        );
        assert_eq!(
            root_cash(&[(Days(0), one)], Days(1)).expect_err("단일 날짜"),
            PerformanceError::InvalidPeriod
        );
        assert_eq!(
            root_cash(&[(Days(0), one), (Days(2), one)], Days(1)).expect_err("기간 밖"),
            PerformanceError::InvalidPeriod
        );
        assert_eq!(
            root_cash(&[(Days(0), zero), (Days(1), zero)], Days(1)).expect_err("영"),
            PerformanceError::Arithmetic
        );
        assert_eq!(
            root_cash(&[(Days(2), one), (Days(1), one), (Days(3), one)], Days(3))
                .expect_err("역순"),
            PerformanceError::InvalidPeriod
        );
    }
}

#[cfg(test)]
mod precision_tests {
    use super::{
        Checked, Days, Number, PerformanceValue, annualize, benchmark_weight, blended_return,
        canonical_cash, certify_return, constant, dietz, ensure_growth, factor_product, mwr,
        price_return,
    };
    use crate::PerformanceError;
    use rust_decimal::Decimal;
    fn number(text: &str) -> Number {
        Number(
            crate::Ratio::parse(&crate::NumericInput::try_new(text).expect("입력"))
                .expect("십진")
                .as_decimal(),
        )
    }
    #[test]
    fn loss_conditioning_underflow_and_rounding_have_closed_results() {
        let one = number("1");
        let tiny = number("0.0000000000000000000000000001");
        let zero = number("0");
        assert_eq!(
            dietz(one, number("-2"), &[], Days(1)),
            Err(PerformanceError::NonPositiveCapital)
        );
        assert_eq!(
            dietz(one, number("-1"), &[], Days(1))
                .expect("실제 전액 손실")
                .to_string(),
            "-1.000000"
        );
        assert_eq!(
            dietz(one, number("-1"), &[(Days(1), one)], Days(1)),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            dietz(one, number("-0.99999999"), &[], Days(1)),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            dietz(one, number("-0.9999999999999999999999999999"), &[], Days(1)),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            dietz(one, one, &[(Days(0), number("-0.99999999999999"))], Days(1)),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            ensure_growth(one, Number(Decimal::MAX), &[]),
            Err(PerformanceError::Arithmetic)
        );
        assert_eq!(
            factor_product(tiny, tiny),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(factor_product(one, zero), Ok(zero));
        assert_eq!(
            ensure_growth(zero, one, &[]),
            Err(PerformanceError::Arithmetic)
        );
        assert_eq!(
            price_return(one, tiny),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            annualize(PerformanceValue(number("-1")), Days(366)),
            Ok(PerformanceValue(number("-1")))
        );
    }
    #[test]
    fn benchmark_weights_and_direction_cannot_amplify_tiny_rounding() {
        let one = number("1");
        let zero = number("0");
        let tiny = number("0.0000000000000000000000000001");
        assert_eq!(benchmark_weight(zero, one), Ok(zero));
        assert_eq!(benchmark_weight(one, one), Ok(one));
        assert_eq!(
            benchmark_weight(tiny, one),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            benchmark_weight(number("0.99999999"), one),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            blended_return(number("0.5"), number("100000000000000000000"), zero),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            PerformanceValue(tiny).direction(),
            crate::DisplayDirection::Flat
        );
        assert_eq!(
            PerformanceValue(tiny.neg()).direction(),
            crate::DisplayDirection::Flat
        );
    }
    #[test]
    fn residual_certificate_rejects_false_roots_and_unrepresentable_cash_sums() {
        let one = number("1");
        let cash = [(Days(0), one.neg()), (Days(365), one)];
        assert_eq!(
            certify_return(&cash, Days(365), Days(365), number("1.01")),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            certify_return(&cash, Days(365), Days(365), one.neg()),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            certify_return(&cash, Days(365), Days(365), Number(Decimal::MAX)),
            Err(PerformanceError::InsufficientPrecision)
        );
        assert_eq!(
            Checked(Some(Number(Decimal::MAX))).add(constant(one)).0,
            None
        );
        assert_eq!(
            Checked(Some(Number(Decimal::MIN))).sub(constant(one)).0,
            None
        );
        assert_eq!(constant(one).div(constant(number("0"))).0, None);
        assert_eq!(
            constant(Number(Decimal::MAX)).mul(constant(number("2"))).0,
            None
        );
        let duplicated = [
            (Days(0), number("-10")),
            (Days(0), number("-10")),
            (Days(2), number("22")),
        ];
        assert_eq!(
            mwr(&duplicated, Days(2), Days(2))
                .expect("동일 날짜 합산")
                .to_string(),
            "0.100000"
        );
        let excess = [(Days(0), number("1.0000000000000000000000000001")); 8];
        assert_eq!(canonical_cash(&excess), Err(PerformanceError::Arithmetic));
    }
}
