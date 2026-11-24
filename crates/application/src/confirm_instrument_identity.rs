use crate::Clock;
use crate::identity_confirmation_prompt::{
    IdentifierChangePrompt, IdentityConfirmationDecision, IdentityConfirmationPrompt,
    LinkCorrectionPrompt, RetirementCorrectionPrompt,
};
use domain::{
    ConfirmedIdentifierChange, ConfirmedLinkCorrection, ConfirmedRetirementCorrection,
    IdentityChangeConfirmation, IdentityConfirmationOrigin, MappingCorrectionConfirmation,
    UserIdentityConfirmation,
};

#[derive(Debug, PartialEq, Eq)]
pub enum IdentityConfirmationOutcome<T> {
    Confirmed(T),
    Denied,
    Unavailable,
}

#[derive(Debug)]
pub struct ConfirmInstrumentIdentity<'a, Prompt, Time> {
    prompt: &'a Prompt,
    clock: &'a Time,
}

impl<'a, Prompt: IdentityConfirmationPrompt, Time: Clock>
    ConfirmInstrumentIdentity<'a, Prompt, Time>
{
    #[must_use]
    pub const fn new(prompt: &'a Prompt, clock: &'a Time) -> Self {
        Self { prompt, clock }
    }

    pub async fn confirm_identifier_change(
        &self,
        request: IdentifierChangePrompt,
    ) -> IdentityConfirmationOutcome<ConfirmedIdentifierChange> {
        match self.prompt.confirm_identifier_change(&request).await {
            IdentityConfirmationDecision::Confirmed => {
                let confirmed_at = self.clock.now();
                let seal = UserIdentityConfirmation::issue_after_user_confirmation();
                let (instrument_id, previous, next, evidence) = request.into_parts();
                IdentityConfirmationOutcome::Confirmed(ConfirmedIdentifierChange::new(
                    instrument_id,
                    previous,
                    next,
                    IdentityChangeConfirmation::new(
                        IdentityConfirmationOrigin::UserConfirmed(seal),
                        evidence,
                        confirmed_at,
                    ),
                ))
            }
            IdentityConfirmationDecision::Denied => IdentityConfirmationOutcome::Denied,
            IdentityConfirmationDecision::Unavailable => IdentityConfirmationOutcome::Unavailable,
        }
    }

    pub async fn confirm_retirement_correction(
        &self,
        request: RetirementCorrectionPrompt,
    ) -> IdentityConfirmationOutcome<ConfirmedRetirementCorrection> {
        match self.prompt.confirm_retirement_correction(&request).await {
            IdentityConfirmationDecision::Confirmed => {
                let corrected_at = self.clock.now();
                let seal = UserIdentityConfirmation::issue_after_user_confirmation();
                let (instrument_id, source, evidence) = request.into_parts();
                IdentityConfirmationOutcome::Confirmed(ConfirmedRetirementCorrection::new(
                    instrument_id,
                    source,
                    MappingCorrectionConfirmation::new(
                        IdentityConfirmationOrigin::UserConfirmed(seal),
                        evidence,
                        corrected_at,
                    ),
                ))
            }
            IdentityConfirmationDecision::Denied => IdentityConfirmationOutcome::Denied,
            IdentityConfirmationDecision::Unavailable => IdentityConfirmationOutcome::Unavailable,
        }
    }

    pub async fn confirm_link_correction(
        &self,
        request: LinkCorrectionPrompt,
    ) -> IdentityConfirmationOutcome<ConfirmedLinkCorrection> {
        match self.prompt.confirm_link_correction(&request).await {
            IdentityConfirmationDecision::Confirmed => {
                let corrected_at = self.clock.now();
                let seal = UserIdentityConfirmation::issue_after_user_confirmation();
                let (instrument_id, previous, mistaken_next, evidence) = request.into_parts();
                IdentityConfirmationOutcome::Confirmed(ConfirmedLinkCorrection::new(
                    instrument_id,
                    previous,
                    mistaken_next,
                    MappingCorrectionConfirmation::new(
                        IdentityConfirmationOrigin::UserConfirmed(seal),
                        evidence,
                        corrected_at,
                    ),
                ))
            }
            IdentityConfirmationDecision::Denied => IdentityConfirmationOutcome::Denied,
            IdentityConfirmationDecision::Unavailable => IdentityConfirmationOutcome::Unavailable,
        }
    }
}
