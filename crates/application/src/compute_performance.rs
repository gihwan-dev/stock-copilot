use crate::{Clock, PortError, SystemPortError, TransactionSyncRepository};
use domain::{
    AccountId, CalendarDate, CashUnavailable, CompleteTransactionResponse, Currency,
    LedgerEvidence, Performance, PerformanceCurrency, PerformanceError, PerformanceRisk,
    PerformanceSpan, RiskMetric, SettledRowSet, SettlementWindow, SnapshotError,
    SnapshotInstrumentHistory, ValuationSnapshot, WindowDay, WindowLedger, derived_window_ledger,
    performance_max_drawdown, performance_volatility, performance_window, recompute_window_day,
    window_dates,
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
    Window(SnapshotError),
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

    pub async fn execute(
        &self,
        account: &AccountId,
        span: PerformanceSpan,
    ) -> Result<PerformanceSummary, PerformanceRunError> {
        let now = self.clock.now();
        let requested = performance_window(now, span).map_err(PerformanceRunError::Window)?;
        let mut days = self.valued_days(requested).await?;
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
            rows: &rows,
            complete: &complete,
            histories: &histories,
            now,
        };
        let mut dropped = Vec::new();
        let mut last_edge = PerformanceRunError::NoValuedDays;
        for _attempt in 0..EDGE_RETRIES {
            match calculate(&inputs, &days) {
                Ok((performance, skipped)) => {
                    dropped.extend(skipped);
                    dropped.sort_by_key(|entry: &(CalendarDate, SnapshotError)| entry.0);
                    return Ok(PerformanceSummary {
                        span,
                        volatility: performance_volatility(&performance),
                        drawdown: performance_max_drawdown(&performance),
                        performance,
                        skipped: dropped,
                    });
                }
                Err(PerformanceRunError::EdgeDay(date, reason)) => {
                    days.retain(|entry| entry.0 != date);
                    dropped.push((date, reason));
                    last_edge = PerformanceRunError::EdgeDay(date, reason);
                }
                Err(other) => return Err(other),
            }
        }
        Err(last_edge)
    }
}

const EDGE_RETRIES: usize = 16;

struct WindowInputs<'a> {
    rows: &'a SettledRowSet,
    complete: &'a CompleteTransactionResponse,
    histories: &'a [SnapshotInstrumentHistory],
    now: domain::Timestamp,
}

fn calculate(
    inputs: &WindowInputs<'_>,
    days: &[(CalendarDate, WindowDay)],
) -> Result<(Performance, Vec<(CalendarDate, SnapshotError)>), PerformanceRunError> {
    let (Some((first, _)), Some((last, _))) = (days.first(), days.last()) else {
        return Err(PerformanceRunError::NoValuedDays);
    };
    let window = SettlementWindow::try_new(*first, *last)
        .map_err(|_error| PerformanceRunError::Window(SnapshotError::WrongPeriod))?;
    let ledger = |currency| -> Result<WindowLedger, PerformanceRunError> {
        let evidence = LedgerEvidence::from_complete_response(inputs.complete.clone())
            .map_err(|reason| PerformanceRunError::Cash(currency, reason))?;
        derived_window_ledger(inputs.rows, window, currency, evidence, inputs.now)
            .map_err(|reason| PerformanceRunError::Cash(currency, reason))
    };
    let kr = ledger(Currency::Krw)?;
    let us = ledger(Currency::Usd)?;
    let Recomputed {
        snapshots,
        rates,
        skipped,
    } = recompute_days(days, inputs.rows, inputs.histories, (&kr, &us), inputs.now)?;
    let performance = Performance::calculate(
        &snapshots,
        (&kr.flow_period(), &us.flow_period()),
        &rates,
        PerformanceCurrency::ConvertedKrw,
    )
    .map_err(PerformanceRunError::Performance)?;
    Ok((performance, skipped))
}

struct Recomputed {
    snapshots: Vec<ValuationSnapshot>,
    rates: Vec<domain::FxRate>,
    skipped: Vec<(CalendarDate, SnapshotError)>,
}

fn recompute_days(
    days: &[(CalendarDate, WindowDay)],
    rows: &SettledRowSet,
    histories: &[SnapshotInstrumentHistory],
    ledgers: (&WindowLedger, &WindowLedger),
    now: domain::Timestamp,
) -> Result<Recomputed, PerformanceRunError> {
    let mut snapshots = Vec::new();
    let mut rates = Vec::new();
    let mut skipped = Vec::new();
    let edges = (
        days.first().map(|entry| entry.0),
        days.last().map(|entry| entry.0),
    );
    for (date, day) in days {
        match recompute_window_day(day, rows, histories, ledgers, now) {
            Ok(snapshot) => {
                snapshots.push(snapshot);
                rates.extend(day.fx);
            }
            Err(reason) if edges.0 == Some(*date) || edges.1 == Some(*date) => {
                return Err(PerformanceRunError::EdgeDay(*date, reason));
            }
            Err(reason) => skipped.push((*date, reason)),
        }
    }
    Ok(Recomputed {
        snapshots,
        rates,
        skipped,
    })
}

#[cfg(test)]
mod tests;
