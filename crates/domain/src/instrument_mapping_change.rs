use crate::{
    CorporateAction, CorporateActionKind, DisclosureKey, InstrumentId, InstrumentSource, Timestamp,
    UserReason,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityConfirmationOrigin {
    UserConfirmed,
    BrokerObservation,
    AiSuggestion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityChangeEvidence {
    None,
    NoteOnly(UserReason),
    DisclosureOnly {
        disclosure: DisclosureKey,
        note: UserReason,
    },
    ActionOnly {
        action: CorporateAction,
        note: UserReason,
    },
    DisclosureAndAction {
        disclosure: DisclosureKey,
        action: CorporateAction,
        note: UserReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityChangeConfirmation {
    origin: IdentityConfirmationOrigin,
    evidence: IdentityChangeEvidence,
    confirmed_at: Timestamp,
}

impl IdentityChangeConfirmation {
    #[must_use]
    pub const fn new(
        origin: IdentityConfirmationOrigin,
        evidence: IdentityChangeEvidence,
        confirmed_at: Timestamp,
    ) -> Self {
        Self {
            origin,
            evidence,
            confirmed_at,
        }
    }

    #[must_use]
    pub const fn origin(&self) -> IdentityConfirmationOrigin {
        self.origin
    }

    #[must_use]
    pub const fn evidence(&self) -> &IdentityChangeEvidence {
        &self.evidence
    }

    #[must_use]
    pub const fn confirmed_at(&self) -> Timestamp {
        self.confirmed_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedIdentifierChange {
    instrument_id: InstrumentId,
    previous: InstrumentSource,
    next: InstrumentSource,
    confirmation: IdentityChangeConfirmation,
}

impl ConfirmedIdentifierChange {
    #[must_use]
    pub const fn new(
        instrument_id: InstrumentId,
        previous: InstrumentSource,
        next: InstrumentSource,
        confirmation: IdentityChangeConfirmation,
    ) -> Self {
        Self {
            instrument_id,
            previous,
            next,
            confirmation,
        }
    }

    #[must_use]
    pub const fn instrument_id(&self) -> &InstrumentId {
        &self.instrument_id
    }

    #[must_use]
    pub const fn previous(&self) -> &InstrumentSource {
        &self.previous
    }

    #[must_use]
    pub const fn next(&self) -> &InstrumentSource {
        &self.next
    }

    #[must_use]
    pub const fn confirmation(&self) -> &IdentityChangeConfirmation {
        &self.confirmation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingCorrectionEvidence {
    None,
    NoteOnly(UserReason),
    DisclosureAndNote {
        disclosure: DisclosureKey,
        note: UserReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingCorrectionConfirmation {
    origin: IdentityConfirmationOrigin,
    evidence: MappingCorrectionEvidence,
    corrected_at: Timestamp,
}

impl MappingCorrectionConfirmation {
    #[must_use]
    pub const fn new(
        origin: IdentityConfirmationOrigin,
        evidence: MappingCorrectionEvidence,
        corrected_at: Timestamp,
    ) -> Self {
        Self {
            origin,
            evidence,
            corrected_at,
        }
    }

    #[must_use]
    pub const fn origin(&self) -> IdentityConfirmationOrigin {
        self.origin
    }

    #[must_use]
    pub const fn evidence(&self) -> &MappingCorrectionEvidence {
        &self.evidence
    }

    #[must_use]
    pub const fn corrected_at(&self) -> Timestamp {
        self.corrected_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedRetirementCorrection {
    instrument_id: InstrumentId,
    source: InstrumentSource,
    confirmation: MappingCorrectionConfirmation,
}

impl ConfirmedRetirementCorrection {
    #[must_use]
    pub const fn new(
        instrument_id: InstrumentId,
        source: InstrumentSource,
        confirmation: MappingCorrectionConfirmation,
    ) -> Self {
        Self {
            instrument_id,
            source,
            confirmation,
        }
    }

    #[must_use]
    pub const fn instrument_id(&self) -> &InstrumentId {
        &self.instrument_id
    }

    #[must_use]
    pub const fn source(&self) -> &InstrumentSource {
        &self.source
    }

    #[must_use]
    pub const fn confirmation(&self) -> &MappingCorrectionConfirmation {
        &self.confirmation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfirmedLinkCorrection {
    instrument_id: InstrumentId,
    previous: InstrumentSource,
    mistaken_next: InstrumentSource,
    confirmation: MappingCorrectionConfirmation,
}

impl ConfirmedLinkCorrection {
    #[must_use]
    pub const fn new(
        instrument_id: InstrumentId,
        previous: InstrumentSource,
        mistaken_next: InstrumentSource,
        confirmation: MappingCorrectionConfirmation,
    ) -> Self {
        Self {
            instrument_id,
            previous,
            mistaken_next,
            confirmation,
        }
    }

    #[must_use]
    pub const fn instrument_id(&self) -> &InstrumentId {
        &self.instrument_id
    }

    #[must_use]
    pub const fn previous(&self) -> &InstrumentSource {
        &self.previous
    }

    #[must_use]
    pub const fn mistaken_next(&self) -> &InstrumentSource {
        &self.mistaken_next
    }

    #[must_use]
    pub const fn confirmation(&self) -> &MappingCorrectionConfirmation {
        &self.confirmation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingHistoryEvent {
    IdentifierChanged {
        instrument_id: InstrumentId,
        previous: InstrumentSource,
        next: InstrumentSource,
        disclosure: DisclosureKey,
        action: Box<CorporateAction>,
        note: UserReason,
        confirmed_at: Timestamp,
    },
    RetirementCorrected {
        instrument_id: InstrumentId,
        source: InstrumentSource,
        disclosure: DisclosureKey,
        note: UserReason,
        corrected_at: Timestamp,
    },
    LinkCorrected {
        instrument_id: InstrumentId,
        restored_previous: InstrumentSource,
        unlinked_next: InstrumentSource,
        disclosure: DisclosureKey,
        note: UserReason,
        corrected_at: Timestamp,
    },
}

impl MappingHistoryEvent {
    #[must_use]
    pub const fn instrument_id(&self) -> &InstrumentId {
        match self {
            Self::IdentifierChanged { instrument_id, .. }
            | Self::RetirementCorrected { instrument_id, .. }
            | Self::LinkCorrected { instrument_id, .. } => instrument_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappingTransitionError {
    IdentityUncertain,
    Missing,
    UnchangedSource,
    MarketMismatch,
    InvalidEvidence,
    SourceNotActive,
    SourceNotRetired,
    SourceBoundToOtherId,
    SourceAlreadyRecorded,
    LinkNotLatest,
}

pub(crate) struct VerifiedChangeEvidence<'a> {
    pub(crate) disclosure: &'a DisclosureKey,
    pub(crate) action: &'a CorporateAction,
    pub(crate) note: &'a UserReason,
    pub(crate) confirmed_at: Timestamp,
}

pub(crate) struct VerifiedCorrectionEvidence<'a> {
    pub(crate) disclosure: &'a DisclosureKey,
    pub(crate) note: &'a UserReason,
    pub(crate) corrected_at: Timestamp,
}

pub(crate) fn verify_change_evidence(
    change: &ConfirmedIdentifierChange,
) -> Result<VerifiedChangeEvidence<'_>, MappingTransitionError> {
    match change.confirmation().origin() {
        IdentityConfirmationOrigin::UserConfirmed => {}
        IdentityConfirmationOrigin::BrokerObservation
        | IdentityConfirmationOrigin::AiSuggestion => {
            return Err(MappingTransitionError::IdentityUncertain);
        }
    }
    let (disclosure, action, note) = match change.confirmation().evidence() {
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure,
            action,
            note,
        } => (disclosure, action, note),
        IdentityChangeEvidence::None
        | IdentityChangeEvidence::NoteOnly(_)
        | IdentityChangeEvidence::DisclosureOnly { .. }
        | IdentityChangeEvidence::ActionOnly { .. } => {
            return Err(MappingTransitionError::IdentityUncertain);
        }
    };
    if change.previous().normalized_key() == change.next().normalized_key() {
        return Err(MappingTransitionError::UnchangedSource);
    }
    if change.previous().market() != change.next().market() {
        return Err(MappingTransitionError::MarketMismatch);
    }
    validate_change_action(change, action)?;
    Ok(VerifiedChangeEvidence {
        disclosure,
        action,
        note,
        confirmed_at: change.confirmation().confirmed_at(),
    })
}

fn validate_change_action(
    change: &ConfirmedIdentifierChange,
    action: &CorporateAction,
) -> Result<(), MappingTransitionError> {
    if action.id().instrument_id() != change.instrument_id() {
        return Err(MappingTransitionError::InvalidEvidence);
    }
    match action.kind() {
        CorporateActionKind::SymbolChange { from, to } => {
            if from.normalized_ascii() != change.previous().symbol().normalized_ascii()
                || to.normalized_ascii() != change.next().symbol().normalized_ascii()
            {
                return Err(MappingTransitionError::InvalidEvidence);
            }
            Ok(())
        }
        CorporateActionKind::Delisting => Err(MappingTransitionError::InvalidEvidence),
        CorporateActionKind::Split(_)
        | CorporateActionKind::ReverseSplit(_)
        | CorporateActionKind::BonusIssue { .. }
        | CorporateActionKind::StockDividend { .. }
        | CorporateActionKind::Merger
        | CorporateActionKind::Demerger
        | CorporateActionKind::Exchange => Ok(()),
    }
}

pub(crate) fn verify_correction_evidence(
    confirmation: &MappingCorrectionConfirmation,
) -> Result<VerifiedCorrectionEvidence<'_>, MappingTransitionError> {
    match confirmation.origin() {
        IdentityConfirmationOrigin::UserConfirmed => {}
        IdentityConfirmationOrigin::BrokerObservation
        | IdentityConfirmationOrigin::AiSuggestion => {
            return Err(MappingTransitionError::IdentityUncertain);
        }
    }
    match confirmation.evidence() {
        MappingCorrectionEvidence::DisclosureAndNote { disclosure, note } => {
            Ok(VerifiedCorrectionEvidence {
                disclosure,
                note,
                corrected_at: confirmation.corrected_at(),
            })
        }
        MappingCorrectionEvidence::None | MappingCorrectionEvidence::NoteOnly(_) => {
            Err(MappingTransitionError::IdentityUncertain)
        }
    }
}
