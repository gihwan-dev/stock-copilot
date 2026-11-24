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

trait SealForge {
    type Seal;
    type Out;
    fn mint() -> Self::Out;
}

pub struct ForgeHandle;

impl SealForge for ForgeHandle {
    type Seal = UserIdentityConfirmationSeal;
    type Out = UserIdentityConfirmation;

    fn mint() -> Self::Out {
        Self::Out { 0: Self::Seal {} }
    }
}
