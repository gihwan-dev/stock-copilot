use crate::chat_number_check::NO_NUMBER_EVIDENCE;
use crate::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, AnalysisPrompt, ChatCard, ChatChunk,
    ChatChunkSink, ChatExecutionMode, ChatFailureReason, ChatHistory, ChatNumberCheck,
    ChatNumberEvidence, ChatNumberEvidenceSet, ChatQueueAcquisition, ChatQueuePort, ChatRepository,
    ChatRunLedger, ChatRunLedgerEntry, ChatRunModelSpec, ChatRunProfile, ChatRunTiming,
    ChatScreenContext, ChatSequence, ChatSessionId, ChatSignalState, ChatSource, ChatSourceKind,
    ChatStopSignal, ChatTurnId, ChatTurnIdentity, ChatTurnMetrics, ChatTurnStatus, ChatTurnSummary,
    Clock, InformationError, IsolationCheck, ModelName, ModelSelection, NoNumberEvidence,
    PromptError, ReasoningEffort, ResetTime, RunInput, RunKind, RunPurpose, RunSpec,
    StoredAssistantAttachments, StoredAssistantReply, StoredChatContent, StoredChatEntry,
    StoredUserMessage, SystemFailure, TokenUsage, ToolStatus, UserMessageId, check_chat_numbers,
};
use domain::{HorizonSelection, KnownMarket, TimeSpan, Timestamp, UntrustedText};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisToolAllowance {
    Allowed,
    Forbidden,
}

#[must_use]
pub fn check_analysis_tool(tool: &UntrustedText) -> AnalysisToolAllowance {
    match tool.as_untrusted_str() {
        "get_portfolio"
        | "get_allocation"
        | "get_transactions"
        | "get_performance"
        | "get_quote"
        | "get_candles"
        | "get_market_overview"
        | "get_rankings"
        | "get_sectors"
        | "get_investor_flow"
        | "search_news"
        | "get_disclosures"
        | "read_article"
        | "get_thesis"
        | "update_thesis"
        | "get_profile"
        | "get_estimate_task"
        | "submit_estimate"
        | "get_scorecard"
        | "request_web_research" => AnalysisToolAllowance::Allowed,
        _ => check_app_command_tool(tool),
    }
}

