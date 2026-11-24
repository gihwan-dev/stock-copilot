use application::{
    AiRunId, ChatCard, ChatCardKind, ChatChunk, ChatExecutionMode, ChatFailureReason,
    ChatModelConfig, ChatNumberCheck, ChatScreenContext, ChatSessionId, ChatSessionRecord,
    ChatSource, ChatSourceKind, ChatTurnId, ChatTurnIdentity, ChatTurnStatus, ChatTurnSummary,
    ModelName, ModelSelection, ReasoningEffort, ResetTime, SendChatOutcome, SendChatRequest,
    StoredChatContent, StoredChatEntry, ToolStatus, UserMessageId,
};
use domain::{HorizonSelection, KnownMarket, Symbol, TimeSpan, UntrustedText};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::error::{IpcError, IpcErrorKind};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub enum ChatModelSelectionDto {
    Default,
    Highest,
    Fast,
    Named { name: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub enum ChatReasoningEffortDto {
    Default,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub enum ChatMarketDto {
    Kr,
    Us,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub enum ChatHorizonDto {
    FewDays,
    WeeksToMonths,
    HalfYearOrMore,
    Hours,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct ChatScreenContextDto {
    pub symbol: Option<String>,
    pub market: Option<ChatMarketDto>,
    pub horizon: Option<ChatHorizonDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct SendChatMessageRequest {
    pub session_id: String,
    pub turn_id: String,
    pub run_id: String,
    pub user_message_id: String,
    pub message: String,
    pub context: Option<ChatScreenContextDto>,
    pub model: ChatModelSelectionDto,
    pub effort: ChatReasoningEffortDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetChatHistoryRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct ResubscribeChatTurnRequest {
    pub turn_id: String,
    pub after_sequence: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct CancelChatTurnRequest {
    pub turn_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum ChatNumberCheckDto {
    NoNumbers,
    Verified,
    Unverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum ChatToolStatusDto {
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum ChatCardKindDto {
    Signal,
    Proposal,
    Holding,
    Quote,
    Briefing,
    Disclosure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum ChatSourceKindDto {
    Tool,
    Disclosure,
    News,
    Web,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ChatCardDto {
    pub kind: ChatCardKindDto,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ChatSourceDto {
    pub kind: ChatSourceKindDto,
    pub reference: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum ChatExecutionModeDto {
    FreshAnalysis,
    ResumedFromHistory,
    WarmPreverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum ChatFailureReasonDto {
    IsolationRejected,
    IsolationNotVerified,
    McpUnavailable,
    ProviderUnavailable,
    InvalidResponse,
    InvalidCredential,
    ForbiddenToolAttempt,
    EmptyMessage,
    PromptTooLong,
    StorageError,
    QueueBusy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum ChatTurnStatusDto {
    InProgress,
    Completed,
    Cancelled,
    Interrupted,
    RateLimited,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum ChatChunkDto {
    TextDelta {
        text: String,
    },
    ToolCallStarted {
        server: String,
        tool: String,
    },
    ToolCallFinished {
        server: String,
        tool: String,
        status: ChatToolStatusDto,
    },
    Card {
        kind: ChatCardKindDto,
        id: String,
    },
    Sources {
        items: Vec<ChatSourceDto>,
    },
    NumberCheck {
        status: ChatNumberCheckDto,
    },
    Done {
        session_id: String,
        turn_id: String,
        run_id: String,
        mode: ChatExecutionModeDto,
        actual_model: Option<String>,
        first_token_latency_ms: Option<u32>,
        finished_at_unix_seconds: String,
    },
    Cancelled,
    Interrupted,
    RateLimited {
        resets_at_unix_seconds: Option<String>,
    },
    Failed {
        reason: ChatFailureReasonDto,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ChatChunkEvent {
    pub sequence: u32,
    pub turn_id: String,
    pub chunk: ChatChunkDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct SendChatMessageResponse {
    pub session_id: String,
    pub turn_id: String,
    pub run_id: String,
    pub status: ChatTurnStatusDto,
    pub mode: ChatExecutionModeDto,
    pub actual_model: Option<String>,
    pub first_token_latency_ms: Option<u32>,
    pub resets_at_unix_seconds: Option<String>,
    pub failure_reason: Option<ChatFailureReasonDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum ActiveTurnStateDto {
    Running,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ActiveChatTurnDto {
    pub turn_id: String,
    pub last_sequence: u32,
    pub state: ActiveTurnStateDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub enum StoredChatEntryRoleDto {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct StoredChatEntryDto {
    pub sequence: u32,
    pub at_unix_seconds: String,
    pub turn_id: String,
    pub role: StoredChatEntryRoleDto,
    pub text: String,
    pub status: Option<ChatTurnStatusDto>,
    pub cards: Vec<ChatCardDto>,
    pub sources: Vec<ChatSourceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct GetChatHistoryResponse {
    pub session_id: String,
    pub entries: Vec<StoredChatEntryDto>,
    pub active_turn: Option<ActiveChatTurnDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct ResubscribeChatTurnResponse {
    pub turn_id: String,
    pub replayed_count: u32,
    pub state: ActiveTurnStateDto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum CancelOutcomeDto {
    Signaled,
    NotRunning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct CancelChatTurnResponse {
    pub turn_id: String,
    pub outcome: CancelOutcomeDto,
}

fn saturate_u32(value: u64) -> u32 {
    let Ok(narrow) = u32::try_from(value) else {
        return u32::MAX;
    };
    narrow
}

#[must_use]
pub fn time_span_to_ms(span: TimeSpan) -> u32 {
    let (seconds, nanos) = span.as_parts();
    let Some(subsec_ms) = nanos.as_u32().checked_div(1_000_000) else {
        return 0;
    };
    let total = seconds
        .as_u64()
        .saturating_mul(1_000)
        .saturating_add(u64::from(subsec_ms));
    saturate_u32(total)
}

#[must_use]
pub fn reset_time_to_string(resets_at: ResetTime) -> Option<String> {
    match resets_at {
        ResetTime::Known(timestamp) => Some(timestamp.utc_seconds().as_i64().to_string()),
        ResetTime::NotProvided => None,
    }
}

pub fn into_domain_request(request: SendChatMessageRequest) -> Result<SendChatRequest, IpcError> {
    let session = ChatSessionId::try_new(&request.session_id).map_err(|_error| IpcError {
        kind: IpcErrorKind::Bug,
    })?;
    let turn = ChatTurnId::try_new(&request.turn_id).map_err(|_error| IpcError {
        kind: IpcErrorKind::Bug,
    })?;
    let run = AiRunId::try_new(&request.run_id).map_err(|_error| IpcError {
        kind: IpcErrorKind::Bug,
    })?;
    let user_message_id =
        UserMessageId::try_new(&request.user_message_id).map_err(|_error| IpcError {
            kind: IpcErrorKind::Bug,
        })?;
    let message = UntrustedText::try_new(&request.message).map_err(|_error| IpcError {
        kind: IpcErrorKind::UserFixable,
    })?;
    let context = convert_context(request.context)?;
    let model = convert_model(request.model)?;
    let effort = convert_effort(request.effort);
    Ok(SendChatRequest::new(
        ChatTurnIdentity { session, turn, run },
        user_message_id,
        message,
        context,
        ChatModelConfig { model, effort },
    ))
}

fn convert_context(
    context: Option<ChatScreenContextDto>,
) -> Result<Option<ChatScreenContext>, IpcError> {
    let Some(dto) = context else {
        return Ok(None);
    };
    let symbol = match dto.symbol {
        Some(raw) => Some(Symbol::try_new(&raw).map_err(|_error| IpcError {
            kind: IpcErrorKind::Bug,
        })?),
        None => None,
    };
    let market = dto.market.map(|value| match value {
        ChatMarketDto::Kr => KnownMarket::Kr,
        ChatMarketDto::Us => KnownMarket::Us,
    });
    let horizon = dto.horizon.map(|value| match value {
        ChatHorizonDto::FewDays => HorizonSelection::FewDays,
        ChatHorizonDto::WeeksToMonths => HorizonSelection::WeeksToMonths,
        ChatHorizonDto::HalfYearOrMore => HorizonSelection::HalfYearOrMore,
        ChatHorizonDto::Hours => HorizonSelection::Hours,
    });
    Ok(Some(ChatScreenContext::new(symbol, market, horizon)))
}

fn convert_model(selection: ChatModelSelectionDto) -> Result<ModelSelection, IpcError> {
    match selection {
        ChatModelSelectionDto::Default => Ok(ModelSelection::Default),
        ChatModelSelectionDto::Highest => Ok(ModelSelection::Highest),
        ChatModelSelectionDto::Fast => Ok(ModelSelection::Fast),
        ChatModelSelectionDto::Named { name } => ModelName::try_new(&name)
            .map(ModelSelection::Named)
            .map_err(|_error| IpcError {
                kind: IpcErrorKind::Bug,
            }),
    }
}

const fn convert_effort(effort: ChatReasoningEffortDto) -> ReasoningEffort {
    match effort {
        ChatReasoningEffortDto::Default => ReasoningEffort::Default,
        ChatReasoningEffortDto::Low => ReasoningEffort::Low,
        ChatReasoningEffortDto::Medium => ReasoningEffort::Medium,
        ChatReasoningEffortDto::High => ReasoningEffort::High,
        ChatReasoningEffortDto::XHigh => ReasoningEffort::XHigh,
        ChatReasoningEffortDto::Max => ReasoningEffort::Max,
    }
}

#[must_use]
pub fn number_check_dto(chunk: &ChatChunk) -> Option<ChatChunkDto> {
    let ChatChunk::Done(summary) = chunk else {
        return None;
    };
    summary
        .number_check()
        .map(|check| ChatChunkDto::NumberCheck {
            status: match check {
                ChatNumberCheck::NoNumbers => ChatNumberCheckDto::NoNumbers,
                ChatNumberCheck::Verified => ChatNumberCheckDto::Verified,
                ChatNumberCheck::Unverified => ChatNumberCheckDto::Unverified,
            },
        })
}

pub fn chunk_to_dto(chunk: &ChatChunk) -> ChatChunkDto {
    match chunk {
        ChatChunk::TextDelta(text) => ChatChunkDto::TextDelta {
            text: text.as_untrusted_str().to_owned(),
        },
        ChatChunk::ToolCallStarted { server, tool } => ChatChunkDto::ToolCallStarted {
            server: server.as_untrusted_str().to_owned(),
            tool: tool.as_untrusted_str().to_owned(),
        },
        ChatChunk::ToolCallFinished {
            server,
            tool,
            status,
        } => ChatChunkDto::ToolCallFinished {
            server: server.as_untrusted_str().to_owned(),
            tool: tool.as_untrusted_str().to_owned(),
            status: tool_status_to_dto(*status),
        },
        ChatChunk::Card(card) => ChatChunkDto::Card {
            kind: card_kind_to_dto(card.kind()),
            id: card.id().as_str().to_owned(),
        },
        ChatChunk::Sources(listed) => ChatChunkDto::Sources {
            items: listed.iter().map(source_to_dto).collect(),
        },
        ChatChunk::Done(summary) => done_chunk_to_dto(summary),
        ChatChunk::Cancelled => ChatChunkDto::Cancelled,
        ChatChunk::Interrupted => ChatChunkDto::Interrupted,
        ChatChunk::RateLimited { resets_at } => ChatChunkDto::RateLimited {
            resets_at_unix_seconds: reset_time_to_string(*resets_at),
        },
        ChatChunk::Failed(reason) => ChatChunkDto::Failed {
            reason: failure_reason_to_dto(*reason),
        },
    }
}

fn done_chunk_to_dto(summary: &ChatTurnSummary) -> ChatChunkDto {
    ChatChunkDto::Done {
        session_id: summary.session_id().as_str().to_owned(),
        turn_id: summary.turn_id().as_str().to_owned(),
        run_id: summary.run_id().as_str().to_owned(),
        mode: execution_mode_to_dto(summary.mode()),
        actual_model: summary
            .actual_model()
            .map(|model| model.as_str().to_owned()),
        first_token_latency_ms: summary.first_token_latency().map(time_span_to_ms),
        finished_at_unix_seconds: summary.completed_at().utc_seconds().as_i64().to_string(),
    }
}

pub fn outcome_to_response(outcome: &SendChatOutcome) -> SendChatMessageResponse {
    let (summary, status, resets_at, failure_reason) = match outcome {
        SendChatOutcome::Completed(summary) => (summary, ChatTurnStatusDto::Completed, None, None),
        SendChatOutcome::Cancelled(summary) => (summary, ChatTurnStatusDto::Cancelled, None, None),
        SendChatOutcome::Interrupted(summary) => {
            (summary, ChatTurnStatusDto::Interrupted, None, None)
        }
        SendChatOutcome::RateLimited { summary, resets_at } => (
            summary,
            ChatTurnStatusDto::RateLimited,
            reset_time_to_string(*resets_at),
            None,
        ),
        SendChatOutcome::Failed { summary, reason } => (
            summary,
            ChatTurnStatusDto::Failed,
            None,
            Some(failure_reason_to_dto(*reason)),
        ),
    };
    SendChatMessageResponse {
        session_id: summary.session_id().as_str().to_owned(),
        turn_id: summary.turn_id().as_str().to_owned(),
        run_id: summary.run_id().as_str().to_owned(),
        status,
        mode: execution_mode_to_dto(summary.mode()),
        actual_model: summary
            .actual_model()
            .map(|model| model.as_str().to_owned()),
        first_token_latency_ms: summary.first_token_latency().map(time_span_to_ms),
        resets_at_unix_seconds: resets_at,
        failure_reason,
    }
}

pub fn session_entries_to_dto(session: Option<&ChatSessionRecord>) -> Vec<StoredChatEntryDto> {
    let Some(record) = session else {
        return Vec::new();
    };
    record.entries().iter().map(stored_entry_to_dto).collect()
}

fn stored_entry_to_dto(entry: &StoredChatEntry) -> StoredChatEntryDto {
    let sequence = saturate_u32(*entry.sequence().as_u64());
    let at_unix_seconds = entry.at().utc_seconds().as_i64().to_string();
    let turn_id = entry.turn_id().as_str().to_owned();
    match entry.content() {
        StoredChatContent::User(message) => StoredChatEntryDto {
            sequence,
            at_unix_seconds,
            turn_id,
            role: StoredChatEntryRoleDto::User,
            text: message.text().as_untrusted_str().to_owned(),
            status: None,
            cards: Vec::new(),
            sources: Vec::new(),
        },
        StoredChatContent::Assistant(reply) => StoredChatEntryDto {
            sequence,
            at_unix_seconds,
            turn_id,
            role: StoredChatEntryRoleDto::Assistant,
            text: reply.text().as_untrusted_str().to_owned(),
            status: Some(turn_status_to_dto(reply.status())),
            cards: reply.cards().iter().map(card_to_dto).collect(),
            sources: reply.sources().iter().map(source_to_dto).collect(),
        },
    }
}

const fn tool_status_to_dto(status: ToolStatus) -> ChatToolStatusDto {
    match status {
        ToolStatus::Running => ChatToolStatusDto::Running,
        ToolStatus::Succeeded => ChatToolStatusDto::Succeeded,
        ToolStatus::Failed => ChatToolStatusDto::Failed,
    }
}

const fn card_kind_to_dto(kind: ChatCardKind) -> ChatCardKindDto {
    match kind {
        ChatCardKind::Signal => ChatCardKindDto::Signal,
        ChatCardKind::Proposal => ChatCardKindDto::Proposal,
        ChatCardKind::Holding => ChatCardKindDto::Holding,
        ChatCardKind::Quote => ChatCardKindDto::Quote,
        ChatCardKind::Briefing => ChatCardKindDto::Briefing,
        ChatCardKind::Disclosure => ChatCardKindDto::Disclosure,
    }
}

fn card_to_dto(card: &ChatCard) -> ChatCardDto {
    ChatCardDto {
        kind: card_kind_to_dto(card.kind()),
        id: card.id().as_str().to_owned(),
    }
}

fn source_to_dto(source: &ChatSource) -> ChatSourceDto {
    let kind = match source.kind() {
        ChatSourceKind::Tool => ChatSourceKindDto::Tool,
        ChatSourceKind::Disclosure => ChatSourceKindDto::Disclosure,
        ChatSourceKind::News => ChatSourceKindDto::News,
        ChatSourceKind::Web => ChatSourceKindDto::Web,
    };
    ChatSourceDto {
        kind,
        reference: source.reference().as_untrusted_str().to_owned(),
        url: source.url().map(|url| url.as_str().to_owned()),
    }
}

const fn execution_mode_to_dto(mode: ChatExecutionMode) -> ChatExecutionModeDto {
    match mode {
        ChatExecutionMode::FreshAnalysis => ChatExecutionModeDto::FreshAnalysis,
        ChatExecutionMode::ResumedFromHistory => ChatExecutionModeDto::ResumedFromHistory,
        ChatExecutionMode::WarmPreverified => ChatExecutionModeDto::WarmPreverified,
    }
}

const fn turn_status_to_dto(status: ChatTurnStatus) -> ChatTurnStatusDto {
    match status {
        ChatTurnStatus::InProgress => ChatTurnStatusDto::InProgress,
        ChatTurnStatus::Completed => ChatTurnStatusDto::Completed,
        ChatTurnStatus::Cancelled => ChatTurnStatusDto::Cancelled,
        ChatTurnStatus::Interrupted => ChatTurnStatusDto::Interrupted,
        ChatTurnStatus::RateLimited(_) => ChatTurnStatusDto::RateLimited,
        ChatTurnStatus::Failed(_) => ChatTurnStatusDto::Failed,
    }
}

#[must_use]
pub const fn failure_reason_to_dto(reason: ChatFailureReason) -> ChatFailureReasonDto {
    match reason {
        ChatFailureReason::IsolationRejected(_) => ChatFailureReasonDto::IsolationRejected,
        ChatFailureReason::IsolationNotVerified => ChatFailureReasonDto::IsolationNotVerified,
        ChatFailureReason::McpUnavailable => ChatFailureReasonDto::McpUnavailable,
        ChatFailureReason::ProviderUnavailable => ChatFailureReasonDto::ProviderUnavailable,
        ChatFailureReason::InvalidResponse(_) => ChatFailureReasonDto::InvalidResponse,
        ChatFailureReason::InvalidCredential(_) => ChatFailureReasonDto::InvalidCredential,
        ChatFailureReason::ForbiddenToolAttempt => ChatFailureReasonDto::ForbiddenToolAttempt,
        ChatFailureReason::EmptyMessage => ChatFailureReasonDto::EmptyMessage,
        ChatFailureReason::PromptTooLong => ChatFailureReasonDto::PromptTooLong,
        ChatFailureReason::StorageError(_) => ChatFailureReasonDto::StorageError,
        ChatFailureReason::QueueBusy => ChatFailureReasonDto::QueueBusy,
    }
}
