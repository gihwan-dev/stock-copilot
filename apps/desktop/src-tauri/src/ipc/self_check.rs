use std::fmt;
use std::sync::Arc;

use application::{
    CheckState, ClassifiedError, ClaudeInstallProblem, ClaudeLoginProblem, DuplicateKeyProblem,
    IpProblem, SelfCheckAction, SelfCheckInput, SourceStatus, SourceStatusRepository,
    SystemPortError, TossKeyProblem, evaluate_self_check,
};
use domain::{DataSource, Timestamp};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::briefing::kst_iso;
use super::error::{IpcError, IpcErrorKind};
use super::state::BoxFuture;
use crate::toss_link::{TossConnectionState, TossLinkState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeInstall {
    Installed,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiLimit {
    Available,
    Waiting(Timestamp),
}

pub trait AiStatusProbe: Send + Sync {
    fn install(&self) -> ClaudeInstall;
    fn limit(&self) -> BoxFuture<'_, AiLimit>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MissingAi;

impl AiStatusProbe for MissingAi {
    fn install(&self) -> ClaudeInstall {
        ClaudeInstall::Missing
    }

    fn limit(&self) -> BoxFuture<'_, AiLimit> {
        Box::pin(std::future::ready(AiLimit::Available))
    }
}

pub trait SourceStatusList: Send + Sync {
    fn list(&self) -> BoxFuture<'_, Result<Vec<SourceStatus>, SystemPortError>>;
}

struct RepositorySources<R>(Arc<R>);

impl<R: SourceStatusRepository + 'static> SourceStatusList for RepositorySources<R> {
    fn list(&self) -> BoxFuture<'_, Result<Vec<SourceStatus>, SystemPortError>> {
        Box::pin(self.0.list_all())
    }
}

#[derive(Clone)]
pub struct SelfCheckIpcState {
    ai: Arc<dyn AiStatusProbe>,
    statuses: Arc<dyn SourceStatusList>,
}

impl fmt::Debug for SelfCheckIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SelfCheckIpcState")
    }
}

