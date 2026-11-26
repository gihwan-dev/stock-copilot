use super::{DiagnosticsLog, LOG_FILE, append_line, format_line};
use application::{
    BrokerCall, CallMethod, CallResult, Clock, DiagnosticEvent, DiagnosticText, DiagnosticsSink,
    HttpStatus, HttpStatusCode, PortError, PortErrorKind, ReadEndpoint, ResponseMismatch,
    RetryDelay, SyncDiagnosis, SyncFailureReason, SyncJob, SyncResult, TokenIssue,
    TransportFailure, UnavailableCause,
};
use domain::{Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds};
use std::sync::Arc;

fn at() -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(1_800_000_000),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("time")
}

struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> Timestamp {
        at()
    }
}

fn call(result: CallResult, code: Option<&str>) -> DiagnosticEvent {
    DiagnosticEvent::BrokerCall(BrokerCall {
        method: CallMethod::Get,
        path: DiagnosticText::new("/api/v1/holdings"),
        result,
        code: code.map(DiagnosticText::new),
        request: code.map(|_code| DiagnosticText::new("req-731")),
        duration: TimeSpan::try_new(SpanSeconds::new(1), Nanosecond::new(234_000_000)).ok(),
    })
}

fn sync(job: SyncJob, result: SyncResult, errors: Vec<PortError>) -> DiagnosticEvent {
    DiagnosticEvent::Sync(SyncDiagnosis {
        job,
        result,
        errors,
    })
}

#[test]
fn every_event_is_one_line_with_a_kst_timestamp() {
    let schema = PortError::new(
        ReadEndpoint::Holdings,
        HttpStatus::Success,
        PortErrorKind::InvalidResponse(ResponseMismatch::Schema),
    );
    let cases = [
        (
            call(CallResult::Status(HttpStatusCode::new(200)), None),
            "2027-01-15T17:00:00+09:00 toss-call GET /api/v1/holdings status=200 ms=1234",
        ),
        (
            call(
                CallResult::Status(HttpStatusCode::new(401)),
                Some("invalid-token"),
            ),
            "2027-01-15T17:00:00+09:00 toss-call GET /api/v1/holdings status=401 code=invalid-token request=req-731 ms=1234",
        ),
        (
            call(CallResult::Transport(TransportFailure::Timeout), None),
            "2027-01-15T17:00:00+09:00 toss-call GET /api/v1/holdings transport=timeout ms=1234",
        ),
        (
            DiagnosticEvent::ResponseRejected {
                endpoint: ReadEndpoint::BuyingPower,
                mismatch: ResponseMismatch::Schema,
                detail: DiagnosticText::new("data: missing field `cashBuyingPower`"),
            },
            "2027-01-15T17:00:00+09:00 toss-response endpoint=BuyingPower mismatch=Schema detail=\"data: missing field `cashBuyingPower`\"",
        ),
        (
            DiagnosticEvent::TokenIssue(TokenIssue::Issued),
            "2027-01-15T17:00:00+09:00 toss-token result=issued",
        ),
        (
            DiagnosticEvent::TokenIssue(TokenIssue::InvalidResponse(DiagnosticText::new(
                "data: unknown variant <value>, expected `Bearer`",
            ))),
            "2027-01-15T17:00:00+09:00 toss-token result=invalid-response detail=\"data: unknown variant <value>, expected `Bearer`\"",
        ),
        (
            sync(SyncJob::Portfolio, SyncResult::Failed, vec![schema]),
            "2027-01-15T17:00:00+09:00 sync job=portfolio result=failed errors=[Holdings:InvalidResponse(Schema)]",
        ),
        (
            sync(SyncJob::InstrumentMaster, SyncResult::Complete, Vec::new()),
            "2027-01-15T17:00:00+09:00 sync job=instrument-master result=complete",
        ),
    ];
    for (event, expected) in cases {
        let line = format_line(at(), &event);
        assert_eq!(line, expected);
        assert!(!line.contains('\n'));
    }
}

