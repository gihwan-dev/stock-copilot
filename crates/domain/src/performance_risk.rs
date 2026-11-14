use crate::performance_arithmetic::{Number, PerformanceValue, linked};
use crate::{BenchmarkSeries, Performance, PerformanceError};
use rust_decimal::Decimal;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskMetric {
    InsufficientSamples,
    Value(PerformanceValue),
    Unavailable(PerformanceError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceRisk {
    daily: RiskMetric,
    annual: RiskMetric,
}
impl PerformanceRisk {
    #[must_use]
    pub const fn daily(self) -> RiskMetric {
        self.daily
    }
    #[must_use]
    pub const fn annual(self) -> RiskMetric {
        self.annual
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceBeta {
    start: crate::CalendarDate,
    end: crate::CalendarDate,
    weeks: PerformanceWeeks,
    beta: PerformanceValue,
    r_squared: PerformanceValue,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerformanceWeeks(usize);
impl std::fmt::Display for PerformanceWeeks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}
impl PerformanceBeta {
    #[must_use]
    pub const fn start(self) -> crate::CalendarDate {
        self.start
    }
    #[must_use]
    pub const fn end(self) -> crate::CalendarDate {
        self.end
    }
    #[must_use]
    pub const fn weeks(self) -> PerformanceWeeks {
        self.weeks
    }
    #[must_use]
    pub const fn beta(self) -> PerformanceValue {
        self.beta
    }
    #[must_use]
    pub const fn r_squared(self) -> PerformanceValue {
        self.r_squared
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BetaMetric {
    InsufficientSamples,
    Value(PerformanceBeta),
    Unavailable(PerformanceError),
}
#[must_use]
pub fn performance_volatility(performance: &Performance) -> PerformanceRisk {
    if performance
        .daily_samples
        .iter()
        .filter(|sample| **sample == crate::performance::DailySample::Trading)
        .count()
        < 60
    {
        return PerformanceRisk {
            daily: RiskMetric::InsufficientSamples,
            annual: RiskMetric::InsufficientSamples,
        };
    }
    let result = daily_cadence(performance)
        .and_then(|()| variance(&performance.returns))
        .and_then(root);
    let daily = metric(result);
    let annual = metric(result.and_then(|value| {
        let variance = crate::performance_arithmetic::annual_variance(value.number())?;
        root(variance)
    }));
    PerformanceRisk { daily, annual }
}
#[must_use]
pub fn performance_beta(performance: &Performance, benchmark: &BenchmarkSeries) -> BetaMetric {
    if crate::benchmark::alignment(benchmark, performance).is_err() {
        return BetaMetric::Unavailable(PerformanceError::BenchmarkMismatch);
    }
    match weekly(performance, &benchmark.returns) {
        Ok(samples) => beta_from_weekly(&samples),
        Err(cause) => BetaMetric::Unavailable(cause),
    }
}
struct WeeklyReturns {
    portfolio: Vec<PerformanceValue>,
    market: Vec<PerformanceValue>,
    start: crate::CalendarDate,
    end: crate::CalendarDate,
}
fn beta_from_weekly(samples: &WeeklyReturns) -> BetaMetric {
    if samples.portfolio.len() < 52 {
        return BetaMetric::InsufficientSamples;
    }
    match covariance_beta(&samples.portfolio, &samples.market) {
        Ok(value) => BetaMetric::Value(PerformanceBeta {
            beta: value.beta,
            r_squared: value.r_squared,
            start: samples.start,
            end: samples.end,
            weeks: PerformanceWeeks(samples.portfolio.len()),
        }),
        Err(cause) => BetaMetric::Unavailable(cause),
    }
}
#[derive(Debug, PartialEq, Eq)]
struct BetaCoefficients {
    beta: PerformanceValue,
    r_squared: PerformanceValue,
}

fn metric(value: Result<PerformanceValue, PerformanceError>) -> RiskMetric {
    match value {
        Ok(result) => RiskMetric::Value(result),
        Err(cause) => RiskMetric::Unavailable(cause),
    }
}
fn root(value: Number) -> Result<PerformanceValue, PerformanceError> {
    crate::volatility_arithmetic::square_root(crate::Ratio::new(value.as_decimal()))
        .map(|result| PerformanceValue::from_number(Number::new(result.as_decimal())))
        .map_err(|_cause| PerformanceError::Arithmetic)
}
fn mean(values: &[PerformanceValue]) -> Result<Number, PerformanceError> {
    values
        .iter()
        .try_fold(Number::new(Decimal::ZERO), |sum, value| {
            sum.add(value.number())
        })?
        .div(Number::new(Decimal::from(values.len())))
}
fn variance(values: &[PerformanceValue]) -> Result<Number, PerformanceError> {
    let average = mean(values)?;
    values
        .iter()
        .try_fold(Number::new(Decimal::ZERO), |sum, value| {
            let difference = value.number().sub(average)?;
            sum.add(difference.mul(difference)?)
        })?
        .div(Number::new(Decimal::from(values.iter().skip(1).count())))
}
fn covariance_beta(
    portfolio: &[PerformanceValue],
    market: &[PerformanceValue],
) -> Result<BetaCoefficients, PerformanceError> {
    let port_mean = mean(portfolio)?;
    let market_mean = mean(market)?;
    let covariance = portfolio
        .iter()
        .zip(market)
        .try_fold(Number::new(Decimal::ZERO), |sum, (left, right)| {
            sum.add(
                left.number()
                    .sub(port_mean)?
                    .mul(right.number().sub(market_mean)?)?,
            )
        })?
        .div(Number::new(Decimal::from(portfolio.iter().skip(1).count())))?;
    let port_var = variance(portfolio)?;
    let market_var = variance(market)?;
    Ok(BetaCoefficients {
        beta: PerformanceValue::from_number(covariance.div(market_var)?),
        r_squared: PerformanceValue::from_number(
            covariance.mul(covariance)?.div(port_var.mul(market_var)?)?,
        ),
    })
}
fn daily_cadence(performance: &Performance) -> Result<(), PerformanceError> {
    if performance
        .daily_samples
        .contains(&crate::performance::DailySample::Unverified)
    {
        return Err(PerformanceError::InvalidPeriod);
    }
    for ((old, today), date) in performance
        .offsets
        .iter()
        .zip(performance.offsets.iter().skip(1))
        .zip(&performance.dates)
    {
        let elapsed = today.as_u32().wrapping_sub(*old.as_u32());
        if elapsed == 1 {
            continue;
        }
        if elapsed == 3 && *crate::domain_time::performance_weekday(*date).as_u8() == 5 {
            continue;
        }
        return Err(PerformanceError::InvalidPeriod);
    }
    Ok(())
}
fn weekly(
    performance: &Performance,
    market: &[PerformanceValue],
) -> Result<WeeklyReturns, PerformanceError> {
    let total = *performance.duration.as_u32();
    let weeks = total.wrapping_div(7);
    let remainder = total.wrapping_rem(7);
    let mut portfolio = Vec::new();
    let mut benchmark = Vec::new();
    let mut first = performance.start();
    let mut last = performance.start();
    let boundaries = (0..=weeks)
        .map(|index| {
            let boundary = remainder.wrapping_add(index.wrapping_mul(7));
            performance
                .offsets
                .iter()
                .zip(&performance.dates)
                .enumerate()
                .rev()
                .find(|(_, (offset, _))| {
                    let day = *offset.as_u32();
                    day <= boundary && boundary.wrapping_sub(day) < 7
                })
                .map(|(position, (_, date))| (position, *date))
        })
        .collect::<Vec<_>>();
    for (old, today) in boundaries.iter().zip(boundaries.iter().skip(1)) {
        if let (Some((start, start_date)), Some((end, end_date))) = (old, today) {
            let count = end.wrapping_sub(*start);
            if portfolio.is_empty() {
                first = *start_date;
            }
            last = *end_date;
            portfolio.push(linked(
                &performance
                    .returns
                    .iter()
                    .skip(*start)
                    .take(count)
                    .copied()
                    .collect::<Vec<_>>(),
            )?);
            benchmark.push(linked(
                &market
                    .iter()
                    .skip(*start)
                    .take(count)
                    .copied()
                    .collect::<Vec<_>>(),
            )?);
        }
    }
    Ok(WeeklyReturns {
        portfolio,
        market: benchmark,
        start: first,
        end: last,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        Number, PerformanceError, PerformanceValue, RiskMetric, covariance_beta, mean, metric,
        root, variance,
    };
    use rust_decimal::Decimal;
    fn returns(prices: &[u32]) -> Vec<PerformanceValue> {
        prices
            .iter()
            .zip(prices.iter().skip(1))
            .map(|(old, today)| {
                PerformanceValue::from_number(
                    Number::new(Decimal::from(*today))
                        .div(Number::new(Decimal::from(*old)))
                        .expect("분모")
                        .sub(Number::new(Decimal::ONE))
                        .expect("차"),
                )
            })
            .collect()
    }
    #[test]
    fn research_example_nine_computes_sample_statistics_without_granting_display_permission() {
        let portfolio = returns(&[100, 102, 99, 101, 104, 100, 98, 103]);
        let market = returns(&[1000, 1010, 995, 1005, 1020, 1000, 990, 1015]);
        let daily = root(variance(&portfolio).expect("분산")).expect("제곱근");
        assert_eq!(daily.to_string(), "0.033867");
        assert_eq!(
            root(
                daily
                    .number()
                    .mul(daily.number())
                    .expect("제곱")
                    .mul(Number::new(Decimal::from(252_u32)))
                    .expect("연율")
            )
            .expect("제곱근")
            .to_string(),
            "0.537618"
        );
        let result = covariance_beta(&portfolio, &market).expect("베타");
        assert_eq!(result.beta.to_string(), "1.993821");
        assert_eq!(result.r_squared.to_string(), "0.999919");
        assert_eq!(mean(&[]), Err(PerformanceError::Arithmetic));
        assert_eq!(variance(&[]), Err(PerformanceError::Arithmetic));
        assert_eq!(
            root(Number::new(Decimal::NEGATIVE_ONE)),
            Err(PerformanceError::Arithmetic)
        );
        assert_eq!(
            metric(Err(PerformanceError::Arithmetic)),
            RiskMetric::Unavailable(PerformanceError::Arithmetic)
        );
        assert_eq!(metric(Ok(daily)), RiskMetric::Value(daily));
    }
    #[test]
    fn extreme_statistics_propagate_checked_errors() {
        let val = |decimal| PerformanceValue::from_number(Number::new(decimal));
        let maximum = val(Decimal::MAX);
        let minimum = val(Decimal::MIN);
        let zero = val(Decimal::ZERO);
        for result in [
            mean(&[maximum, maximum]),
            variance(&[zero]),
            variance(&[maximum, minimum]),
            variance(&[maximum, minimum, minimum]),
        ] {
            assert_eq!(result, Err(PerformanceError::Arithmetic));
        }
        for result in [
            covariance_beta(&[maximum, maximum], &[zero, zero]),
            covariance_beta(&[maximum, minimum, minimum], &[zero, zero, zero]),
            covariance_beta(&[zero], &[zero]),
            covariance_beta(&[zero, zero], &[zero, zero]),
        ] {
            assert_eq!(result, Err(PerformanceError::Arithmetic));
        }
    }
    #[test]
    fn covariance_and_r_squared_check_all_overflow_stages() {
        let val = |decimal| PerformanceValue::from_number(Number::new(decimal));
        let maximum = val(Decimal::MAX);
        let minimum = val(Decimal::MIN);
        let zero = val(Decimal::ZERO);
        let one = val(Decimal::ONE);
        let large = val(Decimal::from(100_000_000_u32));
        let negative = val(Decimal::from(-100_000_000_i32));
        for result in [
            covariance_beta(&[zero, zero], &[maximum, maximum]),
            covariance_beta(&[zero, zero, zero], &[maximum, minimum, minimum]),
            covariance_beta(&[maximum, minimum], &[maximum, minimum]),
            covariance_beta(&[one, val(Decimal::NEGATIVE_ONE)], &[maximum, minimum]),
            covariance_beta(&[maximum, minimum], &[zero, zero]),
            covariance_beta(&[zero, zero], &[maximum, minimum]),
            covariance_beta(&[zero, zero], &[zero, one]),
            covariance_beta(&[zero, large], &[zero, large]),
            covariance_beta(
                &[large, negative, large, negative],
                &[large, large, negative, negative],
            ),
        ] {
            assert_eq!(result, Err(PerformanceError::Arithmetic));
        }
    }
}
