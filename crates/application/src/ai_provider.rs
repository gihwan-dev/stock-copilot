use crate::{InformationError, RetryDelay};
use domain::{EstimateTask, Timestamp, UntrustedText};
use std::future::Future;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind {
    Collection,
    Analysis,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRunId(String);
impl AiRunId {
    pub fn try_new(value: &str) -> Result<Self, InformationError> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        {
            return Err(InformationError::InvalidIdentifier);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunInput {
    Collection(UntrustedText),
    Analysis {
        prompt: UntrustedText,
        assignments: Vec<EstimateTask>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSpec {
    id: AiRunId,
    input: RunInput,
}
impl RunSpec {
    #[must_use]
    pub const fn new(id: AiRunId, input: RunInput) -> Self {
        Self { id, input }
    }
    #[must_use]
    pub const fn id(&self) -> &AiRunId {
        &self.id
    }
    #[must_use]
    pub fn kind(&self) -> RunKind {
        run_kind(self)
    }
    #[must_use]
    pub const fn input(&self) -> &RunInput {
        &self.input
    }
}

#[must_use]
pub fn run_kind(spec: &RunSpec) -> RunKind {
    match spec.input() {
        RunInput::Collection(_) => RunKind::Collection,
        RunInput::Analysis {
            prompt: _prompt,
            assignments: _assignments,
        } => RunKind::Analysis,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationCheck {
    Passed,
    Rejected,
    LoginRequired,
    NotInstalled,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiEvent {
    TextDelta(UntrustedText),
    ToolCall {
        server: UntrustedText,
        tool: UntrustedText,
        args: UntrustedText,
        result: UntrustedText,
        status: ToolStatus,
    },
    WebSearch {
        query: UntrustedText,
        action: UntrustedText,
    },
    TokenUsage(TokenCount),
    Quota(AiUsage),
    RateLimited {
        resets_at: ResetTime,
    },
    ModelRerouted(UntrustedText),
    McpUnavailable,
    Unknown(UntrustedText),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    Running,
    Succeeded,
    Failed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetTime {
    Known(Timestamp),
    NotProvided,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenCount(u64);
impl TokenCount {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u64(&self) -> &u64 {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiUsage {
    Unknown,
    Available,
    Waiting(RetryDelay),
}
pub trait AiEventStream: Send {
    fn next(&mut self) -> impl Future<Output = Result<Option<AiEvent>, InformationError>> + Send;
}
pub trait AiProvider: Send + Sync {
    type Stream: AiEventStream;
    fn run(
        &self,
        spec: RunSpec,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send;
    fn resume(
        &self,
        spec: RunSpec,
        message: UntrustedText,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send;
    fn usage(&self) -> impl Future<Output = Result<AiUsage, InformationError>> + Send;
    fn interrupt(&self, run: &AiRunId)
    -> impl Future<Output = Result<(), InformationError>> + Send;
    fn verify_isolation(
        &self,
        kind: RunKind,
    ) -> impl Future<Output = Result<IsolationCheck, InformationError>> + Send;
}
