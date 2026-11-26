use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use application::{Clock, PortError, SyncPortfolio};
use domain::{
    DisplayInstrumentName, InstrumentId, InstrumentMaster, InstrumentMasterEntry, KnownMarket,
    MasterInstrument, MasterInstrumentFacts, Nanosecond, SpanSeconds, Symbol, TimeSpan, Timestamp,
};
use tokio::sync::Mutex;

use crate::ipc::state::BoxFuture;
use crate::ports::{PortTypes, Ports};
use crate::toss_link::ConnectionListener;

#[cfg(test)]
mod tests;

const MASTER_REFRESH_SECONDS: u64 = 86_400;
const MARKETS: [KnownMarket; 2] = [KnownMarket::Kr, KnownMarket::Us];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedMaster {
    pub market: KnownMarket,
    pub refreshed_at: Timestamp,
    pub instruments: Vec<MasterInstrument>,
}

pub trait InstrumentMasterFeed: Send + Sync {
    fn fetch(&self, market: KnownMarket) -> BoxFuture<'_, Result<InstrumentMaster, PortError>>;
    fn install(&self, facts: MasterInstrumentFacts);
    fn save(&self, master: SavedMaster) -> BoxFuture<'_, ()>;
    fn saved(&self) -> BoxFuture<'_, Vec<SavedMaster>>;
}

#[derive(Debug, Clone, Default)]
pub struct InstrumentDirectory {
    entries: Arc<RwLock<BTreeMap<Symbol, DirectoryEntry>>>,
    masters: Arc<RwLock<DirectoryMasters>>,
}

#[derive(Debug, Default)]
struct DirectoryMasters {
    by_id: BTreeMap<InstrumentId, MasterInstrument>,
    by_symbol: BTreeMap<(KnownMarket, Symbol), InstrumentId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub market: KnownMarket,
    pub name: DisplayInstrumentName,
    pub trading: domain::MasterTradingState,
    pub master: MasterInstrument,
}

fn write_lock<T>(lock: &RwLock<T>, value: T) {
    match lock.write() {
        Ok(mut guard) => *guard = value,
        Err(poisoned) => *poisoned.into_inner() = value,
    }
}

impl InstrumentDirectory {
    pub fn replace(&self, instruments: &[MasterInstrument]) {
        let mut masters = DirectoryMasters::default();
        let entries = instruments
            .iter()
            .filter_map(|instrument| {
                let quote = instrument.quote_instrument().ok()?;
                let id = instrument.row().instrument_id.clone();
                let _symbol = masters.by_symbol.insert(
                    (*quote.market(), instrument.row().symbol.clone()),
                    id.clone(),
                );
                let _master = masters.by_id.insert(id, instrument.clone());
                Some((
                    instrument.row().symbol.clone(),
                    DirectoryEntry {
                        market: *quote.market(),
                        name: instrument.row().name.clone(),
                        trading: instrument.trading_state(),
                        master: instrument.clone(),
                    },
                ))
            })
            .collect();
        write_lock(&self.entries, entries);
        write_lock(&self.masters, masters);
    }

    #[must_use]
    pub fn by_instrument(&self, instrument: &InstrumentId) -> Option<DirectoryEntry> {
        let find = |entries: &BTreeMap<Symbol, DirectoryEntry>| {
            entries
                .values()
                .find(|entry| entry.master.row().instrument_id == *instrument)
                .cloned()
        };
        match self.entries.read() {
            Ok(guard) => find(&guard),
            Err(poisoned) => find(&poisoned.into_inner()),
        }
    }

    #[must_use]
    pub fn entry(&self, symbol: &Symbol) -> Option<DirectoryEntry> {
        match self.entries.read() {
            Ok(guard) => guard.get(symbol).cloned(),
            Err(poisoned) => poisoned.into_inner().get(symbol).cloned(),
        }
    }

    #[must_use]
    pub fn entries(&self) -> Vec<DirectoryEntry> {
        match self.entries.read() {
            Ok(guard) => guard.values().cloned().collect(),
            Err(poisoned) => poisoned.into_inner().values().cloned().collect(),
        }
    }

    #[must_use]
    pub fn name(&self, symbol: &Symbol) -> Option<DisplayInstrumentName> {
        self.entry(symbol).map(|entry| entry.name)
    }

    #[must_use]
    pub fn master(&self, instrument: &InstrumentId) -> Option<MasterInstrument> {
        match self.masters.read() {
            Ok(guard) => guard.by_id.get(instrument).cloned(),
            Err(poisoned) => poisoned.into_inner().by_id.get(instrument).cloned(),
        }
    }

