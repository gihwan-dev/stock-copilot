use std::fs;
use std::future::Future;
use std::path::Path;
use std::sync::{Arc, RwLock};

use application::{
    AiPreferences, AiPreferencesRepository, AiUsage, EventBus, ScheduledBudgetPercent,
    SystemPortError, Utilization, WriteCompleted,
};
use domain::{AiProbability, AiProbabilityInput};
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::{
    AiControls, AiRequest, AiScope, AiSettingsIpcState, BudgetApplied, UiAiEffort, UiAiModel,
    UiAiTask, UiBudgetPercent, UiTaskSettingInput, choose_budget, choose_config_dir,
    choose_task_model, get_ai_settings,
};
use crate::ipc::error::IpcErrorKind;
use crate::ipc::state::BoxFuture;

#[derive(Default)]
struct MemoryPreferences(RwLock<Option<AiPreferences>>);

impl AiPreferencesRepository for MemoryPreferences {
    fn load_ai_preferences(
        &self,
    ) -> impl Future<Output = Result<Option<AiPreferences>, SystemPortError>> + Send {
        std::future::ready(Ok(self.0.read().expect("잠금").clone()))
    }

    fn store_ai_preferences(
        &self,
        preferences: &AiPreferences,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        *self.0.write().expect("잠금") = Some(preferences.clone());
        std::future::ready(Ok(WriteCompleted))
    }
}

#[derive(Default)]
struct FakeControls {
    applied: RwLock<Vec<ScheduledBudgetPercent>>,
    known: RwLock<Option<()>>,
}

impl AiControls for FakeControls {
    fn apply_budget(&self, percent: ScheduledBudgetPercent) -> BoxFuture<'_, BudgetApplied> {
        self.applied.write().expect("잠금").push(percent);
        Box::pin(std::future::ready(BudgetApplied::Applied))
    }

    fn usage(&self) -> BoxFuture<'_, AiUsage> {
        let known = *self.known.read().expect("잠금");
        let usage = match known {
            Some(()) => AiUsage::Available(Utilization::new(
                AiProbability::parse(&AiProbabilityInput::try_new("42").expect("입력"))
                    .expect("사용률")
                    .into_probability(),
            )),
            None => AiUsage::Unknown,
        };
        Box::pin(std::future::ready(usage))
    }
}

fn state(controls: Arc<FakeControls>) -> AiSettingsIpcState {
    AiSettingsIpcState::new(
        Arc::new(MemoryPreferences::default()),
        Arc::new(EventBus::default()),
        controls,
    )
}

fn assert_snapshot<T: serde::Serialize>(name: &str, value: &T) {
    let path = Path::new("ipc/responses").join(format!("{name}.json"));
    let mut actual = serde_json::to_string_pretty(value).expect("serialize");
    actual.push('\n');
    let recorded = fs::read_to_string(&path).expect("read recorded response");
    assert_eq!(recorded, actual, "{} differs", path.display());
}

#[test]
fn initial_settings_show_defaults_apply_the_budget_and_leave_unknown_usage_empty() {
    let controls = Arc::new(FakeControls::default());
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let _state = tauri::Manager::manage(&app, state(Arc::clone(&controls)));
    let view = tauri::async_runtime::block_on(get_ai_settings(
        app.handle().clone(),
        AiRequest {
            scope: AiScope::Current,
        },
    ))
    .expect("AI 설정");
    assert_eq!(view.tasks.len(), 6);
    assert_eq!(view.budget_percent, UiBudgetPercent::Seventy);
    assert_eq!(view.usage_percent, None);
    assert_eq!(
        *controls.applied.read().expect("잠금"),
        vec![ScheduledBudgetPercent::Seventy]
    );
    assert_snapshot("get_ai_settings", &view);
}

#[test]
fn task_budget_and_config_changes_are_saved_applied_and_validated() {
    let controls = Arc::new(FakeControls::default());
    *controls.known.write().expect("잠금") = Some(());
    let ai = state(Arc::clone(&controls));
    let chosen = tauri::async_runtime::block_on(choose_task_model(
        &ai,
        UiTaskSettingInput {
            task: UiAiTask::Briefing,
            model: UiAiModel::Haiku,
            effort: UiAiEffort::Max,
        },
    ))
    .expect("모델");
    let briefing = chosen
        .tasks
        .iter()
        .find(|row| row.task == UiAiTask::Briefing)
        .expect("브리핑");
    assert_eq!(briefing.model, UiAiModel::Haiku);
    assert_eq!(chosen.usage_percent.as_deref(), Some("42"));
    let budgeted =
        tauri::async_runtime::block_on(choose_budget(&ai, UiBudgetPercent::Ninety)).expect("한도");
    assert_eq!(budgeted.budget_percent, UiBudgetPercent::Ninety);
    assert_eq!(
        controls.applied.read().expect("잠금").last(),
        Some(&ScheduledBudgetPercent::Ninety)
    );
    let located = tauri::async_runtime::block_on(choose_config_dir(&ai, Some("/Users/me/.claude")))
        .expect("위치");
    assert_eq!(
        located.claude_config_dir.as_deref(),
        Some("/Users/me/.claude")
    );
    let relative = tauri::async_runtime::block_on(choose_config_dir(&ai, Some("claude")))
        .expect_err("상대 경로");
    assert_eq!(relative.kind, IpcErrorKind::UserFixable);
    let cleared = tauri::async_runtime::block_on(choose_config_dir(&ai, None)).expect("기본 위치");
    assert_eq!(cleared.claude_config_dir, None);
}
