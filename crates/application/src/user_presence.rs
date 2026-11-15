use crate::SystemPortError;
use domain::{OrderSummary, RiskSettingsChange};
use std::future::Future;

#[derive(Debug)]
pub enum ConfirmationRequest<'summary> {
    OrderSummary(&'summary OrderSummary),
    RiskSettingsChange(&'summary RiskSettingsChange),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationOutcome {
    Confirmed,
    Denied,
    Unavailable,
}

pub trait UserPresence: Send + Sync {
    fn confirm(
        &self,
        request: ConfirmationRequest<'_>,
    ) -> impl Future<Output = Result<ConfirmationOutcome, SystemPortError>> + Send;
}
