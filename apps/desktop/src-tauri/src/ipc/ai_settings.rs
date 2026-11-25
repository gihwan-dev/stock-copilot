use std::fmt;
use std::sync::Arc;

use application::{
    AiEffortChoice, AiModelChoice, AiPreferences, AiPreferencesRepository, AiTask,
    AiTaskPreference, AiUsage, ClaudeConfigDir, EventBus, ScheduledBudgetPercent, SystemPortError,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};
use super::portfolio::precision;
use super::state::BoxFuture;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetApplied {
    Applied,
    Refused,
}

pub trait AiControls: Send + Sync {
    fn apply_budget(&self, percent: ScheduledBudgetPercent) -> BoxFuture<'_, BudgetApplied>;
    fn usage(&self) -> BoxFuture<'_, AiUsage>;
}

pub trait AiPreferencesBook: Send + Sync {
    fn load(&self) -> BoxFuture<'_, Result<Option<AiPreferences>, SystemPortError>>;
    fn save(&self, preferences: AiPreferences) -> BoxFuture<'_, Result<(), SystemPortError>>;
}

struct StoredPreferences<R> {
    repository: Arc<R>,
    events: Arc<EventBus>,
}

impl<R: AiPreferencesRepository + 'static> AiPreferencesBook for StoredPreferences<R> {
    fn load(&self) -> BoxFuture<'_, Result<Option<AiPreferences>, SystemPortError>> {
        Box::pin(async move { self.repository.load_ai_preferences().await })
    }

    fn save(&self, preferences: AiPreferences) -> BoxFuture<'_, Result<(), SystemPortError>> {
        Box::pin(async move {
            let _written = self.repository.store_ai_preferences(&preferences).await?;
            let _delivery = self.events.publish(domain::DomainEvent::SettingsChanged);
            Ok(())
        })
    }
}

#[derive(Clone)]
pub struct AiSettingsIpcState {
    book: Arc<dyn AiPreferencesBook>,
    controls: Arc<dyn AiControls>,
}

impl fmt::Debug for AiSettingsIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AiSettingsIpcState")
    }
}

