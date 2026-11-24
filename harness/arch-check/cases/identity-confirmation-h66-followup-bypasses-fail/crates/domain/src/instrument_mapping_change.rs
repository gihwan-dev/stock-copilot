#[derive(Debug, PartialEq, Eq)]
struct UserIdentityConfirmationSeal;

#[derive(Debug, PartialEq, Eq)]
pub struct UserIdentityConfirmation(UserIdentityConfirmationSeal);

impl UserIdentityConfirmation {
    #[must_use]
    pub const fn issue_after_user_confirmation() -> Self {
        Self(UserIdentityConfirmationSeal)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum IdentityConfirmationOrigin {
    UserConfirmed(UserIdentityConfirmation),
    BrokerObservation,
    AiSuggestion,
}

#[derive(Debug, PartialEq, Eq)]
pub struct IdentityChangeConfirmation {
    origin: IdentityConfirmationOrigin,
}

impl IdentityChangeConfirmation {
    #[must_use]
    pub const fn into_origin(self) -> IdentityConfirmationOrigin {
        self.origin
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ConfirmedIdentifierChange {
    confirmation: IdentityChangeConfirmation,
}

impl ConfirmedIdentifierChange {
    #[must_use]
    pub const fn into_confirmation(self) -> IdentityChangeConfirmation {
        self.confirmation
    }
}
