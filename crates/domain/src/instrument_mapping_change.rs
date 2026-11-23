use crate::{
    CorporateAction, CorporateActionKind, DisclosureKey, InstrumentId, InstrumentSource, Timestamp,
    UserReason,
};

#[derive(Debug, PartialEq, Eq)]
struct UserIdentityConfirmationSeal;

#[derive(Debug, PartialEq, Eq)]
pub struct UserIdentityConfirmation(UserIdentityConfirmationSeal);

#[derive(Debug, PartialEq, Eq)]
pub enum IdentityConfirmationOrigin {
    UserConfirmed(UserIdentityConfirmation),
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

#[derive(Debug, PartialEq, Eq)]
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
    pub const fn origin(&self) -> &IdentityConfirmationOrigin {
        &self.origin
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

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
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
    pub const fn origin(&self) -> &IdentityConfirmationOrigin {
        &self.origin
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

#[derive(Debug, PartialEq, Eq)]
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

#[derive(Debug, PartialEq, Eq)]
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

pub(crate) struct VerifiedChange {
    pub(crate) instrument_id: InstrumentId,
    pub(crate) previous: InstrumentSource,
    pub(crate) next: InstrumentSource,
    pub(crate) disclosure: DisclosureKey,
    pub(crate) action: CorporateAction,
    pub(crate) note: UserReason,
    pub(crate) confirmed_at: Timestamp,
}

pub(crate) struct VerifiedRetirementCorrection {
    pub(crate) instrument_id: InstrumentId,
    pub(crate) source: InstrumentSource,
    pub(crate) disclosure: DisclosureKey,
    pub(crate) note: UserReason,
    pub(crate) corrected_at: Timestamp,
}

pub(crate) struct VerifiedLinkCorrection {
    pub(crate) instrument_id: InstrumentId,
    pub(crate) previous: InstrumentSource,
    pub(crate) mistaken_next: InstrumentSource,
    pub(crate) disclosure: DisclosureKey,
    pub(crate) note: UserReason,
    pub(crate) corrected_at: Timestamp,
}

pub(crate) fn verify_change_evidence(
    change: ConfirmedIdentifierChange,
) -> Result<VerifiedChange, MappingTransitionError> {
    let ConfirmedIdentifierChange {
        instrument_id,
        previous,
        next,
        confirmation:
            IdentityChangeConfirmation {
                origin,
                evidence,
                confirmed_at,
            },
    } = change;
    match origin {
        IdentityConfirmationOrigin::UserConfirmed(UserIdentityConfirmation(
            UserIdentityConfirmationSeal,
        )) => {}
        IdentityConfirmationOrigin::BrokerObservation
        | IdentityConfirmationOrigin::AiSuggestion => {
            return Err(MappingTransitionError::IdentityUncertain);
        }
    }
    let (disclosure, action, note) = match evidence {
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
    if previous.normalized_key() == next.normalized_key() {
        return Err(MappingTransitionError::UnchangedSource);
    }
    if previous.market() != next.market() {
        return Err(MappingTransitionError::MarketMismatch);
    }
    validate_change_action(&instrument_id, &previous, &next, &action)?;
    Ok(VerifiedChange {
        instrument_id,
        previous,
        next,
        disclosure,
        action,
        note,
        confirmed_at,
    })
}

pub(crate) fn validate_change_action(
    instrument_id: &InstrumentId,
    previous: &InstrumentSource,
    next: &InstrumentSource,
    action: &CorporateAction,
) -> Result<(), MappingTransitionError> {
    if action.id().instrument_id() != instrument_id || action.receipt().is_some() {
        return Err(MappingTransitionError::InvalidEvidence);
    }
    match action.kind() {
        CorporateActionKind::SymbolChange { from, to } => {
            let from_norm = from.normalized_ascii();
            let to_norm = to.normalized_ascii();
            if from_norm == to_norm
                || from_norm != previous.symbol().normalized_ascii()
                || to_norm != next.symbol().normalized_ascii()
            {
                return Err(MappingTransitionError::InvalidEvidence);
            }
            Ok(())
        }
        CorporateActionKind::Split(_) | CorporateActionKind::ReverseSplit(_) => Ok(()),
        CorporateActionKind::Delisting | CorporateActionKind::Demerger => {
            Err(MappingTransitionError::InvalidEvidence)
        }
        CorporateActionKind::BonusIssue { .. }
        | CorporateActionKind::StockDividend { .. }
        | CorporateActionKind::Merger
        | CorporateActionKind::Exchange => Err(MappingTransitionError::IdentityUncertain),
    }
}

fn verify_correction_confirmation(
    confirmation: MappingCorrectionConfirmation,
) -> Result<(DisclosureKey, UserReason, Timestamp), MappingTransitionError> {
    let MappingCorrectionConfirmation {
        origin,
        evidence,
        corrected_at,
    } = confirmation;
    match origin {
        IdentityConfirmationOrigin::UserConfirmed(UserIdentityConfirmation(
            UserIdentityConfirmationSeal,
        )) => {}
        IdentityConfirmationOrigin::BrokerObservation
        | IdentityConfirmationOrigin::AiSuggestion => {
            return Err(MappingTransitionError::IdentityUncertain);
        }
    }
    match evidence {
        MappingCorrectionEvidence::DisclosureAndNote { disclosure, note } => {
            Ok((disclosure, note, corrected_at))
        }
        MappingCorrectionEvidence::None | MappingCorrectionEvidence::NoteOnly(_) => {
            Err(MappingTransitionError::IdentityUncertain)
        }
    }
}

pub(crate) fn verify_retirement_correction(
    correction: ConfirmedRetirementCorrection,
) -> Result<VerifiedRetirementCorrection, MappingTransitionError> {
    let ConfirmedRetirementCorrection {
        instrument_id,
        source,
        confirmation,
    } = correction;
    let (disclosure, note, corrected_at) = verify_correction_confirmation(confirmation)?;
    Ok(VerifiedRetirementCorrection {
        instrument_id,
        source,
        disclosure,
        note,
        corrected_at,
    })
}

pub(crate) fn verify_link_correction(
    correction: ConfirmedLinkCorrection,
) -> Result<VerifiedLinkCorrection, MappingTransitionError> {
    let ConfirmedLinkCorrection {
        instrument_id,
        previous,
        mistaken_next,
        confirmation,
    } = correction;
    let (disclosure, note, corrected_at) = verify_correction_confirmation(confirmation)?;
    Ok(VerifiedLinkCorrection {
        instrument_id,
        previous,
        mistaken_next,
        disclosure,
        note,
        corrected_at,
    })
}

mod history_cases;
#[cfg(test)]
mod verify_tests;