#[test]
fn the_last_portfolio_sync_decides_the_failure_reason() {
    let log = DiagnosticsLog::start(
        testkit::canonical_temp_dir()
            .expect("temp")
            .join(format!("sc-diag-reason-{}", std::process::id())),
        Arc::new(Fixed),
    );
    assert_eq!(log.portfolio_failure(), None);
    let network = PortError::new(
        ReadEndpoint::Accounts,
        HttpStatus::NotReceived,
        PortErrorKind::Unavailable {
            cause: UnavailableCause::Network,
            retry: RetryDelay::Unknown,
        },
    );
    log.record(&sync(SyncJob::Portfolio, SyncResult::Failed, vec![network]));
    assert_eq!(log.portfolio_failure(), Some(SyncFailureReason::Network));
    log.record(&sync(
        SyncJob::Transactions,
        SyncResult::Complete,
        Vec::new(),
    ));
    assert_eq!(log.portfolio_failure(), Some(SyncFailureReason::Network));
    log.record(&sync(
        SyncJob::Portfolio,
        SyncResult::AccessBlocked,
        Vec::new(),
    ));
    assert_eq!(
        log.portfolio_failure(),
        Some(SyncFailureReason::AccessDenied)
    );
    log.record(&sync(SyncJob::Portfolio, SyncResult::Partial, Vec::new()));
    assert_eq!(log.portfolio_failure(), None);
}

#[test]
fn the_log_file_rotates_once_it_reaches_the_cap() {
    let directory = testkit::canonical_temp_dir()
        .expect("temp")
        .join(format!("sc-diag-rotate-{}", std::process::id()));
    let _cleared = std::fs::remove_dir_all(&directory);
    for index in 0_u8..5_u8 {
        append_line(&directory, &format!("line-{index}-{}", "x".repeat(20)), 64).expect("append");
    }
    let current = std::fs::read_to_string(directory.join(LOG_FILE)).expect("current");
    let rotated = std::fs::read_to_string(directory.join("diagnostics.log.1")).expect("rotated");
    assert_eq!(current.lines().count(), 2);
    assert!(current.starts_with("line-4-") || current.starts_with("line-3-"));
    assert!(rotated.lines().count() >= 2);
    assert!(u64::try_from(current.len()).expect("length") < 128);
    let _removed = std::fs::remove_dir_all(&directory);
}

#[test]
fn scheduled_job_runs_and_full_inspection_get_their_own_lines() {
    use crate::platform::job_settings::job_diagnosis;
    use scheduler::{JobError, JobExecutionRecord, JobRunDetail, JobRunOutcome, ScheduledJobId};
    let record = |id, outcome| JobExecutionRecord {
        id,
        scheduled_for: at(),
        evaluated_at: at(),
        outcome,
    };
    let cases = [
        (
            record(
                ScheduledJobId::DailyValuationSnapshot,
                JobRunOutcome::Executed(JobRunDetail::new("valuation_recorded", 1)),
            ),
            "2027-01-15T17:00:00+09:00 job id=daily_valuation_snapshot result=executed summary=valuation_recorded processed=1",
        ),
        (
            record(
                ScheduledJobId::MorningBriefing,
                JobRunOutcome::SkippedMissedWindow,
            ),
            "2027-01-15T17:00:00+09:00 job id=morning_briefing result=skipped-missed-window",
        ),
        (
            record(
                ScheduledJobId::EstimateCycle,
                JobRunOutcome::DeferredByAiQueue { resumes_at: None },
            ),
            "2027-01-15T17:00:00+09:00 job id=estimate_cycle result=deferred",
        ),
        (
            record(
                ScheduledJobId::EstimateResolution,
                JobRunOutcome::Failed(JobError::PortUnavailable(String::from("market-calendar"))),
            ),
            "2027-01-15T17:00:00+09:00 job id=estimate_resolution result=failed reason=port-unavailable:market-calendar",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(
            format_line(at(), &DiagnosticEvent::Job(job_diagnosis(&input))),
            expected
        );
    }
    assert_eq!(
        format_line(
            at(),
            &sync(
                SyncJob::TransactionsFullInspection,
                SyncResult::Complete,
                Vec::new()
            )
        ),
        "2027-01-15T17:00:00+09:00 sync job=transactions-full-inspection result=complete"
    );
}
