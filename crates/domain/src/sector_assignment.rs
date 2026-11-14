use crate::{Market, ResolvedOrderInstrument, SectorId, Symbol};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AllocationInstrument {
    market: Market,
    symbol: Symbol,
}
impl AllocationInstrument {
    #[must_use]
    pub fn new(instrument: &ResolvedOrderInstrument) -> Self {
        Self {
            market: instrument.market(),
            symbol: instrument.symbol().clone(),
        }
    }
    #[must_use]
    pub fn market(&self) -> &Market {
        &self.market
    }
    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorAssignment {
    instrument: AllocationInstrument,
    sector: SectorId,
}
impl SectorAssignment {
    #[must_use]
    pub fn new(instrument: &ResolvedOrderInstrument, sector: SectorId) -> Self {
        Self {
            instrument: AllocationInstrument::new(instrument),
            sector,
        }
    }
    #[must_use]
    pub fn instrument(&self) -> &AllocationInstrument {
        &self.instrument
    }
    #[must_use]
    pub fn sector(&self) -> &SectorId {
        &self.sector
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AllocationSector {
    Classified(SectorId),
    Unclassified,
}