impl SelfCheckIpcState {
    #[must_use]
    pub fn new<R: SourceStatusRepository + 'static>(
        ai: Arc<dyn AiStatusProbe>,
        repository: Arc<R>,
    ) -> Self {
        Self {
            ai,
            statuses: Arc::new(RepositorySources(repository)),
        }
    }

    #[must_use]
    pub fn with_statuses(ai: Arc<dyn AiStatusProbe>, statuses: Arc<dyn SourceStatusList>) -> Self {
        Self { ai, statuses }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum SelfCheckScope {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetSelfCheckRequest {
    pub scope: SelfCheckScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetStatusRequest {
    pub scope: SelfCheckScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiSelfCheckProblem {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "toss-key-invalid")]
    TossKeyInvalid,
    #[serde(rename = "toss-key-permission")]
    TossKeyPermission,
    #[serde(rename = "toss-account-missing")]
    TossAccountMissing,
    #[serde(rename = "ip-not-allowed")]
    IpNotAllowed,
    #[serde(rename = "toss-key-shared")]
    TossKeyShared,
    #[serde(rename = "claude-missing")]
    ClaudeMissing,
    #[serde(rename = "claude-signed-out")]
    ClaudeSignedOut,
    #[serde(rename = "order-risk-notice")]
    OrderRiskNotice,
    #[serde(rename = "order-exchange-setting")]
    OrderExchangeSetting,
    #[serde(rename = "order-dollar-shortage")]
    OrderDollarShortage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiSelfCheckView {
    pub problem: UiSelfCheckProblem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiAiRuntime {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "missing")]
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSourceStatus {
    pub source: String,
    pub last_success: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiStatusView {
    pub ai_runtime: UiAiRuntime,
    pub ai_resumes_at: Option<String>,
    pub sources: Vec<UiSourceStatus>,
}

const fn toss_records(
    state: Option<TossConnectionState>,
) -> (
    CheckState<TossKeyProblem>,
    CheckState<IpProblem>,
    CheckState<DuplicateKeyProblem>,
) {
    match state {
        Some(TossConnectionState::InvalidKey) => (
            CheckState::Problem(TossKeyProblem::InvalidKey),
            CheckState::Healthy,
            CheckState::Healthy,
        ),
        Some(TossConnectionState::Forbidden) => (
            CheckState::Problem(TossKeyProblem::Permissions),
            CheckState::Healthy,
            CheckState::Healthy,
        ),
        Some(TossConnectionState::AccountNotFound) => (
            CheckState::Problem(TossKeyProblem::AccountNotFound),
            CheckState::Healthy,
            CheckState::Healthy,
        ),
        Some(TossConnectionState::IpNotAllowed) => (
            CheckState::Healthy,
            CheckState::Problem(IpProblem::NotAllowed),
            CheckState::Healthy,
        ),
        Some(TossConnectionState::KeyInUse) => (
            CheckState::Healthy,
            CheckState::Healthy,
            CheckState::Problem(DuplicateKeyProblem::CompetingProgram),
        ),
        Some(TossConnectionState::Connected | TossConnectionState::NeedsKeys) | None => (
            CheckState::Healthy,
            CheckState::Healthy,
            CheckState::Healthy,
        ),
    }
}

#[must_use]
pub fn self_check_view(
    toss: Option<TossConnectionState>,
    claude: ClaudeInstall,
) -> UiSelfCheckView {
    let (toss_key, allowed_ip, duplicate_key_use) = toss_records(toss);
    let claude_install = match claude {
        ClaudeInstall::Installed => CheckState::Healthy,
        ClaudeInstall::Missing => CheckState::Problem(ClaudeInstallProblem::NotInstalled),
    };
    let result = evaluate_self_check(
        SelfCheckInput {
            toss_key,
            allowed_ip,
            duplicate_key_use,
            claude_install,
            claude_login: CheckState::<ClaudeLoginProblem>::Healthy,
            ai_environment: CheckState::Healthy,
            ai_usage: application::AiUsage::Unknown,
            order_prerequisite: CheckState::Healthy,
        },
        Vec::new(),
    );
    let problem = match result.banner {
        None => UiSelfCheckProblem::None,
        Some(SelfCheckAction::TossKey(TossKeyProblem::InvalidKey)) => {
            UiSelfCheckProblem::TossKeyInvalid
        }
        Some(SelfCheckAction::TossKey(TossKeyProblem::Permissions)) => {
            UiSelfCheckProblem::TossKeyPermission
        }
        Some(SelfCheckAction::TossKey(TossKeyProblem::AccountNotFound)) => {
            UiSelfCheckProblem::TossAccountMissing
        }
        Some(SelfCheckAction::RegisterIp) => UiSelfCheckProblem::IpNotAllowed,
        Some(SelfCheckAction::StopOtherProgram) => UiSelfCheckProblem::TossKeyShared,
        Some(SelfCheckAction::InstallClaudeCode) => UiSelfCheckProblem::ClaudeMissing,
        Some(SelfCheckAction::LoginClaudeCode) => UiSelfCheckProblem::ClaudeSignedOut,
        Some(SelfCheckAction::OrderPrerequisite(
            application::OrderPrerequisiteProblem::RiskNotice,
        )) => UiSelfCheckProblem::OrderRiskNotice,
        Some(SelfCheckAction::OrderPrerequisite(
            application::OrderPrerequisiteProblem::ExchangeIntegration,
        )) => UiSelfCheckProblem::OrderExchangeSetting,
        Some(SelfCheckAction::OrderPrerequisite(
            application::OrderPrerequisiteProblem::DollarBuyingPower,
        )) => UiSelfCheckProblem::OrderDollarShortage,
    };
    UiSelfCheckView { problem }
}

const fn source_code(source: DataSource) -> &'static str {
    match source {
        DataSource::KindDisclosure => "kind-disclosure",
        DataSource::DartDisclosureApi => "dart-disclosure-api",
        DataSource::DartDisclosureRss => "dart-disclosure-rss",
        DataSource::DartFundamentals => "dart-fundamentals",
        DataSource::KindCorpList => "kind-corp-list",
        DataSource::Hankyung => "hankyung",
        DataSource::Maekyung => "maekyung",
        DataSource::Yonhap => "yonhap",
        DataSource::Edaily => "edaily",
        DataSource::GoogleNews => "google-news",
        DataSource::WebSearch => "web-search",
        DataSource::YahooFinance => "yahoo-finance",
        DataSource::SecEdgar => "sec-edgar",
        DataSource::BankOfKorea => "bank-of-korea",
        DataSource::FederalReserve => "federal-reserve",
        DataSource::UsTreasury => "us-treasury",
        DataSource::Fred => "fred",
    }
}

#[must_use]
pub fn status_view(
    claude: ClaudeInstall,
    limit: AiLimit,
    statuses: &[SourceStatus],
) -> UiStatusView {
    let mut rows: Vec<UiSourceStatus> = statuses
        .iter()
        .map(|status| UiSourceStatus {
            source: String::from(source_code(status.source())),
            last_success: status.last_success_at().and_then(kst_iso),
        })
        .collect();
    rows.sort_by(|left, right| left.source.cmp(&right.source));
    UiStatusView {
        ai_runtime: match claude {
            ClaudeInstall::Installed => UiAiRuntime::Ready,
            ClaudeInstall::Missing => UiAiRuntime::Missing,
        },
        ai_resumes_at: match limit {
            AiLimit::Available => None,
            AiLimit::Waiting(at) => kst_iso(at),
        },
        sources: rows,
    }
}

fn managed<R: Runtime>(app: &AppHandle<R>) -> Result<SelfCheckIpcState, IpcError> {
    app.try_state::<SelfCheckIpcState>()
        .map(|state| state.inner().clone())
        .ok_or(IpcError {
            kind: IpcErrorKind::NoData,
        })
}

#[tauri::command]
#[specta::specta]
pub async fn get_self_check<R: Runtime>(
    app: AppHandle<R>,
    request: GetSelfCheckRequest,
) -> Result<UiSelfCheckView, IpcError> {
    let GetSelfCheckRequest {
        scope: SelfCheckScope::Current,
    } = request;
    let state = managed(&app)?;
    let link = app
        .try_state::<TossLinkState>()
        .map(|managed_link| managed_link.inner().clone());
    let toss = match link {
        Some(found) => found.backend().check().await.ok(),
        None => None,
    };
    Ok(self_check_view(toss, state.ai.install()))
}

#[tauri::command]
#[specta::specta]
pub async fn get_status<R: Runtime>(
    app: AppHandle<R>,
    request: GetStatusRequest,
) -> Result<UiStatusView, IpcError> {
    let GetStatusRequest {
        scope: SelfCheckScope::Current,
    } = request;
    let state = managed(&app)?;
    let statuses = state
        .statuses
        .list()
        .await
        .map_err(|error| IpcError::from(error.class()))?;
    let limit = state.ai.limit().await;
    Ok(status_view(state.ai.install(), limit, &statuses))
}

#[cfg(test)]
mod tests;
