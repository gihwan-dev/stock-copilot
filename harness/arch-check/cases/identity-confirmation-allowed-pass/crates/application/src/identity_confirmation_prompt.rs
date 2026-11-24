use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityConfirmationDecision {
    Confirmed,
    Denied,
    Unavailable,
}

pub struct IdentifierChangePrompt {
    value: u8,
}

pub struct RetirementCorrectionPrompt {
    value: u8,
}

pub struct LinkCorrectionPrompt {
    value: u8,
}

pub trait IdentityConfirmationPrompt: Send + Sync {
    fn confirm_identifier_change(
        &self,
        request: &IdentifierChangePrompt,
    ) -> impl Future<Output = IdentityConfirmationDecision> + Send;

    fn confirm_retirement_correction(
        &self,
        request: &RetirementCorrectionPrompt,
    ) -> impl Future<Output = IdentityConfirmationDecision> + Send;

    fn confirm_link_correction(
        &self,
        request: &LinkCorrectionPrompt,
    ) -> impl Future<Output = IdentityConfirmationDecision> + Send;
}
