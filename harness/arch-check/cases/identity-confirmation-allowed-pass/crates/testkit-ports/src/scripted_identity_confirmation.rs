use application::identity_confirmation_prompt::IdentityConfirmationPrompt;
use domain::UserIdentityConfirmation;

pub struct Scripted;

impl IdentityConfirmationPrompt for Scripted {}

impl Scripted {
    pub fn issue(decision: IdentityConfirmationDecision) -> Option<UserIdentityConfirmation> {
        match decision {
            IdentityConfirmationDecision::Confirmed => {
                Some(UserIdentityConfirmation::issue_after_user_confirmation())
            }
            IdentityConfirmationDecision::Denied | IdentityConfirmationDecision::Unavailable => None,
        }
    }
}
