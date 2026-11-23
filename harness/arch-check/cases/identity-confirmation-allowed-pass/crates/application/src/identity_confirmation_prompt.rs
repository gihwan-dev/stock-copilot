pub enum IdentityConfirmationDecision {
    Confirmed,
    Denied,
    Unavailable,
}

pub trait IdentityConfirmationPrompt: Send + Sync {}