    #[must_use]
    pub fn instrument_of(&self, market: KnownMarket, symbol: &Symbol) -> Option<InstrumentId> {
        let key = (market, symbol.clone());
        match self.masters.read() {
            Ok(guard) => guard.by_symbol.get(&key).cloned(),
            Err(poisoned) => poisoned.into_inner().by_symbol.get(&key).cloned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    Refreshed,
    Fresh,
    Failed,
}

struct LoadedMarket {
    at: Timestamp,
    instruments: Vec<MasterInstrument>,
}

pub struct MasterRefresh {
    feed: Arc<dyn InstrumentMasterFeed>,
    directory: InstrumentDirectory,
    clock: Arc<dyn Clock>,
    loaded: Mutex<BTreeMap<KnownMarket, LoadedMarket>>,
}

impl std::fmt::Debug for MasterRefresh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MasterRefresh(<redacted>)")
    }
}

fn refresh_window() -> Option<TimeSpan> {
    TimeSpan::try_new(SpanSeconds::new(MASTER_REFRESH_SECONDS), Nanosecond::new(0)).ok()
}

fn available(master: &InstrumentMaster) -> Vec<MasterInstrument> {
    master
        .entries()
        .iter()
        .filter_map(|entry| match entry {
            InstrumentMasterEntry::Available(instrument) => Some(instrument.clone()),
            InstrumentMasterEntry::Unavailable { .. } => None,
        })
        .collect()
}

impl MasterRefresh {
    #[must_use]
    pub fn new(
        feed: Arc<dyn InstrumentMasterFeed>,
        directory: InstrumentDirectory,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            feed,
            directory,
            clock,
            loaded: Mutex::new(BTreeMap::new()),
        }
    }

    fn is_fresh(&self, loaded: Option<&LoadedMarket>) -> bool {
        let now = self.clock.now();
        loaded.is_some_and(|market| {
            refresh_window()
                .and_then(|window| market.at.checked_add(window).ok())
                .is_some_and(|until| now < until)
        })
    }

    pub async fn refresh_due(&self) -> RefreshOutcome {
        let mut loaded = self.loaded.lock().await;
        let mut changed = Vec::new();
        let mut failed = Vec::new();
        for market in MARKETS {
            if self.is_fresh(loaded.get(&market)) {
                continue;
            }
            let fetched = self.feed.fetch(market).await;
            crate::diagnostics_log::record(&application::DiagnosticEvent::Sync(
                application::SyncDiagnosis {
                    job: application::SyncJob::InstrumentMaster,
                    result: match &fetched {
                        Ok(_master) => application::SyncResult::Complete,
                        Err(_error) => application::SyncResult::Failed,
                    },
                    errors: fetched.as_ref().err().copied().into_iter().collect(),
                },
            ));
            match fetched {
                Ok(master) => {
                    let at = self.clock.now();
                    let instruments = available(&master);
                    self.feed
                        .save(SavedMaster {
                            market,
                            refreshed_at: at,
                            instruments: instruments.clone(),
                        })
                        .await;
                    let _previous = loaded.insert(market, LoadedMarket { at, instruments });
                    changed.push(market);
                }
                Err(_error) => failed.push(market),
            }
        }
        if changed.is_empty() {
            return if failed.is_empty() {
                RefreshOutcome::Fresh
            } else {
                RefreshOutcome::Failed
            };
        }
        if self.install(&loaded) {
            RefreshOutcome::Refreshed
        } else {
            for market in changed {
                let _dropped = loaded.remove(&market);
            }
            RefreshOutcome::Failed
        }
    }

    fn install(&self, loaded: &BTreeMap<KnownMarket, LoadedMarket>) -> bool {
        let combined: Vec<MasterInstrument> = loaded
            .values()
            .flat_map(|market| market.instruments.iter().cloned())
            .collect();
        let Ok(facts) = MasterInstrumentFacts::try_new(combined.clone()) else {
            return false;
        };
        self.feed.install(facts);
        self.directory.replace(&combined);
        true
    }

    pub async fn restore(&self) -> RefreshOutcome {
        let saved = self.feed.saved().await;
        if saved.is_empty() {
            return RefreshOutcome::Fresh;
        }
        let mut loaded = self.loaded.lock().await;
        for master in saved {
            let _previous = loaded.entry(master.market).or_insert(LoadedMarket {
                at: master.refreshed_at,
                instruments: master.instruments,
            });
        }
        if self.install(&loaded) {
            RefreshOutcome::Refreshed
        } else {
            loaded.clear();
            RefreshOutcome::Failed
        }
    }
}

