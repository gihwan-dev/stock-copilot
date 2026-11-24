#[derive(Debug, PartialEq, Eq)]
pub struct UserIdentityConfirmation {}

impl UserIdentityConfirmation {
    #[must_use]
    pub const fn issue_after_user_confirmation() -> Self {
        Self {}
    }
}
