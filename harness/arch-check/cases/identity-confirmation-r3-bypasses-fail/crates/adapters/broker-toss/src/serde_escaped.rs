#[derive(serde::Deserialize)]
pub struct EscapedForge {
    #[serde(default = "domain::UserIdentityConfirmation::\x69ssue_after_user_confirmation")]
    pub seal: domain::UserIdentityConfirmation,
}
