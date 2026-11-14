use crate::performance_arithmetic::{Days, Number, annualize, dietz, linked, mwr};
use crate::{
    AccountChange, CalendarDate, ConfirmedFlowPeriod, Currency, FlowDirection, FlowPeriod, FxRate,
    PerformanceValue, SnapshotError, TotalCompleteness, ValuationSnapshot,
};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceError {
    InvalidPeriod,
    UncertainFlows,
    IncompleteSnapshot,
    CashBasisMismatch,
    OutsideConfirmedPeriod,
    MissingFx,
    NonPositiveCapital,
    Arithmetic,
    AmbiguousOrMissingRoot,
    RootOutsideRange,
    NonConvergence,
    NotAnnualized,
    InsufficientPrecision,
    TooManyInputs,
    BenchmarkMismatch,
    InvalidBenchmark,
    Snapshot(SnapshotError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceCurrency {
    Native(Currency),
    ConvertedKrw,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformancePeriod {
    Cumulative,
    Annualized,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwrMethod {
    LinkedValuations,
    LinkedModifiedDietz,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DailySample {
    Trading,
    Unverified,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Performance {
    start: CalendarDate,
    end: CalendarDate,
    currency: PerformanceCurrency,
    period: PerformancePeriod,
    method: TwrMethod,
    twr: PerformanceValue,
    mwr: Result<PerformanceValue, PerformanceError>,
    annual_twr: Result<PerformanceValue, PerformanceError>,
    snapshots: PerformanceSnapshots,
    pub(crate) duration: Days,
    pub(crate) offsets: Vec<Days>,
    pub(crate) returns: Vec<PerformanceValue>,
    pub(crate) dates: Vec<CalendarDate>,
    pub(crate) daily_samples: Vec<DailySample>,
}
#[derive(Clone, PartialEq, Eq)]
struct PerformanceSnapshots(Vec<ValuationSnapshot>);
impl std::fmt::Debug for PerformanceSnapshots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PerformanceSnapshots([REDACTED])")
    }
}
struct Context<'a> {
    periods: [&'a ConfirmedFlowPeriod; 2],
    rates: &'a [FxRate],
    currency: PerformanceCurrency,
}
impl Performance {
    pub fn calculate(
        snapshots: &[ValuationSnapshot],
        periods: (&FlowPeriod, &FlowPeriod),
        rates: &[FxRate],
        currency: PerformanceCurrency,
    ) -> Result<Self, PerformanceError> {
        input_bounds(snapshots, rates)?;
        let OrderedSnapshots {
            snapshots: ordered,
            first,
            last,
            duration,
        } = ordered_snapshots(snapshots)?;
        let tokens = [confirmed(periods.0)?, confirmed(periods.1)?];
        validate((first, last), tokens)?;
        let values = verified_values(&ordered, (first, last), tokens, currency)?;
        let context = Context::new(tokens, rates, currency);
        let mut returns = Vec::new();
        let mut cash = vec![(Days::new(0), values.first.neg())];
        let mut method = TwrMethod::LinkedValuations;
        for ((old, today), (previous, current)) in ordered
            .iter()
            .zip(ordered.iter().skip(1))
            .zip(values.entries.iter().zip(values.entries.iter().skip(1)))
        {
            let (flows, kind) = context.flows(old, today)?;
            if kind == TwrMethod::LinkedModifiedDietz {
                method = kind;
            }
            for (offset, amount) in &flows {
                cash.push((
                    Days::new(previous.offset.as_u32().wrapping_add(*offset.as_u32())),
                    amount.neg(),
                ));
            }
            returns.push(dietz(
                previous.amount,
                profit(old, today, periods, rates, currency)?,
                &flows,
                Days::new(
                    current
                        .offset
                        .as_u32()
                        .wrapping_sub(*previous.offset.as_u32()),
                ),
            )?);
        }
        cash.push((duration, values.last));
        let (period, output_days) = output_period(duration);
        let cumulative = linked(&returns)?;
        Ok(Self {
            start: first.settlement_date(),
            end: last.settlement_date(),
            currency,
            period,
            method,
            twr: cumulative,
            mwr: mwr(&cash, duration, output_days),
            annual_twr: annual_result(cumulative, duration),
            snapshots: retained_snapshots(&ordered),
            duration,
            offsets: values.offsets(),
            daily_samples: snapshot_daily_samples(&ordered, currency),
            returns,
            dates: snapshot_dates(&ordered),
        })
    }
    #[must_use]
    pub const fn start(&self) -> CalendarDate {
        self.start
    }
    #[must_use]
    pub const fn end(&self) -> CalendarDate {
        self.end
    }
    #[must_use]
    pub const fn currency(&self) -> PerformanceCurrency {
        self.currency
    }
    #[must_use]
    pub const fn period(&self) -> PerformancePeriod {
        self.period
    }
    #[must_use]
    pub const fn method(&self) -> TwrMethod {
        self.method
    }
    #[must_use]
    pub const fn twr(&self) -> PerformanceValue {
        self.twr
    }
    pub const fn annual_twr(&self) -> Result<PerformanceValue, PerformanceError> {
        self.annual_twr
    }
    pub(crate) fn snapshots(&self) -> &[ValuationSnapshot] {
        &self.snapshots.0
    }
    pub const fn mwr(&self) -> Result<PerformanceValue, PerformanceError> {
        self.mwr
    }
}
fn annual_result(
    value: PerformanceValue,
    days: Days,
) -> Result<PerformanceValue, PerformanceError> {
    if *days.as_u32() < 365 {
        return Err(PerformanceError::NotAnnualized);
    }
    annualize(value, days)
}
fn retained_snapshots(ordered: &[&ValuationSnapshot]) -> PerformanceSnapshots {
    PerformanceSnapshots(ordered.iter().map(|snapshot| (*snapshot).clone()).collect())
}
fn snapshot_dates(ordered: &[&ValuationSnapshot]) -> Vec<CalendarDate> {
    ordered
        .iter()
        .map(|snapshot| snapshot.settlement_date())
        .collect()
}
fn snapshot_daily_samples(
    ordered: &[&ValuationSnapshot],
    currency: PerformanceCurrency,
) -> Vec<DailySample> {
    ordered
        .iter()
        .zip(ordered.iter().skip(1))
        .map(|(old, today)| daily_sample(old, today, currency))
        .collect()
}
pub(crate) fn daily_sample(
    old: &ValuationSnapshot,
    today: &ValuationSnapshot,
    currency: PerformanceCurrency,
) -> DailySample {
    let units = match currency {
        PerformanceCurrency::Native(unit) => vec![unit],
        PerformanceCurrency::ConvertedKrw => vec![Currency::Krw, Currency::Usd],
    };
    if units
        .into_iter()
        .any(|unit| today.price_date(unit) < old.price_date(unit))
    {
        return DailySample::Unverified;
    }
    DailySample::Trading
}
fn input_bounds(snapshots: &[ValuationSnapshot], rates: &[FxRate]) -> Result<(), PerformanceError> {
    if snapshots.len() > 4097 || rates.len() > 4096 {
        return Err(PerformanceError::TooManyInputs);
    }
    Ok(())
}
fn confirmed(period: &FlowPeriod) -> Result<&ConfirmedFlowPeriod, PerformanceError> {
    match period {
        FlowPeriod::Confirmed(token) => Ok(token),
        FlowPeriod::Uncertain(_) => Err(PerformanceError::UncertainFlows),
    }
}
fn validate(
    endpoints: (&ValuationSnapshot, &ValuationSnapshot),
    tokens: [&ConfirmedFlowPeriod; 2],
) -> Result<(), PerformanceError> {
    let (first, last) = endpoints;
    for (unit, token) in [Currency::Krw, Currency::Usd].into_iter().zip(tokens) {
        if token.currency() != unit
            || token.window().from() != first.settlement_date()
            || token.window().to() != last.settlement_date()
        {
            return Err(PerformanceError::OutsideConfirmedPeriod);
        }
    }
    Ok(())
}
struct VerifiedValue {
    amount: Number,
    offset: Days,
}
struct VerifiedValues {
    entries: Vec<VerifiedValue>,
    first: Number,
    last: Number,
}
impl VerifiedValues {
    fn offsets(&self) -> Vec<Days> {
        self.entries.iter().map(|entry| entry.offset).collect()
    }
}
fn verified_value(
    snapshot: &ValuationSnapshot,
    anchor: CalendarDate,
    tokens: [&ConfirmedFlowPeriod; 2],
    currency: PerformanceCurrency,
) -> Result<VerifiedValue, PerformanceError> {
    let amount = value(snapshot, currency).map_err(|cause| {
        if cause == SnapshotError::WrongCashBasis {
            PerformanceError::CashBasisMismatch
        } else {
            PerformanceError::Snapshot(cause)
        }
    })?;
    for (unit, token) in [Currency::Krw, Currency::Usd].into_iter().zip(tokens) {
        validate_snapshot(snapshot, unit, token)?;
    }
    days(anchor, snapshot.settlement_date()).map(|offset| VerifiedValue { amount, offset })
}
fn verified_values(
    ordered: &[&ValuationSnapshot],
    endpoints: (&ValuationSnapshot, &ValuationSnapshot),
    tokens: [&ConfirmedFlowPeriod; 2],
    currency: PerformanceCurrency,
) -> Result<VerifiedValues, PerformanceError> {
    let (first, last) = endpoints;
    let first_amount = verified_value(first, first.settlement_date(), tokens, currency)?.amount;
    let last_amount = verified_value(last, first.settlement_date(), tokens, currency)?.amount;
    let entries = ordered
        .iter()
        .map(|snapshot| verified_value(snapshot, first.settlement_date(), tokens, currency))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(VerifiedValues {
        entries,
        first: first_amount,
        last: last_amount,
    })
}
fn validate_snapshot(
    snapshot: &ValuationSnapshot,
    unit: Currency,
    token: &ConfirmedFlowPeriod,
) -> Result<(), PerformanceError> {
    if snapshot.completeness() != TotalCompleteness::Complete {
        return Err(PerformanceError::IncompleteSnapshot);
    }
    if snapshot.confirmed_period(unit) != Some(token) {
        return Err(PerformanceError::OutsideConfirmedPeriod);
    }
    Ok(())
}
impl<'a> Context<'a> {
    fn new(
        periods: [&'a ConfirmedFlowPeriod; 2],
        rates: &'a [FxRate],
        currency: PerformanceCurrency,
    ) -> Self {
        Self {
            periods,
            rates,
            currency,
        }
    }

    fn flows(
        &self,
        old: &ValuationSnapshot,
        today: &ValuationSnapshot,
    ) -> Result<(Vec<(Days, Number)>, TwrMethod), PerformanceError> {
        let mut flows = Vec::new();
        let mut kind = TwrMethod::LinkedValuations;
        let mut status = Ok(());
        for (flow, time) in self
            .periods
            .iter()
            .flat_map(|token| token.flows())
            .filter_map(|flow| flow.transaction_time().map(|time| (flow, time)))
        {
            if time <= old.valuation_at() || time > today.valuation_at() {
                continue;
            }
            let amount = flow_value(flow.amount(), self.currency, time, self.rates)?;
            if amount.as_decimal().is_zero() {
                continue;
            }
            let signed = match flow.direction() {
                FlowDirection::Inflow => amount,
                FlowDirection::Outflow => amount.neg(),
            };
            status = status.and_then(|()| {
                append_weighted_flow((old, today), time, signed, &mut flows, &mut kind)
            });
        }
        self.exchanges(old, today, &mut flows, &mut kind)
            .and(status)
            .map(|()| (flows, kind))
    }
    fn exchanges(
        &self,
        old: &ValuationSnapshot,
        today: &ValuationSnapshot,
        flows: &mut Vec<(Days, Number)>,
        kind: &mut TwrMethod,
    ) -> Result<(), PerformanceError> {
        let PerformanceCurrency::Native(unit) = self.currency else {
            return Ok(());
        };
        let mut status = Ok(());
        for (row, time, settled) in today
            .exchanges()
            .iter()
            .filter(|row| row.data().currency == unit)
            .filter_map(|row| {
                row.data()
                    .transaction_time
                    .zip(row.data().net_amount)
                    .map(|(time, amount)| (row, time, amount))
            })
        {
            if time <= old.valuation_at() || time > today.valuation_at() {
                continue;
            }
            let native = Number::new(settled.amount().as_decimal());
            let amount = if row.data().kind == crate::TransactionKind::ExchangeOut {
                native.neg()
            } else {
                native
            };
            status =
                status.and_then(|()| append_weighted_flow((old, today), time, amount, flows, kind));
        }
        status
    }
}
fn append_weighted_flow(
    endpoints: (&ValuationSnapshot, &ValuationSnapshot),
    time: crate::Timestamp,
    amount: Number,
    flows: &mut Vec<(Days, Number)>,
    kind: &mut TwrMethod,
) -> Result<(), PerformanceError> {
    let (old, today) = endpoints;
    flow_elapsed(old, time)
        .ok()
        .zip(flow_elapsed(old, today.valuation_at()).ok())
        .map(|(offset, span)| {
            if offset != span {
                *kind = TwrMethod::LinkedModifiedDietz;
            }
            flows.push((offset, amount));
        })
        .ok_or(PerformanceError::InvalidPeriod)
}
fn flow_elapsed(old: &ValuationSnapshot, time: crate::Timestamp) -> Result<Days, PerformanceError> {
    timestamp_days(old.valuation_at(), time)
}
pub(crate) fn timestamp_days(
    start: crate::Timestamp,
    end: crate::Timestamp,
) -> Result<Days, PerformanceError> {
    if end < start {
        return Err(PerformanceError::InvalidPeriod);
    }
    let (first, last) = start
        .date_at_offset(crate::OffsetSeconds::new(32400))
        .ok()
        .zip(end.date_at_offset(crate::OffsetSeconds::new(32400)).ok())
        .ok_or(PerformanceError::InvalidPeriod)?;
    days(first, last)
}

fn profit(
    old: &ValuationSnapshot,
    today: &ValuationSnapshot,
    periods: (&FlowPeriod, &FlowPeriod),
    rates: &[FxRate],
    currency: PerformanceCurrency,
) -> Result<Number, PerformanceError> {
    let change = AccountChange::between(today, Some(old), periods, rates);
    match currency {
        PerformanceCurrency::Native(unit) => change
            .currency(unit)
            .map(|entry| Number::new(entry.amount().amount().as_decimal())),
        PerformanceCurrency::ConvertedKrw => change.krw().map(|entry| {
            Number::new(
                entry
                    .valuation()
                    .clone()
                    .into_parts()
                    .0
                    .amount()
                    .as_decimal(),
            )
        }),
    }
    .map_err(PerformanceError::Snapshot)
}
pub(crate) fn days(start: CalendarDate, end: CalendarDate) -> Result<Days, PerformanceError> {
    start
        .days_until(end)
        .ok()
        .and_then(|span| u32::try_from(*span.as_i64()).ok())
        .map(Days::new)
        .ok_or(PerformanceError::InvalidPeriod)
}
pub(crate) fn value(
    snapshot: &ValuationSnapshot,
    currency: PerformanceCurrency,
) -> Result<Number, SnapshotError> {
    match currency {
        PerformanceCurrency::Native(unit) => snapshot
            .account_value(unit)
            .map(|money| Number::new(money.amount().as_decimal())),
        PerformanceCurrency::ConvertedKrw => snapshot
            .account_krw_value()
            .map(|entry| Number::new(entry.clone().into_parts().0.amount().as_decimal())),
    }
}
fn flow_value(
    money: crate::Money,
    currency: PerformanceCurrency,
    time: crate::Timestamp,
    rates: &[FxRate],
) -> Result<Number, PerformanceError> {
    match currency {
        PerformanceCurrency::Native(unit) => Ok(Number::new(if money.currency() == unit {
            money.amount().as_decimal()
        } else {
            Decimal::ZERO
        })),
        PerformanceCurrency::ConvertedKrw => converted_flow(money, time, rates),
    }
}
fn converted_flow(
    money: crate::Money,
    time: crate::Timestamp,
    rates: &[FxRate],
) -> Result<Number, PerformanceError> {
    if money.currency() == Currency::Krw || money.amount().as_decimal().is_zero() {
        return Ok(Number::new(money.amount().as_decimal()));
    }
    let candidates = rates
        .iter()
        .filter(|rate| {
            rate.kind() == crate::FxRateKind::Mid
                && rate.validity_at(time) == crate::FxValidity::Valid
        })
        .collect::<Vec<_>>();
    let [rate] = candidates.as_slice() else {
        return Err(PerformanceError::MissingFx);
    };
    rate.convert_for_aggregation(money)
        .map(|converted| Number::new(converted.into_parts().0.amount().as_decimal()))
        .map_err(|_cause| PerformanceError::MissingFx)
}

struct OrderedSnapshots<'a> {
    snapshots: Vec<&'a ValuationSnapshot>,
    first: &'a ValuationSnapshot,
    last: &'a ValuationSnapshot,
    duration: Days,
}
fn ordered_snapshots(
    snapshots: &[ValuationSnapshot],
) -> Result<OrderedSnapshots<'_>, PerformanceError> {
    let mut ordered = snapshots.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|snapshot| snapshot.valuation_at());
    let first = *ordered.first().ok_or(PerformanceError::InvalidPeriod)?;
    let last = ordered.last().copied().unwrap_or(first);
    timestamp_days(first.valuation_at(), last.valuation_at())
        .ok()
        .filter(|duration| *duration.as_u32() != 0 && *duration.as_u32() <= 4096)
        .map(|duration| OrderedSnapshots {
            snapshots: ordered,
            first,
            last,
            duration,
        })
        .ok_or(PerformanceError::InvalidPeriod)
}

fn output_period(duration: Days) -> (PerformancePeriod, Days) {
    if *duration.as_u32() < 365 {
        (PerformancePeriod::Cumulative, duration)
    } else {
        (PerformancePeriod::Annualized, Days::new(365))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{Number, Performance, PerformanceCurrency, PerformancePeriod, TwrMethod};
    use crate::{Currency, PerformanceValue};
    use rust_decimal::Decimal;
    pub(crate) fn series(count: usize) -> Performance {
        let dates = (0..=count)
            .map(|index| {
                crate::valuation_snapshot::tests::date(
                    i64::try_from(index).expect("크기").wrapping_add(5),
                )
            })
            .collect::<Vec<_>>();
        let duration = super::Days::new(u32::try_from(count).expect("표본"));
        let returns = (0..count)
            .map(|index| {
                PerformanceValue::from_number(Number::new(if index.wrapping_rem(2) == 0 {
                    Decimal::new(1, 2)
                } else {
                    Decimal::new(-1, 2)
                }))
            })
            .collect::<Vec<_>>();
        let twr = super::linked(&returns).expect("연결");
        Performance {
            daily_samples: vec![super::DailySample::Trading; count],
            duration,
            offsets: (0..=count)
                .map(|index| super::Days::new(u32::try_from(index).expect("표본")))
                .collect(),
            start: *dates.first().expect("처음"),
            end: *dates.last().expect("끝"),
            currency: PerformanceCurrency::Native(Currency::Krw),
            period: super::output_period(duration).0,
            method: TwrMethod::LinkedValuations,
            twr,
            mwr: Ok(twr),
            annual_twr: super::annual_result(twr, duration),
            snapshots: super::PerformanceSnapshots(vec![]),
            returns,
            dates,
        }
    }
    pub(crate) fn adverse_series() -> Performance {
        let mut portfolio = series(364);
        for point in &mut portfolio.returns {
            *point = PerformanceValue::from_number(Number::new(Decimal::ZERO));
        }
        let one = Number::new(Decimal::ONE);
        *portfolio.returns.get_mut(0).expect("첫 날") =
            PerformanceValue::from_number(Number::new(Decimal::new(1, 28)).sub(one).expect("손실"));
        *portfolio.returns.get_mut(7).expect("큰 수익") = PerformanceValue::from_number(
            Number::new(Decimal::from(10_u128.pow(28)))
                .sub(one)
                .expect("수익"),
        );
        *portfolio.returns.get_mut(8).expect("다음 날") =
            PerformanceValue::from_number(Number::new(Decimal::from(99_u32)));
        *portfolio.returns.get_mut(9).expect("조정") =
            PerformanceValue::from_number(Number::new(Decimal::new(-98, 2)));
        portfolio.twr = super::linked(&portfolio.returns).expect("전체 기간 연결은 표현 가능");
        portfolio.mwr = super::mwr(
            &[
                (super::Days::new(0), one.neg()),
                (
                    portfolio.duration,
                    portfolio.twr.number().add(one).expect("성장"),
                ),
            ],
            portfolio.duration,
            portfolio.duration,
        );
        portfolio
    }
    #[test]
    fn risk_minimum_samples_and_week_boundaries_are_closed() {
        let short = series(59);
        let enough = series(60);
        assert_eq!(
            crate::performance_volatility(&short).daily(),
            crate::RiskMetric::InsufficientSamples
        );
        assert_eq!(
            crate::performance_volatility(&short).annual(),
            crate::RiskMetric::InsufficientSamples
        );
        let extract = |metric| match metric {
            crate::RiskMetric::Value(value) => Some(value),
            crate::RiskMetric::InsufficientSamples | crate::RiskMetric::Unavailable(_) => None,
        };
        assert_eq!(extract(crate::RiskMetric::InsufficientSamples), None);
        assert_eq!(
            extract(crate::RiskMetric::Unavailable(
                super::PerformanceError::Arithmetic
            )),
            None
        );
        assert_eq!(
            extract(crate::performance_volatility(&enough).daily())
                .expect("60개 값")
                .to_string(),
            "0.010084"
        );
        assert_eq!(
            extract(crate::performance_volatility(&enough).annual())
                .expect("60개 연율")
                .to_string(),
            "0.160085"
        );
    }
    #[test]
    fn defensive_gates_return_closed_errors() {
        use crate::valuation_snapshot::tests::{
            at, calendar, date, fixture_window, fx, ledgers, observed, snapshot,
        };
        use crate::{PerformanceError, SettlementWindow, SnapshotObservation, ValuationSnapshot};
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let old = snapshot(&data, 2, &[], "1000");
        let today = snapshot(&data, 5, &[], "1000");
        let native = PerformanceCurrency::Native(Currency::Krw);
        assert_eq!(
            Performance::calculate(&vec![old.clone(); 4098], (&data.kp, &data.up), &[], native),
            Err(PerformanceError::TooManyInputs)
        );
        assert_eq!(
            Performance::calculate(
                &[old.clone(), today.clone()],
                (&data.kp, &data.up),
                &vec![fx(at(3, 1), "1000"); 4097],
                native
            ),
            Err(PerformanceError::TooManyInputs)
        );
        let incomplete = ValuationSnapshot::recalculate(
            &calendar(3),
            &data.rows,
            &[],
            Some(ledgers(&data)),
            SnapshotObservation::new(at(3, 7), at(20, 0), None),
        )
        .expect("불완전");
        assert_eq!(
            Performance::calculate(&[old, today, incomplete], (&data.kp, &data.up), &[], native),
            Err(PerformanceError::IncompleteSnapshot)
        );
        assert_eq!(
            super::days(date(5), date(2)),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            super::output_period(super::Days::new(365)).0,
            PerformancePeriod::Annualized
        );
        assert_eq!(
            super::value(&observed(2, "100", "100"), native),
            Err(crate::SnapshotError::WrongCashBasis)
        );
    }
    #[test]
    fn missing_or_ambiguous_flow_fx_is_rejected() {
        use crate::PerformanceError;
        use crate::valuation_snapshot::tests::{at, fx};
        let usd = crate::valuation_snapshot::tests::money("1", Currency::Usd);
        assert_eq!(
            super::converted_flow(usd, at(3, 1), &[]),
            Err(PerformanceError::MissingFx)
        );
        let rate = fx(at(3, 1), "1000");
        assert_eq!(
            super::converted_flow(usd, at(3, 1), &[rate, rate]),
            Err(PerformanceError::MissingFx)
        );
    }
    #[test]
    fn risk_arithmetic_and_sampling_errors_remain_unavailable() {
        let mut enough = series(60);
        for value in &mut enough.returns {
            *value = PerformanceValue::from_number(Number::new(Decimal::ZERO));
        }
        *enough.returns.first_mut().expect("첫 수익률") =
            PerformanceValue::from_number(Number::new(Decimal::from(200_000_000_000_000_u64)));
        assert_eq!(
            crate::performance_volatility(&enough).annual(),
            crate::RiskMetric::Unavailable(super::PerformanceError::Arithmetic)
        );
        let old_dates = enough.dates.clone();
        for (target, origin) in enough.dates.iter_mut().zip(old_dates) {
            let elapsed = *enough.start.days_until(origin).expect("기간").as_i64();
            *target = crate::valuation_snapshot::tests::date(
                5_i64
                    .checked_add(elapsed.checked_mul(7).expect("주간"))
                    .expect("날짜"),
            );
        }
        enough.end = *enough.dates.last().expect("끝");
        enough.offsets = (0..=60_u32)
            .map(|index| super::Days::new(index.wrapping_mul(7)))
            .collect();
        assert_eq!(
            crate::performance_volatility(&enough).daily(),
            crate::RiskMetric::Unavailable(super::PerformanceError::InvalidPeriod)
        );
    }
    #[test]
    fn daily_sampling_accepts_verified_weekends_and_rejects_other_gaps() {
        let mut portfolio = series(60);
        portfolio.offsets = (0..=60_u32)
            .map(|index| {
                super::Days::new(index.wrapping_add(index.wrapping_div(5).wrapping_mul(2)))
            })
            .collect();
        portfolio.dates = portfolio
            .offsets
            .iter()
            .map(|offset| {
                crate::valuation_snapshot::tests::date(
                    5_i64.wrapping_add(i64::from(*offset.as_u32())),
                )
            })
            .collect();
        portfolio.end = *portfolio.dates.last().expect("끝");
        let extract = |metric| match metric {
            crate::RiskMetric::Value(value) => Some(value),
            crate::RiskMetric::InsufficientSamples | crate::RiskMetric::Unavailable(_) => None,
        };
        assert_eq!(
            extract(crate::performance_volatility(&portfolio).daily())
                .expect("주말 제외")
                .to_string(),
            "0.010084"
        );
        assert_eq!(extract(crate::RiskMetric::InsufficientSamples), None);
        portfolio.offsets = (0..=60_u32)
            .map(|index| super::Days::new(index.wrapping_mul(3)))
            .collect();
        portfolio.dates = portfolio
            .offsets
            .iter()
            .map(|offset| {
                crate::valuation_snapshot::tests::date(
                    5_i64.wrapping_add(i64::from(*offset.as_u32())),
                )
            })
            .collect();
        portfolio.end = *portfolio.dates.last().expect("끝");
        assert_eq!(
            crate::performance_volatility(&portfolio).daily(),
            crate::RiskMetric::Unavailable(super::PerformanceError::InvalidPeriod)
        );
    }
}
