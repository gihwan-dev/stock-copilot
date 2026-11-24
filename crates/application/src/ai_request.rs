use crate::IdentifierError;
use domain::{AiJobId, Probability, Symbol, UntrustedText};
use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRunId(AiJobId);
impl AiRunId {
    pub fn try_new(value: &str) -> Result<Self, IdentifierError> {
        AiJobId::try_new(value)
            .map(Self)
            .map_err(|_error| IdentifierError::InvalidFormat)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    #[must_use]
    pub const fn job_id(&self) -> &AiJobId {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelName(String);
impl ModelName {
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserMessageId(String);
impl UserMessageId {
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
pub enum PromptError {
    Empty,
    TooLong,
}
#[derive(Clone, PartialEq, Eq)]
pub struct AnalysisPrompt(String);
impl AnalysisPrompt {
    pub fn try_new(value: &str) -> Result<Self, PromptError> {
        if value.is_empty() {
            return Err(PromptError::Empty);
        }
        if value.len() > 65536 {
            return Err(PromptError::TooLong);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for AnalysisPrompt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnalysisPrompt").finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionTopic {
    CompanyNews,
    Earnings,
    Disclosures,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredUserMessage {
    id: UserMessageId,
    text: UntrustedText,
}
impl StoredUserMessage {
    #[must_use]
    pub const fn from_store_result(id: UserMessageId, text: UntrustedText) -> Self {
        Self { id, text }
    }
    #[must_use]
    pub const fn id(&self) -> &UserMessageId {
        &self.id
    }
    #[must_use]
    pub const fn text(&self) -> &UntrustedText {
        &self.text
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionQuery {
    CodeTopics {
        instruments: Vec<Symbol>,
        topics: Vec<CollectionTopic>,
    },
    UserMessage(StoredUserMessage),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunKind {
    Collection,
    Analysis,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunInput {
    Collection(CollectionQuery),
    Analysis(AnalysisPrompt),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSelection {
    Default,
    Highest,
    Fast,
    Named(ModelName),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEffort {
    Default,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunPurpose {
    Chat,
    MorningBriefing,
    IntradayMonitoring,
    ClosingReview,
    UsMarketCheck,
    Estimate,
    DeepResearch,
    CandidateDiscovery,
    WeeklyReview,
    NewsClassification,
    NewsSummary,
    WebCollection,
    ThesisReview,
    ClaimVerification,
    PairedComparison,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSpec {
    id: AiRunId,
    input: RunInput,
    model: ModelSelection,
    effort: ReasoningEffort,
    purpose: RunPurpose,
}
impl RunSpec {
    #[must_use]
    pub const fn new(
        id: AiRunId,
        input: RunInput,
        model: ModelSelection,
        effort: ReasoningEffort,
        purpose: RunPurpose,
    ) -> Self {
        Self {
            id,
            input,
            model,
            effort,
            purpose,
        }
    }
    #[must_use]
    pub const fn id(&self) -> &AiRunId {
        &self.id
    }
    #[must_use]
    pub const fn kind(&self) -> RunKind {
        match &self.input {
            RunInput::Collection(_query) => RunKind::Collection,
            RunInput::Analysis(_prompt) => RunKind::Analysis,
        }
    }
    #[must_use]
    pub const fn input(&self) -> &RunInput {
        &self.input
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
    pub const fn purpose(&self) -> RunPurpose {
        self.purpose
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    role: ChatRole,
    text: UntrustedText,
}
impl ChatMessage {
    #[must_use]
    pub const fn new(role: ChatRole, text: UntrustedText) -> Self {
        Self { role, text }
    }
    #[must_use]
    pub const fn role(&self) -> ChatRole {
        self.role
    }
    #[must_use]
    pub const fn text(&self) -> &UntrustedText {
        &self.text
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatHistory(Vec<ChatMessage>);
impl ChatHistory {
    #[must_use]
    pub const fn new(messages: Vec<ChatMessage>) -> Self {
        Self(messages)
    }
    #[must_use]
    pub fn messages(&self) -> &[ChatMessage] {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utilization(Probability);
impl Utilization {
    #[must_use]
    pub const fn new(value: Probability) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_probability(&self) -> &Probability {
        &self.0
    }
}
