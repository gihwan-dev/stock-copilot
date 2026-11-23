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

mod history_cases;

#[cfg(test)]
mod tests {
    use super::UserIdentityConfirmation;

    #[test]
    fn ok() {
        let _value = UserIdentityConfirmation::issue_after_user_confirmation();
    }
}
