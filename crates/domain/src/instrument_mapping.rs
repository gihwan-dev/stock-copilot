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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentMappingStatus {
    Active,
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentMappingRecord {
    source: InstrumentSource,
    id: InstrumentId,
    state: InstrumentMappingStatus,
}

impl InstrumentMappingRecord {
    #[must_use]
    pub const fn new(
        source: InstrumentSource,
        id: InstrumentId,
        state: InstrumentMappingStatus,
    ) -> Self {
        Self { source, id, state }
    }

    #[must_use]
    pub const fn source(&self) -> &InstrumentSource {
        &self.source
    }

    #[must_use]
    pub const fn id(&self) -> &InstrumentId {
        &self.id
    }

    #[must_use]
    pub const fn status(&self) -> InstrumentMappingStatus {
        self.state
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstrumentMappings(Vec<InstrumentMappingRecord>);

impl InstrumentMappings {
    fn find_source(
        &self,
        source: &InstrumentSource,
    ) -> Result<&InstrumentMappingRecord, InstrumentMappingError> {
        if let Some(entry) = self.0.iter().find(|entry| entry.source == *source) {
            return Ok(entry);
        }
        for entry in &self.0 {
            match entry.source.relation(source) {
                SourceRelation::Uncertain => return Err(InstrumentMappingError::IdentityUncertain),
                SourceRelation::Distinct => {}
            }
        }
        Err(InstrumentMappingError::Missing)
    }

    pub fn resolve(
        &self,
        source: &InstrumentSource,
    ) -> Result<InstrumentId, InstrumentMappingError> {
        let entry = self.find_source(source)?;
        match entry.state {
            InstrumentMappingStatus::Active => Ok(entry.id.clone()),
            InstrumentMappingStatus::Retired => Err(InstrumentMappingError::Retired),
        }
    }

    pub fn lookup(
        &self,
        source: &InstrumentSource,
    ) -> Result<InstrumentMappingRecord, InstrumentMappingError> {
        self.find_source(source).cloned()
    }

    pub fn for_id(
        &self,
        id: &InstrumentId,
    ) -> Result<Vec<InstrumentMappingRecord>, InstrumentMappingError> {
        let records: Vec<_> = self
            .0
            .iter()
            .filter(|entry| entry.id == *id)
            .cloned()
            .collect();
        if records.is_empty() {
            return Err(InstrumentMappingError::Missing);
        }
        Ok(records)
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
        self.0.push(InstrumentMappingRecord::new(
            source.clone(),
            candidate.clone(),
            InstrumentMappingStatus::Active,
        ));
        Ok(MappingAssignment::Issued(candidate.clone()))
    }

    pub fn retire(
        &mut self,
        source: &InstrumentSource,
    ) -> Result<InstrumentId, InstrumentMappingError> {
        let mut failure = InstrumentMappingError::Missing;
        for entry in &mut self.0 {
            if entry.source == *source {
                entry.state = InstrumentMappingStatus::Retired;
                return Ok(entry.id.clone());
            }
            match entry.source.relation(source) {
                SourceRelation::Uncertain => failure = InstrumentMappingError::IdentityUncertain,
                SourceRelation::Distinct => {}
            }
        }
        Err(failure)
    }
}
