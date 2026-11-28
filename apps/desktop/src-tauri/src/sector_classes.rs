use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tokio::sync::watch;

use application::{Clock, MarketInfo, PortError};
use domain::{
    DisplayInstrumentName, InstrumentClassification, KnownMarket, MasterInstrument,
    SectorAssignment, SectorAssignmentReason, SectorAssignments, SectorCatalog, SectorComposition,
    SectorDepth, SectorId, Timestamp, sector_assignment_from_compositions,
    sector_assignment_reason,
};

use crate::ipc::state::BoxFuture;

const REFRESH_SECONDS: i64 = 86_400;
const RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(60);

pub trait SectorFeed: Send + Sync {
    fn catalog(&self) -> BoxFuture<'_, Result<SectorCatalog, PortError>>;
    fn composition<'a>(
        &'a self,
        sector: &'a SectorId,
        market: KnownMarket,
    ) -> BoxFuture<'a, Result<SectorComposition, PortError>>;
}

pub struct MarketSectorFeed<M>(pub Arc<M>);

impl<M> std::fmt::Debug for MarketSectorFeed<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MarketSectorFeed")
    }
}

impl<M: MarketInfo + 'static> SectorFeed for MarketSectorFeed<M> {
    fn catalog(&self) -> BoxFuture<'_, Result<SectorCatalog, PortError>> {
        Box::pin(self.0.sectors())
    }

    fn composition<'a>(
        &'a self,
        sector: &'a SectorId,
        market: KnownMarket,
    ) -> BoxFuture<'a, Result<SectorComposition, PortError>> {
        Box::pin(self.0.sector_composition(sector, market))
    }
}

struct Snapshot {
    coverage: CatalogCoverage,
    catalog: SectorCatalog,
    compositions: Vec<SectorComposition>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CatalogCoverage {
    Complete,
    Partial,
}

#[derive(Clone)]
enum Lookup {
    Pending,
    Ready(Arc<Snapshot>),
    Failed(Timestamp),
}

struct CachedLookup {
    started: Timestamp,
    result: watch::Receiver<Lookup>,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, serde::Serialize, specta::Type)]
pub enum SectorRefresh {
    #[serde(rename = "pending")]
    Pending,
    #[default]
    #[serde(rename = "settled")]
    Settled,
}

#[derive(Default)]
pub(crate) struct HoldingSectors {
    pub(crate) assignments: SectorAssignments,
    pub(crate) names: BTreeMap<SectorId, DisplayInstrumentName>,
    pub(crate) refresh: SectorRefresh,
}

pub struct SectorClasses {
    feed: Arc<dyn SectorFeed>,
    clock: Arc<dyn Clock>,
    cache: RwLock<BTreeMap<KnownMarket, CachedLookup>>,
}

impl std::fmt::Debug for SectorClasses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SectorClasses")
    }
}

async fn fetch(feed: &dyn SectorFeed, market: KnownMarket) -> Option<Snapshot> {
    let catalog = feed.catalog().await.ok()?;
    let mut compositions = Vec::new();
    let mut coverage = CatalogCoverage::Complete;
    for sector in catalog
        .sectors()
        .iter()
        .filter(|row| row.depth() == SectorDepth::Major)
    {
        if let Ok(composition) = feed.composition(sector.id(), market).await {
            compositions.push(composition);
        } else {
            coverage = CatalogCoverage::Partial;
        }
    }
    Some(Snapshot {
        coverage,
        catalog,
        compositions,
    })
}

impl SectorClasses {
    #[must_use]
    pub fn new(feed: Arc<dyn SectorFeed>, clock: Arc<dyn Clock>) -> Self {
        Self {
            feed,
            clock,
            cache: RwLock::new(BTreeMap::new()),
        }
    }

