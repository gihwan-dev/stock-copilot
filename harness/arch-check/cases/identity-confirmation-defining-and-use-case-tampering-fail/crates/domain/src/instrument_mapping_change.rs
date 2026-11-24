#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserIdentityConfirmation(UserIdentityConfirmationSeal);

#[derive(Debug, PartialEq, Eq)]
struct UserIdentityConfirmationSeal;

impl UserIdentityConfirmation {
    pub const fn issue_after_user_confirmation() -> Self {
        Self(UserIdentityConfirmationSeal)
    }

    pub fn backdoor() -> Self {
        Self(UserIdentityConfirmationSeal)
    }
}
