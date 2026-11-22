use crate::{InstrumentId, IsinCode, KnownMarket, Symbol};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentSource {
    market: KnownMarket,
    symbol: Symbol,
    isin: IsinCode,
}

impl InstrumentSource {
    #[must_use]
    pub const fn new(market: KnownMarket, symbol: Symbol, isin: IsinCode) -> Self {
        Self {
            market,
            symbol,
            isin,
        }
    }

    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
    }

    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    #[must_use]
    pub const fn isin(&self) -> &IsinCode {
        &self.isin
    }

    fn relation(&self, other: &Self) -> SourceRelation {
        if (self.market == other.market
            && self
                .symbol
                .as_str()
                .eq_ignore_ascii_case(other.symbol.as_str()))
            || self.isin.as_str().eq_ignore_ascii_case(other.isin.as_str())
        {
            SourceRelation::Uncertain
        } else {
            SourceRelation::Distinct
        }
    }
}

#[derive(PartialEq, Eq)]
enum SourceRelation {
    Uncertain,
    Distinct,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentMappingError {
    Missing,
    IdentityUncertain,
    Retired,
    IdAlreadyUsed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingAssignment {
    Issued(InstrumentId),
    Existing(InstrumentId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum MappingState {
    Active,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MappingEntry {
    source: InstrumentSource,
    id: InstrumentId,
    state: MappingState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstrumentMappings(Vec<MappingEntry>);

impl InstrumentMappings {
    pub fn resolve(
        &self,
        source: &InstrumentSource,
    ) -> Result<InstrumentId, InstrumentMappingError> {
        if let Some(entry) = self.0.iter().find(|entry| entry.source == *source) {
            return match entry.state {
                MappingState::Active => Ok(entry.id.clone()),
                MappingState::Retired => Err(InstrumentMappingError::Retired),
            };
        }
        if self
            .0
            .iter()
            .any(|entry| entry.source.relation(source) == SourceRelation::Uncertain)
        {
            return Err(InstrumentMappingError::IdentityUncertain);
        }
        Err(InstrumentMappingError::Missing)
    }

    pub fn ensure(
        &mut self,
        source: &InstrumentSource,
        candidate: &InstrumentId,
    ) -> Result<MappingAssignment, InstrumentMappingError> {
        match self.resolve(source) {
            Ok(id) => return Ok(MappingAssignment::Existing(id)),
            Err(InstrumentMappingError::Missing) => {}
            Err(error) => return Err(error),
        }
        if self.0.iter().any(|entry| entry.id == *candidate) {
            return Err(InstrumentMappingError::IdAlreadyUsed);
        }
        self.0.push(MappingEntry {
            source: source.clone(),
            id: candidate.clone(),
            state: MappingState::Active,
        });
        Ok(MappingAssignment::Issued(candidate.clone()))
    }

    pub fn retire(
        &mut self,
        source: &InstrumentSource,
    ) -> Result<InstrumentId, InstrumentMappingError> {
        let mut failure = InstrumentMappingError::Missing;
        for entry in &mut self.0 {
            if entry.source == *source {
                entry.state = MappingState::Retired;
                return Ok(entry.id.clone());
            }
            if entry.source.relation(source) == SourceRelation::Uncertain {
                failure = InstrumentMappingError::IdentityUncertain;
            }
        }
        Err(failure)
    }
}
