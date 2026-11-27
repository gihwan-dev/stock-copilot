use crate::{Script, SteppedClock, script::State};
use application::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, BackgroundPreemption, ChatChunk,
    ChatChunkSink, ChatHistory, ChatNumberEvidence, ChatNumberEvidenceSet, ChatQueueAcquisition,
    ChatQueuePort, ChatRepository, ChatRunLedger, ChatRunLedgerEntry, ChatSequence, ChatSessionId,
    ChatSessionRecord, ChatSessionSummary, ChatSignalState, ChatSinkClosed, ChatStopSignal,
    InformationError, IsolationCheck, RecordOutcome, RunKind, RunSpec, StoredChatEntry,
    StoredUserMessage, SystemFailure, SystemPort, SystemPortError, ToolPayloadJson, UserMessageId,
    WarmChatId, WarmChatProbe, WarmChatRequest, WarmChatRuntime, WarmChatSession, WriteCompleted,
};
use domain::{TimeSpan, Timestamp, UntrustedText};
use std::collections::BTreeMap;
use std::collections::VecDeque;
use std::future::{Future, ready};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptedStreamStep {
    Event(AiEvent),
    AdvanceAndEvent { step: TimeSpan, event: AiEvent },
    Error(InformationError),
}

#[derive(Debug)]
pub struct ScriptedAiEventStream {
    steps: VecDeque<ScriptedStreamStep>,
    observations: Vec<application::SearchObservation>,
    clock: Option<Arc<SteppedClock>>,
}

impl ScriptedAiEventStream {
    #[must_use]
    pub fn new(steps: Vec<ScriptedStreamStep>) -> Self {
        Self {
            steps: steps.into(),
            observations: Vec::new(),
            clock: None,
        }
    }

    #[must_use]
    pub fn with_clock(steps: Vec<ScriptedStreamStep>, clock: Arc<SteppedClock>) -> Self {
        Self {
            steps: steps.into(),
            observations: Vec::new(),
            clock: Some(clock),
        }
    }

    #[must_use]
    pub fn with_observations(mut self, observations: Vec<application::SearchObservation>) -> Self {
        self.observations = observations;
        self
    }

    #[must_use]
    pub fn from_events(events: Vec<AiEvent>) -> Self {
        Self::new(events.into_iter().map(ScriptedStreamStep::Event).collect())
    }
}

impl AiEventStream for ScriptedAiEventStream {
    fn search_observations(&self) -> &[application::SearchObservation] {
        &self.observations
    }

