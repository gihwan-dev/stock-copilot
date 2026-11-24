use crate::{
    AiEvent, AiEventStream, AiProvider, AiRunId, AuditLog, ChatFailureReason, ChatRepository,
    ChatRunLedger, ChatRunLedgerEntry, ChatRunModelSpec, ChatRunProfile, ChatRunTiming,
    ChatSessionId, ChatSignalState, ChatStopSignal, ChatTurnId, ChatTurnIdentity, ChatTurnStatus,
    Clock, CollectionQuery, CredentialSource, InformationError, IsolationCheck, IsolationFailure,
    ModelName, ModelSelection, ReasoningEffort, ResetTime, ResponseMismatch, RunInput, RunKind,
    RunPurpose, RunSpec, StoredChatContent, StoredUserMessage, SystemFailure, TokenUsage,
    WebSearchGuard, WebSearchGuardError,
};
use domain::{Timestamp, UntrustedText};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatWebResearchRequest {
    session_id: ChatSessionId,
    turn_id: ChatTurnId,
    collection_run_id: AiRunId,
    model: ModelSelection,
    effort: ReasoningEffort,
}

impl ChatWebResearchRequest {
    #[must_use]
    pub const fn new(
        session_id: ChatSessionId,
        turn_id: ChatTurnId,
        collection_run_id: AiRunId,
        model: ModelSelection,
        effort: ReasoningEffort,
    ) -> Self {
        Self {
            session_id,
            turn_id,
            collection_run_id,
            model,
            effort,
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
    pub const fn collection_run_id(&self) -> &AiRunId {
        &self.collection_run_id
    }

    fn identity(&self) -> ChatTurnIdentity {
        ChatTurnIdentity {
            session: self.session_id.clone(),
            turn: self.turn_id.clone(),
            run: self.collection_run_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatWebResearchFailure {
    UserMessageMissing,
    Storage(SystemFailure),
    AuditFailed(SystemFailure),
    IsolationRejected(IsolationFailure),
    IsolationNotVerified,
    ForbiddenToolAttempt,
    ProviderUnavailable,
    InvalidResponse(ResponseMismatch),
    InvalidCredential(CredentialSource),
    RateLimited(ResetTime),
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatWebResearchFindings {
    summary: UntrustedText,
    actual_model: Option<ModelName>,
}

impl ChatWebResearchFindings {
    #[must_use]
    pub const fn summary(&self) -> &UntrustedText {
        &self.summary
    }

    #[must_use]
    pub const fn actual_model(&self) -> Option<&ModelName> {
        self.actual_model.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CollectionIsolation {
    Pending,
    Verified,
}

#[derive(Debug)]
struct CollectionAccumulator {
    isolation: CollectionIsolation,
    summary: UntrustedText,
    actual_model: Option<ModelName>,
    usage: Option<TokenUsage>,
}

impl CollectionAccumulator {
    const fn new() -> Self {
        Self {
            isolation: CollectionIsolation::Pending,
            summary: UntrustedText::empty(),
            actual_model: None,
            usage: None,
        }
    }

    fn append(&mut self, delta: &UntrustedText) {
        let mut combined = self.summary.as_untrusted_str().to_owned();
        for ch in delta.as_untrusted_str().chars() {
            let Some(next_len) = combined.len().checked_add(ch.len_utf8()) else {
                break;
            };
            if next_len > 30_000 {
                break;
            }
            combined.push(ch);
        }
        if let Ok(updated) = UntrustedText::try_new(&combined) {
            self.summary = updated;
        }
    }
}

#[derive(Debug)]
pub struct ChatWebResearch<'a, Provider, Repo, Ledger, Audit, Time> {
    provider: &'a Provider,
    repository: &'a Repo,
    ledger: &'a Ledger,
    audit_log: &'a Audit,
    clock: &'a Time,
}

impl<'a, Provider, Repo, Ledger, Audit, Time>
    ChatWebResearch<'a, Provider, Repo, Ledger, Audit, Time>
where
    Provider: AiProvider,
    Repo: ChatRepository,
    Ledger: ChatRunLedger,
    Audit: AuditLog,
    Time: Clock,
{
    #[must_use]
    pub const fn new(
        provider: &'a Provider,
        repository: &'a Repo,
        ledger: &'a Ledger,
        audit_log: &'a Audit,
        clock: &'a Time,
    ) -> Self {
        Self {
            provider,
            repository,
            ledger,
            audit_log,
            clock,
        }
    }

    pub async fn execute<Stop: ChatStopSignal>(
        &self,
        request: &ChatWebResearchRequest,
        stop: &Stop,
    ) -> Result<ChatWebResearchFindings, ChatWebResearchFailure> {
        let started_at = self.clock.now();
        let message = self.load_turn_message(request).await?;
        let guard = WebSearchGuard::new(self.clock, self.audit_log);
        let direct =
            guard
                .record_user_direct_query(&message)
                .await
                .map_err(|error| match error {
                    WebSearchGuardError::AuditFailed(port) => {
                        ChatWebResearchFailure::AuditFailed(port.failure())
                    }
                })?;
        let spec = RunSpec::new(
            request.collection_run_id().clone(),
            RunInput::Collection(CollectionQuery::UserMessage(direct.message().clone())),
            request.model.clone(),
            request.effort,
            RunPurpose::WebCollection,
        );
        let mut accumulator = CollectionAccumulator::new();
        let result = match self.provider.run(spec).await {
            Ok(mut stream) => {
                self.drive(
                    &mut stream,
                    &mut accumulator,
                    request.collection_run_id(),
                    stop,
                )
                .await
            }
            Err(error) => Err(map_error(error, accumulator.isolation)),
        };
        self.record(request, &accumulator, started_at, &result)
            .await;
        let summary = result?;
        Ok(ChatWebResearchFindings {
            summary,
            actual_model: accumulator.actual_model,
        })
    }

    async fn load_turn_message(
        &self,
        request: &ChatWebResearchRequest,
    ) -> Result<StoredUserMessage, ChatWebResearchFailure> {
        let session = self
            .repository
            .load_session(request.session_id())
            .await
            .map_err(|error| ChatWebResearchFailure::Storage(error.failure()))?
            .ok_or(ChatWebResearchFailure::UserMessageMissing)?;
        session
            .entries()
            .iter()
            .rev()
            .find_map(|entry| match entry.content() {
                StoredChatContent::User(user) if entry.turn_id() == request.turn_id() => {
                    Some(user.clone())
                }
                StoredChatContent::User(_) | StoredChatContent::Assistant(_) => None,
            })
            .ok_or(ChatWebResearchFailure::UserMessageMissing)
    }

    async fn drive<Stream: AiEventStream, Stop: ChatStopSignal>(
        &self,
        stream: &mut Stream,
        accumulator: &mut CollectionAccumulator,
        run_id: &AiRunId,
        stop: &Stop,
    ) -> Result<UntrustedText, ChatWebResearchFailure> {
        loop {
            match stop.state() {
                ChatSignalState::Active => {}
                ChatSignalState::Cancelled => {
                    let _interrupted = self.provider.interrupt(run_id).await;
                    return Err(ChatWebResearchFailure::Cancelled);
                }
                ChatSignalState::Interrupted => {
                    let _interrupted = self.provider.interrupt(run_id).await;
                    return Err(ChatWebResearchFailure::Interrupted);
                }
            }
            let event = match stream.next().await {
                Ok(Some(event)) => event,
                Ok(None) => return finish(accumulator),
                Err(error) => return Err(map_error(error, accumulator.isolation)),
            };
            if let Some(failure) = handle_event(event, accumulator) {
                let _interrupted = self.provider.interrupt(run_id).await;
                return Err(failure);
            }
        }
    }

    async fn record(
        &self,
        request: &ChatWebResearchRequest,
        accumulator: &CollectionAccumulator,
        started_at: Timestamp,
        result: &Result<UntrustedText, ChatWebResearchFailure>,
    ) {
        let status = match *result {
            Ok(_) => ChatTurnStatus::Completed,
            Err(failure) => failure_status(failure),
        };
        let _recorded = self
            .ledger
            .record_chat_run(&ChatRunLedgerEntry::new(
                request.identity(),
                ChatRunProfile {
                    kind: RunKind::Collection,
                    purpose: RunPurpose::WebCollection,
                },
                ChatRunModelSpec {
                    requested_model: request.model.clone(),
                    actual_model: accumulator.actual_model.clone(),
                    effort: request.effort,
                    usage: accumulator.usage.clone(),
                },
                ChatRunTiming {
                    started_at,
                    finished_at: self.clock.now(),
                    first_token_latency: None,
                },
                status,
            ))
            .await;
    }
}

fn handle_event(
    event: AiEvent,
    accumulator: &mut CollectionAccumulator,
) -> Option<ChatWebResearchFailure> {
    if accumulator.isolation == CollectionIsolation::Pending {
        return match event {
            AiEvent::IsolationChecked(IsolationCheck::Passed) => {
                accumulator.isolation = CollectionIsolation::Verified;
                None
            }
            AiEvent::IsolationChecked(IsolationCheck::Rejected(failure)) => {
                Some(ChatWebResearchFailure::IsolationRejected(failure))
            }
            AiEvent::TextDelta(_)
            | AiEvent::ToolCall { .. }
            | AiEvent::WebSearch { .. }
            | AiEvent::TokenUsage(_)
            | AiEvent::Quota(_)
            | AiEvent::RateLimited { .. }
            | AiEvent::ModelRerouted(_)
            | AiEvent::ActualModel(_)
            | AiEvent::McpUnavailable
            | AiEvent::Unknown(_) => Some(ChatWebResearchFailure::IsolationNotVerified),
        };
    }
    match event {
        AiEvent::IsolationChecked(IsolationCheck::Passed)
        | AiEvent::WebSearch { .. }
        | AiEvent::Quota(crate::AiUsage::Available(_) | crate::AiUsage::Unknown)
        | AiEvent::McpUnavailable
        | AiEvent::Unknown(_) => None,
        AiEvent::IsolationChecked(IsolationCheck::Rejected(failure)) => {
            Some(ChatWebResearchFailure::IsolationRejected(failure))
        }
        AiEvent::ToolCall { .. } => Some(ChatWebResearchFailure::ForbiddenToolAttempt),
        AiEvent::TextDelta(delta) => {
            accumulator.append(&delta);
            None
        }
        AiEvent::TokenUsage(usage) => {
            accumulator.usage = Some(usage);
            None
        }
        AiEvent::ActualModel(model) | AiEvent::ModelRerouted(model) => {
            accumulator.actual_model = Some(model);
            None
        }
        AiEvent::RateLimited { resets_at } | AiEvent::Quota(crate::AiUsage::Waiting(resets_at)) => {
            Some(ChatWebResearchFailure::RateLimited(resets_at))
        }
    }
}

fn finish(accumulator: &CollectionAccumulator) -> Result<UntrustedText, ChatWebResearchFailure> {
    match accumulator.isolation {
        CollectionIsolation::Pending => Err(ChatWebResearchFailure::IsolationNotVerified),
        CollectionIsolation::Verified => Ok(accumulator.summary.clone()),
    }
}

const fn map_error(
    error: InformationError,
    isolation: CollectionIsolation,
) -> ChatWebResearchFailure {
    match error {
        InformationError::Unavailable(ResetTime::Known(at)) => {
            ChatWebResearchFailure::RateLimited(ResetTime::Known(at))
        }
        InformationError::Unavailable(ResetTime::NotProvided) => match isolation {
            CollectionIsolation::Verified => ChatWebResearchFailure::Interrupted,
            CollectionIsolation::Pending => ChatWebResearchFailure::ProviderUnavailable,
        },
        InformationError::InvalidResponse(mismatch) => {
            ChatWebResearchFailure::InvalidResponse(mismatch)
        }
        InformationError::InvalidCredential(source) => {
            ChatWebResearchFailure::InvalidCredential(source)
        }
    }
}

const fn failure_status(research: ChatWebResearchFailure) -> ChatTurnStatus {
    match research {
        ChatWebResearchFailure::Cancelled => ChatTurnStatus::Cancelled,
        ChatWebResearchFailure::Interrupted => ChatTurnStatus::Interrupted,
        ChatWebResearchFailure::RateLimited(at) => ChatTurnStatus::RateLimited(at),
        ChatWebResearchFailure::IsolationRejected(failure) => {
            ChatTurnStatus::Failed(ChatFailureReason::IsolationRejected(failure))
        }
        ChatWebResearchFailure::IsolationNotVerified => {
            ChatTurnStatus::Failed(ChatFailureReason::IsolationNotVerified)
        }
        ChatWebResearchFailure::ForbiddenToolAttempt => {
            ChatTurnStatus::Failed(ChatFailureReason::ForbiddenToolAttempt)
        }
        ChatWebResearchFailure::ProviderUnavailable => {
            ChatTurnStatus::Failed(ChatFailureReason::ProviderUnavailable)
        }
        ChatWebResearchFailure::InvalidResponse(mismatch) => {
            ChatTurnStatus::Failed(ChatFailureReason::InvalidResponse(mismatch))
        }
        ChatWebResearchFailure::InvalidCredential(source) => {
            ChatTurnStatus::Failed(ChatFailureReason::InvalidCredential(source))
        }
        ChatWebResearchFailure::UserMessageMissing => {
            ChatTurnStatus::Failed(ChatFailureReason::EmptyMessage)
        }
        ChatWebResearchFailure::Storage(failure) | ChatWebResearchFailure::AuditFailed(failure) => {
            ChatTurnStatus::Failed(ChatFailureReason::StorageError(failure))
        }
    }
}
