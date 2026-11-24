#[derive(serde::Deserialize)]
pub struct Forge {
    #[serde(
        skip_deserializing,
        default = "domain::UserIdentityConfirmation::issue_after_user_confirmation"
    )]
    pub seal: domain::UserIdentityConfirmation,
}
