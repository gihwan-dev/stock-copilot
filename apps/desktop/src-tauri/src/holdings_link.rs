use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use application::{Clock, PortError, SyncPortfolio};
use domain::{
    DisplayInstrumentName, InstrumentMaster, InstrumentMasterEntry, KnownMarket, MasterInstrument,
    MasterInstrumentFacts, Nanosecond, SpanSeconds, Symbol, TimeSpan, Timestamp,
};
use tokio::sync::Mutex;

use crate::ipc::state::BoxFuture;
use crate::ports::{PortTypes, Ports};
use crate::toss_link::ConnectionListener;

#[cfg(test)]
mod tests;

const MASTER_REFRESH_SECONDS: u64 = 86_400;
const MARKETS: [KnownMarket; 2] = [KnownMarket::Kr, KnownMarket::Us];

pub trait InstrumentMasterFeed: Send + Sync {
    fn fetch(&self, market: KnownMarket) -> BoxFuture<'_, Result<InstrumentMaster, PortError>>;
    fn install(&self, facts: MasterInstrumentFacts);
}

#[derive(Debug, Clone, Default)]
pub struct InstrumentDirectory(Arc<RwLock<BTreeMap<Symbol, DirectoryEntry>>>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub market: KnownMarket,
    pub name: DisplayInstrumentName,
}

impl InstrumentDirectory {
    pub fn replace(&self, instruments: &[MasterInstrument]) {
        let entries = instruments
            .iter()
            .filter_map(|instrument| {
                let quote = instrument.quote_instrument().ok()?;
                Some((
                    instrument.row().symbol.clone(),
                    DirectoryEntry {
                        market: *quote.market(),
                        name: instrument.row().name.clone(),
                    },
                ))
            })
            .collect();
        match self.0.write() {
            Ok(mut guard) => *guard = entries,
            Err(poisoned) => *poisoned.into_inner() = entries,
        }
    }

    #[must_use]
    pub fn entry(&self, symbol: &Symbol) -> Option<DirectoryEntry> {
        match self.0.read() {
            Ok(guard) => guard.get(symbol).cloned(),
            Err(poisoned) => poisoned.into_inner().get(symbol).cloned(),
        }
    }

    #[must_use]
    pub fn name(&self, symbol: &Symbol) -> Option<DisplayInstrumentName> {
        self.entry(symbol).map(|entry| entry.name)
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
            match self.feed.fetch(market).await {
                Ok(master) => {
                    let _previous = loaded.insert(
                        market,
                        LoadedMarket {
                            at: self.clock.now(),
                            instruments: available(&master),
                        },
                    );
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
        let combined: Vec<MasterInstrument> = loaded
            .values()
            .flat_map(|market| market.instruments.iter().cloned())
            .collect();
        let Ok(facts) = MasterInstrumentFacts::try_new(combined.clone()) else {
            for market in changed {
                let _dropped = loaded.remove(&market);
            }
            return RefreshOutcome::Failed;
        };
        self.feed.install(facts);
        self.directory.replace(&combined);
        RefreshOutcome::Refreshed
    }
}

pub trait HoldingsSync: Send + Sync {
    fn sync(&self) -> BoxFuture<'_, ()>;
}

pub struct PortsHoldingsSync<P: PortTypes> {
    broker: Arc<P::BrokerRead>,
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
            let use_case = SyncPortfolio::new(
                self.broker.as_ref(),
                self.portfolios.as_ref(),
                self.clock.as_ref(),
                &self.events,
            );
            for account in &self.accounts.current() {
                let _report = use_case.execute(account).await;
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
