use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use application::{
    Clock, ComputePerformance, PerformanceHistorySource, PerformanceRunError, PerformanceSummary,
    PortError, SettledRowsSource, TransactionRead,
};
use domain::{
    AccountId, AssetClass, CalendarDate, Candle, CompleteTransactionResponse, Currency,
    FullInspectionPlan, FxRate, InspectionRangeDays, InstrumentId, KnownMarket, LotIdentity,
    MarketCalendar, PerformanceSpan, QuoteInstrument, SavedQuoteInstrument, SettledRowSet,
    SettlementWindow, SnapshotCalendar, SnapshotInstrumentHistory, Symbol, TimeInterval, Timestamp,
    WindowDay, valuation_time,
};
use tokio::sync::Mutex;

use crate::ipc::state::BoxFuture;

pub const SPANS: [PerformanceSpan; 4] = [
    PerformanceSpan::OneMonth,
    PerformanceSpan::ThreeMonths,
    PerformanceSpan::SixMonths,
    PerformanceSpan::OneYear,
];

pub trait PerformanceFeed: Send + Sync {
    fn transactions<'a>(
        &'a self,
        account: &'a AccountId,
        window: SettlementWindow,
    ) -> BoxFuture<'a, Result<TransactionRead, PortError>>;
    fn calendar_on(
        &self,
        market: KnownMarket,
        date: CalendarDate,
    ) -> BoxFuture<'_, Result<MarketCalendar, PortError>>;
    fn fx_at(&self, at: Timestamp) -> BoxFuture<'_, Result<FxRate, PortError>>;
    fn daily_candles<'a>(
        &'a self,
        instrument: &'a QuoteInstrument,
        window: TimeInterval,
    ) -> BoxFuture<'a, Result<Vec<Candle>, PortError>>;
}

type Stock = (KnownMarket, Symbol);

#[derive(Default)]
struct Caches {
    calendars: BTreeMap<(KnownMarket, CalendarDate), MarketCalendar>,
    rates: BTreeMap<CalendarDate, Option<FxRate>>,
    candles: BTreeMap<(Stock, CalendarDate), Vec<Candle>>,
}

pub struct FeedHistory {
    feed: Arc<dyn PerformanceFeed>,
    clock: Arc<dyn Clock>,
    caches: Mutex<Caches>,
}

impl std::fmt::Debug for FeedHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FeedHistory")
    }
}

const fn currency_of(market: KnownMarket) -> Currency {
    match market {
        KnownMarket::Kr => Currency::Krw,
        KnownMarket::Us => Currency::Usd,
    }
}

fn instrument_id(stock: &Stock) -> Option<InstrumentId> {
    let prefix = match stock.0 {
        KnownMarket::Kr => "kr",
        KnownMarket::Us => "us",
    };
    InstrumentId::try_new(&format!("history:{prefix}:{}", stock.1.as_str())).ok()
}

fn quote(stock: &Stock) -> Option<QuoteInstrument> {
    QuoteInstrument::restore_from_saved_state(SavedQuoteInstrument {
        market: stock.0,
        currency: currency_of(stock.0),
        symbol: stock.1.clone(),
        instrument_id: instrument_id(stock)?,
    })
    .ok()
}

#[must_use]
pub fn traded_stocks(rows: &SettledRowSet) -> Vec<Stock> {
    let mut found: Vec<Stock> = Vec::new();
    for row in rows.history_rows() {
        let data = row.transaction().data();
        let (Some(symbol), Some(&AssetClass::Stock)) =
            (data.symbol.as_ref(), data.asset_class.as_ref())
        else {
            continue;
        };
        let Ok(market) = KnownMarket::try_from_market(&data.market) else {
            continue;
        };
        if !found
            .iter()
            .any(|known| known.0 == market && &known.1 == symbol)
        {
            found.push((market, symbol.clone()));
        }
    }
    found
}

const LOOKBACK_SECONDS: u64 = 14 * 86_400;

fn candle_window(window: SettlementWindow, now: Timestamp) -> Result<TimeInterval, PortError> {
    let lookback = domain::TimeSpan::try_new(
        domain::SpanSeconds::new(LOOKBACK_SECONDS),
        domain::Nanosecond::new(0),
    )
    .map_err(|_error| unavailable())?;
    let start = valuation_time(window.from())
        .checked_sub(lookback)
        .map_err(|_error| unavailable())?;
    TimeInterval::try_new(start, now).map_err(|_error| unavailable())
}

const fn unavailable() -> PortError {
    PortError::new(
        application::ReadEndpoint::Candles,
        application::HttpStatus::NotReceived,
        application::PortErrorKind::Unavailable {
            cause: application::UnavailableCause::ServerError,
            retry: application::RetryDelay::Unknown,
        },
    )
}

impl FeedHistory {
    #[must_use]
    pub fn new(feed: Arc<dyn PerformanceFeed>, clock: Arc<dyn Clock>) -> Self {
        Self {
            feed,
            clock,
            caches: Mutex::new(Caches::default()),
        }
    }

    async fn calendar(
        &self,
        market: KnownMarket,
        date: CalendarDate,
    ) -> Result<MarketCalendar, PortError> {
        if let Some(found) = self.caches.lock().await.calendars.get(&(market, date)) {
            return Ok(found.clone());
        }
        let fetched = self.feed.calendar_on(market, date).await?;
        let _previous = self
            .caches
            .lock()
            .await
            .calendars
            .insert((market, date), fetched.clone());
        Ok(fetched)
    }

    async fn rate(&self, date: CalendarDate) -> Option<FxRate> {
        if let Some(found) = self.caches.lock().await.rates.get(&date) {
            return *found;
        }
        let fetched = self.feed.fx_at(valuation_time(date)).await.ok();
        let _previous = self.caches.lock().await.rates.insert(date, fetched);
        fetched
    }