    fn next(&mut self) -> impl Future<Output = Result<Option<AiEvent>, InformationError>> + Send {
        ready(match self.steps.pop_front() {
            None => Ok(None),
            Some(ScriptedStreamStep::Event(event)) => Ok(Some(event)),
            Some(ScriptedStreamStep::AdvanceAndEvent { step, event }) => {
                if let Some(clock) = &self.clock {
                    let _advanced = clock.advance_by(step);
                }
                Ok(Some(event))
            }
            Some(ScriptedStreamStep::Error(error)) => Err(error),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiProviderCall {
    Run(RunSpec),
    Resume {
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
    },
    Usage,
    Interrupt(AiRunId),
    VerifyIsolation(RunKind),
}

#[derive(Debug, Default)]
pub struct ScriptedAiProvider {
    run: Script<RunSpec, Result<ScriptedAiEventStream, InformationError>>,
    resume: Script<
        (RunSpec, ChatHistory, UntrustedText),
        Result<ScriptedAiEventStream, InformationError>,
    >,
    usage: Script<(), Result<AiUsage, InformationError>>,
    interrupt: Script<AiRunId, Result<(), InformationError>>,
    verify_isolation: Script<RunKind, Result<IsolationCheck, InformationError>>,
    calls: State<Vec<AiProviderCall>>,
}

impl ScriptedAiProvider {
    #[must_use]
    pub fn script_run(&self) -> &Script<RunSpec, Result<ScriptedAiEventStream, InformationError>> {
        &self.run
    }

    #[must_use]
    pub fn script_resume(
        &self,
    ) -> &Script<
        (RunSpec, ChatHistory, UntrustedText),
        Result<ScriptedAiEventStream, InformationError>,
    > {
        &self.resume
    }

    #[must_use]
    pub fn script_usage(&self) -> &Script<(), Result<AiUsage, InformationError>> {
        &self.usage
    }

    #[must_use]
    pub fn script_interrupt(&self) -> &Script<AiRunId, Result<(), InformationError>> {
        &self.interrupt
    }

    #[must_use]
    pub fn script_verify_isolation(
        &self,
    ) -> &Script<RunKind, Result<IsolationCheck, InformationError>> {
        &self.verify_isolation
    }

    #[must_use]
    pub fn calls(&self) -> Vec<AiProviderCall> {
        self.calls.write().clone()
    }
}

impl AiProvider for ScriptedAiProvider {
    type Stream = ScriptedAiEventStream;

    async fn run(&self, spec: RunSpec) -> Result<Self::Stream, InformationError> {
        self.calls.write().push(AiProviderCall::Run(spec.clone()));
        self.run.run(spec).await
    }

    async fn resume(
        &self,
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
    ) -> Result<Self::Stream, InformationError> {
        self.calls.write().push(AiProviderCall::Resume {
            spec: spec.clone(),
            history: history.clone(),
            message: message.clone(),
        });
        self.resume.run((spec, history, message)).await
    }

    async fn usage(&self) -> Result<AiUsage, InformationError> {
        self.calls.write().push(AiProviderCall::Usage);
        self.usage.run(()).await
    }

    async fn interrupt(&self, run: &AiRunId) -> Result<(), InformationError> {
        self.calls
            .write()
            .push(AiProviderCall::Interrupt(run.clone()));
        self.interrupt.run(run.clone()).await
    }

    async fn verify_isolation(&self, kind: RunKind) -> Result<IsolationCheck, InformationError> {
        self.calls
            .write()
            .push(AiProviderCall::VerifyIsolation(kind));
        self.verify_isolation.run(kind).await
    }
}

#[derive(Debug, Default)]
struct ChatSessionMemory {
    sessions: BTreeMap<ChatSessionId, ChatSessionRecord>,
    fail_ensure: Option<SystemPortError>,
    fail_append: Option<SystemPortError>,
}

#[derive(Debug, Default)]
pub struct MemoryChatRepository {
    state: State<ChatSessionMemory>,
}

impl MemoryChatRepository {
    pub fn fail_next_ensure(&self, error: SystemPortError) {
        self.state.write().fail_ensure = Some(error);
    }

    pub fn fail_next_append(&self, error: SystemPortError) {
        self.state.write().fail_append = Some(error);
    }

    #[must_use]
    pub fn stored_session(&self, id: &ChatSessionId) -> Option<ChatSessionRecord> {
        self.state.write().sessions.get(id).cloned()
    }
}

impl ChatRepository for MemoryChatRepository {
    fn ensure_session(
        &self,
        session: &ChatSessionId,
        now: Timestamp,
    ) -> impl Future<Output = Result<ChatSessionRecord, SystemPortError>> + Send {
        let mut state = self.state.write();
        if let Some(error) = state.fail_ensure.take() {
            return ready(Err(error));
        }
        let record = state.sessions.entry(session.clone()).or_insert_with(|| {
            ChatSessionRecord::new(
                ChatSessionSummary::new(session.clone(), now, now, ChatSequence::new(0)),
                Vec::new(),
            )
        });
        ready(Ok(record.clone()))
    }

    fn load_session(
        &self,
        session: &ChatSessionId,
    ) -> impl Future<Output = Result<Option<ChatSessionRecord>, SystemPortError>> + Send {
        ready(Ok(self.state.write().sessions.get(session).cloned()))
    }

    fn list_sessions(
        &self,
    ) -> impl Future<Output = Result<Vec<ChatSessionSummary>, SystemPortError>> + Send {
        ready(Ok(self
            .state
            .write()
            .sessions
            .values()
            .map(|record| record.summary().clone())
            .collect()))
    }

    fn append_entry(
        &self,
        session: &ChatSessionId,
        entry: &StoredChatEntry,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let mut state = self.state.write();
        if let Some(error) = state.fail_append.take() {
            return ready(Err(error));
        }
        let Some(existing) = state.sessions.get(session).cloned() else {
            return ready(Err(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::RepositoryIntegrity,
            )));
        };
        let mut entries = existing.entries().to_vec();
        entries.push(entry.clone());
        let count = match u64::try_from(entries.len()) {
            Ok(count) => count,
            Err(_error) => {
                return ready(Err(SystemPortError::new(
                    SystemPort::Repository,
                    SystemFailure::RepositoryIntegrity,
                )));
            }
        };
        let summary = ChatSessionSummary::new(
            existing.summary().id().clone(),
            existing.summary().created_at(),
            entry.at(),
            ChatSequence::new(count),
        );
        state
            .sessions
            .insert(session.clone(), ChatSessionRecord::new(summary, entries));
        ready(Ok(WriteCompleted))
    }

    fn load_user_message(
        &self,
        session: &ChatSessionId,
        message_id: &UserMessageId,
    ) -> impl Future<Output = Result<Option<StoredUserMessage>, SystemPortError>> + Send {
        ready(Ok(self.state.write().sessions.get(session).and_then(
            |record| record.find_user_message(message_id).cloned(),
        )))
    }
}

#[derive(Debug, Default)]
struct LedgerMemory {
    entries: Vec<ChatRunLedgerEntry>,
    fail_next: Option<SystemPortError>,
}

#[derive(Debug, Default)]
pub struct MemoryChatRunLedger {
    state: State<LedgerMemory>,
}

impl MemoryChatRunLedger {
    #[must_use]
    pub fn entries(&self) -> Vec<ChatRunLedgerEntry> {
        self.state.write().entries.clone()
    }

    pub fn fail_next_record(&self, error: SystemPortError) {
        self.state.write().fail_next = Some(error);
    }
}

impl ChatNumberEvidence for MemoryChatRunLedger {
    fn evidence_for_run(
        &self,
        _run: &AiRunId,
    ) -> impl Future<Output = Result<ChatNumberEvidenceSet, SystemPortError>> + Send {
        ready(Ok(ChatNumberEvidenceSet::NotConfigured))
    }
}

#[derive(Debug, Clone, Default)]
pub struct MemoryNumberEvidence {
    state: Arc<State<MemoryNumberEvidenceState>>,
}

#[derive(Debug, Default)]
struct MemoryNumberEvidenceState {
    payloads: Vec<(AiRunId, ToolPayloadJson)>,
    failure: Option<SystemPortError>,
}

impl MemoryNumberEvidence {
    pub fn record(&self, run: AiRunId, payload: ToolPayloadJson) {
        self.state.write().payloads.push((run, payload));
    }

    pub fn fail_with(&self, error: SystemPortError) {
        self.state.write().failure = Some(error);
    }
}

impl ChatNumberEvidence for MemoryNumberEvidence {
    fn evidence_for_run(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<ChatNumberEvidenceSet, SystemPortError>> + Send {
        let state = self.state.write();
        ready(match &state.failure {
            Some(error) => Err(*error),
            None => Ok(ChatNumberEvidenceSet::Payloads(
                state
                    .payloads
                    .iter()
                    .filter(|(owner, _)| owner == run)
                    .map(|(_, payload)| payload.clone())
                    .collect(),
            )),
        })
    }
}

impl ChatRunLedger for MemoryChatRunLedger {
    fn record_chat_run(
        &self,
        entry: &ChatRunLedgerEntry,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        let mut state = self.state.write();
        if let Some(error) = state.fail_next.take() {
            return ready(Err(error));
        }
        if state.entries.contains(entry) {
            ready(Ok(RecordOutcome::AlreadyRecorded))
        } else {
            state.entries.push(entry.clone());
            ready(Ok(RecordOutcome::Recorded))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatQueueCall {
    Acquire(AiRunId),
    Release(AiRunId),
}

#[derive(Debug)]
struct QueueState {
    next_acquisition: ChatQueueAcquisition,
    active_run: Option<AiRunId>,
    calls: Vec<ChatQueueCall>,
}

impl Default for QueueState {
    fn default() -> Self {
        Self {
            next_acquisition: ChatQueueAcquisition::Acquired {
                preempted_background: BackgroundPreemption::NoneRunning,
            },
            active_run: None,
            calls: Vec::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ScriptedChatQueue {
    state: State<QueueState>,
}

impl ScriptedChatQueue {
    pub fn set_next_acquisition(&self, acquisition: ChatQueueAcquisition) {
        self.state.write().next_acquisition = acquisition;
    }

    #[must_use]
    pub fn calls(&self) -> Vec<ChatQueueCall> {
        self.state.write().calls.clone()
    }

    #[must_use]
    pub fn active_run(&self) -> Option<AiRunId> {
        self.state.write().active_run.clone()
    }
}

impl ChatQueuePort for ScriptedChatQueue {
    fn acquire_chat(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<ChatQueueAcquisition, SystemPortError>> + Send {
        let mut state = self.state.write();
        state.calls.push(ChatQueueCall::Acquire(run.clone()));
        let outcome = state.next_acquisition;
        if matches_acquired(outcome) {
            state.active_run = Some(run.clone());
        }
        ready(Ok(outcome))
    }

    fn release_chat(
        &self,
        run: &AiRunId,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let mut state = self.state.write();
        state.calls.push(ChatQueueCall::Release(run.clone()));
        if state.active_run.as_ref() == Some(run) {
            state.active_run = None;
        }
        ready(Ok(WriteCompleted))
    }
}

const fn matches_acquired(outcome: ChatQueueAcquisition) -> bool {
    match outcome {
        ChatQueueAcquisition::Acquired { .. } => true,
        ChatQueueAcquisition::Busy => false,
    }
}

#[derive(Debug, Default)]
pub struct CollectingChatSink {
    chunks: Vec<ChatChunk>,
    close_after: Option<usize>,
}

impl CollectingChatSink {
    #[must_use]
    pub fn close_after(count: usize) -> Self {
        Self {
            chunks: Vec::new(),
            close_after: Some(count),
        }
    }

    #[must_use]
    pub fn chunks(&self) -> &[ChatChunk] {
        &self.chunks
    }
}

impl ChatChunkSink for CollectingChatSink {
    fn send(
        &mut self,
        chunk: ChatChunk,
    ) -> impl Future<Output = Result<(), ChatSinkClosed>> + Send {
        if let Some(limit) = self.close_after
            && self.chunks.len() >= limit
        {
            return ready(Err(ChatSinkClosed));
        }
        self.chunks.push(chunk);
        ready(Ok(()))
    }
}

#[derive(Debug)]
struct SignalTracker {
    sequence: VecDeque<ChatSignalState>,
    default_state: ChatSignalState,
}

#[derive(Debug)]
pub struct ScriptedStopSignal {
    state: State<SignalTracker>,
}

impl ScriptedStopSignal {
    #[must_use]
    pub fn always_active() -> Self {
        Self {
            state: State::new(SignalTracker {
                sequence: VecDeque::new(),
                default_state: ChatSignalState::Active,
            }),
        }
    }

    #[must_use]
    pub fn with_sequence(sequence: Vec<ChatSignalState>, fallback: ChatSignalState) -> Self {
        Self {
            state: State::new(SignalTracker {
                sequence: sequence.into(),
                default_state: fallback,
            }),
        }
    }

    pub fn set_state(&self, next: ChatSignalState) {
        let mut guard = self.state.write();
        guard.sequence.clear();
        guard.default_state = next;
    }
}

impl ChatStopSignal for ScriptedStopSignal {
    fn state(&self) -> ChatSignalState {
        let mut guard = self.state.write();
        match guard.sequence.pop_front() {
            Some(next) => next,
            None => guard.default_state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarmChatRuntimeCall {
    Prewarm(WarmChatRequest),
    StoreReady(WarmChatId),
    TakeReady,
    SendOnWarm {
        session_id: WarmChatId,
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
    },
    Discard(WarmChatId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptedPrewarmReply {
    pub wait_before_ready: Option<TimeSpan>,
    pub result: Result<WarmChatProbe, InformationError>,
}

#[derive(Debug, Default)]
struct WarmRuntimeSlot {
    ready: Option<WarmChatSession>,
    fail_store: Option<SystemPortError>,
    calls: Vec<WarmChatRuntimeCall>,
}

#[derive(Debug, Default)]
pub struct ScriptedWarmChatRuntime {
    prewarm: Script<WarmChatRequest, ScriptedPrewarmReply>,
    send_on_warm: Script<
        (WarmChatSession, RunSpec, ChatHistory, UntrustedText),
        Result<ScriptedAiEventStream, InformationError>,
    >,
    discard: Script<WarmChatId, Result<(), InformationError>>,
    slot: State<WarmRuntimeSlot>,
    clock: Option<Arc<SteppedClock>>,
}

impl ScriptedWarmChatRuntime {
    #[must_use]
    pub fn with_clock(clock: Arc<SteppedClock>) -> Self {
        Self {
            prewarm: Script::default(),
            send_on_warm: Script::default(),
            discard: Script::default(),
            slot: State::default(),
            clock: Some(clock),
        }
    }

    #[must_use]
    pub fn script_prewarm(&self) -> &Script<WarmChatRequest, ScriptedPrewarmReply> {
        &self.prewarm
    }

    #[must_use]
    pub fn script_send_on_warm(
        &self,
    ) -> &Script<
        (WarmChatSession, RunSpec, ChatHistory, UntrustedText),
        Result<ScriptedAiEventStream, InformationError>,
    > {
        &self.send_on_warm
    }

    #[must_use]
    pub fn script_discard(&self) -> &Script<WarmChatId, Result<(), InformationError>> {
        &self.discard
    }

    pub fn fail_next_store(&self, error: SystemPortError) {
        self.slot.write().fail_store = Some(error);
    }

    #[must_use]
    pub fn ready_session(&self) -> Option<WarmChatSession> {
        self.slot.write().ready.clone()
    }

    #[must_use]
    pub fn calls(&self) -> Vec<WarmChatRuntimeCall> {
        self.slot.write().calls.clone()
    }
}

impl WarmChatRuntime for ScriptedWarmChatRuntime {
    type Stream = ScriptedAiEventStream;

    async fn prewarm(&self, request: WarmChatRequest) -> Result<WarmChatProbe, InformationError> {
        self.slot
            .write()
            .calls
            .push(WarmChatRuntimeCall::Prewarm(request.clone()));
        let reply = self.prewarm.run(request).await;
        if let (Some(step), Some(clock)) = (reply.wait_before_ready, &self.clock) {
            let _advanced = clock.advance_by(step);
        }
        reply.result
    }

    fn store_ready(
        &self,
        session: WarmChatSession,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let mut slot = self.slot.write();
        slot.calls
            .push(WarmChatRuntimeCall::StoreReady(session.id().clone()));
        if let Some(error) = slot.fail_store.take() {
            return ready(Err(error));
        }
        slot.ready = Some(session);
        ready(Ok(WriteCompleted))
    }

    fn take_ready(
        &self,
    ) -> impl Future<Output = Result<Option<WarmChatSession>, SystemPortError>> + Send {
        let mut slot = self.slot.write();
        slot.calls.push(WarmChatRuntimeCall::TakeReady);
        ready(Ok(slot.ready.take()))
    }

    async fn send_on_warm(
        &self,
        session: WarmChatSession,
        spec: RunSpec,
        history: ChatHistory,
        message: UntrustedText,
    ) -> Result<Self::Stream, InformationError> {
        self.slot
            .write()
            .calls
            .push(WarmChatRuntimeCall::SendOnWarm {
                session_id: session.id().clone(),
                spec: spec.clone(),
                history: history.clone(),
                message: message.clone(),
            });
        self.send_on_warm
            .run((session, spec, history, message))
            .await
    }

    async fn discard(&self, id: &WarmChatId) -> Result<(), InformationError> {
        self.slot
            .write()
            .calls
            .push(WarmChatRuntimeCall::Discard(id.clone()));
        self.discard.run(id.clone()).await
    }
}
