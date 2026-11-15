use crate::{
    AiUsage, BugDiagnostic, CredentialSource, ErrorClass, FeedSource, InformationError,
    IsolationFailure, MissingData, RetryDelay, UserAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState<Problem> {
    Healthy,
    Problem(Problem),
    Waiting(RetryDelay),
    Bug(BugDiagnostic),
    NoData(MissingData),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossKeyProblem {
    InvalidKey,
    Permissions,
    AccountNotFound,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpProblem {
    NotAllowed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateKeyProblem {
    CompetingProgram,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeInstallProblem {
    NotInstalled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeLoginProblem {
    LoginRequired,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiEnvironmentProblem {
    Tools,
    McpServers,
    Plugins,
    Skills,
    SlashCommands,
    PermissionMode,
    ApiKeySource,
    CliVersion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderPrerequisiteProblem {
    RiskNotice,
    ExchangeIntegration,
    DollarBuyingPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelfCheckInput {
    pub toss_key: CheckState<TossKeyProblem>,
    pub allowed_ip: CheckState<IpProblem>,
    pub duplicate_key_use: CheckState<DuplicateKeyProblem>,
    pub claude_install: CheckState<ClaudeInstallProblem>,
    pub claude_login: CheckState<ClaudeLoginProblem>,
    pub ai_environment: CheckState<AiEnvironmentProblem>,
    pub ai_usage: AiUsage,
    pub order_prerequisite: CheckState<OrderPrerequisiteProblem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfCheckAction {
    TossKey(TossKeyProblem),
    RegisterIp,
    StopOtherProgram,
    InstallClaudeCode,
    LoginClaudeCode,
    OrderPrerequisite(OrderPrerequisiteProblem),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewsCheckState {
    Healthy,
    NotConfigured,
    NoData,
    Failed(InformationError),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCheckRecord {
    Feed {
        source: FeedSource,
        state: NewsCheckState,
    },
    OptionalCredential(CredentialSource),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfCheckResult {
    pub banner: Option<SelfCheckAction>,
    pub records: SelfCheckInput,
    pub sources: Vec<SourceCheckRecord>,
}

#[must_use]
pub fn evaluate_self_check(
    records: SelfCheckInput,
    sources: Vec<SourceCheckRecord>,
) -> SelfCheckResult {
    let banner = match records {
        SelfCheckInput {
            toss_key: CheckState::Problem(problem),
            ..
        } => Some(SelfCheckAction::TossKey(problem)),
        SelfCheckInput {
            allowed_ip: CheckState::Problem(IpProblem::NotAllowed),
            ..
        } => Some(SelfCheckAction::RegisterIp),
        SelfCheckInput {
            duplicate_key_use: CheckState::Problem(DuplicateKeyProblem::CompetingProgram),
            ..
        } => Some(SelfCheckAction::StopOtherProgram),
        SelfCheckInput {
            claude_install: CheckState::Problem(ClaudeInstallProblem::NotInstalled),
            ..
        } => Some(SelfCheckAction::InstallClaudeCode),
        SelfCheckInput {
            claude_login: CheckState::Problem(ClaudeLoginProblem::LoginRequired),
            ..
        } => Some(SelfCheckAction::LoginClaudeCode),
        SelfCheckInput {
            order_prerequisite: CheckState::Problem(problem),
            ..
        } => Some(SelfCheckAction::OrderPrerequisite(problem)),
        _ => None,
    };
    SelfCheckResult {
        banner,
        records,
        sources,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfCheckChange {
    Unchanged,
    Publish,
}
#[must_use]
pub fn self_check_change(previous: &SelfCheckResult, next: &SelfCheckResult) -> SelfCheckChange {
    let before = previous.records;
    let after = next.records;
    if before.claude_install == after.claude_install
        && before.claude_login == after.claude_login
        && before.ai_environment == after.ai_environment
        && before.ai_usage == after.ai_usage
        && before.order_prerequisite == after.order_prerequisite
    {
        SelfCheckChange::Unchanged
    } else {
        SelfCheckChange::Publish
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TossCheckObservation {
    Key(CheckState<TossKeyProblem>),
    Ip(CheckState<IpProblem>),
    DuplicateKey(CheckState<DuplicateKeyProblem>),
}
#[must_use]
pub fn toss_check_observation(class: ErrorClass) -> TossCheckObservation {
    use CheckState::{Bug, NoData, Problem, Waiting};
    use TossCheckObservation::{DuplicateKey, Ip, Key};
    match class {
        ErrorClass::UserAction(action) => match action {
            UserAction::CheckCredentials => Key(Problem(TossKeyProblem::InvalidKey)),
            UserAction::CheckPermissions => Key(Problem(TossKeyProblem::Permissions)),
            UserAction::CheckAccount => Key(Problem(TossKeyProblem::AccountNotFound)),
            UserAction::CheckIpAllowlist => Ip(Problem(IpProblem::NotAllowed)),
            UserAction::StopCompetingKeyUse => {
                DuplicateKey(Problem(DuplicateKeyProblem::CompetingProgram))
            }
        },
        ErrorClass::Wait(delay) => Key(Waiting(delay)),
        ErrorClass::Bug(reason) => Key(Bug(reason)),
        ErrorClass::NoData(data) => Key(NoData(data)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiCheckObservation {
    Install(ClaudeInstallProblem),
    Login(ClaudeLoginProblem),
    Environment(AiEnvironmentProblem),
}
#[must_use]
pub fn isolation_check_observation(failure: IsolationFailure) -> AiCheckObservation {
    use AiCheckObservation::{Environment, Install, Login};
    match failure {
        IsolationFailure::NotInstalled => Install(ClaudeInstallProblem::NotInstalled),
        IsolationFailure::LoginRequired => Login(ClaudeLoginProblem::LoginRequired),
        IsolationFailure::Tools => Environment(AiEnvironmentProblem::Tools),
        IsolationFailure::McpServers => Environment(AiEnvironmentProblem::McpServers),
        IsolationFailure::Plugins => Environment(AiEnvironmentProblem::Plugins),
        IsolationFailure::Skills => Environment(AiEnvironmentProblem::Skills),
        IsolationFailure::SlashCommands => Environment(AiEnvironmentProblem::SlashCommands),
        IsolationFailure::PermissionMode => Environment(AiEnvironmentProblem::PermissionMode),
        IsolationFailure::ApiKeySource => Environment(AiEnvironmentProblem::ApiKeySource),
        IsolationFailure::CliVersion => Environment(AiEnvironmentProblem::CliVersion),
    }
}
