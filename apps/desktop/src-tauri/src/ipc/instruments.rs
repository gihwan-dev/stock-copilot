use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use application::{
    BrokerRead, Clock, EventBus, ManageWatchlist, PortError, QuoteObservation, StoredView,
    Watchlist, WatchlistError, WatchlistOutcome, WatchlistRepository,
};
use domain::{
    CalendarDate, Candle, CandlePeriod, InstrumentId, MasterInstrument, Nanosecond, OffsetSeconds,
    PortfolioValuation, Price, PriceAdjustment, QuoteInstrument, SpanSeconds, Symbol, TimeInterval,
    TimeSpan, Timestamp, ValuedHolding,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::briefing::kst_iso;
use super::error::{IpcError, IpcErrorKind};
use super::portfolio::{
    UiDirection, UiMoney, precision, return_rate, signed, ui_direction, ui_money,
};
use super::state::{BoxFuture, ScreenState};
use crate::holdings_link::{DirectoryEntry, InstrumentDirectory};

#[cfg(test)]
mod tests;

const DAY_SECONDS: u64 = 86_400;
const CLOSE_LOOKBACK_DAYS: u64 = 14;
const CHART_LOOKBACK_DAYS: u64 = 120;
const CHART_CANDLES: usize = 60;

pub trait InstrumentMarket: Send + Sync {
    fn quotes(
        &self,
        instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>>;
    fn daily(
        &self,
        instrument: QuoteInstrument,
        window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>>;
}

impl<B: BrokerRead + 'static> InstrumentMarket for B {
    fn quotes(
        &self,
        instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>> {
        Box::pin(async move { BrokerRead::quotes(self, &instruments).await })
    }

    fn daily(
        &self,
        instrument: QuoteInstrument,
        window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>> {
        Box::pin(async move {
            BrokerRead::candles(
                self,
                &instrument,
                window,
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            )
            .await
        })
    }
}

pub trait WatchlistBook: Send + Sync {
    fn list(&self) -> BoxFuture<'_, Result<Watchlist, WatchlistError>>;
    fn add(
        &self,
        master: MasterInstrument,
    ) -> BoxFuture<'_, Result<WatchlistOutcome, WatchlistError>>;
    fn remove(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<WatchlistOutcome, WatchlistError>>;
}

struct ManagedWatchlist<W, C> {
    repository: Arc<W>,
    clock: Arc<C>,
    events: Arc<EventBus>,
}

impl<W: WatchlistRepository + 'static, C: Clock + 'static> WatchlistBook
    for ManagedWatchlist<W, C>
{
    fn list(&self) -> BoxFuture<'_, Result<Watchlist, WatchlistError>> {
        Box::pin(async move {
            ManageWatchlist::new(self.repository.as_ref(), self.clock.as_ref(), &self.events)
                .list()
                .await
        })
    }

    fn add(
        &self,
        master: MasterInstrument,
    ) -> BoxFuture<'_, Result<WatchlistOutcome, WatchlistError>> {
        Box::pin(async move {
            ManageWatchlist::new(self.repository.as_ref(), self.clock.as_ref(), &self.events)
                .add(&master)
                .await
        })
    }

    fn remove(
        &self,
        instrument: InstrumentId,
    ) -> BoxFuture<'_, Result<WatchlistOutcome, WatchlistError>> {
        Box::pin(async move {
            ManageWatchlist::new(self.repository.as_ref(), self.clock.as_ref(), &self.events)
                .remove(&instrument)
                .await
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CachedClose {
    offset: OffsetSeconds,
    date: CalendarDate,
    close: Price,
}

#[derive(Clone)]
pub struct InstrumentsIpcState {
    market: Arc<dyn InstrumentMarket>,
    watchlist: Arc<dyn WatchlistBook>,
    clock: Arc<dyn Clock>,
    closes: Arc<RwLock<BTreeMap<Symbol, CachedClose>>>,
}

impl fmt::Debug for InstrumentsIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InstrumentsIpcState")
    }
}

impl InstrumentsIpcState {
    #[must_use]
    pub fn new<W: WatchlistRepository + 'static, C: Clock + 'static>(
        market: Arc<dyn InstrumentMarket>,
        repository: Arc<W>,
        clock: Arc<C>,
        events: Arc<EventBus>,
    ) -> Self {
        Self {
            market,
            watchlist: Arc::new(ManagedWatchlist {
                repository,
                clock: Arc::clone(&clock),
                events,
            }),
            clock,
            closes: Arc::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetInstrumentsRequest {
    pub scope: InstrumentsScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum InstrumentsScope {
    #[serde(rename = "tracked")]
    Tracked,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct InstrumentRequest {
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiInstrumentOrigin {
    #[serde(rename = "holding")]
    Holding,
    #[serde(rename = "watchlist")]
    Watchlist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiDailyChange {
    pub rate: String,
    pub direction: UiDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiInstrumentRow {
    pub symbol: String,
    pub name: String,
    pub origin: UiInstrumentOrigin,
    pub today: Option<UiDailyChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiInstrumentsView {
    pub rows: Vec<UiInstrumentRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiInstrumentCandle {
    pub date: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiInstrumentHolding {
    pub quantity: String,
    pub value: UiMoney,
    pub profit: UiMoney,
    pub profit_rate: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiInstrumentDetail {
    pub symbol: String,
    pub name: String,
    pub price: UiMoney,
    pub today: Option<UiDailyChange>,
    pub quoted_at: String,
    pub candles: Vec<UiInstrumentCandle>,
    pub holding: Option<UiInstrumentHolding>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiWatchlistOutcome {
    #[serde(rename = "applied")]
    Applied,
    #[serde(rename = "rejected")]
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiWatchlistRejection {
    #[serde(rename = "unknown-instrument")]
    UnknownInstrument,
    #[serde(rename = "inactive-instrument")]
    InactiveInstrument,
    #[serde(rename = "capacity-exceeded")]
    CapacityExceeded,
    #[serde(rename = "not-in-watchlist")]
    NotInWatchlist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiWatchlistChange {
    pub outcome: UiWatchlistOutcome,
    pub reason: Option<UiWatchlistRejection>,
}

const APPLIED: UiWatchlistChange = UiWatchlistChange {
    outcome: UiWatchlistOutcome::Applied,
    reason: None,
};

const fn rejected(reason: UiWatchlistRejection) -> UiWatchlistChange {
    UiWatchlistChange {
        outcome: UiWatchlistOutcome::Rejected,
        reason: Some(reason),
    }
}

const fn error(kind: IpcErrorKind) -> IpcError {
    IpcError { kind }
}

fn days(count: u64) -> Option<TimeSpan> {
    TimeSpan::try_new(
        SpanSeconds::new(count.checked_mul(DAY_SECONDS)?),
        Nanosecond::new(0),
    )
    .ok()
}

fn lookback(now: Timestamp, count: u64) -> Option<TimeInterval> {
    let start = now.checked_sub(days(count)?).ok()?;
    TimeInterval::try_new(start, now).ok()
}

fn decimal(price: Price) -> String {
    price
        .as_money()
        .amount()
        .as_decimal()
        .normalize()
        .to_string()
}

fn date_text(date: CalendarDate) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year().as_i32(),
        date.month().as_u8(),
        date.day().as_u8()
    )
}

fn change(previous: Option<Price>, current: Price) -> Option<UiDailyChange> {
    let moved = domain::daily_change(&previous?, &current).ok()?;
    let (rate, _direction) = domain::display_quotient_percent(&moved.rate(), precision(2)?);
    Some(UiDailyChange {
        rate: rate.as_str().to_owned(),
        direction: ui_direction(moved.direction()),
    })
}

struct Quoted {
    price: Price,
    at: Timestamp,
}

fn quoted(observation: QuoteObservation) -> Option<(Symbol, Quoted)> {
    let quote = observation.result.ok()?;
    let price = quote.last_price()?;
    Some((
        observation.instrument.symbol().clone(),
        Quoted {
            price,
            at: quote.timestamp().unwrap_or_else(|| quote.collected_at()),
        },
    ))
}

fn cached_close(state: &InstrumentsIpcState, symbol: &Symbol, at: Timestamp) -> Option<Price> {
    let closes = match state.closes.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let cached = closes.get(symbol)?;
    let today = at.date_at_offset(cached.offset).ok()?;
    (today == cached.date).then_some(cached.close)
}

fn remember_close(
    state: &InstrumentsIpcState,
    symbol: &Symbol,
    at: Timestamp,
    candles: &[Candle],
) -> Option<Price> {
    let close = domain::previous_session_close(candles, at)?;
    let offset = candles.first()?.time().timestamp().offset_seconds();
    let date = at.date_at_offset(offset).ok()?;
    let mut closes = match state.closes.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let _previous = closes.insert(
        symbol.clone(),
        CachedClose {
            offset,
            date,
            close,
        },
    );
    Some(close)
}

async fn previous_close(
    state: &InstrumentsIpcState,
    quote: &QuoteInstrument,
    at: Timestamp,
) -> Option<Price> {
    if let Some(close) = cached_close(state, quote.symbol(), at) {
        return Some(close);
    }
    let window = lookback(state.clock.now(), CLOSE_LOOKBACK_DAYS)?;
    let candles = state.market.daily(quote.clone(), window).await.ok()?;
    remember_close(state, quote.symbol(), at, &candles)
}

async fn quotes(
    state: &InstrumentsIpcState,
    instruments: Vec<QuoteInstrument>,
) -> Result<BTreeMap<Symbol, Quoted>, PortError> {
    if instruments.is_empty() {
        return Ok(BTreeMap::new());
    }
    let observations = state.market.quotes(instruments).await?;
    Ok(observations.into_iter().filter_map(quoted).collect())
}

pub(crate) struct Tracked {
    pub(crate) symbol: Symbol,
    pub(crate) entry: DirectoryEntry,
    pub(crate) origin: UiInstrumentOrigin,
}

pub(crate) async fn tracked_instruments(
    state: &InstrumentsIpcState,
    screen: Option<&ScreenState>,
    directory: &InstrumentDirectory,
) -> Result<Vec<Tracked>, IpcError> {
    let watchlist = state.watchlist.list().await.map_err(list_error)?;
    let held = holdings(screen).await;
    Ok(tracked(&held, &watchlist, directory))
}

pub(crate) async fn last_prices(
    state: &InstrumentsIpcState,
    instruments: Vec<QuoteInstrument>,
) -> Option<BTreeMap<Symbol, Price>> {
    let found = quotes(state, instruments).await.ok()?;
    Some(
        found
            .into_iter()
            .map(|(symbol, quote)| (symbol, quote.price))
            .collect(),
    )
}

fn holdings_of(view: &StoredView<PortfolioValuation>) -> Vec<ValuedHolding> {
    match view {
        StoredView::Missing => Vec::new(),
        StoredView::Recorded(recorded) => match recorded.outcome() {
            domain::SyncOutcome::Complete(valuation) | domain::SyncOutcome::Partial(valuation) => {
                valuation.holdings().to_vec()
            }
            domain::SyncOutcome::Failed | domain::SyncOutcome::AccessBlocked => Vec::new(),
        },
    }
}

async fn holdings(screen: Option<&ScreenState>) -> Vec<ValuedHolding> {
    let Some(backend) = screen.map(ScreenState::backend) else {
        return Vec::new();
    };
    match backend.load_portfolio_view().await {
        Ok(view) => holdings_of(&view),
        Err(_unreadable) => Vec::new(),
    }
}

fn tracked(
    held: &[ValuedHolding],
    watchlist: &Watchlist,
    directory: &InstrumentDirectory,
) -> Vec<Tracked> {
    let mut seen = Vec::<Symbol>::new();
    let mut rows = Vec::new();
    for holding in held {
        let symbol = holding.instrument().symbol().clone();
        if let Some(entry) = directory.entry(&symbol)
            && !seen.contains(&symbol)
        {
            seen.push(symbol.clone());
            rows.push(Tracked {
                symbol,
                entry,
                origin: UiInstrumentOrigin::Holding,
            });
        }
    }
    for item in watchlist.entries() {
        if let Some(entry) = directory.by_instrument(item.instrument()) {
            let symbol = entry.master.row().symbol.clone();
            if !seen.contains(&symbol) {
                seen.push(symbol.clone());
                rows.push(Tracked {
                    symbol,
                    entry,
                    origin: UiInstrumentOrigin::Watchlist,
                });
            }
        }
    }
    rows
}

fn watchlist_error(error: WatchlistError) -> Result<UiWatchlistChange, IpcError> {
    match error {
        WatchlistError::CapacityExceeded => Ok(rejected(UiWatchlistRejection::CapacityExceeded)),
        WatchlistError::DuplicateInstrument => Ok(APPLIED),
        WatchlistError::InactiveInstrument => {
            Ok(rejected(UiWatchlistRejection::InactiveInstrument))
        }
        WatchlistError::ConcurrentUpdate => Err(error_kind_retryable()),
        WatchlistError::Storage(failure) => Err(IpcError::from(
            application::ClassifiedError::class(&failure),
        )),
    }
}

const fn error_kind_retryable() -> IpcError {
    error(IpcErrorKind::Retryable)
}

fn list_error(failure: WatchlistError) -> IpcError {
    match failure {
        WatchlistError::Storage(storage) => {
            IpcError::from(application::ClassifiedError::class(&storage))
        }
        WatchlistError::CapacityExceeded
        | WatchlistError::DuplicateInstrument
        | WatchlistError::InactiveInstrument
        | WatchlistError::ConcurrentUpdate => error_kind_retryable(),
    }
}

pub async fn load_instruments(
    state: &InstrumentsIpcState,
    screen: Option<&ScreenState>,
    directory: &InstrumentDirectory,
) -> Result<UiInstrumentsView, IpcError> {
    let watchlist = state.watchlist.list().await.map_err(list_error)?;
    let held = holdings(screen).await;
    let rows = tracked(&held, &watchlist, directory);
    let instruments = rows
        .iter()
        .filter_map(|row| row.entry.master.quote_instrument().ok())
        .collect::<Vec<_>>();
    let priced = quotes(state, instruments.clone()).await.ok();
    let mut converted = Vec::new();
    for row in rows {
        let today = match (
            priced.as_ref().and_then(|found| found.get(&row.symbol)),
            instruments
                .iter()
                .find(|instrument| *instrument.symbol() == row.symbol),
        ) {
            (Some(quote), Some(instrument)) => change(
                previous_close(state, instrument, quote.at).await,
                quote.price,
            ),
            (None, _) | (Some(_), None) => None,
        };
        converted.push(UiInstrumentRow {
            symbol: row.symbol.as_str().to_owned(),
            name: row.entry.name.as_str().to_owned(),
            origin: row.origin,
            today,
        });
    }
    Ok(UiInstrumentsView { rows: converted })
}

fn holding_view(holding: &ValuedHolding) -> Option<UiInstrumentHolding> {
    let value = holding.valuation().selected().ok()?;
    let profit = holding.valuation().selected_profit().ok()?.value();
    let shown = signed(profit, Some(String::from("0")))?;
    Some(UiInstrumentHolding {
        quantity: holding
            .holding()
            .quantity()
            .as_decimal()
            .normalize()
            .to_string(),
        value: ui_money(&value)?,
        profit: shown.amount,
        profit_rate: return_rate(holding).unwrap_or_else(|| String::from("0")),
    })
}

fn parse_symbol(raw: &str) -> Result<Symbol, IpcError> {
    Symbol::try_new(raw).map_err(|_invalid| error(IpcErrorKind::UserFixable))
}

pub async fn load_instrument_detail(
    state: &InstrumentsIpcState,
    screen: Option<&ScreenState>,
    directory: &InstrumentDirectory,
    symbol: &Symbol,
) -> Result<UiInstrumentDetail, IpcError> {
    let entry = directory.entry(symbol).ok_or(error(IpcErrorKind::NoData))?;
    let instrument = entry
        .master
        .quote_instrument()
        .map_err(|_unquotable| error(IpcErrorKind::NoData))?;
    let quote = quotes(state, vec![instrument.clone()])
        .await
        .map_err(|_unpriced| error_kind_retryable())?
        .remove(symbol)
        .ok_or(error_kind_retryable())?;
    let window =
        lookback(state.clock.now(), CHART_LOOKBACK_DAYS).ok_or(error(IpcErrorKind::Bug))?;
    let fetched = state.market.daily(instrument, window).await.ok();
    let mut candles = fetched.as_deref().unwrap_or(&[]).to_vec();
    candles.sort_by_key(|candle| candle.time().timestamp());
    let previous = remember_close(state, symbol, quote.at, &candles)
        .or_else(|| cached_close(state, symbol, quote.at));
    let chart = candles
        .iter()
        .rev()
        .take(CHART_CANDLES)
        .rev()
        .map(|candle| {
            let values = candle.values();
            UiInstrumentCandle {
                date: date_text(candle.time().date()),
                open: decimal(values.open()),
                high: decimal(values.high()),
                low: decimal(values.low()),
                close: decimal(values.close()),
            }
        })
        .collect();
    let held = holdings(screen).await;
    Ok(UiInstrumentDetail {
        symbol: symbol.as_str().to_owned(),
        name: entry.name.as_str().to_owned(),
        price: ui_money(&quote.price.as_money()).ok_or(error(IpcErrorKind::Bug))?,
        today: change(previous, quote.price),
        quoted_at: kst_iso(quote.at).ok_or(error(IpcErrorKind::Bug))?,
        candles: chart,
        holding: held
            .iter()
            .find(|holding| holding.instrument().symbol() == symbol)
            .and_then(holding_view),
    })
}

pub async fn add_to_watchlist(
    state: &InstrumentsIpcState,
    directory: &InstrumentDirectory,
    symbol: &Symbol,
) -> Result<UiWatchlistChange, IpcError> {
    let Some(entry) = directory.entry(symbol) else {
        return Ok(rejected(UiWatchlistRejection::UnknownInstrument));
    };
    match state.watchlist.add(entry.master).await {
        Ok(WatchlistOutcome::Added { .. } | WatchlistOutcome::AlreadyPresent(_)) => Ok(APPLIED),
        Ok(WatchlistOutcome::Removed { .. } | WatchlistOutcome::NotFound(_)) => {
            Err(error(IpcErrorKind::Bug))
        }
        Err(failure) => watchlist_error(failure),
    }
}

pub async fn remove_from_watchlist(
    state: &InstrumentsIpcState,
    directory: &InstrumentDirectory,
    symbol: &Symbol,
) -> Result<UiWatchlistChange, IpcError> {
    let Some(entry) = directory.entry(symbol) else {
        return Ok(rejected(UiWatchlistRejection::NotInWatchlist));
    };
    match state
        .watchlist
        .remove(entry.master.row().instrument_id.clone())
        .await
    {
        Ok(WatchlistOutcome::Removed { .. }) => Ok(APPLIED),
        Ok(WatchlistOutcome::NotFound(_)) => Ok(rejected(UiWatchlistRejection::NotInWatchlist)),
        Ok(WatchlistOutcome::Added { .. } | WatchlistOutcome::AlreadyPresent(_)) => {
            Err(error(IpcErrorKind::Bug))
        }
        Err(failure) => watchlist_error(failure),
    }
}

fn managed<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(InstrumentsIpcState, InstrumentDirectory), IpcError> {
    let state = app
        .try_state::<InstrumentsIpcState>()
        .map(|managed_state| managed_state.inner().clone())
        .ok_or(error(IpcErrorKind::NoData))?;
    let directory = app
        .try_state::<InstrumentDirectory>()
        .map_or_else(InstrumentDirectory::default, |names| names.inner().clone());
    Ok((state, directory))
}

#[tauri::command]
#[specta::specta]
pub async fn get_instruments<R: Runtime>(
    app: AppHandle<R>,
    request: GetInstrumentsRequest,
) -> Result<UiInstrumentsView, IpcError> {
    let GetInstrumentsRequest {
        scope: InstrumentsScope::Tracked,
    } = request;
    let (state, directory) = managed(&app)?;
    let screen = app.try_state::<ScreenState>();
    load_instruments(&state, screen.as_ref().map(tauri::State::inner), &directory).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_instrument_detail<R: Runtime>(
    app: AppHandle<R>,
    request: InstrumentRequest,
) -> Result<UiInstrumentDetail, IpcError> {
    let symbol = parse_symbol(&request.symbol)?;
    let (state, directory) = managed(&app)?;
    let screen = app.try_state::<ScreenState>();
    load_instrument_detail(
        &state,
        screen.as_ref().map(tauri::State::inner),
        &directory,
        &symbol,
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn add_watchlist<R: Runtime>(
    app: AppHandle<R>,
    request: InstrumentRequest,
) -> Result<UiWatchlistChange, IpcError> {
    let symbol = parse_symbol(&request.symbol)?;
    let (state, directory) = managed(&app)?;
    add_to_watchlist(&state, &directory, &symbol).await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_watchlist<R: Runtime>(
    app: AppHandle<R>,
    request: InstrumentRequest,
) -> Result<UiWatchlistChange, IpcError> {
    let symbol = parse_symbol(&request.symbol)?;
    let (state, directory) = managed(&app)?;
    remove_from_watchlist(&state, &directory, &symbol).await
}
