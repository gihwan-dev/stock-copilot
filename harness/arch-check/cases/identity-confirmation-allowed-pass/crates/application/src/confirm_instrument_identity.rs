use crate::Clock;
use crate::identity_confirmation_prompt::{
    IdentifierChangePrompt, IdentityConfirmationDecision, IdentityConfirmationPrompt,
};
use domain::{
    ConfirmedIdentifierChange, IdentityChangeConfirmation, IdentityConfirmationOrigin,
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
}
