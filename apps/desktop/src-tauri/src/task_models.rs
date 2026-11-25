use application::{
    AiEffortChoice, AiModelChoice, AiPreferences, AiTask, ModelName, ModelSelection,
    ReasoningEffort, RunSpec,
};

#[cfg(test)]
mod tests;

const OPUS: &str = "claude-opus-5-5";
const SONNET: &str = "claude-sonnet-5-5";
const HAIKU: &str = "claude-haiku-5-5";

fn model(choice: AiModelChoice) -> Option<ModelSelection> {
    let name = match choice {
        AiModelChoice::Opus => OPUS,
        AiModelChoice::Sonnet => SONNET,
        AiModelChoice::Haiku => HAIKU,
    };
    ModelName::try_new(name).ok().map(ModelSelection::Named)
}

const fn effort(choice: AiEffortChoice) -> ReasoningEffort {
    match choice {
        AiEffortChoice::Low => ReasoningEffort::Low,
        AiEffortChoice::Medium => ReasoningEffort::Medium,
        AiEffortChoice::High => ReasoningEffort::High,
        AiEffortChoice::XHigh => ReasoningEffort::XHigh,
        AiEffortChoice::Max => ReasoningEffort::Max,
    }
}

#[must_use]
pub fn tuned(preferences: &AiPreferences, spec: RunSpec) -> RunSpec {
    let task = AiTask::for_purpose(spec.purpose());
    let Some(chosen) = preferences.chosen(task) else {
        return spec;
    };
    let caller_picked_model = task == AiTask::Chat && *spec.model() != ModelSelection::Default;
    let caller_picked_effort = task == AiTask::Chat && spec.effort() != ReasoningEffort::Default;
    let selection = if caller_picked_model {
        spec.model().clone()
    } else {
        model(chosen.model).unwrap_or_else(|| spec.model().clone())
    };
    let strength = if caller_picked_effort {
        spec.effort()
    } else {
        effort(chosen.effort)
    };
    RunSpec::new(
        spec.id().clone(),
        spec.input().clone(),
        selection,
        strength,
        spec.purpose(),
    )
}
