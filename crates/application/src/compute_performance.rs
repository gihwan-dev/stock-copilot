use crate::{Clock, PortError, SystemPortError, TransactionSyncRepository};
use domain::{
    AccountId, BuyingPower, CalendarDate, CashAnchor, CashUnavailable, CompleteTransactionResponse,
    Currency, DayFx, LedgerEvidence, Performance, PerformanceCurrency, PerformanceError,
    PerformanceRisk, PerformanceSpan, RiskMetric, SettledRowSet, SettlementWindow, SnapshotError,
    SnapshotInstrumentHistory, ValuationSnapshot, WindowDay, WindowLedger, anchored_window_ledger,
    derived_window_ledger, performance_max_drawdown, performance_volatility, performance_window,
    recompute_window_day, recompute_window_day_with_fallback, window_dates,
};
use std::future::Future;

pub trait SettledRowsSource: Send + Sync {
    fn settled_rows(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<SettledRowSet, SystemPortError>> + Send;
}

impl<T: TransactionSyncRepository> SettledRowsSource for T {
    fn settled_rows(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<SettledRowSet, SystemPortError>> + Send {
        self.state(account)
    }
}

pub trait PerformanceHistorySource: Send + Sync {
    fn reported_cash(
        &self,
        account: &AccountId,
        currency: Currency,
    ) -> impl Future<Output = Option<BuyingPower>> + Send;
    fn complete_transactions(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<CompleteTransactionResponse, PortError>> + Send;
    fn window_day(
        &self,
        date: CalendarDate,
    ) -> impl Future<Output = Result<Option<WindowDay>, PortError>> + Send;
    fn histories(
        &self,
        rows: &SettledRowSet,
        window: SettlementWindow,
    ) -> impl Future<Output = Result<Vec<SnapshotInstrumentHistory>, PortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceRunError {
    Repository(SystemPortError),
    Source(PortError),
    ValuationPeriod(SnapshotError),
    NoValuedDays,
    Cash(Currency, CashUnavailable),
    EdgeDay(CalendarDate, SnapshotError),
    Performance(PerformanceError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceSummary {
    pub span: PerformanceSpan,
    pub performance: Performance,
    pub volatility: PerformanceRisk,
    pub drawdown: RiskMetric,
    pub skipped: Vec<(CalendarDate, SnapshotError)>,
    pub cash: Vec<(Currency, CashAnchor)>,
    pub stale_fx: Vec<CalendarDate>,
}

#[derive(Debug)]
pub struct ComputePerformance<'a, Rows, Source, Time> {
    pub rows: &'a Rows,
    pub source: &'a Source,
    pub clock: &'a Time,
}

impl<Rows, Source, Time> ComputePerformance<'_, Rows, Source, Time>
where
    Rows: SettledRowsSource,
    Source: PerformanceHistorySource,
    Time: Clock,
{
    async fn valued_days(
        &self,
        requested: SettlementWindow,
    ) -> Result<Vec<(CalendarDate, WindowDay)>, PerformanceRunError> {
        let mut days = Vec::new();
        for date in window_dates(requested) {
            if let Some(day) = self
                .source
                .window_day(date)
                .await
                .map_err(PerformanceRunError::Source)?
            {
                days.push((date, day));
            }
        }
        Ok(days)
    }

    async fn reported(&self, account: &AccountId) -> [(Currency, Option<BuyingPower>); 2] {
        [
            (
                Currency::Krw,
                self.source.reported_cash(account, Currency::Krw).await,
            ),
            (
                Currency::Usd,
                self.source.reported_cash(account, Currency::Usd).await,
            ),
        ]
    }

    pub async fn recompute_day(
        &self,
        account: &AccountId,
        date: CalendarDate,
    ) -> Result<ValuationSnapshot, PerformanceRunError> {
        let now = self.clock.now();
        let window = SettlementWindow::try_new(date, date)
            .map_err(|_error| PerformanceRunError::ValuationPeriod(SnapshotError::WrongPeriod))?;
        let day = self
            .source
            .window_day(date)
            .await
            .map_err(PerformanceRunError::Source)?
            .ok_or(PerformanceRunError::NoValuedDays)?;
        let rows = self
            .rows
            .settled_rows(account)
            .await
            .map_err(PerformanceRunError::Repository)?;
        let complete = self
            .source
            .complete_transactions(account)
            .await
            .map_err(PerformanceRunError::Source)?;
        let histories = self
            .source
            .histories(&rows, window)
            .await
            .map_err(PerformanceRunError::Source)?;
        let inputs = WindowInputs {
            reported: self.reported(account).await,
            rows: &rows,
            complete: &complete,
            histories: &histories,
            now,
        };
        let [(_, kr_cash), (_, us_cash)] = inputs.reported;
        let (kr, _kr_anchor) = cash_ledger(&inputs, window, Currency::Krw, kr_cash)?;
        let (us, _us_anchor) = cash_ledger(&inputs, window, Currency::Usd, us_cash)?;
        recompute_window_day(&day, &rows, &histories, (&kr, &us), now)
            .map_err(|reason| PerformanceRunError::EdgeDay(date, reason))
    }

    pub async fn execute(
        &self,
        account: &AccountId,
        span: PerformanceSpan,
    ) -> Result<PerformanceSummary, PerformanceRunError> {
        self.execute_with_recovery(account, span, &[])
            .await
            .map(|(summary, _recovered)| summary)
    }

    pub async fn execute_with_recovery(
        &self,
        account: &AccountId,
        span: PerformanceSpan,
        missing: &[PerformanceSpan],
    ) -> Result<(PerformanceSummary, Vec<PerformanceSummary>), PerformanceRunError> {
        let now = self.clock.now();
        let requested =
            performance_window(now, span).map_err(PerformanceRunError::ValuationPeriod)?;
        let days = self.valued_days(requested).await?;
        if days.is_empty() {
            return Err(PerformanceRunError::NoValuedDays);
        }
        let rows = self
            .rows
            .settled_rows(account)
            .await
            .map_err(PerformanceRunError::Repository)?;
        let complete = self
            .source
            .complete_transactions(account)
            .await
            .map_err(PerformanceRunError::Source)?;
        let histories = self
            .source
            .histories(&rows, requested)
            .await
            .map_err(PerformanceRunError::Source)?;
        let inputs = WindowInputs {
            reported: self.reported(account).await,
            rows: &rows,
            complete: &complete,
            histories: &histories,
            now,
        };
        let summary = calculate_span(&inputs, span, &days)?;
        let mut recovered = Vec::new();
        for &shorter in missing {
            let Ok(window) = performance_window(now, shorter) else {
                continue;
            };
            if window.from() <= requested.from() || window.to() != requested.to() {
                continue;
            }
            let subset = days.partition_point(|(date, _day)| *date < window.from());
            if let Some(dates) = days.get(subset..)
                && let Ok(found) = calculate_span(&inputs, shorter, dates)
            {
                recovered.push(found);
            }
        }
        Ok((summary, recovered))
    }
}

fn calculate_span(
    inputs: &WindowInputs<'_>,
    span: PerformanceSpan,
    days: &[(CalendarDate, WindowDay)],
) -> Result<PerformanceSummary, PerformanceRunError> {
    let mut eligible = days.to_vec();
    let mut dropped = Vec::new();
    for _attempt in 0_u8..4_u8 {
        match calculate(inputs, &eligible)? {
            Calculated::Done(done) => return Ok(summarize(span, *done, dropped)),
            Calculated::Trim(edges) => {
                eligible.retain(|entry| edges.iter().all(|edge| edge.0 != entry.0));
                dropped.extend(edges);
            }
        }
    }
    Err(dropped
        .first()
        .map_or(PerformanceRunError::NoValuedDays, |(date, reason)| {
            PerformanceRunError::EdgeDay(*date, *reason)
        }))
}

struct WindowInputs<'a> {
    reported: [(Currency, Option<BuyingPower>); 2],
    rows: &'a SettledRowSet,
    complete: &'a CompleteTransactionResponse,
    histories: &'a [SnapshotInstrumentHistory],
    now: domain::Timestamp,
}

fn cash_ledger(
    inputs: &WindowInputs<'_>,
    window: SettlementWindow,
    currency: Currency,
    reported: Option<BuyingPower>,
) -> Result<(WindowLedger, CashAnchor), PerformanceRunError> {
    let evidence = || {
        LedgerEvidence::from_complete_response(inputs.complete.clone())
            .map_err(|reason| PerformanceRunError::Cash(currency, reason))
    };
    let anchored = reported.map(|cash| {
        anchored_window_ledger(inputs.rows, window, currency, evidence()?, cash)
            .map_err(|reason| PerformanceRunError::Cash(currency, reason))
    });
    match anchored {
        Some(Ok(found)) => Ok(found),
        Some(Err(_)) | None => {
            derived_window_ledger(inputs.rows, window, currency, evidence()?, inputs.now)
                .map(|ledger| (ledger, CashAnchor::TransactionsOnly))
                .map_err(|reason| PerformanceRunError::Cash(currency, reason))
        }
    }
}

fn calculate(
    inputs: &WindowInputs<'_>,
    days: &[(CalendarDate, WindowDay)],
) -> Result<Calculated, PerformanceRunError> {
    let (Some((first, _)), Some((last, _))) = (days.first(), days.last()) else {
        return Err(PerformanceRunError::NoValuedDays);
    };
    let window = SettlementWindow::try_new(*first, *last)
        .map_err(|_error| PerformanceRunError::ValuationPeriod(SnapshotError::WrongPeriod))?;
    let [(_, kr_cash), (_, us_cash)] = inputs.reported;
    let (kr, kr_anchor) = cash_ledger(inputs, window, Currency::Krw, kr_cash)?;
    let (us, us_anchor) = cash_ledger(inputs, window, Currency::Usd, us_cash)?;
    let Recomputed {
        snapshots,
        rates,
        skipped,
        stale_fx,
    } = recompute_days(days, inputs.rows, inputs.histories, (&kr, &us), inputs.now);
    let valued: Vec<CalendarDate> = snapshots
        .iter()
        .map(ValuationSnapshot::settlement_date)
        .collect();
    let edges: Vec<(CalendarDate, SnapshotError)> = skipped
        .iter()
        .filter(|(date, _reason)| {
            valued.first().is_none_or(|start| date < start)
                || valued.last().is_none_or(|end| date > end)
        })
        .copied()
        .collect();
    if !edges.is_empty() {
        return Ok(Calculated::Trim(edges));
    }
    let performance = Performance::calculate(
        &snapshots,
        (&kr.flow_period(), &us.flow_period()),
        &rates,
        PerformanceCurrency::ConvertedKrw,
    )
    .map_err(PerformanceRunError::Performance)?;
    Ok(Calculated::Done(Box::new(Done {
        performance,
        skipped,
        cash: vec![(Currency::Krw, kr_anchor), (Currency::Usd, us_anchor)],
        stale_fx,
    })))
}

fn summarize(
    span: PerformanceSpan,
    done: Done,
    mut dropped: Vec<(CalendarDate, SnapshotError)>,
) -> PerformanceSummary {
    dropped.extend(done.skipped);
    dropped.sort_by_key(|entry: &(CalendarDate, SnapshotError)| entry.0);
    PerformanceSummary {
        span,
        volatility: performance_volatility(&done.performance),
        drawdown: performance_max_drawdown(&done.performance),
        performance: done.performance,
        skipped: dropped,
        cash: done.cash,
        stale_fx: done.stale_fx,
    }
}

struct Done {
    performance: Performance,
    skipped: Vec<(CalendarDate, SnapshotError)>,
    cash: Vec<(Currency, CashAnchor)>,
    stale_fx: Vec<CalendarDate>,
}

enum Calculated {
    Done(Box<Done>),
    Trim(Vec<(CalendarDate, SnapshotError)>),
}

struct Recomputed {
    snapshots: Vec<ValuationSnapshot>,
    rates: Vec<domain::FxRate>,
    skipped: Vec<(CalendarDate, SnapshotError)>,
    stale_fx: Vec<CalendarDate>,
}

fn recompute_days(
    days: &[(CalendarDate, WindowDay)],
    rows: &SettledRowSet,
    histories: &[SnapshotInstrumentHistory],
    ledgers: (&WindowLedger, &WindowLedger),
    now: domain::Timestamp,
) -> Recomputed {
    let mut snapshots = Vec::new();
    let mut rates: Vec<domain::FxRate> = Vec::new();
    let mut skipped = Vec::new();
    let mut stale_fx = Vec::new();
    for (date, day) in days {
        let fallback = rates.last().copied();
        match recompute_window_day_with_fallback(day, rows, histories, ledgers, (now, fallback)) {
            Ok((snapshot, used)) => {
                snapshots.push(snapshot);
                match used {
                    DayFx::OfTheDay => rates.extend(day.fx),
                    DayFx::StaleWithSmallDollarShare => stale_fx.push(*date),
                }
            }
            Err(reason) => skipped.push((*date, reason)),
        }
    }
    Recomputed {
        snapshots,
        rates,
        skipped,
        stale_fx,
    }
}

#[cfg(test)]
mod tests;
