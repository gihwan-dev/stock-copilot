use crate::{
    Database, DatabasePath, SharedClosure, SharedDatabase, SqliteRepositories,
    tests::{Fixture, clock, key, runtime},
};
use application::{
    SettingsRepository, SystemFailure, SystemPort, SystemPortError, ThesisRepository,
    TossSpecStatusRepository, TransactionSyncRepository,
};

#[test]
fn repositories_report_unavailable_until_the_database_is_installed() {
    let repositories = SqliteRepositories::new(SharedDatabase::new());
    runtime().block_on(async {
        assert_eq!(
            repositories.risk_settings().await,
            Err(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::Unavailable(application::RetryDelay::Unknown)
            ))
        );
        let instrument = domain::InstrumentId::try_new("005930").expect("종목");
        let _missing = repositories.load(&instrument).await.expect_err("미설치");
        let _latest = TossSpecStatusRepository::latest(&repositories)
            .await
            .expect_err("미설치");
        let _state = repositories
            .state(&domain::AccountId::new(1))
            .await
            .expect_err("미설치");
        let _compaction = repositories
            .compact_intraday(&clock())
            .await
            .expect_err("미설치");
        let _vacuum = repositories.reclaim_space().await.expect_err("미설치");
    });
}

#[test]
fn installed_database_serves_repositories_maintenance_and_closes() {
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    runtime().block_on(async {
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        assert!(shared.install(database).await.is_none());
        assert_eq!(repositories.risk_settings().await, Ok(None));
        assert_eq!(
            TossSpecStatusRepository::latest(&repositories).await,
            Ok(None)
        );
        let _compacted = repositories.compact_intraday(&clock()).await.expect("축소");
        let _vacuumed = repositories.reclaim_space().await.expect("정리");
        match shared.close().await {
            Ok(SharedClosure::Closed(_receipt)) => {}
            other => panic!("명시적 종료 필요: {other:?}"),
        }
        let _closed = repositories.risk_settings().await.expect_err("닫힘");
        assert_eq!(shared.close().await, Ok(SharedClosure::NotOpened));
    });
}

fn mcp_tool_call() -> application::ToolCallRecord {
    let at = |seconds: i64| {
        domain::Timestamp::from_parts(
            domain::UnixSeconds::new(seconds),
            domain::Nanosecond::new(0_u32),
            domain::OffsetSeconds::new(0_i32),
        )
        .expect("시각")
    };
    application::ToolCallRecord::new(application::ToolCallRecordInput {
        run_id: application::AiRunId::try_new("chat-run-1").expect("run"),
        request_id: application::ToolRequestId::try_new("1").expect("request"),
        tool_name: application::ToolName::try_new("get_portfolio").expect("tool"),
        arguments_json: application::ToolPayloadJson::try_new("{}").expect("args"),
        result_json: application::ToolPayloadJson::try_new("{}").expect("result"),
        outcome: application::ToolCallOutcome::ToolError,
        started_at: at(1_790_000_000_i64),
        finished_at: at(1_790_000_001_i64),
        protocol_version: application::ToolProtocolVersion::try_new("2025-06-18").expect("version"),
    })
}

#[test]
fn mcp_tool_ledger_and_estimate_reads_follow_the_installed_database() {
    use application::{
        EstimateAssignmentRepository, EstimateSubmissionRepository, ScorecardObservationRepository,
        ToolCallLedger,
    };
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    runtime().block_on(async {
        let _ledger = repositories
            .record_call(&mcp_tool_call())
            .await
            .expect_err("미설치");
        let _assignments = repositories.load_assignments().await.expect_err("미설치");
        let _observations = repositories
            .scorecard_observations()
            .await
            .expect_err("미설치");
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        assert!(shared.install(database).await.is_none());
        assert_eq!(
            repositories.record_call(&mcp_tool_call()).await,
            Ok(application::RecordOutcome::Recorded)
        );
        assert_eq!(
            EstimateAssignmentRepository::load_assignments(&repositories).await,
            EstimateSubmissionRepository::assignments(&repositories).await
        );
        assert_eq!(repositories.scorecard_observations().await, Ok(Vec::new()));
        let instrument = domain::InstrumentId::try_new("005930").expect("종목");
        assert_eq!(
            repositories.submission_records(&instrument).await,
            Ok(Vec::new())
        );
        let _closed = shared.close().await;
    });
}
