use crate::{
    CollectedCount, MarketToolName, PortError, PortErrorKind, ReadEndpoint, ResponseMismatch,
    UnavailableCause,
};
use domain::{DataSource, SyncOutcome, TimeSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticText(String);

impl DiagnosticText {
    #[must_use]
    pub fn new(value: &str) -> Self {
        let mut text = String::new();
        for (index, character) in value.chars().enumerate() {
            if index >= 240 {
                text.push('~');
                break;
            }
            text.push(
                if character.is_ascii_alphanumeric() || " _-./{}`:,()<>='".contains(character) {
                    character
                } else {
                    '?'
                },
            );
        }
        Self(text)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpStatusCode(u16);

impl HttpStatusCode {
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u16(&self) -> &u16 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportFailure {
    Timeout,
    Connect,
    Dns,
    Refused,
    Reset,
    Tls,
    Blocked,
    Body,
    TooLarge,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallResult {
    Status(HttpStatusCode),
    Transport(TransportFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerCall {
    pub method: CallMethod,
    pub path: DiagnosticText,
    pub result: CallResult,
    pub code: Option<DiagnosticText>,
    pub request: Option<DiagnosticText>,
    pub duration: Option<TimeSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenIssue {
    Issued,
    MissingCredentials,
    Keychain,
    Rejected(PortErrorKind),
    InvalidResponse(DiagnosticText),
    Transport,
    Renewal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncJob {
    Portfolio,
    Transactions,
    TransactionsFullInspection,
    InstrumentMaster,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncResult {
    Complete,
    Partial,
    Failed,
    AccessBlocked,
}

impl SyncResult {
    #[must_use]
    pub const fn of<Value>(outcome: &SyncOutcome<Value>) -> Self {
        match outcome {
            SyncOutcome::Complete(_) => Self::Complete,
            SyncOutcome::Partial(_) => Self::Partial,
            SyncOutcome::Failed => Self::Failed,
            SyncOutcome::AccessBlocked => Self::AccessBlocked,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncDiagnosis {
    pub job: SyncJob,
    pub result: SyncResult,
    pub errors: Vec<PortError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedFailure {
    Unavailable,
    InvalidResponse,
    InvalidCredential,
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedRunResult {
    Collected,
    NoData,
    NotConfigured,
    Failed(FeedFailure),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedDiagnosis {
    pub source: DataSource,
    pub result: FeedRunResult,
    pub fetched: CollectedCount,
    pub duplicates: CollectedCount,
    pub known: CollectedCount,
    pub stored: CollectedCount,
    pub linked: CollectedCount,
    pub probe: LinkProbe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinkProbe {
    pub name_seen: CollectedCount,
    pub matched: CollectedCount,
    pub needs_context: CollectedCount,
    pub group_only: CollectedCount,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCall {
    pub source: DataSource,
    pub host: DiagnosticText,
    pub result: CallResult,
    pub duration: Option<TimeSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobRunNote {
    Executed {
        summary: DiagnosticText,
        processed: crate::JobItemCount,
    },
    SkippedMissedWindow,
    Deferred,
    Failed(DiagnosticText),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobDiagnosis {
    pub job: DiagnosticText,
    pub note: JobRunNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolReadResult {
    NotConnected,
    Failed(PortError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolReadDiagnosis {
    pub tool: MarketToolName,
    pub result: ToolReadResult,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticEvent {
    BrokerCall(BrokerCall),
    ResponseRejected {
        endpoint: ReadEndpoint,
        mismatch: ResponseMismatch,
        detail: DiagnosticText,
    },
    TokenIssue(TokenIssue),
    Sync(SyncDiagnosis),
    AiCommandApplied {
        tool: DiagnosticText,
        summary: DiagnosticText,
    },
    Job(JobDiagnosis),
    Feed(FeedDiagnosis),
    SourceCall(SourceCall),
    SourceRejected {
        source: DataSource,
        detail: DiagnosticText,
    },
    ToolRead(ToolReadDiagnosis),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SyncFailureReason {
    AccessDenied,
    ResponseMismatch,
    Rejected,
    InstrumentsNotReady,
    ServerBusy,
    Network,
    Unknown,
}

#[must_use]
pub fn sync_failure_reason(errors: &[PortError]) -> SyncFailureReason {
    errors
        .iter()
        .map(|error| failure_reason(error.kind()))
        .min()
        .unwrap_or(SyncFailureReason::Unknown)
}

#[must_use]
pub const fn failure_reason(kind: PortErrorKind) -> SyncFailureReason {
    match kind {
        PortErrorKind::AccessDenied(_) => SyncFailureReason::AccessDenied,
        PortErrorKind::InvalidResponse(_) => SyncFailureReason::ResponseMismatch,
        PortErrorKind::InvalidRequest(_)
        | PortErrorKind::NotFound(_)
        | PortErrorKind::Unsupported(_) => SyncFailureReason::Rejected,
        PortErrorKind::MasterNotReady(_) => SyncFailureReason::InstrumentsNotReady,
        PortErrorKind::Unavailable { cause, .. } => match cause {
            UnavailableCause::Network => SyncFailureReason::Network,
            UnavailableCause::RateLimited
            | UnavailableCause::Maintenance
            | UnavailableCause::ServerError
            | UnavailableCause::ExpiredToken => SyncFailureReason::ServerBusy,
        },
    }
}

pub trait DiagnosticsSink: Send + Sync {
    fn record(&self, event: &DiagnosticEvent);
}

#[cfg(test)]
mod tests;