    fn lookup(
        &self,
        market: KnownMarket,
        now: Timestamp,
        retry_after_failure: std::time::Duration,
    ) -> watch::Receiver<Lookup> {
        let mut cache = match self.cache.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Some(cached) = cache.get(&market) {
            let elapsed = now
                .utc_seconds()
                .as_i64()
                .saturating_sub(*cached.started.utc_seconds().as_i64());
            let reuse = match &*cached.result.borrow() {
                Lookup::Pending => true,
                Lookup::Ready(snapshot) => match snapshot.coverage {
                    CatalogCoverage::Complete => elapsed < REFRESH_SECONDS,
                    CatalogCoverage::Partial => elapsed < RETRY_DELAY.as_secs().cast_signed(),
                },
                Lookup::Failed(at) => {
                    now.utc_seconds()
                        .as_i64()
                        .saturating_sub(*at.utc_seconds().as_i64())
                        < retry_after_failure.as_secs().cast_signed()
                }
            };
            if reuse {
                return cached.result.clone();
            }
        }
        let (sender, result) = watch::channel(Lookup::Pending);
        let feed = Arc::clone(&self.feed);
        let clock = Arc::clone(&self.clock);
        tauri::async_runtime::spawn(async move {
            let response = fetch(feed.as_ref(), market).await.map_or_else(
                || Lookup::Failed(clock.now()),
                |value| Lookup::Ready(Arc::new(value)),
            );
            sender.send_replace(response);
        });
        let _previous = cache.insert(
            market,
            CachedLookup {
                started: now,
                result: result.clone(),
            },
        );
        result
    }

    fn current(&self, market: KnownMarket) -> Lookup {
        self.lookup(market, self.clock.now(), RETRY_DELAY)
            .borrow()
            .clone()
    }

    async fn snapshot(&self, market: KnownMarket, now: Timestamp) -> Option<Arc<Snapshot>> {
        let mut response = self.lookup(market, now, std::time::Duration::ZERO);
        response
            .wait_for(|current| match current {
                Lookup::Pending => false,
                Lookup::Ready(_) | Lookup::Failed(_) => true,
            })
            .await
            .ok()
            .and_then(|state| match &*state {
                Lookup::Ready(snapshot) => Some(Arc::clone(snapshot)),
                Lookup::Pending | Lookup::Failed(_) => None,
            })
    }

    pub async fn assignments(&self, masters: &[MasterInstrument]) -> Option<SectorAssignments> {
        let now = self.clock.now();
        let mut found: Vec<SectorAssignment> = Vec::new();
        for master in masters {
            let Ok(quote) = master.quote_instrument() else {
                continue;
            };
            let assignment = match self.snapshot(*quote.market(), now).await {
                Some(snapshot) => sector_assignment_from_compositions(
                    master,
                    &snapshot.catalog,
                    &snapshot.compositions,
                ),
                None => SectorCatalog::from_response(Vec::new(), now).map_or_else(
                    |_error| Err(domain::SectorSelectionError::InvalidCatalog),
                    |empty| sector_assignment_from_compositions(master, &empty, &[]),
                ),
            };
            if let Ok(value) = assignment
                && !found
                    .iter()
                    .any(|known| known.instrument() == value.instrument())
            {
                found.push(value);
            }
        }
        SectorAssignments::try_new(found).ok()
    }
}

impl HoldingSectors {
    fn select(
        &mut self,
        master: &MasterInstrument,
        state: &Lookup,
        fallback: SectorAssignment,
    ) -> SectorAssignment {
        let snapshot = match state {
            Lookup::Failed(_) => return fallback,
            Lookup::Pending => {
                self.refresh = SectorRefresh::Pending;
                return fallback;
            }
            Lookup::Ready(snapshot) => snapshot,
        };
        let Ok(classified) =
            sector_assignment_from_compositions(master, &snapshot.catalog, &snapshot.compositions)
        else {
            return fallback;
        };
        if let InstrumentClassification::Sector(id) = classified.classification()
            && let Some(row) = snapshot.catalog.sectors().iter().find(|row| row.id() == id)
        {
            self.names.insert(id.clone(), row.name().clone());
        }
        classified
    }

    pub(crate) fn read(
        masters: &[MasterInstrument],
        source: Option<&SectorClasses>,
        now: Timestamp,
    ) -> Option<Self> {
        let mut result = Self::default();
        let empty = SectorCatalog::from_response(Vec::new(), now).ok()?;
        let mut assignments = Vec::new();
        let mut lookups = BTreeMap::new();
        for master in masters {
            let Ok(fallback) = sector_assignment_from_compositions(master, &empty, &[]) else {
                continue;
            };
            let value = if sector_assignment_reason(&fallback)
                == Some(SectorAssignmentReason::NoMajorMembership)
                && let Some(service) = source
                && let Ok(quote) = master.quote_instrument()
            {
                let market = *quote.market();
                let state = lookups
                    .entry(market)
                    .or_insert_with(|| service.current(market));
                result.select(master, state, fallback)
            } else {
                fallback
            };
            assignments.push(value);
        }
        result.assignments = SectorAssignments::try_new(assignments).ok()?;
        Some(result)
    }
}

#[cfg(test)]
mod tests;
