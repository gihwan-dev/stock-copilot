use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock, RwLock};

use application::{
    BrokerCall, CallMethod, CallResult, Clock, DiagnosticEvent, DiagnosticsSink, JobDiagnosis,
    JobRunNote, SyncDiagnosis, SyncFailureReason, SyncJob, SyncResult, TokenIssue,
    TransportFailure, sync_failure_reason,
};
use domain::{TimeSpan, Timestamp};

use crate::ipc::briefing::kst_iso;

pub const LOG_FILE: &str = "diagnostics.log";
const ROTATED_FILE: &str = "diagnostics.log.1";
pub const LOG_LIMIT: u64 = 1_048_576;

static LOG: OnceLock<Arc<DiagnosticsLog>> = OnceLock::new();

pub fn install(log: &Arc<DiagnosticsLog>) {
    let _installed = LOG.set(Arc::clone(log));
}

pub fn record(event: &DiagnosticEvent) {
    if let Some(log) = LOG.get() {
        log.record(event);
    }
}

pub struct DiagnosticsLog {
    clock: Arc<dyn Clock>,
    lines: Sender<String>,
    portfolio: RwLock<Option<SyncFailureReason>>,
}

impl std::fmt::Debug for DiagnosticsLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DiagnosticsLog(<redacted>)")
    }
}

impl DiagnosticsLog {
    #[must_use]
    pub fn start(directory: PathBuf, clock: Arc<dyn Clock>) -> Arc<Self> {
        let (lines, received) = channel();
        let _writer = std::thread::Builder::new()
            .name(String::from("diagnostics-log"))
            .spawn(move || write_lines(&directory, &received));
        Arc::new(Self {
            clock,
            lines,
            portfolio: RwLock::new(None),
        })
    }

