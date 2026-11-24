use crate::{
    AiRunId, BugDiagnostic, ChatHistory, ChatMessage, ChatRole, ClassifiedError, CredentialSource,
    ErrorClass, IdentifierError, IsolationFailure, ModelName, ModelSelection, PromptError,
    ReasoningEffort, RequestRejection, ResetTime, ResponseMismatch, RetryDelay, RunKind,
    RunPurpose, StoredUserMessage, SystemDiagnostic, SystemFailure, TokenUsage, ToolStatus,
    UserAction, UserMessageId,
};
use domain::{
    HorizonSelection, KnownMarket, NormalizedUrl, Symbol, TimeSpan, Timestamp, UntrustedText,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChatSessionId(String);

impl ChatSessionId {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChatTurnId(String);

impl ChatTurnId {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChatCardId(String);

impl ChatCardId {
    pub fn try_new(value: &str) -> Result<Self, IdentifierError> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-:".contains(&byte))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChatSequence(u64);

impl ChatSequence {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(&self) -> &u64 {
        &self.0
    }

    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(next_value) => Some(Self(next_value)),
            None => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatScreenContext {
    symbol: Option<Symbol>,
    market: Option<KnownMarket>,
    horizon: Option<HorizonSelection>,
}

impl ChatScreenContext {
    #[must_use]
    pub const fn new(
        symbol: Option<Symbol>,
        market: Option<KnownMarket>,
        horizon: Option<HorizonSelection>,
    ) -> Self {
        Self {
            symbol,
            market,
            horizon,
        }
    }

    #[must_use]
    pub const fn symbol(&self) -> Option<&Symbol> {
        self.symbol.as_ref()
    }

    #[must_use]
    pub const fn market(&self) -> Option<KnownMarket> {
        self.market
    }

    #[must_use]
    pub const fn horizon(&self) -> Option<HorizonSelection> {
        self.horizon
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatCardKind {
    Signal,
    Proposal,
    Holding,
    Quote,
    Briefing,
    Disclosure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCard {
    kind: ChatCardKind,
    id: ChatCardId,
}

impl ChatCard {
    #[must_use]
    pub const fn new(kind: ChatCardKind, id: ChatCardId) -> Self {
        Self { kind, id }
    }

    #[must_use]
    pub const fn kind(&self) -> ChatCardKind {
        self.kind
    }

    #[must_use]
    pub const fn id(&self) -> &ChatCardId {
        &self.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatSourceKind {
    Tool,
    Disclosure,
    News,
    Web,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSource {
    kind: ChatSourceKind,
    reference: UntrustedText,
    url: Option<NormalizedUrl>,
}

impl ChatSource {
    #[must_use]
    pub const fn new(
        kind: ChatSourceKind,
        reference: UntrustedText,
        url: Option<NormalizedUrl>,
    ) -> Self {
        Self {
            kind,
            reference,
            url,
        }
    }

    #[must_use]
    pub const fn kind(&self) -> ChatSourceKind {
        self.kind
    }

    #[must_use]
    pub const fn reference(&self) -> &UntrustedText {
        &self.reference
    }

    #[must_use]
    pub const fn url(&self) -> Option<&NormalizedUrl> {
        self.url.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatFailureReason {
    IsolationRejected(IsolationFailure),
    IsolationNotVerified,
    McpUnavailable,
    ProviderUnavailable,
    InvalidResponse(ResponseMismatch),
    InvalidCredential(CredentialSource),
    ForbiddenToolAttempt,
    EmptyMessage,
    PromptTooLong,
    StorageError(SystemFailure),
    QueueBusy,
}

impl ClassifiedError for ChatFailureReason {
    fn class(&self) -> ErrorClass {
        match *self {
            Self::IsolationRejected(
                IsolationFailure::LoginRequired | IsolationFailure::NotInstalled,
            )
            | Self::InvalidCredential(_) => ErrorClass::UserAction(UserAction::CheckCredentials),
            Self::IsolationRejected(
                IsolationFailure::Tools
                | IsolationFailure::McpServers
                | IsolationFailure::Plugins
                | IsolationFailure::Skills
                | IsolationFailure::SlashCommands
                | IsolationFailure::PermissionMode
                | IsolationFailure::ApiKeySource
                | IsolationFailure::CliVersion,
            )
            | Self::IsolationNotVerified
            | Self::ForbiddenToolAttempt => {
                ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Validation))
            }
            Self::McpUnavailable | Self::ProviderUnavailable | Self::QueueBusy => {
                ErrorClass::Wait(RetryDelay::Unknown)
            }
            Self::InvalidResponse(mismatch) => ErrorClass::Bug(BugDiagnostic::Response(mismatch)),
            Self::EmptyMessage | Self::PromptTooLong => {
                ErrorClass::Bug(BugDiagnostic::Request(RequestRejection::MalformedRequest))
            }
            Self::StorageError(failure) => classify_storage_failure(failure),
        }
    }
}

const fn classify_storage_failure(failure: SystemFailure) -> ErrorClass {
    match failure {
        SystemFailure::PermissionDenied => ErrorClass::UserAction(UserAction::CheckPermissions),
        SystemFailure::Unavailable(retry) => ErrorClass::Wait(retry),
        SystemFailure::InvalidData => {
            ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::InvalidStoredData))
        }
        SystemFailure::RepositoryIntegrity => {
            ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::RepositoryIntegrity))
        }
        SystemFailure::SecretIntegrity => {
            ErrorClass::Bug(BugDiagnostic::System(SystemDiagnostic::SecretIntegrity))
        }
    }
}

impl From<PromptError> for ChatFailureReason {
    fn from(error: PromptError) -> Self {
        match error {
            PromptError::Empty => Self::EmptyMessage,
            PromptError::TooLong => Self::PromptTooLong,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatTurnStatus {
    InProgress,
    Completed,
    Cancelled,
    Interrupted,
    RateLimited(ResetTime),
    Failed(ChatFailureReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatExecutionMode {
    FreshAnalysis,
    ResumedFromHistory,
    WarmPreverified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurnIdentity {
    pub session: ChatSessionId,
    pub turn: ChatTurnId,
    pub run: AiRunId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurnMetrics {
    pub actual_model: Option<ModelName>,
    pub usage: Option<TokenUsage>,
    pub first_token_latency: Option<TimeSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTurnSummary {
    session_id: ChatSessionId,
    turn_id: ChatTurnId,
    run_id: AiRunId,
    mode: ChatExecutionMode,
    actual_model: Option<ModelName>,
    usage: Option<TokenUsage>,
    first_token_latency: Option<TimeSpan>,
    completed_at: Timestamp,
    number_check: Option<crate::ChatNumberCheck>,
}

impl ChatTurnSummary {
    #[must_use]
    pub const fn with_number_check(mut self, check: crate::ChatNumberCheck) -> Self {
        self.number_check = Some(check);
        self
    }

    #[must_use]
    pub const fn number_check(&self) -> Option<crate::ChatNumberCheck> {
        self.number_check
    }

    #[must_use]
    pub fn new(
        ids: ChatTurnIdentity,
        mode: ChatExecutionMode,
        metrics: ChatTurnMetrics,
        completed_at: Timestamp,
    ) -> Self {
        Self {
            session_id: ids.session,
            turn_id: ids.turn,
            run_id: ids.run,
            mode,
            actual_model: metrics.actual_model,
            usage: metrics.usage,
            first_token_latency: metrics.first_token_latency,
            completed_at,
            number_check: None,
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
    pub const fn mode(&self) -> ChatExecutionMode {
        self.mode
    }

    #[must_use]
    pub const fn actual_model(&self) -> Option<&ModelName> {
        self.actual_model.as_ref()
    }

    #[must_use]
    pub const fn usage(&self) -> Option<&TokenUsage> {
        self.usage.as_ref()
    }

    #[must_use]
    pub const fn first_token_latency(&self) -> Option<TimeSpan> {
        self.first_token_latency
    }

    #[must_use]
    pub const fn completed_at(&self) -> Timestamp {
        self.completed_at
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatChunk {
    TextDelta(UntrustedText),
    ToolCallStarted {
        server: UntrustedText,
        tool: UntrustedText,
    },
    ToolCallFinished {
        server: UntrustedText,
        tool: UntrustedText,
        status: ToolStatus,
    },
    Card(ChatCard),
    Sources(Vec<ChatSource>),
    Done(ChatTurnSummary),
    Cancelled,
    Interrupted,
    RateLimited {
        resets_at: ResetTime,
    },
    Failed(ChatFailureReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAssistantAttachments {
    pub actual_model: Option<ModelName>,
    pub sources: Vec<ChatSource>,
    pub cards: Vec<ChatCard>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAssistantReply {
    run_id: AiRunId,
    text: UntrustedText,
    status: ChatTurnStatus,
    actual_model: Option<ModelName>,
    sources: Vec<ChatSource>,
    cards: Vec<ChatCard>,
}

impl StoredAssistantReply {
    #[must_use]
    pub fn new(
        run_id: AiRunId,
        text: UntrustedText,
        status: ChatTurnStatus,
        attachments: StoredAssistantAttachments,
    ) -> Self {
        Self {
            run_id,
            text,
            status,
            actual_model: attachments.actual_model,
            sources: attachments.sources,
            cards: attachments.cards,
        }
    }

    #[must_use]
    pub const fn run_id(&self) -> &AiRunId {
        &self.run_id
    }

    #[must_use]
    pub const fn text(&self) -> &UntrustedText {
        &self.text
    }

    #[must_use]
    pub const fn status(&self) -> ChatTurnStatus {
        self.status
    }

    #[must_use]
    pub const fn actual_model(&self) -> Option<&ModelName> {
        self.actual_model.as_ref()
    }

    #[must_use]
    pub fn sources(&self) -> &[ChatSource] {
        &self.sources
    }

    #[must_use]
    pub fn cards(&self) -> &[ChatCard] {
        &self.cards
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredChatContent {
    User(StoredUserMessage),
    Assistant(StoredAssistantReply),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChatEntry {
    sequence: ChatSequence,
    at: Timestamp,
    turn_id: ChatTurnId,
    content: StoredChatContent,
}

impl StoredChatEntry {
    #[must_use]
    pub const fn new(
        sequence: ChatSequence,
        at: Timestamp,
        turn_id: ChatTurnId,
        content: StoredChatContent,
    ) -> Self {
        Self {
            sequence,
            at,
            turn_id,
            content,
        }
    }

    #[must_use]
    pub const fn sequence(&self) -> ChatSequence {
        self.sequence
    }

    #[must_use]
    pub const fn at(&self) -> Timestamp {
        self.at
    }

    #[must_use]
    pub const fn turn_id(&self) -> &ChatTurnId {
        &self.turn_id
    }

    #[must_use]
    pub const fn content(&self) -> &StoredChatContent {
        &self.content
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSessionSummary {
    id: ChatSessionId,
    created_at: Timestamp,
    updated_at: Timestamp,
    entry_count: ChatSequence,
}

impl ChatSessionSummary {
    #[must_use]
    pub const fn new(
        id: ChatSessionId,
        created_at: Timestamp,
        updated_at: Timestamp,
        entry_count: ChatSequence,
    ) -> Self {
        Self {
            id,
            created_at,
            updated_at,
            entry_count,
        }
    }

    #[must_use]
    pub const fn id(&self) -> &ChatSessionId {
        &self.id
    }

    #[must_use]
    pub const fn created_at(&self) -> Timestamp {
        self.created_at
    }

    #[must_use]
    pub const fn updated_at(&self) -> Timestamp {
        self.updated_at
    }

    #[must_use]
    pub const fn entry_count(&self) -> ChatSequence {
        self.entry_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSessionRecord {
    summary: ChatSessionSummary,
    entries: Vec<StoredChatEntry>,
}

impl ChatSessionRecord {
    #[must_use]
    pub const fn new(summary: ChatSessionSummary, entries: Vec<StoredChatEntry>) -> Self {
        Self { summary, entries }
    }

    #[must_use]
    pub const fn summary(&self) -> &ChatSessionSummary {
        &self.summary
    }

    #[must_use]
    pub fn entries(&self) -> &[StoredChatEntry] {
        &self.entries
    }

    #[must_use]
    pub fn next_sequence(&self) -> Option<ChatSequence> {
        match self.entries.last() {
            Some(last) => last.sequence().next(),
            None => Some(ChatSequence::new(1)),
        }
    }

    #[must_use]
    pub fn to_chat_history(&self) -> ChatHistory {
        let messages = self
            .entries
            .iter()
            .map(|entry| match entry.content() {
                StoredChatContent::User(user) => {
                    ChatMessage::new(ChatRole::User, user.text().clone())
                }
                StoredChatContent::Assistant(assistant) => {
                    ChatMessage::new(ChatRole::Assistant, assistant.text().clone())
                }
            })
            .collect();
        ChatHistory::new(messages)
    }

    #[must_use]
    pub fn find_user_message(&self, id: &UserMessageId) -> Option<&StoredUserMessage> {
        self.entries.iter().find_map(|entry| match entry.content() {
            StoredChatContent::User(user) if user.id() == id => Some(user),
            StoredChatContent::User(_) | StoredChatContent::Assistant(_) => None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatSignalState {
    Active,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatSinkClosed;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundPreemption {
    NoneRunning,
    SignaledPause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatQueueAcquisition {
    Acquired {
        preempted_background: BackgroundPreemption,
    },
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatRunProfile {
    pub kind: RunKind,
    pub purpose: RunPurpose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChatRunTiming {
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub first_token_latency: Option<TimeSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRunModelSpec {
    pub requested_model: ModelSelection,
    pub actual_model: Option<ModelName>,
    pub effort: ReasoningEffort,
    pub usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRunLedgerEntry {
    run_id: AiRunId,
    session_id: ChatSessionId,
    turn_id: ChatTurnId,
    kind: RunKind,
    purpose: RunPurpose,
    requested_model: ModelSelection,
    actual_model: Option<ModelName>,
    effort: ReasoningEffort,
    usage: Option<TokenUsage>,
    started_at: Timestamp,
    finished_at: Timestamp,
    status: ChatTurnStatus,
    first_token_latency: Option<TimeSpan>,
}

impl ChatRunLedgerEntry {
    #[must_use]
    pub fn new(
        ids: ChatTurnIdentity,
        profile: ChatRunProfile,
        model: ChatRunModelSpec,
        timing: ChatRunTiming,
        status: ChatTurnStatus,
    ) -> Self {
        Self {
            run_id: ids.run,
            session_id: ids.session,
            turn_id: ids.turn,
            kind: profile.kind,
            purpose: profile.purpose,
            requested_model: model.requested_model,
            actual_model: model.actual_model,
            effort: model.effort,
            usage: model.usage,
            started_at: timing.started_at,
            finished_at: timing.finished_at,
            status,
            first_token_latency: timing.first_token_latency,
        }
    }

    #[must_use]
    pub const fn run_id(&self) -> &AiRunId {
        &self.run_id
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
    pub const fn kind(&self) -> RunKind {
        self.kind
    }

    #[must_use]
    pub const fn purpose(&self) -> RunPurpose {
        self.purpose
    }

    #[must_use]
    pub const fn requested_model(&self) -> &ModelSelection {
        &self.requested_model
    }

    #[must_use]
    pub const fn actual_model(&self) -> Option<&ModelName> {
        self.actual_model.as_ref()
    }

    #[must_use]
    pub const fn effort(&self) -> ReasoningEffort {
        self.effort
    }

    #[must_use]
    pub const fn usage(&self) -> Option<&TokenUsage> {
        self.usage.as_ref()
    }

    #[must_use]
    pub const fn started_at(&self) -> Timestamp {
        self.started_at
    }

    #[must_use]
    pub const fn finished_at(&self) -> Timestamp {
        self.finished_at
    }

    #[must_use]
    pub const fn status(&self) -> ChatTurnStatus {
        self.status
    }

    #[must_use]
    pub const fn first_token_latency(&self) -> Option<TimeSpan> {
        self.first_token_latency
    }
}
