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

    pub async fn recompute_day(
        &self,
        account: &AccountId,
        date: CalendarDate,
    ) -> Result<ValuationSnapshot, PerformanceRunError> {
        let now = self.clock.now();
        let window = SettlementWindow::try_new(date, date)
            .map_err(|_error| PerformanceRunError::Window(SnapshotError::WrongPeriod))?;
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
        let ledger = |currency| -> Result<WindowLedger, PerformanceRunError> {
            let evidence = LedgerEvidence::from_complete_response(complete.clone())
                .map_err(|reason| PerformanceRunError::Cash(currency, reason))?;
            derived_window_ledger(&rows, window, currency, evidence, now)
                .map_err(|reason| PerformanceRunError::Cash(currency, reason))
        };
        let (kr, us) = (ledger(Currency::Krw)?, ledger(Currency::Usd)?);
        recompute_window_day(&day, &rows, &histories, (&kr, &us), now)
            .map_err(|reason| PerformanceRunError::EdgeDay(date, reason))
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
        for _attempt in 0_u8..4_u8 {
            match calculate(&inputs, &days)? {
                Calculated::Done(performance, skipped) => {
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
                Calculated::Trim(edges) => {
                    days.retain(|entry| edges.iter().all(|edge| edge.0 != entry.0));
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
}

struct WindowInputs<'a> {
    rows: &'a SettledRowSet,
    complete: &'a CompleteTransactionResponse,
    histories: &'a [SnapshotInstrumentHistory],
    now: domain::Timestamp,
}

fn calculate(
    inputs: &WindowInputs<'_>,
    days: &[(CalendarDate, WindowDay)],
) -> Result<Calculated, PerformanceRunError> {
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
    Ok(Calculated::Done(performance, skipped))
}

enum Calculated {
    Done(Performance, Vec<(CalendarDate, SnapshotError)>),
    Trim(Vec<(CalendarDate, SnapshotError)>),
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
) -> Recomputed {
    let mut snapshots = Vec::new();
    let mut rates = Vec::new();
    let mut skipped = Vec::new();
    for (date, day) in days {
        match recompute_window_day(day, rows, histories, ledgers, now) {
            Ok(snapshot) => {
                snapshots.push(snapshot);
                rates.extend(day.fx);
            }
            Err(reason) => skipped.push((*date, reason)),
        }
    }
    Recomputed {
        snapshots,
        rates,
        skipped,
    }
}

#[cfg(test)]
mod tests;
