use application::{InformationError, IsolationFailure, ResponseMismatch};
use domain::UntrustedText;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldName {
    Type,
    Subtype,
    Event,
    EventType,
    Delta,
    DeltaType,
    Text,
    Message,
    Model,
    Content,
    BlockType,
    Id,
    Name,
    Input,
    ToolUseId,
    IsError,
    RateLimitInfo,
    Status,
    ResetsAt,
    Utilization,
    Usage,
    InputTokens,
    OutputTokens,
    CacheReadInputTokens,
    CacheCreationInputTokens,
    ModelUsage,
    ApiError,
    Tools,
    McpServers,
    Plugins,
    Skills,
    SlashCommands,
    PermissionMode,
    ApiKeySource,
    ClaudeCodeVersion,
    Query,
    Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaFailure {
    ExpectedObject,
    MissingField(FieldName),
    InvalidFieldType(FieldName),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationFailure {
    InvalidModelName,
    InvalidTimestamp,
    InvalidUtilization,
    InvalidToolName,
    InvalidTokenCount,
    FieldTooLong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOrderError {
    MissingInitialIsolationCheck,
    EventAfterRejection,
    DuplicateToolCallId,
    UnexpectedToolResult,
    UnclosedToolCall,
    IsolationRegressed(IsolationFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamDecodeError {
    EmptyLine,
    LineTooLong { byte_len: usize },
    MalformedJson { raw: UntrustedText },
    Schema(SchemaFailure),
    Validation(ValidationFailure),
    Order(StreamOrderError),
}

impl StreamDecodeError {
    #[must_use]
    pub const fn to_information_error(&self) -> InformationError {
        match self {
            Self::EmptyLine
            | Self::LineTooLong { .. }
            | Self::MalformedJson { .. }
            | Self::Schema(_) => InformationError::InvalidResponse(ResponseMismatch::Schema),
            Self::Validation(_) | Self::Order(_) => {
                InformationError::InvalidResponse(ResponseMismatch::Validation)
            }
        }
    }
}

impl fmt::Display for StreamDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLine => f.write_str("empty stream line"),
            Self::LineTooLong { byte_len } => {
                write!(f, "stream line exceeds byte limit ({byte_len})")
            }
            Self::MalformedJson { raw } => {
                write!(
                    f,
                    "malformed json line (byte_len={})",
                    raw.as_untrusted_str().len()
                )
            }
            Self::Schema(failure) => match failure {
                SchemaFailure::ExpectedObject => f.write_str("expected json object"),
                SchemaFailure::MissingField(_) => f.write_str("missing required field"),
                SchemaFailure::InvalidFieldType(_) => f.write_str("invalid field type"),
            },
            Self::Validation(failure) => match failure {
                ValidationFailure::InvalidModelName => f.write_str("invalid model name"),
                ValidationFailure::InvalidTimestamp => f.write_str("invalid timestamp"),
                ValidationFailure::InvalidUtilization => f.write_str("invalid utilization"),
                ValidationFailure::InvalidToolName => f.write_str("invalid tool name"),
                ValidationFailure::InvalidTokenCount => f.write_str("invalid token count"),
                ValidationFailure::FieldTooLong => f.write_str("field exceeds byte limit"),
            },
            Self::Order(failure) => match failure {
                StreamOrderError::MissingInitialIsolationCheck => {
                    f.write_str("missing initial isolation check")
                }
                StreamOrderError::EventAfterRejection => {
                    f.write_str("event received after isolation rejection")
                }
                StreamOrderError::DuplicateToolCallId => f.write_str("duplicate open tool call id"),
                StreamOrderError::UnexpectedToolResult => {
                    f.write_str("tool result without matching open tool call")
                }
                StreamOrderError::UnclosedToolCall => f.write_str("unclosed tool call at turn end"),
                StreamOrderError::IsolationRegressed(_) => {
                    f.write_str("isolation regressed on subsequent turn")
                }
            },
        }
    }
}

impl std::error::Error for StreamDecodeError {}