    #[must_use]
    pub fn portfolio_failure(&self) -> Option<SyncFailureReason> {
        match self.portfolio.read() {
            Ok(guard) => *guard,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    fn remember(&self, diagnosis: &SyncDiagnosis) {
        let reason = match (diagnosis.job, diagnosis.result) {
            (SyncJob::Portfolio, SyncResult::Complete | SyncResult::Partial) => None,
            (SyncJob::Portfolio, SyncResult::Failed) => {
                Some(sync_failure_reason(&diagnosis.errors))
            }
            (SyncJob::Portfolio, SyncResult::AccessBlocked) => {
                Some(SyncFailureReason::AccessDenied)
            }
            (
                SyncJob::Transactions
                | SyncJob::TransactionsFullInspection
                | SyncJob::InstrumentMaster,
                _,
            ) => return,
        };
        match self.portfolio.write() {
            Ok(mut guard) => *guard = reason,
            Err(poisoned) => *poisoned.into_inner() = reason,
        }
    }
}

impl DiagnosticsSink for DiagnosticsLog {
    fn record(&self, event: &DiagnosticEvent) {
        if let DiagnosticEvent::Sync(diagnosis) = event {
            self.remember(diagnosis);
        }
        let line = format_line(self.clock.now(), event);
        let _queued = self.lines.send(line);
    }
}

fn write_lines(directory: &Path, received: &Receiver<String>) {
    while let Ok(line) = received.recv() {
        let _written = append_line(directory, &line, LOG_LIMIT);
    }
}

pub fn append_line(directory: &Path, line: &str, limit: u64) -> std::io::Result<()> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join(LOG_FILE);
    let length = std::fs::metadata(&path).map_or(0, |metadata| metadata.len());
    if length >= limit {
        std::fs::rename(&path, directory.join(ROTATED_FILE))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(format!("{line}\n").as_bytes())
}

#[must_use]
pub fn format_line(at: Timestamp, event: &DiagnosticEvent) -> String {
    let stamp = kst_iso(at).unwrap_or_else(|| String::from("unknown-time"));
    format!("{stamp} {}", describe(event))
}

fn describe(event: &DiagnosticEvent) -> String {
    match event {
        DiagnosticEvent::BrokerCall(call) => describe_call(call),
        DiagnosticEvent::ResponseRejected {
            endpoint,
            mismatch,
            detail,
        } => format!(
            "toss-response endpoint={endpoint:?} mismatch={mismatch:?} detail=\"{}\"",
            detail.as_str()
        ),
        DiagnosticEvent::TokenIssue(issue) => describe_token(issue),
        DiagnosticEvent::Sync(diagnosis) => describe_sync(diagnosis),
        DiagnosticEvent::Job(diagnosis) => describe_job(diagnosis),
        DiagnosticEvent::AiCommandApplied { tool, summary } => format!(
            "ai-command tool={} summary=\"{}\"",
            tool.as_str(),
            summary.as_str()
        ),
    }
}

fn describe_job(diagnosis: &JobDiagnosis) -> String {
    let job = diagnosis.job.as_str();
    match &diagnosis.note {
        JobRunNote::Executed { summary, processed } => format!(
            "job id={job} result=executed summary={} processed={}",
            summary.as_str(),
            processed.as_u32()
        ),
        JobRunNote::SkippedMissedWindow => format!("job id={job} result=skipped-missed-window"),
        JobRunNote::Deferred => format!("job id={job} result=deferred"),
        JobRunNote::Failed(reason) => {
            format!("job id={job} result=failed reason={}", reason.as_str())
        }
    }
}

fn describe_call(call: &BrokerCall) -> String {
    let method = match call.method {
        CallMethod::Get => "GET",
        CallMethod::Post => "POST",
    };
    let mut line = format!("toss-call {method} {}", call.path.as_str());
    let _status = match call.result {
        CallResult::Status(status) => write!(line, " status={}", status.as_u16()),
        CallResult::Transport(failure) => write!(line, " transport={}", transport_name(failure)),
    };
    if let Some(code) = &call.code {
        let _code = write!(line, " code={}", code.as_str());
    }
    if let Some(request) = &call.request {
        let _request = write!(line, " request={}", request.as_str());
    }
    if let Some(duration) = call.duration.and_then(millis) {
        let _duration = write!(line, " ms={duration}");
    }
    line
}

const fn transport_name(failure: TransportFailure) -> &'static str {
    match failure {
        TransportFailure::Timeout => "timeout",
        TransportFailure::Connect => "connect",
        TransportFailure::Body => "body",
        TransportFailure::TooLarge => "too-large",
        TransportFailure::Other => "other",
    }
}

fn millis(span: TimeSpan) -> Option<u64> {
    let (seconds, nanos) = span.as_parts();
    seconds
        .as_u64()
        .checked_mul(1_000)?
        .checked_add(u64::from(nanos.as_u32().checked_div(1_000_000)?))
}

fn describe_token(issue: &TokenIssue) -> String {
    match issue {
        TokenIssue::Issued => String::from("toss-token result=issued"),
        TokenIssue::MissingCredentials => String::from("toss-token result=missing-keys"),
        TokenIssue::Keychain => String::from("toss-token result=keychain-failed"),
        TokenIssue::Rejected(kind) => format!("toss-token result=rejected kind={kind:?}"),
        TokenIssue::InvalidResponse(detail) => format!(
            "toss-token result=invalid-response detail=\"{}\"",
            detail.as_str()
        ),
        TokenIssue::Transport => String::from("toss-token result=transport-failed"),
        TokenIssue::Renewal => String::from("toss-token result=renewal-failed"),
    }
}

fn describe_sync(diagnosis: &SyncDiagnosis) -> String {
    let job = match diagnosis.job {
        SyncJob::Portfolio => "portfolio",
        SyncJob::Transactions => "transactions",
        SyncJob::TransactionsFullInspection => "transactions-full-inspection",
        SyncJob::InstrumentMaster => "instrument-master",
    };
    let result = match diagnosis.result {
        SyncResult::Complete => "complete",
        SyncResult::Partial => "partial",
        SyncResult::Failed => "failed",
        SyncResult::AccessBlocked => "access-blocked",
    };
    let errors: Vec<String> = diagnosis
        .errors
        .iter()
        .map(|error| format!("{:?}:{:?}", error.endpoint(), error.kind()))
        .collect();
    if errors.is_empty() {
        format!("sync job={job} result={result}")
    } else {
        format!(
            "sync job={job} result={result} errors=[{}]",
            errors.join(", ")
        )
    }
}

#[cfg(test)]
mod tests;
