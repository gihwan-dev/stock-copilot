use std::future::Future;

use domain::EvidenceBundleId;

use crate::{EstimateToolArguments, PromptFile, SystemPortError, ToolCallOutcome, ToolCallRecord};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayCase {
    pub bundle: EvidenceBundleId,
    pub ledger: Vec<ToolCallRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayOutput {
    SchemaRejected,
    Submitted(EstimateToolArguments),
}

pub trait PromptReplayRunner: Send + Sync {
    fn replay(
        &self,
        prompt: &PromptFile,
        case: &ReplayCase,
    ) -> impl Future<Output = Result<ReplayOutput, SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplayMetrics {
    pub cases: ReplayCount,
    pub schema_passed: ReplayCount,
    pub with_sources: ReplayCount,
    pub numeric_mismatches: ReplayCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct ReplayCount(usize);

impl ReplayCount {
    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    const fn increment(self) -> Self {
        Self(self.0.saturating_add(1_usize))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayComparison {
    pub baseline: ReplayMetrics,
    pub candidate: ReplayMetrics,
}

#[derive(Debug)]
pub struct ComparePromptReplay<'a, Runner> {
    runner: &'a Runner,
}

impl<'a, Runner: PromptReplayRunner> ComparePromptReplay<'a, Runner> {
    #[must_use]
    pub const fn new(runner: &'a Runner) -> Self {
        Self { runner }
    }

    pub async fn execute(
        &self,
        baseline: &PromptFile,
        candidate: &PromptFile,
        cases: &[ReplayCase],
    ) -> Result<ReplayComparison, SystemPortError> {
        Ok(ReplayComparison {
            baseline: self.measure(baseline, cases).await?,
            candidate: self.measure(candidate, cases).await?,
        })
    }

    async fn measure(
        &self,
        prompt: &PromptFile,
        cases: &[ReplayCase],
    ) -> Result<ReplayMetrics, SystemPortError> {
        let mut metrics = ReplayMetrics::default();
        for case in cases {
            let output = self.runner.replay(prompt, case).await?;
            metrics = record(metrics, &output, case);
        }
        Ok(metrics)
    }
}

fn record(metrics: ReplayMetrics, output: &ReplayOutput, case: &ReplayCase) -> ReplayMetrics {
    let counted = ReplayMetrics {
        cases: metrics.cases.increment(),
        ..metrics
    };
    match output {
        ReplayOutput::SchemaRejected => counted,
        ReplayOutput::Submitted(arguments) => ReplayMetrics {
            schema_passed: counted.schema_passed.increment(),
            with_sources: if arguments.sources.is_empty() {
                counted.with_sources
            } else {
                counted.with_sources.increment()
            },
            numeric_mismatches: if has_numeric_mismatch(arguments, &case.ledger) {
                counted.numeric_mismatches.increment()
            } else {
                counted.numeric_mismatches
            },
            ..counted
        },
    }
}

fn has_numeric_mismatch(arguments: &EstimateToolArguments, ledger: &[ToolCallRecord]) -> bool {
    let cited: Vec<&str> = arguments
        .sources
        .iter()
        .filter_map(|source| source.as_str().strip_prefix("tool:"))
        .filter_map(|request| {
            ledger
                .iter()
                .find(|call| {
                    call.request_id().as_str() == request
                        && call.outcome() == ToolCallOutcome::Succeeded
                })
                .map(|call| call.result_json().as_str())
        })
        .collect();
    [
        arguments.rationale.as_untrusted_str(),
        arguments.counterargument.as_untrusted_str(),
    ]
    .iter()
    .flat_map(|text| numbers(text))
    .any(|number| !cited.iter().any(|result| result.contains(number)))
}

fn numbers(text: &str) -> Vec<&str> {
    text.split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .map(|token| token.trim_matches('.'))
        .filter(|token| token.bytes().any(|byte| byte.is_ascii_digit()))
        .collect()
}