#[derive(Debug)]
pub struct SyncParts<'a, B, P, C, M, R> {
    pub broker: &'a B,
    pub portfolios: &'a P,
    pub clock: &'a C,
    pub events: &'a application::EventBus,
    pub market: &'a M,
    pub market_data: &'a R,
}

pub async fn sync_with_rate<B, P, C, M, R>(
    parts: &SyncParts<'_, B, P, C, M, R>,
    account: &domain::AccountId,
) -> Result<application::PortfolioSyncReport, application::SystemPortError>
where
    B: application::BrokerRead,
    P: application::PortfolioRepository,
    C: Clock,
    M: application::MarketInfo,
    R: application::MarketDataRepository,
{
    let report = SyncPortfolio::new(parts.broker, parts.portfolios, parts.clock, parts.events)
        .execute(account)
        .await?;
    crate::diagnostics_log::record(&application::DiagnosticEvent::Sync(
        application::SyncDiagnosis {
            job: application::SyncJob::Portfolio,
            result: application::SyncResult::of(report.record.outcome()),
            errors: report.failures.clone(),
        },
    ));
    let refreshed = application::RefreshFxRate::new(parts.market, parts.market_data, parts.events)
        .execute(
            domain::FxPair::UsdKrw,
            domain::FxRateKind::Mid,
            Some(report.record.recorded_at()),
        )
        .await;
    if refreshed.is_ok() {
        let _delivery = parts
            .events
            .publish(domain::DomainEvent::PortfolioSyncRecorded(account.clone()));
    }
    Ok(report)
}

pub trait HoldingsSync: Send + Sync {
    fn sync(&self) -> BoxFuture<'_, ()>;
}

pub struct PortsHoldingsSync<P: PortTypes> {
    broker: Arc<P::BrokerRead>,
    market: Arc<P::MarketInfo>,
    market_data: Arc<P::MarketDataRepository>,
    portfolios: Arc<P::PortfolioRepository>,
    clock: Arc<P::Clock>,
    events: Arc<application::EventBus>,
    accounts: crate::broker_link::AccountSlot,
}

impl<P: PortTypes> std::fmt::Debug for PortsHoldingsSync<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PortsHoldingsSync(<redacted>)")
    }
}

impl<P: PortTypes> PortsHoldingsSync<P> {
    #[must_use]
    pub fn from_ports(ports: &Ports<P>) -> Self {
        Self {
            broker: Arc::clone(&ports.broker_read),
            market: Arc::clone(&ports.market_info),
            market_data: Arc::clone(&ports.market_data),
            portfolios: Arc::clone(&ports.portfolios),
            clock: Arc::clone(&ports.clock),
            events: Arc::clone(&ports.events),
            accounts: ports.saved_accounts.clone(),
        }
    }
}

impl<P: PortTypes> HoldingsSync for PortsHoldingsSync<P> {
    fn sync(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let parts = SyncParts {
                broker: self.broker.as_ref(),
                portfolios: self.portfolios.as_ref(),
                clock: self.clock.as_ref(),
                events: &self.events,
                market: self.market.as_ref(),
                market_data: self.market_data.as_ref(),
            };
            for account in &self.accounts.current() {
                let _report = sync_with_rate(&parts, account).await;
            }
        })
    }
}

#[derive(Clone)]
pub struct HoldingsListener {
    accounts: Arc<dyn ConnectionListener>,
    refresh: Arc<MasterRefresh>,
    sync: Arc<dyn HoldingsSync>,
    running: Arc<Mutex<()>>,
}

impl std::fmt::Debug for HoldingsListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HoldingsListener(<redacted>)")
    }
}

impl HoldingsListener {
    #[must_use]
    pub fn new(
        accounts: Arc<dyn ConnectionListener>,
        refresh: Arc<MasterRefresh>,
        sync: Arc<dyn HoldingsSync>,
    ) -> Self {
        Self {
            accounts,
            refresh,
            sync,
            running: Arc::new(Mutex::new(())),
        }
    }

    pub async fn restore_saved(&self) -> RefreshOutcome {
        self.refresh.restore().await
    }

    pub async fn refresh_holdings(&self) -> RefreshOutcome {
        let Ok(_running) = self.running.try_lock() else {
            return RefreshOutcome::Fresh;
        };
        let outcome = self.refresh.refresh_due().await;
        if outcome == RefreshOutcome::Refreshed {
            self.sync.sync().await;
        }
        outcome
    }
}

impl ConnectionListener for HoldingsListener {
    fn connected(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            self.accounts.connected().await;
            let background = self.clone();
            let _task = tauri::async_runtime::spawn(async move {
                let _outcome = background.refresh_holdings().await;
            });
        })
    }
}
