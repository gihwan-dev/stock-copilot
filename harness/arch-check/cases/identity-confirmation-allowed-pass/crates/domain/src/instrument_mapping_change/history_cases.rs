#[cfg(test)]
mod tests {
    use super::super::{UserIdentityConfirmation, UserIdentityConfirmationSeal};

    #[test]
    fn internal() {
        let _value = UserIdentityConfirmation(UserIdentityConfirmationSeal);
    }
}