fn check_app_command_tool(tool: &UntrustedText) -> AnalysisToolAllowance {
    match tool.as_untrusted_str() {
        "app_app_version"
        | "app_get_settings"
        | "app_get_chat_history"
        | "app_get_briefings"
        | "app_get_portfolio"
        | "app_get_allocation"
        | "app_get_job_schedules"
        | "app_get_public_ip"
        | "app_check_toss_connection"
        | "app_get_update_status"
        | "app_get_risk_settings"
        | "app_get_self_check"
        | "app_get_status"
        | "app_get_data_status"
        | "app_get_scorecard"
        | "app_get_transactions"
        | "app_get_instruments"
        | "app_get_instrument_detail"
        | "app_get_thesis"
        | "app_get_signals"
        | "app_get_account_selection"
        | "app_get_principles"
        | "app_get_news_settings"
        | "app_get_ai_settings"
        | "app_set_job_schedule"
        | "app_save_investor_profile"
        | "app_save_order_settings"
        | "app_add_watchlist"
        | "app_remove_watchlist"
        | "app_skip_signal"
        | "app_select_account"
        | "app_set_principles"
        | "app_backup_now"
        | "app_export_data"
        | "app_set_news_source"
        | "app_set_sec_contact"
        | "app_set_wake_criteria"
        | "app_set_aliases"
        | "app_set_task_model"
        | "app_set_budget_percent"
        | "app_request_order_approval" => AnalysisToolAllowance::Allowed,
        _ => AnalysisToolAllowance::Forbidden,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatModelConfig {
    pub model: ModelSelection,
    pub effort: ReasoningEffort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendChatRequest {
    session_id: ChatSessionId,
    turn_id: ChatTurnId,
    run_id: AiRunId,
    user_message_id: UserMessageId,
    message: UntrustedText,
    context: Option<ChatScreenContext>,
    model: ModelSelection,
    effort: ReasoningEffort,
}

impl SendChatRequest {
    #[must_use]
    pub fn new(
        ids: ChatTurnIdentity,
        user_message_id: UserMessageId,
        message: UntrustedText,
        context: Option<ChatScreenContext>,
        model: ChatModelConfig,
    ) -> Self {
        Self {
            session_id: ids.session,
            turn_id: ids.turn,
            run_id: ids.run,
            user_message_id,
            message,
            context,
            model: model.model,
            effort: model.effort,
        }
    }

    #[must_use]
    pub const fn session_id(&self) -> &ChatSessionId {
        &self.session_id
    }

    #[must_use]
    pub const fn turn_id(&self) -> &ChatTurnId {
        &self.turn_id
    }

    #[must_use]
    pub const fn run_id(&self) -> &AiRunId {
        &self.run_id
    }

    #[must_use]
    pub const fn user_message_id(&self) -> &UserMessageId {
        &self.user_message_id
    }

    #[must_use]
    pub const fn message(&self) -> &UntrustedText {
        &self.message
    }

    #[must_use]
    pub const fn context(&self) -> Option<&ChatScreenContext> {
        self.context.as_ref()
    }

    #[must_use]
    pub const fn model(&self) -> &ModelSelection {
        &self.model
    }

    #[must_use]
    pub const fn effort(&self) -> ReasoningEffort {
        self.effort
    }

    pub(crate) fn identity(&self) -> ChatTurnIdentity {
        ChatTurnIdentity {
            session: self.session_id.clone(),
            turn: self.turn_id.clone(),
            run: self.run_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendChatOutcome {
    Completed(ChatTurnSummary),
    Cancelled(ChatTurnSummary),
    Interrupted(ChatTurnSummary),
    RateLimited {
        summary: ChatTurnSummary,
        resets_at: ResetTime,
    },
    Failed {
        summary: ChatTurnSummary,
        reason: ChatFailureReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IsolationPhase {
    Pending,
    Verified,
}

#[derive(Debug)]
struct AssistantTextBuffer(UntrustedText);

impl AssistantTextBuffer {
    const fn empty() -> Self {
        Self(UntrustedText::empty())
    }

    fn append(&mut self, delta: &UntrustedText) {
        let mut combined = self.0.as_untrusted_str().to_owned();
        for ch in delta.as_untrusted_str().chars() {
            let Some(next_len) = combined.len().checked_add(ch.len_utf8()) else {
                break;
            };
            if next_len > 65_536 {
                break;
            }
            combined.push(ch);
        }
        if let Ok(updated) = UntrustedText::try_new(&combined) {
            self.0 = updated;
        }
    }

    fn into_text(self) -> UntrustedText {
        self.0
    }

    const fn text(&self) -> &UntrustedText {
        &self.0
    }
}

#[derive(Debug)]
struct StreamAccumulator {
    isolation: IsolationPhase,
    text: AssistantTextBuffer,
    actual_model: Option<ModelName>,
    usage: Option<TokenUsage>,
    sources: Vec<ChatSource>,
    cards: Vec<ChatCard>,
    first_token_latency: Option<TimeSpan>,
}

impl StreamAccumulator {
    const fn new(isolation: IsolationPhase) -> Self {
        Self {
            isolation,
            text: AssistantTextBuffer::empty(),
            actual_model: None,
            usage: None,
            sources: Vec::new(),
            cards: Vec::new(),
            first_token_latency: None,
        }
    }
}

pub fn build_chat_prompt(
    message: &UntrustedText,
    context: Option<&ChatScreenContext>,
) -> Result<AnalysisPrompt, PromptError> {
    if message.as_untrusted_str().trim().is_empty() {
        return Err(PromptError::Empty);
    }
    let mut prompt = String::new();
    if let Some(screen) = context {
        let mut parts = Vec::new();
        if let Some(symbol) = screen.symbol() {
            parts.push(format!("symbol={}", symbol.as_str()));
        }
        if let Some(market) = screen.market() {
            let code = match market {
                KnownMarket::Kr => "KR",
                KnownMarket::Us => "US",
            };
            parts.push(format!("market={code}"));
        }
        if let Some(horizon) = screen.horizon() {
            let label = match horizon {
                HorizonSelection::FewDays => "few_days",
                HorizonSelection::WeeksToMonths => "weeks_to_months",
                HorizonSelection::HalfYearOrMore => "half_year_or_more",
                HorizonSelection::Hours => "hours",
            };
            parts.push(format!("horizon={label}"));
        }
        if !parts.is_empty() {
            prompt.push_str("[screen_context:");
            prompt.push_str(&parts.join(","));
            prompt.push_str("]\n");
        }
    }
    prompt.push_str(message.as_untrusted_str());
    AnalysisPrompt::try_new(&prompt)
}

pub(crate) struct PreparedChatTurn {
    pub(crate) started_at: Timestamp,
    pub(crate) assistant_seq: ChatSequence,
    pub(crate) prior_history: ChatHistory,
    pub(crate) spec: RunSpec,
}

pub(crate) struct StreamExecutionContext<'a> {
    pub(crate) request: &'a SendChatRequest,
    pub(crate) started_at: Timestamp,
    pub(crate) assistant_seq: ChatSequence,
    pub(crate) mode: ChatExecutionMode,
    pub(crate) initial_isolation: IsolationPhase,
}

struct FinalizeInput {
    started_at: Timestamp,
    assistant_seq: ChatSequence,
    mode: ChatExecutionMode,
    status: ChatTurnStatus,
    accumulator: StreamAccumulator,
}

#[derive(Debug)]
pub struct SendChatMessage<'a, Provider, Repo, Ledger, Queue, Time, Evidence = NoNumberEvidence> {
    provider: &'a Provider,
    repository: &'a Repo,
    ledger: &'a Ledger,
    queue: &'a Queue,
    clock: &'a Time,
    evidence: &'a Evidence,
}

impl<'a, Provider, Repo, Ledger, Queue, Time>
    SendChatMessage<'a, Provider, Repo, Ledger, Queue, Time, NoNumberEvidence>
where
    Provider: AiProvider,
    Repo: ChatRepository,
    Ledger: ChatRunLedger,
    Queue: ChatQueuePort,
    Time: Clock,
{
    #[must_use]
    pub const fn new(
        provider: &'a Provider,
        repository: &'a Repo,
        ledger: &'a Ledger,
        queue: &'a Queue,
        clock: &'a Time,
    ) -> Self {
        Self {
            provider,
            repository,
            ledger,
            queue,
            clock,
            evidence: &NO_NUMBER_EVIDENCE,
        }
    }

    #[must_use]
    pub const fn with_number_evidence<Evidence: ChatNumberEvidence>(
        self,
        evidence: &'a Evidence,
    ) -> SendChatMessage<'a, Provider, Repo, Ledger, Queue, Time, Evidence> {
        SendChatMessage {
            provider: self.provider,
            repository: self.repository,
            ledger: self.ledger,
            queue: self.queue,
            clock: self.clock,
            evidence,
        }
    }
}

impl<'a, Provider, Repo, Ledger, Queue, Time, Evidence>
    SendChatMessage<'a, Provider, Repo, Ledger, Queue, Time, Evidence>
where
    Provider: AiProvider,
    Repo: ChatRepository,
    Ledger: ChatRunLedger,
    Queue: ChatQueuePort,
    Time: Clock,
    Evidence: ChatNumberEvidence,
{
    pub(crate) const fn provider(&self) -> &'a Provider {
        self.provider
    }

    pub(crate) const fn queue(&self) -> &'a Queue {
        self.queue
    }

    pub(crate) const fn clock(&self) -> &'a Time {
        self.clock
    }

    pub async fn execute<Sink: ChatChunkSink, Stop: ChatStopSignal>(
        &self,
        request: SendChatRequest,
        sink: &mut Sink,
        stop: &Stop,
    ) -> SendChatOutcome {
        let started_at = self.clock.now();
        let prepared = match self.prepare_turn(&request, started_at).await {
            Ok(prepared) => prepared,
            Err(reason) => {
                return self
                    .finish_pre_stream_failure(&request, started_at, reason, sink)
                    .await;
            }
        };
        let mode = if prepared.prior_history.messages().is_empty() {
            ChatExecutionMode::FreshAnalysis
        } else {
            ChatExecutionMode::ResumedFromHistory
        };
        let stream_result = match mode {
            ChatExecutionMode::FreshAnalysis => self.provider.run(prepared.spec).await,
            ChatExecutionMode::ResumedFromHistory | ChatExecutionMode::WarmPreverified => {
                self.provider
                    .resume(
                        prepared.spec,
                        prepared.prior_history,
                        request.message().clone(),
                    )
                    .await
            }
        };
        let context = StreamExecutionContext {
            request: &request,
            started_at: prepared.started_at,
            assistant_seq: prepared.assistant_seq,
            mode,
            initial_isolation: IsolationPhase::Pending,
        };
        let outcome = self
            .run_opened_stream(stream_result, context, sink, stop)
            .await;
        let _released = self.queue.release_chat(request.run_id()).await;
        outcome
    }

    pub(crate) async fn prepare_turn(
        &self,
        request: &SendChatRequest,
        started_at: Timestamp,
    ) -> Result<PreparedChatTurn, ChatFailureReason> {
        let prompt = build_chat_prompt(request.message(), request.context())
            .map_err(ChatFailureReason::from)?;
        let session = self
            .repository
            .ensure_session(request.session_id(), started_at)
            .await
            .map_err(|error| ChatFailureReason::StorageError(error.failure()))?;
        let prior_history = session.to_chat_history();
        let user_seq = session
            .next_sequence()
            .ok_or(ChatFailureReason::StorageError(
                SystemFailure::RepositoryIntegrity,
            ))?;
        let assistant_seq = user_seq.next().ok_or(ChatFailureReason::StorageError(
            SystemFailure::RepositoryIntegrity,
        ))?;
        let user_entry = StoredChatEntry::new(
            user_seq,
            started_at,
            request.turn_id().clone(),
            StoredChatContent::User(StoredUserMessage::from_store_result(
                request.user_message_id().clone(),
                request.message().clone(),
            )),
        );
        self.repository
            .append_entry(request.session_id(), &user_entry)
            .await
            .map_err(|error| ChatFailureReason::StorageError(error.failure()))?;
        match self.queue.acquire_chat(request.run_id()).await {
            Ok(ChatQueueAcquisition::Acquired { .. }) => {}
            Ok(ChatQueueAcquisition::Busy) => return Err(ChatFailureReason::QueueBusy),
            Err(error) => return Err(ChatFailureReason::StorageError(error.failure())),
        }
        let spec = RunSpec::new(
            request.run_id().clone(),
            RunInput::Analysis(prompt),
            request.model().clone(),
            request.effort(),
            RunPurpose::Chat,
        );
        Ok(PreparedChatTurn {
            started_at,
            assistant_seq,
            prior_history,
            spec,
        })
    }

    pub(crate) async fn run_opened_stream<
        Stream: AiEventStream,
        Sink: ChatChunkSink,
        Stop: ChatStopSignal,
    >(
        &self,
        stream_result: Result<Stream, InformationError>,
        context: StreamExecutionContext<'_>,
        sink: &mut Sink,
        stop: &Stop,
    ) -> SendChatOutcome {
        match stream_result {
            Ok(mut stream) => self.drive_stream(&mut stream, context, sink, stop).await,
            Err(error) => {
                let status = map_stream_error(error, context.initial_isolation);
                self.finalize_turn(
                    context.request,
                    FinalizeInput {
                        started_at: context.started_at,
                        assistant_seq: context.assistant_seq,
                        mode: context.mode,
                        status,
                        accumulator: StreamAccumulator::new(context.initial_isolation),
                    },
                    sink,
                )
                .await
            }
        }
    }

    pub(crate) async fn finish_pre_stream_failure<Sink: ChatChunkSink>(
        &self,
        request: &SendChatRequest,
        started_at: Timestamp,
        reason: ChatFailureReason,
        sink: &mut Sink,
    ) -> SendChatOutcome {
        let finished_at = self.clock.now();
        let summary = ChatTurnSummary::new(
            request.identity(),
            ChatExecutionMode::FreshAnalysis,
            ChatTurnMetrics {
                actual_model: None,
                usage: None,
                first_token_latency: None,
            },
            finished_at,
        );
        let _sent = sink.send(ChatChunk::Failed(reason)).await;
        let _recorded = self
            .ledger
            .record_chat_run(&ChatRunLedgerEntry::new(
                request.identity(),
                ChatRunProfile {
                    kind: RunKind::Analysis,
                    purpose: RunPurpose::Chat,
                },
                ChatRunModelSpec {
                    requested_model: request.model().clone(),
                    actual_model: None,
                    effort: request.effort(),
                    usage: None,
                },
                ChatRunTiming {
                    started_at,
                    finished_at,
                    first_token_latency: None,
                },
                ChatTurnStatus::Failed(reason),
            ))
            .await;
        SendChatOutcome::Failed { summary, reason }
    }

    async fn drive_stream<Stream: AiEventStream, Sink: ChatChunkSink, Stop: ChatStopSignal>(
        &self,
        stream: &mut Stream,
        context: StreamExecutionContext<'_>,
        sink: &mut Sink,
        stop: &Stop,
    ) -> SendChatOutcome {
        let mut accumulator = StreamAccumulator::new(context.initial_isolation);
        let status = self
            .poll_until_terminal(
                &mut StreamDriver {
                    stream,
                    accumulator: &mut accumulator,
                    sink,
                },
                &context,
                stop,
            )
            .await;
        self.finalize_turn(
            context.request,
            FinalizeInput {
                started_at: context.started_at,
                assistant_seq: context.assistant_seq,
                mode: context.mode,
                status,
                accumulator,
            },
            sink,
        )
        .await
    }

    async fn poll_until_terminal<
        Stream: AiEventStream,
        Sink: ChatChunkSink,
        Stop: ChatStopSignal,
    >(
        &self,
        driver: &mut StreamDriver<'_, Stream, Sink>,
        context: &StreamExecutionContext<'_>,
        stop: &Stop,
    ) -> ChatTurnStatus {
        loop {
            if let Some(stopped) = self.check_stop(stop, context.request.run_id()).await {
                return stopped;
            }
            let event = match driver.stream.next().await {
                Ok(Some(event)) => event,
                Ok(None) => return finish_eof_status(driver.accumulator.isolation),
                Err(error) => return map_stream_error(error, driver.accumulator.isolation),
            };
            if let Some(terminal) = self
                .handle_event(event, driver.accumulator, context, driver.sink)
                .await
            {
                return terminal;
            }
        }
    }

    async fn check_stop<Stop: ChatStopSignal>(
        &self,
        stop: &Stop,
        run_id: &AiRunId,
    ) -> Option<ChatTurnStatus> {
        match stop.state() {
            ChatSignalState::Active => None,
            ChatSignalState::Cancelled => {
                let _interrupted = self.provider.interrupt(run_id).await;
                Some(ChatTurnStatus::Cancelled)
            }
            ChatSignalState::Interrupted => {
                let _interrupted = self.provider.interrupt(run_id).await;
                Some(ChatTurnStatus::Interrupted)
            }
        }
    }

    async fn handle_event<Sink: ChatChunkSink>(
        &self,
        event: AiEvent,
        accumulator: &mut StreamAccumulator,
        context: &StreamExecutionContext<'_>,
        sink: &mut Sink,
    ) -> Option<ChatTurnStatus> {
        let run_id = context.request.run_id();
        if accumulator.isolation == IsolationPhase::Pending {
            return self
                .handle_pre_isolation_event(event, accumulator, run_id)
                .await;
        }
        match event {
            AiEvent::IsolationChecked(IsolationCheck::Passed)
            | AiEvent::Quota(AiUsage::Available(_) | AiUsage::Unknown)
            | AiEvent::Unknown(_) => None,
            AiEvent::IsolationChecked(IsolationCheck::Rejected(failure)) => Some(
                ChatTurnStatus::Failed(ChatFailureReason::IsolationRejected(failure)),
            ),
            AiEvent::TextDelta(delta) => {
                self.handle_text_delta(delta, accumulator, context.started_at, sink)
                    .await;
                None
            }
            AiEvent::ToolCall {
                server,
                tool,
                status,
                ..
            } => {
                self.handle_tool_call(
                    ToolCallInfo {
                        server,
                        tool,
                        status,
                    },
                    accumulator,
                    run_id,
                    sink,
                )
                .await
            }
            AiEvent::WebSearch { .. } => {
                let _interrupted = self.provider.interrupt(run_id).await;
                Some(ChatTurnStatus::Failed(
                    ChatFailureReason::ForbiddenToolAttempt,
                ))
            }
            AiEvent::TokenUsage(usage) => {
                accumulator.usage = Some(usage);
                None
            }
            AiEvent::ActualModel(model) | AiEvent::ModelRerouted(model) => {
                accumulator.actual_model = Some(model);
                None
            }
            AiEvent::RateLimited { resets_at } | AiEvent::Quota(AiUsage::Waiting(resets_at)) => {
                Some(ChatTurnStatus::RateLimited(resets_at))
            }
            AiEvent::McpUnavailable => {
                Some(ChatTurnStatus::Failed(ChatFailureReason::McpUnavailable))
            }
        }
    }

    async fn handle_text_delta<Sink: ChatChunkSink>(
        &self,
        delta: UntrustedText,
        accumulator: &mut StreamAccumulator,
        started_at: Timestamp,
        sink: &mut Sink,
    ) {
        if accumulator.first_token_latency.is_none() && !delta.as_untrusted_str().is_empty() {
            let now = self.clock.now();
            accumulator.first_token_latency = started_at.elapsed_until(now).ok();
        }
        accumulator.text.append(&delta);
        let _sent = sink.send(ChatChunk::TextDelta(delta)).await;
    }

    async fn handle_pre_isolation_event(
        &self,
        event: AiEvent,
        accumulator: &mut StreamAccumulator,
        run_id: &AiRunId,
    ) -> Option<ChatTurnStatus> {
        match event {
            AiEvent::IsolationChecked(IsolationCheck::Passed) => {
                accumulator.isolation = IsolationPhase::Verified;
                None
            }
            AiEvent::IsolationChecked(IsolationCheck::Rejected(failure)) => Some(
                ChatTurnStatus::Failed(ChatFailureReason::IsolationRejected(failure)),
            ),
            AiEvent::TextDelta(_)
            | AiEvent::ToolCall { .. }
            | AiEvent::WebSearch { .. }
            | AiEvent::TokenUsage(_)
            | AiEvent::Quota(_)
            | AiEvent::RateLimited { .. }
            | AiEvent::ModelRerouted(_)
            | AiEvent::ActualModel(_)
            | AiEvent::McpUnavailable
            | AiEvent::Unknown(_) => {
                let _interrupted = self.provider.interrupt(run_id).await;
                Some(ChatTurnStatus::Failed(
                    ChatFailureReason::IsolationNotVerified,
                ))
            }
        }
    }

    async fn handle_tool_call<Sink: ChatChunkSink>(
        &self,
        call: ToolCallInfo,
        accumulator: &mut StreamAccumulator,
        run_id: &AiRunId,
        sink: &mut Sink,
    ) -> Option<ChatTurnStatus> {
        if check_analysis_tool(&call.tool) == AnalysisToolAllowance::Forbidden {
            let _interrupted = self.provider.interrupt(run_id).await;
            return Some(ChatTurnStatus::Failed(
                ChatFailureReason::ForbiddenToolAttempt,
            ));
        }
        match call.status {
            ToolStatus::Running => {
                let _sent = sink
                    .send(ChatChunk::ToolCallStarted {
                        server: call.server,
                        tool: call.tool,
                    })
                    .await;
            }
            ToolStatus::Succeeded => {
                accumulator.sources.push(ChatSource::new(
                    ChatSourceKind::Tool,
                    call.tool.clone(),
                    None,
                ));
                let _sent = sink
                    .send(ChatChunk::ToolCallFinished {
                        server: call.server,
                        tool: call.tool,
                        status: call.status,
                    })
                    .await;
            }
            ToolStatus::Failed => {
                let _sent = sink
                    .send(ChatChunk::ToolCallFinished {
                        server: call.server,
                        tool: call.tool,
                        status: call.status,
                    })
                    .await;
            }
        }
        None
    }

    async fn finalize_turn<Sink: ChatChunkSink>(
        &self,
        request: &SendChatRequest,
        input: FinalizeInput,
        sink: &mut Sink,
    ) -> SendChatOutcome {
        let finished_at = self.clock.now();
        let actual_model = input.accumulator.actual_model;
        let usage = input.accumulator.usage;
        let first_token_latency = input.accumulator.first_token_latency;
        let sources = input.accumulator.sources;
        let number_check = self
            .number_check(request, input.status, input.accumulator.text.text())
            .await;
        let assistant_entry = StoredChatEntry::new(
            input.assistant_seq,
            finished_at,
            request.turn_id().clone(),
            StoredChatContent::Assistant(StoredAssistantReply::new(
                request.run_id().clone(),
                input.accumulator.text.into_text(),
                input.status,
                StoredAssistantAttachments {
                    actual_model: actual_model.clone(),
                    sources: sources.clone(),
                    cards: input.accumulator.cards,
                },
            )),
        );
        let final_status = self
            .store_reply(request, &assistant_entry, input.status)
            .await;
        self.record_ledger(
            request,
            ChatRunModelSpec {
                requested_model: request.model().clone(),
                actual_model: actual_model.clone(),
                effort: request.effort(),
                usage: usage.clone(),
            },
            ChatRunTiming {
                started_at: input.started_at,
                finished_at,
                first_token_latency,
            },
            final_status,
        )
        .await;
        let metrics = ChatTurnMetrics {
            actual_model,
            usage,
            first_token_latency,
        };
        let summary = ChatTurnSummary::new(request.identity(), input.mode, metrics, finished_at);
        emit_terminal_chunk(
            final_status,
            sources,
            attach_check(summary, number_check),
            sink,
        )
        .await
    }

    async fn store_reply(
        &self,
        request: &SendChatRequest,
        entry: &StoredChatEntry,
        status: ChatTurnStatus,
    ) -> ChatTurnStatus {
        match self
            .repository
            .append_entry(request.session_id(), entry)
            .await
        {
            Ok(_) => status,
            Err(error) => ChatTurnStatus::Failed(ChatFailureReason::StorageError(error.failure())),
        }
    }

    async fn number_check(
        &self,
        request: &SendChatRequest,
        status: ChatTurnStatus,
        answer: &UntrustedText,
    ) -> Option<ChatNumberCheck> {
        if status != ChatTurnStatus::Completed {
            return None;
        }
        match self.evidence.evidence_for_run(request.run_id()).await {
            Ok(ChatNumberEvidenceSet::NotConfigured) => None,
            Ok(ChatNumberEvidenceSet::Payloads(payloads)) => {
                Some(check_chat_numbers(answer, &payloads))
            }
            Err(_unavailable) => Some(ChatNumberCheck::Unverified),
        }
    }

    async fn record_ledger(
        &self,
        request: &SendChatRequest,
        model: ChatRunModelSpec,
        timing: ChatRunTiming,
        status: ChatTurnStatus,
    ) {
        let _recorded = self
            .ledger
            .record_chat_run(&ChatRunLedgerEntry::new(
                request.identity(),
                ChatRunProfile {
                    kind: RunKind::Analysis,
                    purpose: RunPurpose::Chat,
                },
                model,
                timing,
                status,
            ))
            .await;
    }
}

struct StreamDriver<'a, Stream, Sink> {
    stream: &'a mut Stream,
    accumulator: &'a mut StreamAccumulator,
    sink: &'a mut Sink,
}

struct ToolCallInfo {
    server: UntrustedText,
    tool: UntrustedText,
    status: ToolStatus,
}

fn attach_check(summary: ChatTurnSummary, check: Option<ChatNumberCheck>) -> ChatTurnSummary {
    match check {
        Some(found) => summary.with_number_check(found),
        None => summary,
    }
}

async fn emit_terminal_chunk<Sink: ChatChunkSink>(
    status: ChatTurnStatus,
    sources: Vec<ChatSource>,
    summary: ChatTurnSummary,
    sink: &mut Sink,
) -> SendChatOutcome {
    match status {
        ChatTurnStatus::Completed => {
            let _sent_sources = sink.send(ChatChunk::Sources(sources)).await;
            let _sent_done = sink.send(ChatChunk::Done(summary.clone())).await;
            SendChatOutcome::Completed(summary)
        }
        ChatTurnStatus::Cancelled => {
            let _sent = sink.send(ChatChunk::Cancelled).await;
            SendChatOutcome::Cancelled(summary)
        }
        ChatTurnStatus::Interrupted | ChatTurnStatus::InProgress => {
            let _sent = sink.send(ChatChunk::Interrupted).await;
            SendChatOutcome::Interrupted(summary)
        }
        ChatTurnStatus::RateLimited(resets_at) => {
            let _sent = sink.send(ChatChunk::RateLimited { resets_at }).await;
            SendChatOutcome::RateLimited { summary, resets_at }
        }
        ChatTurnStatus::Failed(reason) => {
            let _sent = sink.send(ChatChunk::Failed(reason)).await;
            SendChatOutcome::Failed { summary, reason }
        }
    }
}

const fn finish_eof_status(isolation: IsolationPhase) -> ChatTurnStatus {
    match isolation {
        IsolationPhase::Verified => ChatTurnStatus::Completed,
        IsolationPhase::Pending => ChatTurnStatus::Failed(ChatFailureReason::IsolationNotVerified),
    }
}

const fn map_stream_error(error: InformationError, isolation: IsolationPhase) -> ChatTurnStatus {
    match error {
        InformationError::Unavailable(ResetTime::Known(ts)) => {
            ChatTurnStatus::RateLimited(ResetTime::Known(ts))
        }
        InformationError::Unavailable(ResetTime::NotProvided) => match isolation {
            IsolationPhase::Verified => ChatTurnStatus::Interrupted,
            IsolationPhase::Pending => {
                ChatTurnStatus::Failed(ChatFailureReason::ProviderUnavailable)
            }
        },
        InformationError::InvalidResponse(mismatch) => {
            ChatTurnStatus::Failed(ChatFailureReason::InvalidResponse(mismatch))
        }
        InformationError::InvalidCredential(source) => {
            ChatTurnStatus::Failed(ChatFailureReason::InvalidCredential(source))
        }
    }
}
