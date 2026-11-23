use crate::{InstrumentMappingError, InstrumentSource, MasterInstrument, MasterInstrumentFacts};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentMasterEntry {
    Available(MasterInstrument),
    Unavailable {
        source: InstrumentSource,
        reason: InstrumentMappingError,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentMasterError {
    DuplicateIdentity,
    InvalidEntry,
    UnavailableInstrument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentMaster(Vec<InstrumentMasterEntry>);

impl InstrumentMaster {
    pub fn try_new(entries: Vec<InstrumentMasterEntry>) -> Result<Self, InstrumentMasterError> {
        let mut identities = BTreeSet::new();
        let mut available = Vec::new();
        for entry in &entries {
            let (market, symbol) = identity(entry, &mut available)?;
            if !identities.insert((market, symbol)) {
                return Err(InstrumentMasterError::DuplicateIdentity);
            }
        }
        MasterInstrumentFacts::try_new(available)
            .map_err(|_error| InstrumentMasterError::DuplicateIdentity)?;
        Ok(Self(entries))
    }

    #[must_use]
    pub fn entries(&self) -> &[InstrumentMasterEntry] {
        &self.0
    }

    pub fn complete_facts(&self) -> Result<MasterInstrumentFacts, InstrumentMasterError> {
        let mut available = Vec::new();
        for entry in &self.0 {
            match entry {
                InstrumentMasterEntry::Available(instrument) => available.push(instrument.clone()),
                InstrumentMasterEntry::Unavailable { .. } => {
                    return Err(InstrumentMasterError::UnavailableInstrument);
                }
            }
        }
        MasterInstrumentFacts::try_new(available)
            .map_err(|_error| InstrumentMasterError::DuplicateIdentity)
    }
}

fn identity(
    entry: &InstrumentMasterEntry,
    available: &mut Vec<MasterInstrument>,
) -> Result<(crate::KnownMarket, crate::Symbol), InstrumentMasterError> {
    match entry {
        InstrumentMasterEntry::Available(instrument) => {
            let quote = instrument
                .quote_instrument()
                .map_err(|_error| InstrumentMasterError::InvalidEntry)?;
            available.push(instrument.clone());
            Ok((*quote.market(), quote.symbol().clone()))
        }
        InstrumentMasterEntry::Unavailable {
            reason: InstrumentMappingError::Missing,
            ..
        } => Err(InstrumentMasterError::InvalidEntry),
        InstrumentMasterEntry::Unavailable {
            source,
            reason:
                InstrumentMappingError::IdentityUncertain
                | InstrumentMappingError::Retired
                | InstrumentMappingError::IdAlreadyUsed,
        } => {
            let symbol = source.symbol().normalized_ascii();
            Ok((source.market(), symbol))
        }
    }
}
