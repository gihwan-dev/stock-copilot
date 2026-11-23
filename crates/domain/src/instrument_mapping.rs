use crate::instrument_mapping_change::{verify_change_evidence, verify_correction_evidence};
use crate::{
    ConfirmedIdentifierChange, ConfirmedLinkCorrection, ConfirmedRetirementCorrection, IdAdoption,
    InstrumentId, IsinCode, IssuedInstrumentId, KnownMarket, MappingHistoryEvent,
    MappingTransitionError, ReservationStatus, Symbol,
};
use std::collections::{BTreeMap, BTreeSet};

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

    fn symbol_key(&self) -> (KnownMarket, Symbol) {
        (self.market, self.symbol.normalized_ascii())
    }

    fn isin_key(&self) -> IsinCode {
        self.isin.normalized_ascii()
    }

    pub(crate) fn normalized_key(&self) -> (KnownMarket, Symbol, IsinCode) {
        (
            self.market,
            self.symbol.normalized_ascii(),
            self.isin.normalized_ascii(),
        )
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstrumentSourceConflict {
    Distinct,
    IdentityUncertain,
}

enum SourceGroup<'a> {
    Identical(&'a InstrumentSource),
    Conflict,
}

impl<'a> SourceGroup<'a> {
    fn observe(&mut self, source: &'a InstrumentSource) {
        match self {
            Self::Identical(first) if *first != source => *self = Self::Conflict,
            Self::Identical(_) | Self::Conflict => {}
        }
    }
}

#[must_use]
pub fn instrument_source_conflicts(sources: &[InstrumentSource]) -> Vec<InstrumentSourceConflict> {
    let mut symbols: BTreeMap<(KnownMarket, Symbol), SourceGroup<'_>> = BTreeMap::new();
    let mut isins: BTreeMap<IsinCode, SourceGroup<'_>> = BTreeMap::new();
    for source in sources {
        symbols
            .entry(source.symbol_key())
            .and_modify(|group| group.observe(source))
            .or_insert(SourceGroup::Identical(source));
        isins
            .entry(source.isin_key())
            .and_modify(|group| group.observe(source))
            .or_insert(SourceGroup::Identical(source));
    }
    sources
        .iter()
        .map(|source| {
            match (
                symbols.get(&source.symbol_key()),
                isins.get(&source.isin_key()),
            ) {
                (Some(SourceGroup::Conflict), _) | (_, Some(SourceGroup::Conflict)) => {
                    InstrumentSourceConflict::IdentityUncertain
                }
                (
                    Some(SourceGroup::Identical(_)) | None,
                    Some(SourceGroup::Identical(_)) | None,
                ) => InstrumentSourceConflict::Distinct,
            }
        })
        .collect()
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
    Superseded,
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
    Superseded,
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

    fn mark_retired(&mut self) -> Result<InstrumentId, InstrumentMappingError> {
        match self.state {
            InstrumentMappingStatus::Active | InstrumentMappingStatus::Retired => {
                self.state = InstrumentMappingStatus::Retired;
                Ok(self.id.clone())
            }
            InstrumentMappingStatus::Superseded => Err(InstrumentMappingError::Superseded),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveLink {
    instrument_id: InstrumentId,
    previous: InstrumentSource,
    next: InstrumentSource,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstrumentMappings {
    records: Vec<InstrumentMappingRecord>,
    reserved_ids: BTreeSet<InstrumentId>,
    active_links: Vec<ActiveLink>,
    history: Vec<MappingHistoryEvent>,
}

impl InstrumentMappings {
    fn find_source(
        &self,
        source: &InstrumentSource,
    ) -> Result<&InstrumentMappingRecord, InstrumentMappingError> {
        if let Some(entry) = self.records.iter().find(|entry| entry.source == *source) {
            return Ok(entry);
        }
        for entry in &self.records {
            match entry.source.relation(source) {
                SourceRelation::Uncertain => return Err(InstrumentMappingError::IdentityUncertain),
                SourceRelation::Distinct => {}
            }
        }
        Err(InstrumentMappingError::Missing)
    }

    fn find_owned_status(
        &self,
        source: &InstrumentSource,
        expected_id: &InstrumentId,
    ) -> Result<InstrumentMappingStatus, MappingTransitionError> {
        if let Some(entry) = self.records.iter().find(|entry| entry.source == *source) {
            if entry.id != *expected_id {
                return Err(MappingTransitionError::SourceBoundToOtherId);
            }
            return Ok(entry.state);
        }
        for entry in &self.records {
            match entry.source.relation(source) {
                SourceRelation::Uncertain => return Err(MappingTransitionError::IdentityUncertain),
                SourceRelation::Distinct => {}
            }
        }
        Err(MappingTransitionError::Missing)
    }

    fn check_next_target(
        &self,
        next: &InstrumentSource,
        target_id: &InstrumentId,
    ) -> Result<(), MappingTransitionError> {
        if let Some(existing) = self.records.iter().find(|entry| entry.source == *next) {
            if existing.id != *target_id {
                return Err(MappingTransitionError::SourceBoundToOtherId);
            }
            return Err(MappingTransitionError::SourceAlreadyRecorded);
        }
        let next_key = next.normalized_key();
        if self
            .records
            .iter()
            .any(|entry| entry.id == *target_id && entry.source.normalized_key() == next_key)
        {
            return Err(MappingTransitionError::SourceAlreadyRecorded);
        }
        for entry in &self.records {
            if entry.id == *target_id {
                continue;
            }
            match entry.source.relation(next) {
                SourceRelation::Uncertain => {
                    return Err(MappingTransitionError::IdentityUncertain);
                }
                SourceRelation::Distinct => {}
            }
        }
        Ok(())
    }

    pub fn resolve(
        &self,
        source: &InstrumentSource,
    ) -> Result<InstrumentId, InstrumentMappingError> {
        let entry = self.find_source(source)?;
        match entry.state {
            InstrumentMappingStatus::Active => Ok(entry.id.clone()),
            InstrumentMappingStatus::Superseded => Err(InstrumentMappingError::Superseded),
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
            .records
            .iter()
            .filter(|entry| entry.id == *id)
            .cloned()
            .collect();
        if records.is_empty() {
            return Err(InstrumentMappingError::Missing);
        }
        Ok(records)
    }

    pub fn adopt_existing_id(&mut self, id: InstrumentId) -> IdAdoption {
        if self.reserved_ids.insert(id) {
            IdAdoption::Adopted
        } else {
            IdAdoption::AlreadyKnown
        }
    }

    #[must_use]
    pub fn id_reservation(&self, id: &InstrumentId) -> ReservationStatus {
        if self.reserved_ids.contains(id) {
            ReservationStatus::Reserved
        } else {
            ReservationStatus::Available
        }
    }

    pub fn ensure(
        &mut self,
        source: &InstrumentSource,
        candidate: IssuedInstrumentId,
    ) -> Result<MappingAssignment, InstrumentMappingError> {
        match self.resolve(source) {
            Ok(id) => return Ok(MappingAssignment::Existing(id)),
            Err(InstrumentMappingError::Missing) => {}
            Err(error) => return Err(error),
        }
        match self.id_reservation(candidate.as_instrument_id()) {
            ReservationStatus::Reserved => return Err(InstrumentMappingError::IdAlreadyUsed),
            ReservationStatus::Available => {}
        }
        let issued = candidate.into_instrument_id();
        self.reserved_ids.insert(issued.clone());
        self.records.push(InstrumentMappingRecord::new(
            source.clone(),
            issued.clone(),
            InstrumentMappingStatus::Active,
        ));
        Ok(MappingAssignment::Issued(issued))
    }

    pub fn retire(
        &mut self,
        source: &InstrumentSource,
    ) -> Result<InstrumentId, InstrumentMappingError> {
        let mut failure = InstrumentMappingError::Missing;
        for entry in &mut self.records {
            if entry.source == *source {
                return entry.mark_retired();
            }
            match entry.source.relation(source) {
                SourceRelation::Uncertain => failure = InstrumentMappingError::IdentityUncertain,
                SourceRelation::Distinct => {}
            }
        }
        Err(failure)
    }

    pub fn link_confirmed_change(
        &mut self,
        change: &ConfirmedIdentifierChange,
    ) -> Result<InstrumentId, MappingTransitionError> {
        let evidence = verify_change_evidence(change)?;
        match self.find_owned_status(change.previous(), change.instrument_id())? {
            InstrumentMappingStatus::Active => {}
            InstrumentMappingStatus::Superseded | InstrumentMappingStatus::Retired => {
                return Err(MappingTransitionError::SourceNotActive);
            }
        }
        self.check_next_target(change.next(), change.instrument_id())?;
        for entry in &mut self.records {
            if entry.source == *change.previous() {
                entry.state = InstrumentMappingStatus::Superseded;
            }
        }
        self.records.push(InstrumentMappingRecord::new(
            change.next().clone(),
            change.instrument_id().clone(),
            InstrumentMappingStatus::Active,
        ));
        self.active_links.push(ActiveLink {
            instrument_id: change.instrument_id().clone(),
            previous: change.previous().clone(),
            next: change.next().clone(),
        });
        self.history.push(MappingHistoryEvent::IdentifierChanged {
            instrument_id: change.instrument_id().clone(),
            previous: change.previous().clone(),
            next: change.next().clone(),
            disclosure: evidence.disclosure.clone(),
            action: Box::new(evidence.action.clone()),
            note: evidence.note.clone(),
            confirmed_at: evidence.confirmed_at,
        });
        Ok(change.instrument_id().clone())
    }

    pub fn correct_retirement(
        &mut self,
        correction: &ConfirmedRetirementCorrection,
    ) -> Result<InstrumentId, MappingTransitionError> {
        let evidence = verify_correction_evidence(correction.confirmation())?;
        match self.find_owned_status(correction.source(), correction.instrument_id())? {
            InstrumentMappingStatus::Retired => {}
            InstrumentMappingStatus::Active | InstrumentMappingStatus::Superseded => {
                return Err(MappingTransitionError::SourceNotRetired);
            }
        }
        for entry in &mut self.records {
            if entry.source == *correction.source() {
                entry.state = InstrumentMappingStatus::Active;
            }
        }
        self.history.push(MappingHistoryEvent::RetirementCorrected {
            instrument_id: correction.instrument_id().clone(),
            source: correction.source().clone(),
            disclosure: evidence.disclosure.clone(),
            note: evidence.note.clone(),
            corrected_at: evidence.corrected_at,
        });
        Ok(correction.instrument_id().clone())
    }

    pub fn correct_link(
        &mut self,
        correction: &ConfirmedLinkCorrection,
    ) -> Result<InstrumentId, MappingTransitionError> {
        let evidence = verify_correction_evidence(correction.confirmation())?;
        if correction.previous() == correction.mistaken_next() {
            return Err(MappingTransitionError::UnchangedSource);
        }
        let _prev_status =
            self.find_owned_status(correction.previous(), correction.instrument_id())?;
        let next_status =
            self.find_owned_status(correction.mistaken_next(), correction.instrument_id())?;
        let expected = ActiveLink {
            instrument_id: correction.instrument_id().clone(),
            previous: correction.previous().clone(),
            next: correction.mistaken_next().clone(),
        };
        let matches_latest = self
            .active_links
            .iter()
            .rev()
            .filter(|link| link.instrument_id == *correction.instrument_id())
            .take(1)
            .any(|latest| *latest == expected);
        if !matches_latest || next_status != InstrumentMappingStatus::Active {
            return Err(MappingTransitionError::LinkNotLatest);
        }
        self.active_links.retain(|link| *link != expected);
        for entry in &mut self.records {
            if entry.source == *correction.previous() {
                entry.state = InstrumentMappingStatus::Active;
            }
        }
        self.records
            .retain(|entry| entry.source != *correction.mistaken_next());
        self.history.push(MappingHistoryEvent::LinkCorrected {
            instrument_id: correction.instrument_id().clone(),
            restored_previous: correction.previous().clone(),
            unlinked_next: correction.mistaken_next().clone(),
            disclosure: evidence.disclosure.clone(),
            note: evidence.note.clone(),
            corrected_at: evidence.corrected_at,
        });
        Ok(correction.instrument_id().clone())
    }

    #[must_use]
    pub fn history(&self) -> &[MappingHistoryEvent] {
        &self.history
    }

    #[must_use]
    pub fn history_for_id(&self, id: &InstrumentId) -> Vec<MappingHistoryEvent> {
        self.history
            .iter()
            .filter(|event| event.instrument_id() == id)
            .cloned()
            .collect()
    }
}
