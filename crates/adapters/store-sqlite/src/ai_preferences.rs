use crate::{Database, sql::ai_preferences as sql};
use application::{
    AiEffortChoice, AiModelChoice, AiPreferences, AiPreferencesRepository, AiTask,
    AiTaskPreference, ClaudeConfigDir, ScheduledBudgetPercent, SystemPortError, WriteCompleted,
};
use rusqlite::OptionalExtension;

#[derive(Debug)]
pub struct AiPreferencesStore<'database> {
    database: &'database Database,
}

impl<'database> AiPreferencesStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}

fn invalid<E>(_: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}

const fn task_name(task: AiTask) -> &'static str {
    match task {
        AiTask::Chat => "chat",
        AiTask::Estimate => "estimate",
        AiTask::DeepResearch => "deep-research",
        AiTask::Briefing => "briefing",
        AiTask::NewsSummary => "news-summary",
        AiTask::Collection => "collection",
    }
}

fn task(raw: &str) -> rusqlite::Result<AiTask> {
    AiTask::ALL
        .into_iter()
        .find(|candidate| task_name(*candidate) == raw)
        .ok_or_else(|| invalid(()))
}

const fn model_name(model: AiModelChoice) -> &'static str {
    match model {
        AiModelChoice::Opus => "opus",
        AiModelChoice::Sonnet => "sonnet",
        AiModelChoice::Haiku => "haiku",
    }
}

fn model(raw: &str) -> rusqlite::Result<AiModelChoice> {
    [
        AiModelChoice::Opus,
        AiModelChoice::Sonnet,
        AiModelChoice::Haiku,
    ]
    .into_iter()
    .find(|candidate| model_name(*candidate) == raw)
    .ok_or_else(|| invalid(()))
}

const fn effort_name(effort: AiEffortChoice) -> &'static str {
    match effort {
        AiEffortChoice::Low => "low",
        AiEffortChoice::Medium => "medium",
        AiEffortChoice::High => "high",
        AiEffortChoice::XHigh => "xhigh",
        AiEffortChoice::Max => "max",
    }
}

fn effort(raw: &str) -> rusqlite::Result<AiEffortChoice> {
    [
        AiEffortChoice::Low,
        AiEffortChoice::Medium,
        AiEffortChoice::High,
        AiEffortChoice::XHigh,
        AiEffortChoice::Max,
    ]
    .into_iter()
    .find(|candidate| effort_name(*candidate) == raw)
    .ok_or_else(|| invalid(()))
}

const fn budget_text(value: ScheduledBudgetPercent) -> &'static str {
    match value {
        ScheduledBudgetPercent::Fifty => "50",
        ScheduledBudgetPercent::Sixty => "60",
        ScheduledBudgetPercent::Seventy => "70",
        ScheduledBudgetPercent::Eighty => "80",
        ScheduledBudgetPercent::Ninety => "90",
    }
}

fn budget(raw: &str) -> rusqlite::Result<ScheduledBudgetPercent> {
    [
        ScheduledBudgetPercent::Fifty,
        ScheduledBudgetPercent::Sixty,
        ScheduledBudgetPercent::Seventy,
        ScheduledBudgetPercent::Eighty,
        ScheduledBudgetPercent::Ninety,
    ]
    .into_iter()
    .find(|candidate| budget_text(*candidate) == raw)
    .ok_or_else(|| invalid(()))
}

pub(crate) fn encode(preferences: &AiPreferences) -> String {
    let mut payload = String::from("budget:");
    payload.push_str(budget_text(preferences.budget()));
    payload.push_str("\ndir:");
    payload.push_str(preferences.config_dir().map_or("", ClaudeConfigDir::as_str));
    for preference in preferences.chosen_tasks() {
        for part in [
            "\ntask:",
            task_name(preference.task),
            ":",
            model_name(preference.model),
            ":",
            effort_name(preference.effort),
        ] {
            payload.push_str(part);
        }
    }
    payload
}

pub(crate) fn decode(payload: &str) -> rusqlite::Result<AiPreferences> {
    let mut lines = payload.lines();
    let chosen_budget = budget(
        lines
            .next()
            .and_then(|line| line.strip_prefix("budget:"))
            .ok_or_else(|| invalid(()))?,
    )?;
    let raw_dir = lines
        .next()
        .and_then(|line| line.strip_prefix("dir:"))
        .ok_or_else(|| invalid(()))?;
    let config_dir = if raw_dir.is_empty() {
        None
    } else {
        Some(ClaudeConfigDir::try_new(raw_dir).map_err(invalid)?)
    };
    let tasks = lines
        .map(|line| {
            let fields: Vec<&str> = line
                .strip_prefix("task:")
                .ok_or_else(|| invalid(()))?
                .split(':')
                .collect();
            match fields.as_slice() {
                [name, chosen_model, chosen_effort] => Ok(AiTaskPreference {
                    task: task(name)?,
                    model: model(chosen_model)?,
                    effort: effort(chosen_effort)?,
                }),
                _ => Err(invalid(())),
            }
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(AiPreferences::restore(&tasks, chosen_budget, config_dir))
}

impl AiPreferencesRepository for AiPreferencesStore<'_> {
    async fn load_ai_preferences(&self) -> Result<Option<AiPreferences>, SystemPortError> {
        self.database
            .read(|connection| {
                let stored: Option<String> = connection
                    .query_row(sql::LOAD, [], |row| row.get(0))
                    .optional()?;
                stored.as_deref().map(decode).transpose()
            })
            .await
    }

    async fn store_ai_preferences(
        &self,
        preferences: &AiPreferences,
    ) -> Result<WriteCompleted, SystemPortError> {
        let payload = encode(preferences);
        self.database
            .write(move |transaction| {
                transaction.execute(sql::STORE, [payload.as_str()])?;
                Ok(WriteCompleted)
            })
            .await
    }
}

#[cfg(test)]
mod tests;
