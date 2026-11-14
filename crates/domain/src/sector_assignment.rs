use crate::{Market, ResolvedOrderInstrument, SectorId, Symbol};
use std::collections::BTreeMap;

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
    pub const fn market(&self) -> &Market {
        &self.market
    }
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentClassification {
    Sector(SectorId),
    EtfFund,
    Unclassified,
}
impl InstrumentClassification {
    pub(crate) fn allocation_sector(&self) -> AllocationSector {
        match self {
            Self::Sector(id) => AllocationSector::Classified(id.clone()),
            Self::EtfFund => AllocationSector::EtfFund,
            Self::Unclassified => AllocationSector::Unclassified,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorAssignment {
    instrument: AllocationInstrument,
    classification: InstrumentClassification,
}
impl SectorAssignment {
    #[must_use]
    pub fn new(
        instrument: &ResolvedOrderInstrument,
        classification: InstrumentClassification,
    ) -> Self {
        Self {
            instrument: AllocationInstrument::new(instrument),
            classification,
        }
    }
    #[must_use]
    pub const fn instrument(&self) -> &AllocationInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn classification(&self) -> &InstrumentClassification {
        &self.classification
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorAssignmentsError {
    DuplicateInstrument,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectorAssignments(BTreeMap<AllocationInstrument, InstrumentClassification>);
impl SectorAssignments {
    pub fn try_new(assignments: Vec<SectorAssignment>) -> Result<Self, SectorAssignmentsError> {
        let mut values = BTreeMap::new();
        for assignment in assignments {
            if values
                .insert(assignment.instrument, assignment.classification)
                .is_some()
            {
                return Err(SectorAssignmentsError::DuplicateInstrument);
            }
        }
        Ok(Self(values))
    }
    #[must_use]
    pub fn classification(
        &self,
        instrument: &AllocationInstrument,
    ) -> Option<&InstrumentClassification> {
        self.0.get(instrument)
    }
    pub(crate) fn sector(&self, instrument: &AllocationInstrument) -> AllocationSector {
        self.classification(instrument).map_or(
            AllocationSector::Unclassified,
            InstrumentClassification::allocation_sector,
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AllocationSector {
    Classified(SectorId),
    EtfFund,
    Cash,
    Unclassified,
}
