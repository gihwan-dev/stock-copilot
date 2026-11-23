use crate::identity_confirmation_prompt::{
    IdentityConfirmationDecision, IdentityConfirmationPrompt,
};
use domain::UserIdentityConfirmation;

pub struct ConfirmInstrumentIdentity<'a, Prompt> {
    prompt: &'a Prompt,
}

impl<'a, Prompt: IdentityConfirmationPrompt> ConfirmInstrumentIdentity<'a, Prompt> {
    pub async fn confirm_identifier_change(&self) {
        match self.prompt.confirm_identifier_change().await {
            IdentityConfirmationDecision::Confirmed => {
                let _seal = UserIdentityConfirmation::issue_after_user_confirmation();
            }
            IdentityConfirmationDecision::Denied | IdentityConfirmationDecision::Unavailable => {}
        }
    }
}
