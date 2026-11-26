use crate::holdings_link::InstrumentDirectory;
use crate::ipc::instruments::{InstrumentsIpcState, tracked_instruments};
use crate::ipc::state::ScreenState;
use crate::ports::{PortTypes, Ports};
use application::{
    NewsDataSettingsRepository, SecretKey, SecretStore, StockAlias, TrackedInstrument,
};
use domain::{InstrumentId, KnownMarket};
use sources::{ContactFuture, DartKeySource, SecContactSource, SecretFuture, TrackedTargets};
use std::fmt;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};
use store_sqlite::SqliteRepositories;
use tauri::{AppHandle, Manager, Runtime};

#[cfg(test)]
mod tests;

const SHORT_NAME_CHARS: usize = 2;
const MIN_TICKER_CHARS: usize = 3;

type TrackedList<'a> = Pin<Box<dyn Future<Output = Vec<TrackedInstrument>> + Send + 'a>>;

pub trait NewsInputs: TrackedTargets + DartKeySource + SecContactSource {}

impl<T: TrackedTargets + DartKeySource + SecContactSource> NewsInputs for T {}

pub struct AppNewsInputs<R: Runtime, P: PortTypes> {
    app: AppHandle<R>,
    ports: PhantomData<fn() -> P>,
}

impl<R: Runtime, P: PortTypes> fmt::Debug for AppNewsInputs<R, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AppNewsInputs")
    }
}

impl<R: Runtime, P: PortTypes> AppNewsInputs<R, P> {
    #[must_use]
    pub const fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            ports: PhantomData,
        }
    }
}

impl<R: Runtime, P: PortTypes> DartKeySource for AppNewsInputs<R, P> {
    fn dart_key(&self) -> SecretFuture<'_> {
        Box::pin(async move {
            let ports = self.app.try_state::<Ports<P>>()?;
            ports
                .secrets
                .load(SecretKey::DartApiKey)
                .await
                .ok()
                .flatten()
        })
    }
}

impl<R: Runtime, P: PortTypes> SecContactSource for AppNewsInputs<R, P> {
    fn sec_contact(&self) -> ContactFuture<'_> {
        Box::pin(async move {
            let store = self.app.try_state::<Arc<SqliteRepositories>>()?;
            let settings = NewsDataSettingsRepository::load(store.inner().as_ref())
                .await
                .ok()
                .flatten()?;
            settings
                .sec_contact()
                .map(|contact| contact.as_str().to_owned())
        })
    }
}

impl<R: Runtime, P: PortTypes> TrackedTargets for AppNewsInputs<R, P> {
    fn tracked(&self) -> TrackedList<'_> {
        Box::pin(async move {
            let Some(state) = self.app.try_state::<InstrumentsIpcState>() else {
                return Vec::new();
            };
            let directory = self
                .app
                .try_state::<InstrumentDirectory>()
                .map_or_else(InstrumentDirectory::default, |names| names.inner().clone());
            let screen = self.app.try_state::<ScreenState>();
            match tracked_instruments(
                state.inner(),
                screen.as_ref().map(tauri::State::inner),
                &directory,
            )
            .await
            {
                Ok(rows) => rows
                    .into_iter()
                    .map(|row| TrackedInstrument {
                        symbol: row.symbol,
                        market: row.entry.market,
                        name: row.entry.name,
                    })
                    .collect(),
                Err(_unreadable) => Vec::new(),
            }
        })
    }
}

#[derive(Clone, Default)]
pub struct NewsTracking {
    inputs: Arc<OnceLock<Arc<dyn NewsInputs>>>,
}

impl fmt::Debug for NewsTracking {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewsTracking")
            .field("bound", &self.inputs.get().is_some())
            .finish()
    }
}

impl NewsTracking {
    pub fn bind(&self, inputs: Arc<dyn NewsInputs>) {
        let _bound = self.inputs.set(inputs);
    }
}

impl TrackedTargets for NewsTracking {
    fn tracked(&self) -> TrackedList<'_> {
        Box::pin(async move {
            match self.inputs.get() {
                Some(inputs) => inputs.tracked().await,
                None => Vec::new(),
            }
        })
    }
}

impl DartKeySource for NewsTracking {
    fn dart_key(&self) -> SecretFuture<'_> {
        Box::pin(async move {
            match self.inputs.get() {
                Some(inputs) => inputs.dart_key().await,
                None => None,
            }
        })
    }
}

impl SecContactSource for NewsTracking {
    fn sec_contact(&self) -> ContactFuture<'_> {
        Box::pin(async move {
            match self.inputs.get() {
                Some(inputs) => inputs.sec_contact().await,
                None => None,
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchNameRole {
    Official,
    Alias,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchNameClarity {
    Distinct,
    NeedsContext,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchName {
    pub instrument: InstrumentId,
    pub name: String,
    pub role: MatchNameRole,
    pub clarity: MatchNameClarity,
}

fn official(instrument: &InstrumentId, name: &str) -> MatchName {
    MatchName {
        instrument: instrument.clone(),
        name: name.trim().to_owned(),
        role: MatchNameRole::Official,
        clarity: if name.trim().chars().count() <= SHORT_NAME_CHARS {
            MatchNameClarity::NeedsContext
        } else {
            MatchNameClarity::Distinct
        },
    }
}

fn ticker(tracked: &TrackedInstrument, instrument: &InstrumentId) -> Option<MatchName> {
    let code = tracked.symbol.as_str();
    let usable = match tracked.market {
        KnownMarket::Kr => code.chars().count() == 6,
        KnownMarket::Us => {
            code.chars().count() >= MIN_TICKER_CHARS
                && code.chars().all(|character| character.is_ascii_uppercase())
        }
    };
    usable.then(|| MatchName {
        instrument: instrument.clone(),
        name: code.to_owned(),
        role: MatchNameRole::Official,
        clarity: MatchNameClarity::Distinct,
    })
}

#[must_use]
pub fn match_names(
    tracked: &[TrackedInstrument],
    aliases: &[StockAlias],
    directory: &InstrumentDirectory,
) -> Vec<MatchName> {
    let mut names = Vec::new();
    for item in tracked {
        let Some(instrument) = item.link_id() else {
            continue;
        };
        names.push(official(&instrument, item.name.as_str()));
        names.extend(ticker(item, &instrument));
    }
    for alias in aliases {
        let Some(entry) = directory.by_instrument(&alias.instrument) else {
            continue;
        };
        let Ok(instrument) = InstrumentId::try_new(entry.master.row().symbol.as_str()) else {
            continue;
        };
        names.push(MatchName {
            instrument,
            name: alias.alias.as_str().to_owned(),
            role: MatchNameRole::Alias,
            clarity: MatchNameClarity::Distinct,
        });
    }
    names
}
