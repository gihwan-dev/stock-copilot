use application::{InformationError, IsolationFailure, ResponseMismatch};
use domain::UntrustedText;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldName {
    Method,
    Params,
    Result,
    Error,
    Type,
    Item,
    ItemType,
    Id,
    Server,
    Tool,
    Arguments,
    Status,
    Query,
    Action,
    Delta,
    Text,
    Usage,
    InputTokens,
    CachedInputTokens,
    CacheWriteInputTokens,
    OutputTokens,
    RateLimits,
    OrdinaryUsageAllowed,
    UsedPercent,
    ResetsAt,
    Model,
    CodexErrorInfo,
    Sandbox,
    InstructionSources,
    MultiAgentMode,
    Data,
    Tools,
    Name,
    CliVersion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaFailure {
    ExpectedObject,
    MissingField(FieldName),
    InvalidFieldType(FieldName),
    UnrecognizedEnvelope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationFailure {
    InvalidModelName,
    InvalidTimestamp,
    InvalidUtilization,
    InvalidToolStatus,
    InvalidTokenCount,
    FieldTooLong,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamOrderError {
    MissingInitialIsolationCheck,
    EventAfterRejection,
    DuplicateToolCallId,
    UnexpectedToolCompletion,
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
            Self::EmptyLine => f.write_str("empty codex stream line"),
            Self::LineTooLong { byte_len } => {
                write!(f, "codex stream line exceeds byte limit ({byte_len})")
            }
            Self::MalformedJson { raw } => {
                write!(
                    f,
                    "malformed codex json line (byte_len={})",
                    raw.as_untrusted_str().len()
                )
            }
            Self::Schema(failure) => match failure {
                SchemaFailure::ExpectedObject => f.write_str("expected json object"),
                SchemaFailure::MissingField(_) => f.write_str("missing required field"),
                SchemaFailure::InvalidFieldType(_) => f.write_str("invalid field type"),
                SchemaFailure::UnrecognizedEnvelope => {
                    f.write_str("unrecognized json-rpc envelope")
                }
            },
            Self::Validation(failure) => match failure {
                ValidationFailure::InvalidModelName => f.write_str("invalid model name"),
                ValidationFailure::InvalidTimestamp => f.write_str("invalid timestamp"),
                ValidationFailure::InvalidUtilization => f.write_str("invalid utilization"),
                ValidationFailure::InvalidToolStatus => f.write_str("invalid tool status"),
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
                StreamOrderError::UnexpectedToolCompletion => {
                    f.write_str("tool completion without matching open tool call")
                }
                StreamOrderError::UnclosedToolCall => f.write_str("unclosed tool call at turn end"),
                StreamOrderError::IsolationRegressed(_) => {
                    f.write_str("isolation regressed on subsequent check")
                }
            },
        }
    }
}

impl std::error::Error for StreamDecodeError {}