    async fn candles_of(
        &self,
        stock: &Stock,
        window: SettlementWindow,
    ) -> Result<Vec<Candle>, PortError> {
        let key = (stock.clone(), window.from());
        if let Some(found) = self.caches.lock().await.candles.get(&key) {
            return Ok(found.clone());
        }
        let instrument = quote(stock).ok_or_else(unavailable)?;
        let fetched = self
            .feed
            .daily_candles(&instrument, candle_window(window, self.clock.now())?)
            .await?;
        let _previous = self
            .caches
            .lock()
            .await
            .candles
            .insert(key, fetched.clone());
        Ok(fetched)
    }
}

impl PerformanceHistorySource for FeedHistory {
    async fn complete_transactions(
        &self,
        account: &AccountId,
    ) -> Result<CompleteTransactionResponse, PortError> {
        let limit = InspectionRangeDays::try_new(u32::MAX).map_err(|_error| unavailable())?;
        let plan = FullInspectionPlan::new(None, self.clock.now(), limit)
            .map_err(|_error| unavailable())?;
        let window = *plan.ranges().first().ok_or_else(unavailable)?;
        match self.feed.transactions(account, window).await? {
            TransactionRead::Complete(observed) => Ok(observed.normalized().clone()),
            TransactionRead::Incomplete(_partial) => Err(unavailable()),
        }
    }

    async fn window_day(&self, date: CalendarDate) -> Result<Option<WindowDay>, PortError> {
        let kr = self.calendar(KnownMarket::Kr, date).await?;
        let us = self.calendar(KnownMarket::Us, date).await?;
        let Ok(calendar) = SnapshotCalendar::new(&kr, &us) else {
            return Ok(None);
        };
        Ok(Some(WindowDay {
            calendar,
            fx: self.rate(date).await,
        }))
    }

    async fn histories(
        &self,
        rows: &SettledRowSet,
        window: SettlementWindow,
    ) -> Result<Vec<SnapshotInstrumentHistory>, PortError> {
        let mut histories = Vec::new();
        for stock in traded_stocks(rows) {
            let id = instrument_id(&stock).ok_or_else(unavailable)?;
            let identity = LotIdentity::new(
                id,
                stock.1.clone(),
                domain::Market::from(stock.0),
                currency_of(stock.0),
            );
            let candles = self.candles_of(&stock, window).await?;
            histories.push(
                SnapshotInstrumentHistory::new(identity, Vec::new(), candles)
                    .map_err(|_error| unavailable())?,
            );
        }
        Ok(histories)
    }
}

struct SharedClock(Arc<dyn Clock>);

impl Clock for SharedClock {
    fn now(&self) -> Timestamp {
        self.0.now()
    }
}

pub struct PerformanceBook {
    history: Arc<FeedHistory>,
    clock: Arc<dyn Clock>,
    results: RwLock<BTreeMap<PerformanceSpan, PerformanceSummary>>,
    running: Mutex<()>,
}

impl std::fmt::Debug for PerformanceBook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PerformanceBook")
    }
}

const fn span_code(span: PerformanceSpan) -> &'static str {
    match span {
        PerformanceSpan::OneMonth => "1m",
        PerformanceSpan::ThreeMonths => "3m",
        PerformanceSpan::SixMonths => "6m",
        PerformanceSpan::OneYear => "1y",
    }
}

fn record_outcome(
    span: PerformanceSpan,
    outcome: &Result<PerformanceSummary, PerformanceRunError>,
) {
    let detail = match outcome {
        Ok(summary) => format!("result=complete skipped_days={}", summary.skipped.len()),
        Err(error) => format!("result=failed reason={error:?}"),
    };
    crate::diagnostics_log::record(&application::DiagnosticEvent::Job(
        application::JobDiagnosis {
            job: application::DiagnosticText::new(&format!("performance_{}", span_code(span))),
            note: match outcome {
                Ok(_summary) => application::JobRunNote::Executed {
                    summary: application::DiagnosticText::new(&detail),
                    processed: application::JobItemCount::new(1),
                },
                Err(_error) => {
                    application::JobRunNote::Failed(application::DiagnosticText::new(&detail))
                }
            },
        },
    ));
}

impl PerformanceBook {
    #[must_use]
    pub fn new(history: Arc<FeedHistory>, clock: Arc<dyn Clock>) -> Self {
        Self {
            history,
            clock,
            results: RwLock::new(BTreeMap::new()),
            running: Mutex::new(()),
        }
    }

    #[must_use]
    pub fn summary(&self, span: PerformanceSpan) -> Option<PerformanceSummary> {
        match self.results.read() {
            Ok(guard) => guard.get(&span).cloned(),
            Err(poisoned) => poisoned.into_inner().get(&span).cloned(),
        }
    }

    fn store(&self, span: PerformanceSpan, value: Option<PerformanceSummary>) {
        let mut guard = match self.results.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match value {
            Some(summary) => {
                let _previous = guard.insert(span, summary);
            }
            None => {
                let _removed = guard.remove(&span);
            }
        }
    }

    pub async fn refresh<Rows: SettledRowsSource>(&self, rows: &Rows, account: &AccountId) {
        let Ok(_running) = self.running.try_lock() else {
            return;
        };
        for span in SPANS {
            let outcome = ComputePerformance {
                rows,
                source: self.history.as_ref(),
                clock: &SharedClock(Arc::clone(&self.clock)),
            }
            .execute(account, span)
            .await;
            record_outcome(span, &outcome);
            self.store(span, outcome.ok());
        }
    }
}

#[cfg(test)]
mod tests;
