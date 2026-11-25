use application::{
    AiEffortChoice, AiModelChoice, AiPreferences, AiRunId, AiTask, AiTaskPreference,
    AnalysisPrompt, ModelName, ModelSelection, ReasoningEffort, RunInput, RunPurpose, RunSpec,
};

use super::tuned;

fn spec(purpose: RunPurpose, model: ModelSelection, effort: ReasoningEffort) -> RunSpec {
    RunSpec::new(
        AiRunId::try_new("run-1").expect("id"),
        RunInput::Analysis(AnalysisPrompt::try_new("check").expect("prompt")),
        model,
        effort,
        purpose,
    )
}

fn named(text: &str) -> ModelSelection {
    ModelSelection::Named(ModelName::try_new(text).expect("모델"))
}

fn with(task: AiTask, model: AiModelChoice, effort: AiEffortChoice) -> AiPreferences {
    let mut preferences = AiPreferences::initial();
    preferences.set_task(AiTaskPreference {
        task,
        model,
        effort,
    });
    preferences
}

#[test]
fn without_a_chosen_task_the_caller_defaults_stay() {
    let original = spec(
        RunPurpose::MorningBriefing,
        ModelSelection::Fast,
        ReasoningEffort::Low,
    );
    assert_eq!(tuned(&AiPreferences::initial(), original.clone()), original);
    let other = with(AiTask::Estimate, AiModelChoice::Haiku, AiEffortChoice::Max);
    assert_eq!(tuned(&other, original.clone()), original);
}

#[test]
fn a_chosen_task_sets_model_and_effort_for_every_purpose_of_that_task() {
    let preferences = with(AiTask::Briefing, AiModelChoice::Opus, AiEffortChoice::High);
    for purpose in [RunPurpose::MorningBriefing, RunPurpose::ClosingReview] {
        let result = tuned(
            &preferences,
            spec(purpose, ModelSelection::Fast, ReasoningEffort::Low),
        );
        assert_eq!(*result.model(), named("claude-opus-5-5"));
        assert_eq!(result.effort(), ReasoningEffort::High);
        assert_eq!(result.purpose(), purpose);
    }
}

#[test]
fn a_model_picked_in_the_chat_itself_wins_over_the_setting() {
    let preferences = with(AiTask::Chat, AiModelChoice::Haiku, AiEffortChoice::Low);
    let picked = tuned(
        &preferences,
        spec(
            RunPurpose::Chat,
            ModelSelection::Highest,
            ReasoningEffort::Max,
        ),
    );
    assert_eq!(*picked.model(), ModelSelection::Highest);
    assert_eq!(picked.effort(), ReasoningEffort::Max);
    let defaulted = tuned(
        &preferences,
        spec(
            RunPurpose::Chat,
            ModelSelection::Default,
            ReasoningEffort::Default,
        ),
    );
    assert_eq!(*defaulted.model(), named("claude-haiku-5-5"));
    assert_eq!(defaulted.effort(), ReasoningEffort::Low);
}
