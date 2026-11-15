use crate::{AiRunId, ChatHistory, InformationError, ModelName, RunKind, RunSpec, Utilization};
use domain::{Timestamp, UntrustedText};
use std::future::Future;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationFailure {
    Tools,
    McpServers,
    Plugins,
    Skills,
    SlashCommands,
    PermissionMode,
    ApiKeySource,
    CliVersion,
    LoginRequired,
    NotInstalled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsolationCheck {
    Passed,
    Rejected(IsolationFailure),
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenUsage {
    input: TokenCount,
    output: TokenCount,
    cache_read: TokenCount,
    cache_creation: TokenCount,
}
impl TokenUsage {
    #[must_use]
    pub const fn new(
        input: TokenCount,
        output: TokenCount,
        cache_read: TokenCount,
        cache_creation: TokenCount,
    ) -> Self {
        Self {
            input,
            output,
            cache_read,
            cache_creation,
        }
    }
    #[must_use]
    pub const fn input(&self) -> TokenCount {
        self.input
    }
    #[must_use]
    pub const fn output(&self) -> TokenCount {
        self.output
    }
    #[must_use]
    pub const fn cache_read(&self) -> TokenCount {
        self.cache_read
    }
    #[must_use]
    pub const fn cache_creation(&self) -> TokenCount {
        self.cache_creation
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiUsage {
    Unknown,
    Available(Utilization),
    Waiting(ResetTime),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiEvent {
    IsolationChecked(IsolationCheck),
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
    TokenUsage(TokenUsage),
    Quota(AiUsage),
    RateLimited {
        resets_at: ResetTime,
    },
    ModelRerouted(ModelName),
    ActualModel(ModelName),
    McpUnavailable,
    Unknown(UntrustedText),
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
        history: ChatHistory,
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
