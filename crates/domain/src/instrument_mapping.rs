use crate::instrument_mapping_change::{
    VerifiedChange, VerifiedLinkCorrection, VerifiedRetirementCorrection, validate_change_action,
    verify_change_evidence, verify_link_correction, verify_retirement_correction,
};
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

fn find_owned_status_in(
    records: &[InstrumentMappingRecord],
    source: &InstrumentSource,
    expected_id: &InstrumentId,
) -> Result<InstrumentMappingStatus, MappingTransitionError> {
    if let Some(entry) = records.iter().find(|entry| entry.source == *source) {
        if entry.id != *expected_id {
            return Err(MappingTransitionError::SourceBoundToOtherId);
        }
        return Ok(entry.state);
    }
    for entry in records {
        match entry.source.relation(source) {
            SourceRelation::Uncertain => return Err(MappingTransitionError::IdentityUncertain),
            SourceRelation::Distinct => {}
        }
    }
    Err(MappingTransitionError::Missing)
}

fn check_next_target_in(
    records: &[InstrumentMappingRecord],
    next: &InstrumentSource,
    target_id: &InstrumentId,
) -> Result<(), MappingTransitionError> {
    if let Some(existing) = records.iter().find(|entry| entry.source == *next) {
        if existing.id != *target_id {
            return Err(MappingTransitionError::SourceBoundToOtherId);
        }
        return Err(MappingTransitionError::SourceAlreadyRecorded);
    }
    let next_key = next.normalized_key();
    if records
        .iter()
        .any(|entry| entry.id == *target_id && entry.source.normalized_key() == next_key)
    {
        return Err(MappingTransitionError::SourceAlreadyRecorded);
    }
    for entry in records {
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

fn latest_active_link_in<'a>(
    history: &'a [MappingHistoryEvent],
    id: &InstrumentId,
) -> Option<(&'a InstrumentSource, &'a InstrumentSource)> {
    let mut stack = Vec::new();
    for event in history {
        match event {
            MappingHistoryEvent::IdentifierChanged {
                instrument_id,
                previous,
                next,
                ..
            } if instrument_id == id => stack.push((previous, next)),
            MappingHistoryEvent::LinkCorrected { instrument_id, .. } if instrument_id == id => {
                let _popped = stack.pop();
            }
            MappingHistoryEvent::IdentifierChanged { .. }
            | MappingHistoryEvent::RetirementCorrected { .. }
            | MappingHistoryEvent::LinkCorrected { .. } => {}
        }
    }
    stack.pop()
}

fn plan_change_inner(
    related_records: &[InstrumentMappingRecord],
    change: ConfirmedIdentifierChange,
) -> Result<VerifiedChange, MappingTransitionError> {
    let verified = verify_change_evidence(change)?;
    match find_owned_status_in(related_records, &verified.previous, &verified.instrument_id)? {
        InstrumentMappingStatus::Active => {}
        InstrumentMappingStatus::Superseded | InstrumentMappingStatus::Retired => {
            return Err(MappingTransitionError::SourceNotActive);
        }
    }
    check_next_target_in(related_records, &verified.next, &verified.instrument_id)?;
    Ok(verified)
}

fn plan_retirement_inner(
    related_records: &[InstrumentMappingRecord],
    correction: ConfirmedRetirementCorrection,
) -> Result<VerifiedRetirementCorrection, MappingTransitionError> {
    let verified = verify_retirement_correction(correction)?;
    match find_owned_status_in(related_records, &verified.source, &verified.instrument_id)? {
        InstrumentMappingStatus::Retired => {}
        InstrumentMappingStatus::Active | InstrumentMappingStatus::Superseded => {
            return Err(MappingTransitionError::SourceNotRetired);
        }
    }
    Ok(verified)
}

fn plan_link_inner(
    related_records: &[InstrumentMappingRecord],
    history: &[MappingHistoryEvent],
    correction: ConfirmedLinkCorrection,
) -> Result<VerifiedLinkCorrection, MappingTransitionError> {
    let verified = verify_link_correction(correction)?;
    if verified.previous == verified.mistaken_next {
        return Err(MappingTransitionError::UnchangedSource);
    }
    let prev_status =
        find_owned_status_in(related_records, &verified.previous, &verified.instrument_id)?;
    let next_status = find_owned_status_in(
        related_records,
        &verified.mistaken_next,
        &verified.instrument_id,
    )?;
    let matches_latest = latest_active_link_in(history, &verified.instrument_id)
        == Some((&verified.previous, &verified.mistaken_next));
    match (prev_status, next_status) {
        (InstrumentMappingStatus::Superseded, InstrumentMappingStatus::Active)
            if matches_latest => {}
        (
            InstrumentMappingStatus::Active
            | InstrumentMappingStatus::Superseded
            | InstrumentMappingStatus::Retired,
            InstrumentMappingStatus::Active
            | InstrumentMappingStatus::Superseded
            | InstrumentMappingStatus::Retired,
        ) => {
            return Err(MappingTransitionError::LinkNotLatest);
        }
    }
    Ok(verified)
}

pub fn plan_confirmed_identifier_change(
    related_records: &[InstrumentMappingRecord],
    change: ConfirmedIdentifierChange,
) -> Result<MappingHistoryEvent, MappingTransitionError> {
    let verified = plan_change_inner(related_records, change)?;
    Ok(MappingHistoryEvent::IdentifierChanged {
        instrument_id: verified.instrument_id,
        previous: verified.previous,
        next: verified.next,
        disclosure: verified.disclosure,
        action: Box::new(verified.action),
        note: verified.note,
        confirmed_at: verified.confirmed_at,
    })
}

pub fn plan_confirmed_retirement_correction(
    related_records: &[InstrumentMappingRecord],
    correction: ConfirmedRetirementCorrection,
) -> Result<MappingHistoryEvent, MappingTransitionError> {
    let verified = plan_retirement_inner(related_records, correction)?;
    Ok(MappingHistoryEvent::RetirementCorrected {
        instrument_id: verified.instrument_id,
        source: verified.source,
        disclosure: verified.disclosure,
        note: verified.note,
        corrected_at: verified.corrected_at,
    })
}

pub fn plan_confirmed_link_correction(
    related_records: &[InstrumentMappingRecord],
    history: &[MappingHistoryEvent],
    correction: ConfirmedLinkCorrection,
) -> Result<MappingHistoryEvent, MappingTransitionError> {
    let verified = plan_link_inner(related_records, history, correction)?;
    Ok(MappingHistoryEvent::LinkCorrected {
        instrument_id: verified.instrument_id,
        restored_previous: verified.previous,
        unlinked_next: verified.mistaken_next,
        disclosure: verified.disclosure,
        note: verified.note,
        corrected_at: verified.corrected_at,
    })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstrumentMappings {
    records: Vec<InstrumentMappingRecord>,
    reserved_ids: BTreeSet<InstrumentId>,
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
        change: ConfirmedIdentifierChange,
    ) -> Result<InstrumentId, MappingTransitionError> {
        let verified = plan_change_inner(&self.records, change)?;
        for entry in &mut self.records {
            if entry.source == verified.previous {
                entry.state = InstrumentMappingStatus::Superseded;
            }
        }
        self.records.push(InstrumentMappingRecord::new(
            verified.next.clone(),
            verified.instrument_id.clone(),
            InstrumentMappingStatus::Active,
        ));
        let result_id = verified.instrument_id.clone();
        self.history.push(MappingHistoryEvent::IdentifierChanged {
            instrument_id: verified.instrument_id,
            previous: verified.previous,
            next: verified.next,
            disclosure: verified.disclosure,
            action: Box::new(verified.action),
            note: verified.note,
            confirmed_at: verified.confirmed_at,
        });
        Ok(result_id)
    }

    pub fn correct_retirement(
        &mut self,
        correction: ConfirmedRetirementCorrection,
    ) -> Result<InstrumentId, MappingTransitionError> {
        let verified = plan_retirement_inner(&self.records, correction)?;
        for entry in &mut self.records {
            if entry.source == verified.source {
                entry.state = InstrumentMappingStatus::Active;
            }
        }
        let result_id = verified.instrument_id.clone();
        self.history.push(MappingHistoryEvent::RetirementCorrected {
            instrument_id: verified.instrument_id,
            source: verified.source,
            disclosure: verified.disclosure,
            note: verified.note,
            corrected_at: verified.corrected_at,
        });
        Ok(result_id)
    }

    pub fn correct_link(
        &mut self,
        correction: ConfirmedLinkCorrection,
    ) -> Result<InstrumentId, MappingTransitionError> {
        let verified = plan_link_inner(&self.records, &self.history, correction)?;
        for entry in &mut self.records {
            if entry.source == verified.previous {
                entry.state = InstrumentMappingStatus::Active;
            }
        }
        self.records
            .retain(|entry| entry.source != verified.mistaken_next);
        let result_id = verified.instrument_id.clone();
        self.history.push(MappingHistoryEvent::LinkCorrected {
            instrument_id: verified.instrument_id,
            restored_previous: verified.previous,
            unlinked_next: verified.mistaken_next,
            disclosure: verified.disclosure,
            note: verified.note,
            corrected_at: verified.corrected_at,
        });
        Ok(result_id)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingLedgerIntegrityError {
    DuplicateRecord,
    OverlappingSources,
    MultipleTerminalRecordsForId,
    OrphanSupersededRecord,
    InvalidEventEvidence,
    EventReplayMismatch,
    LedgerStateMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceConflictCheck {
    Clear,
    Conflicting,
}

struct OwnerSourceEntries {
    owner: InstrumentId,
    first_source: (KnownMarket, Symbol, IsinCode),
}

impl OwnerSourceEntries {
    fn new(owner: &InstrumentId, source: &InstrumentSource) -> Self {
        Self {
            owner: owner.clone(),
            first_source: source.normalized_key(),
        }
    }

    fn has_other_owner(&self, candidate: &InstrumentId) -> SourceConflictCheck {
        if self.owner == *candidate {
            SourceConflictCheck::Clear
        } else {
            SourceConflictCheck::Conflicting
        }
    }

    fn remove(&self, source: &InstrumentSource) -> EntryPresence {
        if self.first_source == source.normalized_key() {
            EntryPresence::Empty
        } else {
            EntryPresence::Occupied
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryPresence {
    Empty,
    Occupied,
}

#[derive(Default)]
struct ReplaySourceIndex {
    symbols: BTreeMap<(KnownMarket, Symbol), OwnerSourceEntries>,
    isins: BTreeMap<IsinCode, OwnerSourceEntries>,
}

impl ReplaySourceIndex {
    fn conflict_with_other_id(
        &self,
        owner: &InstrumentId,
        source: &InstrumentSource,
    ) -> SourceConflictCheck {
        let symbol_conflict = self
            .symbols
            .get(&source.symbol_key())
            .map_or(SourceConflictCheck::Clear, |entry| {
                entry.has_other_owner(owner)
            });
        let isin_conflict = self
            .isins
            .get(&source.isin_key())
            .map_or(SourceConflictCheck::Clear, |entry| {
                entry.has_other_owner(owner)
            });
        match (symbol_conflict, isin_conflict) {
            (SourceConflictCheck::Clear, SourceConflictCheck::Clear) => SourceConflictCheck::Clear,
            _ => SourceConflictCheck::Conflicting,
        }
    }

    fn insert(&mut self, owner: &InstrumentId, source: &InstrumentSource) {
        self.symbols
            .entry(source.symbol_key())
            .or_insert_with(|| OwnerSourceEntries::new(owner, source));
        self.isins
            .entry(source.isin_key())
            .or_insert_with(|| OwnerSourceEntries::new(owner, source));
    }

    fn remove(&mut self, source: &InstrumentSource) {
        let symbol_key = source.symbol_key();
        match self
            .symbols
            .get(&symbol_key)
            .map(|entry| entry.remove(source))
        {
            Some(EntryPresence::Empty) => {
                self.symbols.remove(&symbol_key);
            }
            Some(EntryPresence::Occupied) | None => {}
        }
        let isin_key = source.isin_key();
        match self.isins.get(&isin_key).map(|entry| entry.remove(source)) {
            Some(EntryPresence::Empty) => {
                self.isins.remove(&isin_key);
            }
            Some(EntryPresence::Occupied) | None => {}
        }
    }
}

struct ReplayIdState {
    superseded: Vec<InstrumentSource>,
    tip: InstrumentSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NormalizedMembership {
    Present,
    Absent,
}

impl ReplayIdState {
    fn superseded_contains_normalized(&self, candidate: &InstrumentSource) -> NormalizedMembership {
        let candidate_key = candidate.normalized_key();
        if self
            .superseded
            .iter()
            .any(|entry| entry.normalized_key() == candidate_key)
        {
            NormalizedMembership::Present
        } else {
            NormalizedMembership::Absent
        }
    }
}

#[derive(Default)]
struct ReplayLedger {
    live_by_id: BTreeMap<InstrumentId, ReplayIdState>,
    live_index: ReplaySourceIndex,
}

impl ReplayLedger {
    fn apply_identifier_changed(
        &mut self,
        instrument_id: &InstrumentId,
        previous: &InstrumentSource,
        next: &InstrumentSource,
        action: &crate::CorporateAction,
    ) -> Result<(), MappingLedgerIntegrityError> {
        if previous.normalized_key() == next.normalized_key()
            || previous.market() != next.market()
            || validate_change_action(instrument_id, previous, next, action).is_err()
        {
            return Err(MappingLedgerIntegrityError::InvalidEventEvidence);
        }
        if let Some(state) = self.live_by_id.get(instrument_id) {
            if state.tip != *previous {
                return Err(MappingLedgerIntegrityError::EventReplayMismatch);
            }
            match state.superseded_contains_normalized(next) {
                NormalizedMembership::Present => {
                    return Err(MappingLedgerIntegrityError::EventReplayMismatch);
                }
                NormalizedMembership::Absent => {}
            }
        } else {
            match self
                .live_index
                .conflict_with_other_id(instrument_id, previous)
            {
                SourceConflictCheck::Conflicting => {
                    return Err(MappingLedgerIntegrityError::EventReplayMismatch);
                }
                SourceConflictCheck::Clear => {}
            }
        }
        match self.live_index.conflict_with_other_id(instrument_id, next) {
            SourceConflictCheck::Conflicting => {
                return Err(MappingLedgerIntegrityError::EventReplayMismatch);
            }
            SourceConflictCheck::Clear => {}
        }
        self.live_index.insert(instrument_id, previous);
        self.live_index.insert(instrument_id, next);
        match self.live_by_id.get_mut(instrument_id) {
            Some(state) => {
                state.superseded.push(previous.clone());
                state.tip = next.clone();
            }
            None => {
                self.live_by_id.insert(
                    instrument_id.clone(),
                    ReplayIdState {
                        superseded: vec![previous.clone()],
                        tip: next.clone(),
                    },
                );
            }
        }
        Ok(())
    }

    fn apply_retirement_corrected(
        &mut self,
        instrument_id: &InstrumentId,
        source: &InstrumentSource,
    ) -> Result<(), MappingLedgerIntegrityError> {
        if let Some(state) = self.live_by_id.get(instrument_id) {
            if state.tip != *source {
                return Err(MappingLedgerIntegrityError::EventReplayMismatch);
            }
            return Ok(());
        }
        match self
            .live_index
            .conflict_with_other_id(instrument_id, source)
        {
            SourceConflictCheck::Conflicting => {
                return Err(MappingLedgerIntegrityError::EventReplayMismatch);
            }
            SourceConflictCheck::Clear => {}
        }
        self.live_index.insert(instrument_id, source);
        self.live_by_id.insert(
            instrument_id.clone(),
            ReplayIdState {
                superseded: Vec::new(),
                tip: source.clone(),
            },
        );
        Ok(())
    }

    fn apply_link_corrected(
        &mut self,
        instrument_id: &InstrumentId,
        restored_previous: &InstrumentSource,
        unlinked_next: &InstrumentSource,
    ) -> Result<(), MappingLedgerIntegrityError> {
        if restored_previous.normalized_key() == unlinked_next.normalized_key() {
            return Err(MappingLedgerIntegrityError::InvalidEventEvidence);
        }
        let Some(state) = self.live_by_id.get_mut(instrument_id) else {
            return Err(MappingLedgerIntegrityError::EventReplayMismatch);
        };
        if state.tip != *unlinked_next {
            return Err(MappingLedgerIntegrityError::EventReplayMismatch);
        }
        let Some(previous_tip) = state.superseded.pop() else {
            return Err(MappingLedgerIntegrityError::EventReplayMismatch);
        };
        if previous_tip != *restored_previous {
            return Err(MappingLedgerIntegrityError::EventReplayMismatch);
        }
        state.tip = previous_tip;
        self.live_index.remove(unlinked_next);
        Ok(())
    }
}

fn validate_stored_records(
    records: &[InstrumentMappingRecord],
) -> Result<BTreeMap<&InstrumentId, Vec<&InstrumentMappingRecord>>, MappingLedgerIntegrityError> {
    let mut stored_by_id: BTreeMap<&InstrumentId, Vec<&InstrumentMappingRecord>> = BTreeMap::new();
    let mut seen_exact: BTreeSet<(KnownMarket, &Symbol, &IsinCode)> = BTreeSet::new();
    let mut seen_normalized_per_id: BTreeSet<(&InstrumentId, KnownMarket, Symbol, IsinCode)> =
        BTreeSet::new();
    let mut id_market: BTreeMap<&InstrumentId, KnownMarket> = BTreeMap::new();
    let mut symbols: BTreeMap<(KnownMarket, Symbol), &InstrumentId> = BTreeMap::new();
    let mut isins: BTreeMap<IsinCode, &InstrumentId> = BTreeMap::new();

    for record in records {
        let id = record.id();
        let source = record.source();
        if !seen_exact.insert((source.market(), source.symbol(), source.isin())) {
            return Err(MappingLedgerIntegrityError::DuplicateRecord);
        }
        let (norm_market, norm_symbol, norm_isin) = source.normalized_key();
        if !seen_normalized_per_id.insert((id, norm_market, norm_symbol.clone(), norm_isin.clone()))
        {
            return Err(MappingLedgerIntegrityError::DuplicateRecord);
        }
        if let Some(existing_market) = id_market.insert(id, norm_market)
            && existing_market != norm_market
        {
            return Err(MappingLedgerIntegrityError::LedgerStateMismatch);
        }
        if let Some(existing_owner) = symbols.insert((norm_market, norm_symbol), id)
            && existing_owner != id
        {
            return Err(MappingLedgerIntegrityError::OverlappingSources);
        }
        if let Some(existing_owner) = isins.insert(norm_isin, id)
            && existing_owner != id
        {
            return Err(MappingLedgerIntegrityError::OverlappingSources);
        }
        stored_by_id.entry(id).or_default().push(record);
    }

    for id_records in stored_by_id.values() {
        let terminal_count = id_records
            .iter()
            .filter(|entry| match entry.status() {
                InstrumentMappingStatus::Active | InstrumentMappingStatus::Retired => true,
                InstrumentMappingStatus::Superseded => false,
            })
            .count();
        match terminal_count {
            0 => return Err(MappingLedgerIntegrityError::OrphanSupersededRecord),
            1 => {}
            _ => return Err(MappingLedgerIntegrityError::MultipleTerminalRecordsForId),
        }
    }

    Ok(stored_by_id)
}

fn replay_history(
    history: &[MappingHistoryEvent],
) -> Result<BTreeMap<InstrumentId, ReplayIdState>, MappingLedgerIntegrityError> {
    let mut ledger = ReplayLedger::default();

    for event in history {
        match event {
            MappingHistoryEvent::IdentifierChanged {
                instrument_id,
                previous,
                next,
                action,
                ..
            } => ledger.apply_identifier_changed(instrument_id, previous, next, action)?,
            MappingHistoryEvent::RetirementCorrected {
                instrument_id,
                source,
                ..
            } => ledger.apply_retirement_corrected(instrument_id, source)?,
            MappingHistoryEvent::LinkCorrected {
                instrument_id,
                restored_previous,
                unlinked_next,
                ..
            } => ledger.apply_link_corrected(instrument_id, restored_previous, unlinked_next)?,
        }
    }

    Ok(ledger.live_by_id)
}

fn verify_replayed_id_records(
    replayed: &ReplayIdState,
    stored: &[&InstrumentMappingRecord],
) -> Result<(), MappingLedgerIntegrityError> {
    if stored.len() != replayed.superseded.len().saturating_add(1) {
        return Err(MappingLedgerIntegrityError::LedgerStateMismatch);
    }
    for record in stored {
        let matched = match record.status() {
            InstrumentMappingStatus::Superseded => replayed.superseded.contains(record.source()),
            InstrumentMappingStatus::Active | InstrumentMappingStatus::Retired => {
                record.source() == &replayed.tip
            }
        };
        if !matched {
            return Err(MappingLedgerIntegrityError::LedgerStateMismatch);
        }
    }
    Ok(())
}

pub fn verify_persisted_mapping_ledger(
    records: &[InstrumentMappingRecord],
    history: &[MappingHistoryEvent],
) -> Result<(), MappingLedgerIntegrityError> {
    let stored_by_id = validate_stored_records(records)?;
    let replayed_by_id = replay_history(history)?;

    for (id, replayed) in &replayed_by_id {
        let Some(stored) = stored_by_id.get(id) else {
            return Err(MappingLedgerIntegrityError::LedgerStateMismatch);
        };
        verify_replayed_id_records(replayed, stored)?;
    }

    for (id, stored) in &stored_by_id {
        if !replayed_by_id.contains_key(*id)
            && stored.iter().any(|record| match record.status() {
                InstrumentMappingStatus::Superseded => true,
                InstrumentMappingStatus::Active | InstrumentMappingStatus::Retired => false,
            })
        {
            return Err(MappingLedgerIntegrityError::OrphanSupersededRecord);
        }
    }

    Ok(())
}
