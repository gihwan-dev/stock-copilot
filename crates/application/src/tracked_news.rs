use domain::{DisplayInstrumentName, InstrumentId, KnownMarket, Symbol};

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
