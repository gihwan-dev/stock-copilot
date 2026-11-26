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
        let days = self.valued_days(requested).await?;
        let (Some((first, _)), Some((last, _))) = (days.first(), days.last()) else {
            return Err(PerformanceRunError::NoValuedDays);
        };
        let window = SettlementWindow::try_new(*first, *last)
            .map_err(|_error| PerformanceRunError::Window(SnapshotError::WrongPeriod))?;
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
        let ledger = |currency| -> Result<WindowLedger, PerformanceRunError> {
            let evidence = LedgerEvidence::from_complete_response(complete.clone())
                .map_err(|reason| PerformanceRunError::Cash(currency, reason))?;
            derived_window_ledger(&rows, window, currency, evidence, now)
                .map_err(|reason| PerformanceRunError::Cash(currency, reason))
        };
        let kr = ledger(Currency::Krw)?;
        let us = ledger(Currency::Usd)?;
        let histories = self
            .source
            .histories(&rows, window)
            .await
            .map_err(PerformanceRunError::Source)?;
        let Recomputed {
            snapshots,
            rates,
            skipped,
        } = recompute_days(&days, &rows, &histories, (&kr, &us), now)?;
        let performance = Performance::calculate(
            &snapshots,
            (&kr.flow_period(), &us.flow_period()),
            &rates,
            PerformanceCurrency::ConvertedKrw,
        )
        .map_err(PerformanceRunError::Performance)?;
        Ok(PerformanceSummary {
            span,
            volatility: performance_volatility(&performance),
            drawdown: performance_max_drawdown(&performance),
            performance,
            skipped,
        })
    }
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
