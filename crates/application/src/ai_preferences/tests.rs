use super::{
    AiEffortChoice, AiModelChoice, AiPreferences, AiTask, AiTaskPreference, ClaudeConfigDir,
    ClaudeConfigDirError, ScheduledBudgetPercent,
};

#[test]
fn initial_preferences_cover_every_task_once_with_a_seventy_percent_budget() {
    let initial = AiPreferences::initial();
    assert_eq!(initial.tasks().len(), AiTask::ALL.len());
    assert_eq!(initial.budget(), ScheduledBudgetPercent::Seventy);
    assert_eq!(initial.config_dir(), None);
}

#[test]
fn setting_a_task_replaces_only_that_task_and_restore_fills_missing_tasks() {
    let mut preferences = AiPreferences::initial();
    let chosen = AiTaskPreference {
        task: AiTask::Briefing,
        model: AiModelChoice::Haiku,
        effort: AiEffortChoice::Max,
    };
    preferences.set_task(chosen);
    assert_eq!(
        preferences
            .tasks()
            .iter()
            .filter(|slot| **slot == chosen)
            .count(),
        1
    );
    assert_eq!(preferences.tasks().len(), AiTask::ALL.len());
    let restored = AiPreferences::restore(&[chosen], ScheduledBudgetPercent::Ninety, None);
    assert_eq!(restored.tasks(), preferences.tasks());
    assert_eq!(restored.budget(), ScheduledBudgetPercent::Ninety);
}

#[test]
fn config_dir_must_be_an_absolute_single_line_path() {
    assert_eq!(
        ClaudeConfigDir::try_new(" /Users/me/.claude ")
            .expect("경로")
            .as_str(),
        "/Users/me/.claude"
    );
    assert_eq!(
        ClaudeConfigDir::try_new("relative/path"),
        Err(ClaudeConfigDirError::NotAbsolute)
    );
    assert_eq!(
        ClaudeConfigDir::try_new("/a\nb"),
        Err(ClaudeConfigDirError::InvalidCharacter)
    );
}
