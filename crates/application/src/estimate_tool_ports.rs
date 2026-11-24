use std::future::{Future, ready};

use domain::{InvestorProfile, RiskSettings};

use crate::{
    AiRunId, EstimateRunContext, SettingsRepository, SystemFailure, SystemPort, SystemPortError,
    WriteCompleted,
};

pub trait EstimateRunContextSource: Send + Sync {
    fn estimate_context(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<EstimateRunContext, SystemPortError>> + Send;
}

pub trait ProfileSource: Send + Sync {
    fn profile(
        &self,
    ) -> impl Future<Output = Result<Option<InvestorProfile>, SystemPortError>> + Send;
}

#[derive(Debug)]
pub struct ReadOnlySettings<'a, P> {
    source: &'a P,
}

impl<'a, P> ReadOnlySettings<'a, P> {
    #[must_use]
    pub const fn new(source: &'a P) -> Self {
        Self { source }
    }
}

const fn write_refused() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied)
}

impl<P: ProfileSource> SettingsRepository for ReadOnlySettings<'_, P> {
    fn store_profile(
        &self,
        _profile: &InvestorProfile,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        ready(Err(write_refused()))
    }

    fn profile(
        &self,
    ) -> impl Future<Output = Result<Option<InvestorProfile>, SystemPortError>> + Send {
        self.source.profile()
    }

    fn store_risk_settings(
        &self,
        _settings: &RiskSettings,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        ready(Err(write_refused()))
    }

    fn risk_settings(
        &self,
    ) -> impl Future<Output = Result<Option<RiskSettings>, SystemPortError>> + Send {
        ready(Ok(None))
    }
}
