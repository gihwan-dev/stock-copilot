use crate::{
    AiEventStream, AiProvider, AiRunId, ChatChunkSink, ChatExecutionMode, ChatHistory,
    ChatModelConfig, ChatNumberEvidence, ChatQueuePort, ChatRepository, ChatRunLedger,
    ChatStopSignal, Clock, IdentifierError, InformationError, IsolationCheck, IsolationFailure,
    ModelSelection, NoNumberEvidence, ReasoningEffort, RunKind, RunSpec, SendChatMessage,
    SendChatOutcome, SendChatRequest, SystemFailure, SystemPortError, WriteCompleted,
    send_chat_message::{IsolationPhase, PreparedChatTurn, StreamExecutionContext},
};
use domain::{Nanosecond, SpanSeconds, TimeSpan, Timestamp, UntrustedText};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WarmChatId(String);

impl WarmChatId {
    pub fn try_new(value: &str) -> Result<Self, IdentifierError> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        {
            return Err(IdentifierError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpConnectionState {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarmChatProbe {
    id: WarmChatId,
    kind: RunKind,
    isolation: IsolationCheck,
    mcp: McpConnectionState,
    model: ModelSelection,
    effort: ReasoningEffort,
}

impl WarmChatProbe {
    #[must_use]
    pub fn new(
        id: WarmChatId,
        kind: RunKind,
        isolation: IsolationCheck,
        mcp: McpConnectionState,
        config: ChatModelConfig,
    ) -> Self {
        Self {
            id,
            kind,
            isolation,
            mcp,
            model: config.model,
            effort: config.effort,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &WarmChatId {
        &self.id
    }

    #[must_use]
    pub const fn kind(&self) -> RunKind {
        self.kind
    }

    #[must_use]
    pub const fn isolation(&self) -> IsolationCheck {
        self.isolation
    }

    #[must_use]
    pub const fn mcp(&self) -> McpConnectionState {
        self.mcp
    }

    #[must_use]
    pub fn model_config(&self) -> ChatModelConfig {
        ChatModelConfig {
            model: self.model.clone(),
            effort: self.effort,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmChatRejection {
    WrongRunKind,
    IsolationRejected(IsolationFailure),
    McpNotConnected,
    ProviderError(InformationError),
    StorageError(SystemFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarmChatReadiness {
    kind: RunKind,
    isolation: IsolationCheck,
    mcp: McpConnectionState,
}

impl WarmChatReadiness {
    pub fn try_verify(probe: &WarmChatProbe) -> Result<Self, WarmChatRejection> {
        match probe.kind {
            RunKind::Analysis => {}
            RunKind::Collection => return Err(WarmChatRejection::WrongRunKind),
        }
        match probe.isolation {
            IsolationCheck::Passed => {}
            IsolationCheck::Rejected(failure) => {
                return Err(WarmChatRejection::IsolationRejected(failure));
            }
        }
        match probe.mcp {
            McpConnectionState::Connected => {}
            McpConnectionState::Disconnected => return Err(WarmChatRejection::McpNotConnected),
        }
        Ok(Self {
            kind: RunKind::Analysis,
            isolation: IsolationCheck::Passed,
            mcp: McpConnectionState::Connected,
        })
    }

    #[must_use]
    pub const fn kind(self) -> RunKind {
        self.kind
    }

    #[must_use]
    pub const fn isolation(self) -> IsolationCheck {
        self.isolation
    }

    #[must_use]
    pub const fn mcp(self) -> McpConnectionState {
        self.mcp
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmMatch {
    Matched,
    Mismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarmFreshness {
    Fresh,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarmChatSession {
    id: WarmChatId,
    readiness: WarmChatReadiness,
    model: ModelSelection,
    effort: ReasoningEffort,
    warmed_at: Timestamp,
}

impl WarmChatSession {
    #[must_use]
    pub fn new(
        id: WarmChatId,
        readiness: WarmChatReadiness,
        config: ChatModelConfig,
        warmed_at: Timestamp,
    ) -> Self {
        Self {
            id,
            readiness,
            model: config.model,
            effort: config.effort,
            warmed_at,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &WarmChatId {
        &self.id
    }

    #[must_use]
    pub const fn readiness(&self) -> WarmChatReadiness {
        self.readiness
    }

    #[must_use]
    pub const fn model(&self) -> &ModelSelection {
        &self.model
    }

    #[must_use]
    pub const fn effort(&self) -> ReasoningEffort {
        self.effort
    }

    #[must_use]
    pub const fn warmed_at(&self) -> Timestamp {
        self.warmed_at
    }

    #[must_use]
    pub fn matches_request(&self, model: &ModelSelection, effort: ReasoningEffort) -> WarmMatch {
        if self.model == *model && self.effort == effort {
            WarmMatch::Matched
        } else {
            WarmMatch::Mismatch
        }
    }

    #[must_use]
    pub fn default_max_age() -> TimeSpan {
        match TimeSpan::try_new(SpanSeconds::new(600), Nanosecond::new(0)) {
            Ok(span) => span,
            Err(_) => TimeSpan::DAY,
        }
    }

    #[must_use]
    pub fn freshness(&self, now: Timestamp, max_age: TimeSpan) -> WarmFreshness {
        match self.warmed_at.elapsed_until(now) {
            Ok(elapsed) if elapsed <= max_age => WarmFreshness::Fresh,
            Ok(_) | Err(_) => WarmFreshness::Stale,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WarmChatRequest {
    run_id: AiRunId,
    model: ModelSelection,
    effort: ReasoningEffort,
}

impl WarmChatRequest {
    #[must_use]
    pub fn new(run_id: AiRunId, config: ChatModelConfig) -> Self {
        Self {
            run_id,
            model: config.model,
            effort: config.effort,
        }
    }

    #[must_use]
    pub const fn run_id(&self) -> &AiRunId {
        &self.run_id
    }

    #[must_use]
    pub const fn model(&self) -> &ModelSelection {
        &self.model
    }

    #[must_use]
    pub const fn effort(&self) -> ReasoningEffort {
        self.effort
    }
}

pub trait WarmChatRuntime: Send + Sync {
    type Stream: AiEventStream;

    fn prewarm(
        &self,
        request: WarmChatRequest,
    ) -> impl Future<Output = Result<WarmChatProbe, InformationError>> + Send;

    fn store_ready(
        &self,
        session: WarmChatSession,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;

    fn take_ready(
        &self,
    ) -> impl Future<Output = Result<Option<WarmChatSession>, SystemPortError>> + Send;

    fn send_on_warm(
        &self,
        session: WarmChatSession,
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
    ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send;

    fn discard(&self, id: &WarmChatId)
    -> impl Future<Output = Result<(), InformationError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrewarmChatOutcome {
    Ready(WarmChatSession),
    Rejected(WarmChatRejection),
}

#[derive(Debug)]
pub struct PrewarmChat<'a, Runtime, Time> {
    runtime: &'a Runtime,
    clock: &'a Time,
}

impl<'a, Runtime: WarmChatRuntime, Time: Clock> PrewarmChat<'a, Runtime, Time> {
    #[must_use]
    pub const fn new(runtime: &'a Runtime, clock: &'a Time) -> Self {
        Self { runtime, clock }
    }

    pub async fn execute(&self, request: WarmChatRequest) -> PrewarmChatOutcome {
        let probe = match self.runtime.prewarm(request).await {
            Ok(probe) => probe,
            Err(error) => {
                return PrewarmChatOutcome::Rejected(WarmChatRejection::ProviderError(error));
            }
        };
        let readiness = match WarmChatReadiness::try_verify(&probe) {
            Ok(readiness) => readiness,
            Err(rejection) => {
                let _discarded = self.runtime.discard(probe.id()).await;
                return PrewarmChatOutcome::Rejected(rejection);
            }
        };
        if let Ok(Some(previous)) = self.runtime.take_ready().await {
            let _discarded = self.runtime.discard(previous.id()).await;
        }
        let session = WarmChatSession::new(
            probe.id().clone(),
            readiness,
            probe.model_config(),
            self.clock.now(),
        );
        match self.runtime.store_ready(session.clone()).await {
            Ok(WriteCompleted) => PrewarmChatOutcome::Ready(session),
            Err(error) => {
                let _discarded = self.runtime.discard(session.id()).await;
                PrewarmChatOutcome::Rejected(WarmChatRejection::StorageError(error.failure()))
            }
        }
    }
}

struct WarmTurnInput<'a> {
    session: WarmChatSession,
    request: &'a SendChatRequest,
    prepared: PreparedChatTurn,
}

#[derive(Debug)]
pub struct SendWarmChatMessage<
    'a,
    Provider,
    Warm,
    Repo,
    Ledger,
    Queue,
    Time,
    Evidence = NoNumberEvidence,
> {
    sender: SendChatMessage<'a, Provider, Repo, Ledger, Queue, Time, Evidence>,
    warm: &'a Warm,
    max_age: TimeSpan,
}

impl<'a, Provider, Warm, Repo, Ledger, Queue, Time, Evidence>
    SendWarmChatMessage<'a, Provider, Warm, Repo, Ledger, Queue, Time, Evidence>
where
    Provider: AiProvider,
    Warm: WarmChatRuntime,
    Repo: ChatRepository,
    Ledger: ChatRunLedger,
    Queue: ChatQueuePort,
    Time: Clock,
    Evidence: ChatNumberEvidence,
{
    #[must_use]
    pub const fn new(
        sender: SendChatMessage<'a, Provider, Repo, Ledger, Queue, Time, Evidence>,
        warm: &'a Warm,
        max_age: TimeSpan,
    ) -> Self {
        Self {
            sender,
            warm,
            max_age,
        }
    }

    #[must_use]
    pub fn with_default_max_age(
        sender: SendChatMessage<'a, Provider, Repo, Ledger, Queue, Time, Evidence>,
        warm: &'a Warm,
    ) -> Self {
        Self::new(sender, warm, WarmChatSession::default_max_age())
    }

    pub async fn execute<Sink: ChatChunkSink, Stop: ChatStopSignal>(
        &self,
        request: SendChatRequest,
        sink: &mut Sink,
        stop: &Stop,
    ) -> SendChatOutcome {
        let started_at = self.sender.clock().now();
        let prepared = match self.sender.prepare_turn(&request, started_at).await {
            Ok(prepared) => prepared,
            Err(reason) => {
                return self
                    .sender
                    .finish_pre_stream_failure(&request, started_at, reason, sink)
                    .await;
            }
        };
        let outcome = match self.take_usable_warm(&request, started_at).await {
            Some(warm_session) => {
                self.run_warm(
                    WarmTurnInput {
                        session: warm_session,
                        request: &request,
                        prepared,
                    },
                    sink,
                    stop,
                )
                .await
            }
            None => self.run_cold(&request, prepared, sink, stop).await,
        };
        let _released = self.sender.queue().release_chat(request.run_id()).await;
        outcome
    }

    async fn take_usable_warm(
        &self,
        request: &SendChatRequest,
        started_at: Timestamp,
    ) -> Option<WarmChatSession> {
        let Ok(Some(candidate)) = self.warm.take_ready().await else {
            return None;
        };
        let matched = candidate.matches_request(request.model(), request.effort());
        let freshness = candidate.freshness(started_at, self.max_age);
        match (matched, freshness) {
            (WarmMatch::Matched, WarmFreshness::Fresh) => Some(candidate),
            (WarmMatch::Mismatch, WarmFreshness::Fresh | WarmFreshness::Stale)
            | (WarmMatch::Matched, WarmFreshness::Stale) => {
                let _discarded = self.warm.discard(candidate.id()).await;
                None
            }
        }
    }

    async fn run_warm<Sink: ChatChunkSink, Stop: ChatStopSignal>(
        &self,
        input: WarmTurnInput<'_>,
        sink: &mut Sink,
        stop: &Stop,
    ) -> SendChatOutcome {
        let stream_result = self
            .warm
            .send_on_warm(
                input.session,
                input.prepared.spec,
                input.prepared.prior_history,
                input.request.message().clone(),
            )
            .await;
        let context = StreamExecutionContext {
            request: input.request,
            started_at: input.prepared.started_at,
            assistant_seq: input.prepared.assistant_seq,
            mode: ChatExecutionMode::WarmPreverified,
            initial_isolation: IsolationPhase::Verified,
        };
        self.sender
            .run_opened_stream(stream_result, context, sink, stop)
            .await
    }

    async fn run_cold<Sink: ChatChunkSink, Stop: ChatStopSignal>(
        &self,
        request: &SendChatRequest,
        prepared: PreparedChatTurn,
        sink: &mut Sink,
        stop: &Stop,
    ) -> SendChatOutcome {
        let mode = if prepared.prior_history.messages().is_empty() {
            ChatExecutionMode::FreshAnalysis
        } else {
            ChatExecutionMode::ResumedFromHistory
        };
        let stream_result = match mode {
            ChatExecutionMode::FreshAnalysis => self.sender.provider().run(prepared.spec).await,
            ChatExecutionMode::ResumedFromHistory | ChatExecutionMode::WarmPreverified => {
                self.sender
                    .provider()
                    .resume(
                        prepared.spec,
                        prepared.prior_history,
                        request.message().clone(),
                    )
                    .await
            }
        };
        let context = StreamExecutionContext {
            request,
            started_at: prepared.started_at,
            assistant_seq: prepared.assistant_seq,
            mode,
            initial_isolation: IsolationPhase::Pending,
        };
        self.sender
            .run_opened_stream(stream_result, context, sink, stop)
            .await
    }
}