impl AiSettingsIpcState {
    #[must_use]
    pub fn new<R: AiPreferencesRepository + 'static>(
        repository: Arc<R>,
        events: Arc<EventBus>,
        controls: Arc<dyn AiControls>,
    ) -> Self {
        Self {
            book: Arc::new(StoredPreferences { repository, events }),
            controls,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiAiTask {
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "estimate")]
    Estimate,
    #[serde(rename = "deep-research")]
    DeepResearch,
    #[serde(rename = "briefing")]
    Briefing,
    #[serde(rename = "news-summary")]
    NewsSummary,
    #[serde(rename = "collection")]
    Collection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiAiModel {
    #[serde(rename = "opus")]
    Opus,
    #[serde(rename = "sonnet")]
    Sonnet,
    #[serde(rename = "haiku")]
    Haiku,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiAiEffort {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    #[serde(rename = "max")]
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum UiBudgetPercent {
    #[serde(rename = "50")]
    Fifty,
    #[serde(rename = "60")]
    Sixty,
    #[serde(rename = "70")]
    Seventy,
    #[serde(rename = "80")]
    Eighty,
    #[serde(rename = "90")]
    Ninety,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct UiTaskSetting {
    pub task: UiAiTask,
    pub model: UiAiModel,
    pub effort: UiAiEffort,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct UiTaskSettingInput {
    pub task: UiAiTask,
    pub model: UiAiModel,
    pub effort: UiAiEffort,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiAiSettings {
    pub tasks: Vec<UiTaskSetting>,
    pub budget_percent: UiBudgetPercent,
    pub usage_percent: Option<String>,
    pub claude_config_dir: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum AiScope {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct AiRequest {
    pub scope: AiScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetTaskModelRequest {
    pub setting: UiTaskSettingInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetBudgetPercentRequest {
    pub percent: UiBudgetPercent,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SetClaudeConfigDirRequest {
    pub path: Option<String>,
}

const fn task(value: UiAiTask) -> AiTask {
    match value {
        UiAiTask::Chat => AiTask::Chat,
        UiAiTask::Estimate => AiTask::Estimate,
        UiAiTask::DeepResearch => AiTask::DeepResearch,
        UiAiTask::Briefing => AiTask::Briefing,
        UiAiTask::NewsSummary => AiTask::NewsSummary,
        UiAiTask::Collection => AiTask::Collection,
    }
}

const fn ui_task(value: AiTask) -> UiAiTask {
    match value {
        AiTask::Chat => UiAiTask::Chat,
        AiTask::Estimate => UiAiTask::Estimate,
        AiTask::DeepResearch => UiAiTask::DeepResearch,
        AiTask::Briefing => UiAiTask::Briefing,
        AiTask::NewsSummary => UiAiTask::NewsSummary,
        AiTask::Collection => UiAiTask::Collection,
    }
}

const fn model(value: UiAiModel) -> AiModelChoice {
    match value {
        UiAiModel::Opus => AiModelChoice::Opus,
        UiAiModel::Sonnet => AiModelChoice::Sonnet,
        UiAiModel::Haiku => AiModelChoice::Haiku,
    }
}

const fn ui_model(value: AiModelChoice) -> UiAiModel {
    match value {
        AiModelChoice::Opus => UiAiModel::Opus,
        AiModelChoice::Sonnet => UiAiModel::Sonnet,
        AiModelChoice::Haiku => UiAiModel::Haiku,
    }
}

const fn effort(value: UiAiEffort) -> AiEffortChoice {
    match value {
        UiAiEffort::Low => AiEffortChoice::Low,
        UiAiEffort::Medium => AiEffortChoice::Medium,
        UiAiEffort::High => AiEffortChoice::High,
        UiAiEffort::XHigh => AiEffortChoice::XHigh,
        UiAiEffort::Max => AiEffortChoice::Max,
    }
}

const fn ui_effort(value: AiEffortChoice) -> UiAiEffort {
    match value {
        AiEffortChoice::Low => UiAiEffort::Low,
        AiEffortChoice::Medium => UiAiEffort::Medium,
        AiEffortChoice::High => UiAiEffort::High,
        AiEffortChoice::XHigh => UiAiEffort::XHigh,
        AiEffortChoice::Max => UiAiEffort::Max,
    }
}

const fn budget(value: UiBudgetPercent) -> ScheduledBudgetPercent {
    match value {
        UiBudgetPercent::Fifty => ScheduledBudgetPercent::Fifty,
        UiBudgetPercent::Sixty => ScheduledBudgetPercent::Sixty,
        UiBudgetPercent::Seventy => ScheduledBudgetPercent::Seventy,
        UiBudgetPercent::Eighty => ScheduledBudgetPercent::Eighty,
        UiBudgetPercent::Ninety => ScheduledBudgetPercent::Ninety,
    }
}

const fn ui_budget(value: ScheduledBudgetPercent) -> UiBudgetPercent {
    match value {
        ScheduledBudgetPercent::Fifty => UiBudgetPercent::Fifty,
        ScheduledBudgetPercent::Sixty => UiBudgetPercent::Sixty,
        ScheduledBudgetPercent::Seventy => UiBudgetPercent::Seventy,
        ScheduledBudgetPercent::Eighty => UiBudgetPercent::Eighty,
        ScheduledBudgetPercent::Ninety => UiBudgetPercent::Ninety,
    }
}

fn usage_text(usage: &AiUsage) -> Option<String> {
    match usage {
        AiUsage::Available(utilization) => Some(
            domain::display_probability_percent(*utilization.as_probability(), precision(0)?)
                .as_str()
                .to_owned(),
        ),
        AiUsage::Unknown | AiUsage::Waiting(_) => None,
    }
}

fn to_view(preferences: &AiPreferences, usage: &AiUsage) -> UiAiSettings {
    UiAiSettings {
        tasks: preferences
            .tasks()
            .iter()
            .map(|preference| UiTaskSetting {
                task: ui_task(preference.task),
                model: ui_model(preference.model),
                effort: ui_effort(preference.effort),
            })
            .collect(),
        budget_percent: ui_budget(preferences.budget()),
        usage_percent: usage_text(usage),
        claude_config_dir: preferences.config_dir().map(|dir| dir.as_str().to_owned()),
    }
}

const fn error(kind: IpcErrorKind) -> IpcError {
    IpcError { kind }
}

fn storage(failure: &SystemPortError) -> IpcError {
    IpcError::from(application::ClassifiedError::class(failure))
}

async fn current(state: &AiSettingsIpcState) -> Result<AiPreferences, IpcError> {
    Ok(state
        .book
        .load()
        .await
        .map_err(|failure| storage(&failure))?
        .unwrap_or_else(AiPreferences::initial))
}

async fn answer(
    state: &AiSettingsIpcState,
    preferences: &AiPreferences,
) -> Result<UiAiSettings, IpcError> {
    match state.controls.apply_budget(preferences.budget()).await {
        BudgetApplied::Applied => Ok(to_view(preferences, &state.controls.usage().await)),
        BudgetApplied::Refused => Err(error(IpcErrorKind::Bug)),
    }
}

async fn change(
    state: &AiSettingsIpcState,
    edit: impl FnOnce(&mut AiPreferences),
) -> Result<UiAiSettings, IpcError> {
    let mut preferences = current(state).await?;
    edit(&mut preferences);
    state
        .book
        .save(preferences.clone())
        .await
        .map_err(|failure| storage(&failure))?;
    answer(state, &preferences).await
}

pub async fn load_ai_settings(state: &AiSettingsIpcState) -> Result<UiAiSettings, IpcError> {
    let preferences = current(state).await?;
    answer(state, &preferences).await
}

pub async fn choose_task_model(
    state: &AiSettingsIpcState,
    setting: UiTaskSettingInput,
) -> Result<UiAiSettings, IpcError> {
    change(state, |preferences| {
        preferences.set_task(AiTaskPreference {
            task: task(setting.task),
            model: model(setting.model),
            effort: effort(setting.effort),
        });
    })
    .await
}

pub async fn choose_budget(
    state: &AiSettingsIpcState,
    percent: UiBudgetPercent,
) -> Result<UiAiSettings, IpcError> {
    change(state, |preferences| preferences.set_budget(budget(percent))).await
}

pub async fn choose_config_dir(
    state: &AiSettingsIpcState,
    path: Option<&str>,
) -> Result<UiAiSettings, IpcError> {
    let dir = match path {
        Some(text) => Some(
            ClaudeConfigDir::try_new(text).map_err(|_invalid| error(IpcErrorKind::UserFixable))?,
        ),
        None => None,
    };
    change(state, |preferences| preferences.set_config_dir(dir)).await
}

fn managed<R: Runtime>(app: &AppHandle<R>) -> Result<AiSettingsIpcState, IpcError> {
    app.try_state::<AiSettingsIpcState>()
        .map(|managed_state| managed_state.inner().clone())
        .ok_or(error(IpcErrorKind::NoData))
}

#[tauri::command]
#[specta::specta]
pub async fn get_ai_settings<R: Runtime>(
    app: AppHandle<R>,
    request: AiRequest,
) -> Result<UiAiSettings, IpcError> {
    let AiRequest {
        scope: AiScope::Current,
    } = request;
    load_ai_settings(&managed(&app)?).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_task_model<R: Runtime>(
    app: AppHandle<R>,
    request: SetTaskModelRequest,
) -> Result<UiAiSettings, IpcError> {
    choose_task_model(&managed(&app)?, request.setting).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_budget_percent<R: Runtime>(
    app: AppHandle<R>,
    request: SetBudgetPercentRequest,
) -> Result<UiAiSettings, IpcError> {
    choose_budget(&managed(&app)?, request.percent).await
}

#[tauri::command]
#[specta::specta]
pub async fn set_claude_config_dir<R: Runtime>(
    app: AppHandle<R>,
    request: SetClaudeConfigDirRequest,
) -> Result<UiAiSettings, IpcError> {
    choose_config_dir(&managed(&app)?, request.path.as_deref()).await
}
