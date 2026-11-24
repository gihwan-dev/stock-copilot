use crate::{
    AiCheckObservation, AiEnvironmentProblem, AiProvider, CheckState, DeliveryReport, EventBus,
    IdentifierError, IsolationCheck, RunKind, SelfCheckInput, SystemPortError,
    isolation_check_observation,
};
use domain::DomainEvent;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CliVersion(String);

impl CliVersion {
    pub fn try_new(value: &str) -> Result<Self, IdentifierError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b".+-_".contains(&byte))
        {
            return Err(IdentifierError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub trait CliVersionSource: Send + Sync {
    fn observed_version(&self) -> impl Future<Output = Option<CliVersion>> + Send;
}

pub trait VerifiedCliVersions: Send + Sync {
    fn verified(&self) -> impl Future<Output = Result<Vec<CliVersion>, SystemPortError>> + Send;
    fn record_verified(
        &self,
        version: &CliVersion,
    ) -> impl Future<Output = Result<crate::WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliVersionGateOutcome {
    AlreadyVerified(CliVersion),
    NewlyVerified(CliVersion),
    Disabled(AiCheckObservation),
    ProbeFailed,
    VersionUnknown,
    Storage(SystemPortError),
}

impl CliVersionGateOutcome {
    #[must_use]
    pub const fn ai_enabled(&self) -> AiGate {
        match self {
            Self::AlreadyVerified(_) | Self::NewlyVerified(_) => AiGate::Enabled,
            Self::Disabled(_) | Self::ProbeFailed | Self::VersionUnknown | Self::Storage(_) => {
                AiGate::Disabled
            }
        }
    }

    #[must_use]
    pub const fn login_recheck(&self) -> LoginRecheck {
        match self {
            Self::ProbeFailed => LoginRecheck::Needed,
            Self::AlreadyVerified(_)
            | Self::NewlyVerified(_)
            | Self::Disabled(_)
            | Self::VersionUnknown
            | Self::Storage(_) => LoginRecheck::NotNeeded,
        }
    }

    pub fn apply_to_self_check(&self, input: &mut SelfCheckInput) {
        match self {
            Self::AlreadyVerified(_) | Self::NewlyVerified(_) => {
                input.claude_install = CheckState::Healthy;
                input.claude_login = CheckState::Healthy;
                input.ai_environment = CheckState::Healthy;
            }
            Self::Disabled(AiCheckObservation::Install(problem)) => {
                input.claude_install = CheckState::Problem(*problem);
            }
            Self::Disabled(AiCheckObservation::Login(problem)) => {
                input.claude_install = CheckState::Healthy;
                input.claude_login = CheckState::Problem(*problem);
            }
            Self::Disabled(AiCheckObservation::Environment(problem)) => {
                input.ai_environment = CheckState::Problem(*problem);
            }
            Self::ProbeFailed | Self::VersionUnknown | Self::Storage(_) => {
                input.ai_environment = CheckState::Problem(AiEnvironmentProblem::CliVersion);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginRecheck {
    Needed,
    NotNeeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeFailure {
    Rejected(AiCheckObservation),
    CouldNotRun,
}

impl ProbeFailure {
    const fn into_outcome(self) -> CliVersionGateOutcome {
        match self {
            Self::Rejected(observation) => CliVersionGateOutcome::Disabled(observation),
            Self::CouldNotRun => CliVersionGateOutcome::ProbeFailed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiGate {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliVersionGateReport {
    pub outcome: CliVersionGateOutcome,
    pub delivery: DeliveryReport,
}

#[derive(Debug)]
pub struct EnsureVerifiedCliVersion<'a, Provider, Source, Ledger> {
    pub provider: &'a Provider,
    pub source: &'a Source,
    pub ledger: &'a Ledger,
    pub events: &'a EventBus,
}

impl<Provider: AiProvider, Source: CliVersionSource, Ledger: VerifiedCliVersions>
    EnsureVerifiedCliVersion<'_, Provider, Source, Ledger>
{
    pub async fn execute(&self, previous: &CliVersionGateOutcome) -> CliVersionGateReport {
        let outcome = self.decide().await;
        let delivery = if &outcome == previous {
            DeliveryReport::default()
        } else {
            self.events.publish(DomainEvent::SelfCheckChanged)
        };
        CliVersionGateReport { outcome, delivery }
    }

    async fn decide(&self) -> CliVersionGateOutcome {
        if let Err(failure) = self.probe(RunKind::Analysis).await {
            return failure.into_outcome();
        }
        let Some(version) = self.source.observed_version().await else {
            return CliVersionGateOutcome::VersionUnknown;
        };
        let verified = match self.ledger.verified().await {
            Ok(list) => list,
            Err(error) => return CliVersionGateOutcome::Storage(error),
        };
        if verified.contains(&version) {
            return CliVersionGateOutcome::AlreadyVerified(version);
        }
        if let Err(failure) = self.probe(RunKind::Collection).await {
            return failure.into_outcome();
        }
        if self.source.observed_version().await.as_ref() != Some(&version) {
            return CliVersionGateOutcome::VersionUnknown;
        }
        match self.ledger.record_verified(&version).await {
            Ok(_written) => CliVersionGateOutcome::NewlyVerified(version),
            Err(error) => CliVersionGateOutcome::Storage(error),
        }
    }

    async fn probe(&self, kind: RunKind) -> Result<(), ProbeFailure> {
        match self.provider.verify_isolation(kind).await {
            Ok(IsolationCheck::Passed) => Ok(()),
            Ok(IsolationCheck::Rejected(failure)) => {
                Err(ProbeFailure::Rejected(isolation_check_observation(failure)))
            }
            Err(_error) => Err(ProbeFailure::CouldNotRun),
        }
    }
}
