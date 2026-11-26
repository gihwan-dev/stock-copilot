use crate::SystemPortError;
use domain::{DisplayInstrumentName, InstrumentId, KnownMarket, NormalizedUrl, Symbol};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedInstrument {
    pub symbol: Symbol,
    pub market: KnownMarket,
    pub name: DisplayInstrumentName,
}

impl TrackedInstrument {
    #[must_use]
    pub fn link_id(&self) -> Option<InstrumentId> {
        InstrumentId::try_new(self.symbol.as_str()).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectedPresence {
    Stored,
    New,
}

pub trait CollectedPresenceCheck: Send + Sync {
    fn presence(
        &self,
        url: &NormalizedUrl,
    ) -> impl Future<Output = Result<CollectedPresence, SystemPortError>> + Send;
}
