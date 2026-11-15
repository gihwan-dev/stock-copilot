use crate::{
    CredentialSource, ErrorClass, FeedSource, InformationError, IsolationFailure, ResetTime,
    UserAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfCheckKind {
    TossKey,
    AllowedIp,
    DuplicateKeyUse,
    ClaudeCodeInstallation,
    ClaudeCodeLogin,
    AiEnvironment,
    AiUsageLimit,
    OrderPrerequisites,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfCheckAction {
    ReplaceTossKey,
    RegisterIp,
    StopOtherProgram,
    InstallClaudeCode,
    LoginClaudeCode,
    ReviewRiskNotice,
    IntegrateExchange,
    ProvideDollarBuyingPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfCheckObservation {
    Healthy,
    NeedsAction(SelfCheckAction),
    Classified(ErrorClass),
    Waiting(ResetTime),
    EnvironmentRejected(IsolationFailure),
    OptionalCredential(CredentialSource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewsCheckState {
    Healthy,
    NotConfigured,
    NoData,
    Failed(InformationError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsCheckRecord {
    pub source: FeedSource,
    pub state: NewsCheckState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelfCheckRecord {
    pub check: SelfCheckKind,
    pub observation: SelfCheckObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelfCheckProblem {
    pub check: SelfCheckKind,
    pub action: SelfCheckAction,
    pub resumes_at: ResetTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfCheckResult {
    pub banner: Option<SelfCheckProblem>,
    pub records: [SelfCheckRecord; 8],
    pub news: Vec<NewsCheckRecord>,
}

#[must_use]
pub fn evaluate_self_check(
    observations: [SelfCheckObservation; 8],
    news: Vec<NewsCheckRecord>,
) -> SelfCheckResult {
    let [
        key,
        ip,
        duplicate,
        installation,
        login,
        environment,
        usage,
        order,
    ] = observations;
    let records = [
        (SelfCheckKind::TossKey, key),
        (SelfCheckKind::AllowedIp, ip),
        (SelfCheckKind::DuplicateKeyUse, duplicate),
        (SelfCheckKind::ClaudeCodeInstallation, installation),
        (SelfCheckKind::ClaudeCodeLogin, login),
        (SelfCheckKind::AiEnvironment, environment),
        (SelfCheckKind::AiUsageLimit, usage),
        (SelfCheckKind::OrderPrerequisites, order),
    ]
    .map(|(check, observation)| SelfCheckRecord { check, observation });
    let banner = records.iter().find_map(problem);
    SelfCheckResult {
        banner,
        records,
        news,
    }
}

fn problem(record: &SelfCheckRecord) -> Option<SelfCheckProblem> {
    let candidate = match record.observation {
        SelfCheckObservation::Healthy
        | SelfCheckObservation::Waiting(_)
        | SelfCheckObservation::EnvironmentRejected(_)
        | SelfCheckObservation::OptionalCredential(_) => None,
        SelfCheckObservation::NeedsAction(action) => Some(action),
        SelfCheckObservation::Classified(class) => match class {
            ErrorClass::Wait(_) | ErrorClass::Bug(_) | ErrorClass::NoData(_) => None,
            ErrorClass::UserAction(action) => match action {
                UserAction::CheckCredentials => Some(SelfCheckAction::ReplaceTossKey),
                UserAction::CheckIpAllowlist => Some(SelfCheckAction::RegisterIp),
                UserAction::StopCompetingKeyUse => Some(SelfCheckAction::StopOtherProgram),
                UserAction::CheckAccount | UserAction::CheckPermissions => None,
            },
        },
    }?;
    match (record.check, candidate) {
        (SelfCheckKind::TossKey, SelfCheckAction::ReplaceTossKey)
        | (SelfCheckKind::AllowedIp, SelfCheckAction::RegisterIp)
        | (SelfCheckKind::DuplicateKeyUse, SelfCheckAction::StopOtherProgram)
        | (SelfCheckKind::ClaudeCodeInstallation, SelfCheckAction::InstallClaudeCode)
        | (SelfCheckKind::ClaudeCodeLogin, SelfCheckAction::LoginClaudeCode)
        | (
            SelfCheckKind::OrderPrerequisites,
            SelfCheckAction::ReviewRiskNotice
            | SelfCheckAction::IntegrateExchange
            | SelfCheckAction::ProvideDollarBuyingPower,
        ) => Some(SelfCheckProblem {
            check: record.check,
            action: candidate,
            resumes_at: ResetTime::NotProvided,
        }),
        (
            SelfCheckKind::TossKey
            | SelfCheckKind::AllowedIp
            | SelfCheckKind::DuplicateKeyUse
            | SelfCheckKind::ClaudeCodeInstallation
            | SelfCheckKind::ClaudeCodeLogin
            | SelfCheckKind::AiEnvironment
            | SelfCheckKind::AiUsageLimit
            | SelfCheckKind::OrderPrerequisites,
            _,
        ) => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfCheckChangeCause {
    BrokerConnection,
    OtherCheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfCheckChange {
    Unchanged,
    Publish,
}

#[must_use]
pub fn self_check_change(
    previous: &SelfCheckResult,
    next: &SelfCheckResult,
    cause: SelfCheckChangeCause,
) -> SelfCheckChange {
    match cause {
        SelfCheckChangeCause::BrokerConnection => SelfCheckChange::Unchanged,
        SelfCheckChangeCause::OtherCheck => {
            if previous.records[3..] == next.records[3..] {
                SelfCheckChange::Unchanged
            } else {
                SelfCheckChange::Publish
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfCheckDifference {
    Same,
    Changed,
}

#[must_use]
pub fn self_check_difference(
    previous: &SelfCheckResult,
    next: &SelfCheckResult,
) -> SelfCheckDifference {
    if previous == next {
        SelfCheckDifference::Same
    } else {
        SelfCheckDifference::Changed
    }
}
