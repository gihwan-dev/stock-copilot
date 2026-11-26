use std::collections::BTreeMap;
use std::sync::Arc;

use application::{Clock, MarketInfo, PortError};
use domain::{
    KnownMarket, MasterInstrument, SectorAssignment, SectorAssignments, SectorCatalog,
    SectorComposition, SectorDepth, SectorId, Timestamp, sector_assignment_from_compositions,
};

use crate::ipc::state::BoxFuture;

const REFRESH_SECONDS: i64 = 86_400;

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
    at: Timestamp,
    catalog: SectorCatalog,
    compositions: Vec<SectorComposition>,
}

pub struct SectorClasses {
    feed: Arc<dyn SectorFeed>,
    clock: Arc<dyn Clock>,
    cache: tokio::sync::Mutex<BTreeMap<KnownMarket, Arc<Snapshot>>>,
}

impl std::fmt::Debug for SectorClasses {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SectorClasses")
    }
}

fn fresh(snapshot: &Snapshot, now: Timestamp) -> bool {
    now.utc_seconds()
        .as_i64()
        .saturating_sub(*snapshot.at.utc_seconds().as_i64())
        < REFRESH_SECONDS
}

impl SectorClasses {
    #[must_use]
    pub fn new(feed: Arc<dyn SectorFeed>, clock: Arc<dyn Clock>) -> Self {
        Self {
            feed,
            clock,
            cache: tokio::sync::Mutex::new(BTreeMap::new()),
        }
    }

    async fn snapshot(&self, market: KnownMarket, now: Timestamp) -> Option<Arc<Snapshot>> {
        let mut cache = self.cache.lock().await;
        if let Some(found) = cache.get(&market).filter(|found| fresh(found, now)) {
            return Some(Arc::clone(found));
        }
        let catalog = self.feed.catalog().await.ok()?;
        let mut compositions = Vec::new();
        for sector in catalog
            .sectors()
            .iter()
            .filter(|sector| sector.depth() == SectorDepth::Major)
        {
            if let Ok(found) = self.feed.composition(sector.id(), market).await {
                compositions.push(found);
            }
        }
        let snapshot = Arc::new(Snapshot {
            at: now,
            catalog,
            compositions,
        });
        let _previous = cache.insert(market, Arc::clone(&snapshot));
        Some(snapshot)
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

#[cfg(test)]
mod tests;
