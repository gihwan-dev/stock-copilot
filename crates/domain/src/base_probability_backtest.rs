use crate::calibration_arithmetic::{CalibrationNumber, average, squared_error, sum};
use crate::estimate_outcome::{Touch, touch};
use crate::estimate_outcome_arithmetic::OutcomeBoundaries;
use crate::{
    BaseProbability, Candle, HorizonSelection, RiskAppetite, TradingDayHorizon, Volatility,
    VolatilityError,
};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CandleCount(usize);

const VOLATILITY_CANDLES: CandleCount = CandleCount(61);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpperEdge {
    Open,
    Closed,
}

fn bins() -> impl Iterator<Item = (BacktestStatistic, BacktestStatistic, UpperEdge)> {
    [20_i64, 22, 24, 26, 28, 30, 32, 34, 36, 38]
        .into_iter()
        .map(|low| {
            let high = low.saturating_add(2);
            (
                BacktestStatistic(Decimal::new(low, 2)),
                BacktestStatistic(Decimal::new(high, 2)),
                if high == 40 {
                    UpperEdge::Closed
                } else {
                    UpperEdge::Open
                },
            )
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktestHorizon {
    FewDays,
    WeeksToMonths,
    HalfYearOrMore,
}
impl BacktestHorizon {
    const fn selection(self) -> HorizonSelection {
        match self {
            Self::FewDays => HorizonSelection::FewDays,
            Self::WeeksToMonths => HorizonSelection::WeeksToMonths,
            Self::HalfYearOrMore => HorizonSelection::HalfYearOrMore,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BacktestError {
    Series(VolatilityError),
    Arithmetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BacktestCount(usize);
impl std::fmt::Display for BacktestCount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct BacktestStatistic(Decimal);
impl std::fmt::Display for BacktestStatistic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.normalize().fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Hit,
    Stop,
    Ambiguous,
    Expired,
}
impl Outcome {
    const fn observed(self) -> CalibrationNumber {
        match self {
            Self::Hit => CalibrationNumber::ONE,
            Self::Stop | Self::Ambiguous | Self::Expired => CalibrationNumber::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Evaluation {
    probability: BaseProbability,
    outcome: Outcome,
}
impl Evaluation {
    fn predicted(self) -> CalibrationNumber {
        CalibrationNumber::new(self.probability.as_decimal())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Attempt {
    Evaluated(Evaluation),
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BacktestSummary {
    mean_probability: BacktestStatistic,
    hit_rate: BacktestStatistic,
    brier: BacktestStatistic,
    reference_brier: BacktestStatistic,
    expanding_brier: BacktestStatistic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BinRates {
    mean_probability: BacktestStatistic,
    hit_rate: BacktestStatistic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ReliabilityBin {
    lower: BacktestStatistic,
    upper: BacktestStatistic,
    edge: UpperEdge,
    count: BacktestCount,
    rates: Option<BinRates>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseProbabilityBacktest {
    days: CandleCount,
    appetite: RiskAppetite,
    evaluated: BacktestCount,
    skipped: BacktestCount,
    hits: BacktestCount,
    stops: BacktestCount,
    ambiguous: BacktestCount,
    expired: BacktestCount,
    summary: Option<BacktestSummary>,
    bins: Vec<ReliabilityBin>,
}
impl BaseProbabilityBacktest {
    pub fn evaluate(
        candles: &[Candle],
        horizon: BacktestHorizon,
        appetite: RiskAppetite,
    ) -> Result<Self, BacktestError> {
        Volatility::validate_series(candles).map_err(BacktestError::Series)?;
        let trading_days = TradingDayHorizon::from_estimate_policy(horizon.selection()).as_ratio();
        let days =
            usize::try_from(trading_days.as_decimal()).map_err(|_err| BacktestError::Arithmetic)?;
        let mut remaining = candles;
        let mut attempts = Vec::new();
        while let Some((history, rest)) = remaining.split_at_checked(VOLATILITY_CANDLES.0) {
            let Some(future) = rest.get(..days) else {
                break;
            };
            let current = history.last().ok_or(BacktestError::Arithmetic)?;
            attempts.push(attempt(history, current, future, trading_days, appetite)?);
            remaining = remaining.get(days..).ok_or(BacktestError::Arithmetic)?;
        }
        Self::from_attempts(CandleCount(days), appetite, &attempts)
    }
    fn from_attempts(
        days: CandleCount,
        appetite: RiskAppetite,
        attempts: &[Attempt],
    ) -> Result<Self, BacktestError> {
        let evaluations: Vec<Evaluation> = attempts
            .iter()
            .filter_map(|attempt| match attempt {
                Attempt::Evaluated(evaluation) => Some(*evaluation),
                Attempt::Skipped => None,
            })
            .collect();
        let tally = |outcome: Outcome| {
            BacktestCount(
                evaluations
                    .iter()
                    .filter(|evaluation| evaluation.outcome == outcome)
                    .count(),
            )
        };
        Ok(Self {
            days,
            appetite,
            evaluated: BacktestCount(evaluations.len()),
            skipped: BacktestCount(
                attempts
                    .iter()
                    .filter(|attempt| **attempt == Attempt::Skipped)
                    .count(),
            ),
            hits: tally(Outcome::Hit),
            stops: tally(Outcome::Stop),
            ambiguous: tally(Outcome::Ambiguous),
            expired: tally(Outcome::Expired),
            summary: summarize(&evaluations)?,
            bins: reliability(&evaluations)?,
        })
    }
}
impl std::fmt::Display for BaseProbabilityBacktest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let appetite = match self.appetite {
            RiskAppetite::Neutral => "neutral",
            RiskAppetite::Conservative => "conservative",
            RiskAppetite::Aggressive => "aggressive",
        };
        writeln!(f, "days={} appetite={appetite}", self.days.0)?;
        writeln!(
            f,
            "evaluated={} skipped={} hit={} stop={} ambiguous={} expired={}",
            self.evaluated, self.skipped, self.hits, self.stops, self.ambiguous, self.expired
        )?;
        match self.summary {
            Some(summary) => writeln!(
                f,
                "mean_p0={} hit_rate={} brier={} fixed_third_brier={} expanding_brier={}",
                summary.mean_probability,
                summary.hit_rate,
                summary.brier,
                summary.reference_brier,
                summary.expanding_brier
            )?,
            None => writeln!(f, "summary=none")?,
        }
        for bin in &self.bins {
            let close = match bin.edge {
                UpperEdge::Open => ')',
                UpperEdge::Closed => ']',
            };
            write!(
                f,
                "bin=[{},{}{} count={}",
                bin.lower, bin.upper, close, bin.count
            )?;
            match bin.rates {
                Some(rates) => writeln!(
                    f,
                    " mean_p0={} hit_rate={}",
                    rates.mean_probability, rates.hit_rate
                )?,
                None => writeln!(f, " mean_p0=- hit_rate=-")?,
            }
        }
        Ok(())
    }
}

fn attempt(
    history: &[Candle],
    current: &Candle,
    future: &[Candle],
    days: crate::Ratio,
    appetite: RiskAppetite,
) -> Result<Attempt, BacktestError> {
    let volatility = Volatility::calculate(history).map_err(BacktestError::Series)?;
    let Ok((target, stop, probability)) =
        crate::base_probability::distances(appetite, volatility.daily(), days)
    else {
        return Ok(Attempt::Skipped);
    };
    let boundaries = OutcomeBoundaries::new(current.values().close(), target, stop, &[]);
    let outcome = future
        .iter()
        .find_map(|candle| match touch(candle, &boundaries) {
            Touch::Neither => None,
            Touch::Target => Some(Outcome::Hit),
            Touch::Stop => Some(Outcome::Stop),
            Touch::Both => Some(Outcome::Ambiguous),
        })
        .unwrap_or(Outcome::Expired);
    Ok(Attempt::Evaluated(Evaluation {
        probability,
        outcome,
    }))
}

fn total(
    mut values: impl Iterator<Item = CalibrationNumber>,
) -> Result<CalibrationNumber, BacktestError> {
    values.try_fold(CalibrationNumber::ZERO, |left, right| {
        sum(left, right).ok_or(BacktestError::Arithmetic)
    })
}

fn mean(
    values: impl Iterator<Item = CalibrationNumber>,
    count: CalibrationNumber,
) -> Result<CalibrationNumber, BacktestError> {
    average(total(values)?, count).ok_or(BacktestError::Arithmetic)
}

fn statistic(value: CalibrationNumber) -> BacktestStatistic {
    BacktestStatistic(value.as_decimal())
}

fn rates(evaluations: &[&Evaluation]) -> Result<Option<BinRates>, BacktestError> {
    if evaluations.is_empty() {
        return Ok(None);
    }
    let count = CalibrationNumber::new(Decimal::from(evaluations.len()));
    Ok(Some(BinRates {
        mean_probability: statistic(mean(
            evaluations.iter().map(|evaluation| evaluation.predicted()),
            count,
        )?),
        hit_rate: statistic(mean(
            evaluations
                .iter()
                .map(|evaluation| evaluation.outcome.observed()),
            count,
        )?),
    }))
}

fn brier(
    evaluations: &[Evaluation],
    forecast: impl Fn(Evaluation) -> CalibrationNumber,
) -> Result<BacktestStatistic, BacktestError> {
    let count = CalibrationNumber::new(Decimal::from(evaluations.len()));
    let total = evaluations
        .iter()
        .try_fold(CalibrationNumber::ZERO, |acc, value| {
            let error = squared_error(forecast(*value), value.outcome.observed())?;
            sum(acc, error)
        })
        .ok_or(BacktestError::Arithmetic)?;
    average(total, count)
        .map(statistic)
        .ok_or(BacktestError::Arithmetic)
}

fn one_third() -> Result<CalibrationNumber, BacktestError> {
    average(
        CalibrationNumber::ONE,
        CalibrationNumber::new(Decimal::from(3_u8)),
    )
    .ok_or(BacktestError::Arithmetic)
}

fn historical_brier(
    outcomes: impl Iterator<Item = Outcome>,
    initial: CalibrationNumber,
) -> Result<BacktestStatistic, BacktestError> {
    let mut count = CalibrationNumber::ZERO;
    let mut hits = CalibrationNumber::ZERO;
    let mut squared_errors = CalibrationNumber::ZERO;
    for outcome in outcomes {
        let predicted = if count == CalibrationNumber::ZERO {
            initial
        } else {
            average(hits, count).ok_or(BacktestError::Arithmetic)?
        };
        let observed = outcome.observed();
        let error = squared_error(predicted, observed).ok_or(BacktestError::Arithmetic)?;
        squared_errors = sum(squared_errors, error).ok_or(BacktestError::Arithmetic)?;
        hits = sum(hits, observed).ok_or(BacktestError::Arithmetic)?;
        count = sum(count, CalibrationNumber::ONE).ok_or(BacktestError::Arithmetic)?;
    }
    average(squared_errors, count)
        .map(statistic)
        .ok_or(BacktestError::Arithmetic)
}

fn summarize(evaluations: &[Evaluation]) -> Result<Option<BacktestSummary>, BacktestError> {
    let references: Vec<&Evaluation> = evaluations.iter().collect();
    let Some(rates) = rates(&references)? else {
        return Ok(None);
    };
    let third = one_third()?;
    Ok(Some(BacktestSummary {
        mean_probability: rates.mean_probability,
        hit_rate: rates.hit_rate,
        brier: brier(evaluations, Evaluation::predicted)?,
        reference_brier: brier(evaluations, |_| third)?,
        expanding_brier: historical_brier(evaluations.iter().map(|item| item.outcome), third)?,
    }))
}

fn reliability(evaluations: &[Evaluation]) -> Result<Vec<ReliabilityBin>, BacktestError> {
    bins()
        .map(|(lower, upper, edge)| {
            let members: Vec<&Evaluation> = evaluations
                .iter()
                .filter(|evaluation| {
                    let value = evaluation.probability.as_decimal();
                    value >= lower.0
                        && match edge {
                            UpperEdge::Open => value < upper.0,
                            UpperEdge::Closed => value <= upper.0,
                        }
                })
                .collect();
            Ok(ReliabilityBin {
                lower,
                upper,
                edge,
                count: BacktestCount(members.len()),
                rates: rates(&members)?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests;
