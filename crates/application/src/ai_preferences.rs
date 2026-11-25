use std::future::Future;

use crate::{SystemPortError, WriteCompleted};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AiTask {
    Chat,
    Estimate,
    DeepResearch,
    Briefing,
    NewsSummary,
    Collection,
}

impl AiTask {
    pub const ALL: [Self; 6] = [
        Self::Chat,
        Self::Estimate,
        Self::DeepResearch,
        Self::Briefing,
        Self::NewsSummary,
        Self::Collection,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiModelChoice {
    Opus,
    Sonnet,
    Haiku,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiEffortChoice {
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduledBudgetPercent {
    Fifty,
    Sixty,
    Seventy,
    Eighty,
    Ninety,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AiTaskPreference {
    pub task: AiTask,
    pub model: AiModelChoice,
    pub effort: AiEffortChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeConfigDirError {
    NotAbsolute,
    TooLong,
    InvalidCharacter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeConfigDir(String);

impl ClaudeConfigDir {
    pub fn try_new(value: &str) -> Result<Self, ClaudeConfigDirError> {
        let trimmed = value.trim();
        if !trimmed.starts_with('/') {
            return Err(ClaudeConfigDirError::NotAbsolute);
        }
        if trimmed.len() > 1024 {
            return Err(ClaudeConfigDirError::TooLong);
        }
        if trimmed
            .chars()
            .any(|character| character.is_control() || character == ';')
        {
            return Err(ClaudeConfigDirError::InvalidCharacter);
        }
        Ok(Self(trimmed.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiPreferences {
    tasks: Vec<AiTaskPreference>,
    budget: ScheduledBudgetPercent,
    config_dir: Option<ClaudeConfigDir>,
}

const fn default_task(task: AiTask) -> AiTaskPreference {
    let (model, effort) = match task {
        AiTask::Chat => (AiModelChoice::Opus, AiEffortChoice::Medium),
        AiTask::Estimate | AiTask::DeepResearch => (AiModelChoice::Opus, AiEffortChoice::High),
        AiTask::Briefing => (AiModelChoice::Sonnet, AiEffortChoice::Medium),
        AiTask::NewsSummary | AiTask::Collection => (AiModelChoice::Haiku, AiEffortChoice::Low),
    };
    AiTaskPreference {
        task,
        model,
        effort,
    }
}

impl AiPreferences {
    #[must_use]
    pub fn initial() -> Self {
        Self {
            tasks: AiTask::ALL.into_iter().map(default_task).collect(),
            budget: ScheduledBudgetPercent::Seventy,
            config_dir: None,
        }
    }

    #[must_use]
    pub fn restore(
        tasks: &[AiTaskPreference],
        budget: ScheduledBudgetPercent,
        config_dir: Option<ClaudeConfigDir>,
    ) -> Self {
        let mut restored = Self {
            tasks: AiTask::ALL.into_iter().map(default_task).collect(),
            budget,
            config_dir,
        };
        for preference in tasks {
            restored.set_task(*preference);
        }
        restored
    }

    #[must_use]
    pub fn tasks(&self) -> &[AiTaskPreference] {
        &self.tasks
    }

    #[must_use]
    pub const fn budget(&self) -> ScheduledBudgetPercent {
        self.budget
    }

    #[must_use]
    pub const fn config_dir(&self) -> Option<&ClaudeConfigDir> {
        self.config_dir.as_ref()
    }

    pub fn set_task(&mut self, preference: AiTaskPreference) {
        for slot in &mut self.tasks {
            if slot.task == preference.task {
                *slot = preference;
            }
        }
    }

    pub const fn set_budget(&mut self, budget: ScheduledBudgetPercent) {
        self.budget = budget;
    }

    pub fn set_config_dir(&mut self, config_dir: Option<ClaudeConfigDir>) {
        self.config_dir = config_dir;
    }
}

pub trait AiPreferencesRepository: Send + Sync {
    fn load_ai_preferences(
        &self,
    ) -> impl Future<Output = Result<Option<AiPreferences>, SystemPortError>> + Send;
    fn store_ai_preferences(
        &self,
        preferences: &AiPreferences,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[cfg(test)]
mod tests;
