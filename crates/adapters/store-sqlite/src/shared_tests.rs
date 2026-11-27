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
        let _history = application::ToolCallHistory::calls_in_run(
            &repositories,
            &application::AiRunId::try_new("chat-run-1").expect("run"),
        )
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
            application::ToolCallHistory::calls_in_run(
                &repositories,
                &application::AiRunId::try_new("chat-run-1").expect("run"),
            )
            .await,
            Ok(vec![mcp_tool_call()])
        );
        assert_eq!(
            application::ToolCallHistory::calls_in_run(
                &repositories,
                &application::AiRunId::try_new("chat-run-2").expect("run"),
            )
            .await,
            Ok(Vec::new())
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

fn moment(seconds: i64) -> domain::Timestamp {
    domain::Timestamp::from_parts(
        domain::UnixSeconds::new(seconds),
        domain::Nanosecond::new(0_u32),
        domain::OffsetSeconds::new(32_400_i32),
    )
    .expect("시각")
}

fn news(path: &str) -> application::CollectedEntry {
    let mut url = String::from("https://www.yna.co.kr/view/");
    url.push_str(path);
    application::CollectedEntry::News(domain::NewsItem::new(
        domain::NewsSource::Yonhap,
        domain::NormalizedUrl::try_new(&url).expect("주소"),
        domain::UntrustedText::try_new("반도체 수출 증가").expect("제목"),
        Some(domain::UntrustedText::try_new("요약").expect("본문")),
    ))
}

fn disclosure() -> application::CollectedEntry {
    application::CollectedEntry::Disclosure(domain::Disclosure::new(
        domain::ReceiptNo::Dart(domain::DartReceiptNo::try_new("20261125000001").expect("접수")),
        domain::NormalizedUrl::try_new(
            "https://dart.fss.or.kr/dsaf001/main.do?rcpNo=20261125000001",
        )
        .expect("주소"),
        domain::UntrustedText::try_new("주요사항보고서").expect("제목"),
        None,
    ))
}

async fn seed_collected(repositories: &SqliteRepositories) {
    use application::{CollectedEntry, CollectedTimes, DisclosureRepository, NewsRepository};
    for entry in [news("AKR1"), news("AKR2"), disclosure()] {
        let _stored = match &entry {
            CollectedEntry::News(item) => NewsRepository::store(repositories, item).await,
            CollectedEntry::Disclosure(item) => repositories.store_disclosure(item).await,
        }
        .expect("저장");
    }
    let _times = repositories
        .record_times(
            &news("AKR1"),
            application::PublicationTime::Known(moment(1_790_000_000_i64)),
            moment(1_790_000_100_i64),
        )
        .await
        .expect("시각");
    let _filed = repositories
        .record_times(
            &disclosure(),
            application::PublicationTime::NotProvided,
            moment(1_790_000_200_i64),
        )
        .await
        .expect("시각");
}

#[test]
fn collected_times_give_each_item_a_handle_and_bound_reads_by_collection_time() {
    use application::{
        CollectedItemHandle as Handle, CollectedItemReads, CollectedKind, CollectedTimes,
        PublicationTime,
    };
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    runtime().block_on(async {
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        assert!(shared.install(database).await.is_none());
        seed_collected(&repositories).await;
        let _missing = repositories
            .record_times(&news("AKR9"), PublicationTime::NotProvided, moment(1_i64))
            .await
            .expect_err("저장되지 않은 항목");
        let recent = repositories
            .collected_since(CollectedKind::News, moment(1_790_000_000_i64))
            .await
            .expect("조회");
        assert_eq!(recent.len(), 1_usize, "시각 없는 항목은 내보내지 않는다");
        let item = recent.first().expect("항목");
        assert_eq!(item.handle, Handle::try_new("n-1").expect("핸들"));
        assert_eq!(item.entry, news("AKR1"));
        assert_eq!(
            item.published_at,
            PublicationTime::Known(moment(1_790_000_000_i64))
        );
        assert_eq!(item.collected_at, moment(1_790_000_100_i64));
        let later = repositories
            .collected_since(CollectedKind::News, moment(1_790_000_101_i64))
            .await
            .expect("조회");
        assert_eq!(later.len(), 0_usize);
        let filings = repositories
            .collected_since(CollectedKind::Disclosure, moment(0_i64))
            .await
            .expect("조회");
        let filing = filings.first().expect("공시");
        assert_eq!(filing.handle, Handle::try_new("d-3").expect("핸들"));
        assert_eq!(filing.published_at, PublicationTime::NotProvided);
        assert_eq!(
            repositories
                .collected_item(&item.handle)
                .await
                .expect("조회")
                .as_ref(),
            Some(item)
        );
        for other in ["d-1", "n-3", "n-01", "n-2", "x-1", "n-"] {
            let found = repositories
                .collected_item(&Handle::try_new(other).expect("핸들"))
                .await
                .expect("조회");
            assert_eq!(found, None, "{other}");
        }
        let _closed = shared.close().await;
    });
}

#[test]
fn information_times_migration_file_matches_the_runtime_statement() {
    assert_eq!(
        std::fs::read_to_string("migrations/0021_information_times.sql").expect("파일"),
        crate::sql::information::MIGRATE_0021
    );
}

#[test]
fn news_settings_and_watchlist_follow_the_installed_database() {
    use application::{NewsDataSettingsRepository, WatchlistRepository};
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    runtime().block_on(async {
        let _settings = NewsDataSettingsRepository::load(&repositories)
            .await
            .expect_err("미설치");
        let _watchlist = WatchlistRepository::load(&repositories)
            .await
            .expect_err("미설치");
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        assert!(shared.install(database).await.is_none());
        let initial = application::NewsDataSettings::initial();
        let _stored = NewsDataSettingsRepository::store(&repositories, &initial)
            .await
            .expect("저장");
        assert_eq!(
            NewsDataSettingsRepository::load(&repositories).await,
            Ok(Some(initial))
        );
        assert_eq!(
            WatchlistRepository::load(&repositories).await,
            Ok(application::Watchlist::empty())
        );
        let _closed = shared.close().await;
    });
}

#[test]
fn production_collected_reads_supply_the_persisted_news_and_disclosure_citation_digests() {
    use application::{CollectedItemReads, CollectedKind};
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    runtime().block_on(async {
        let db =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("저장소");
        assert!(shared.install(db).await.is_none());
        seed_collected(&repositories).await;
        for (kind, entry, expected) in [
            (
                CollectedKind::News,
                news("AKR1"),
                "266729d2adf9391c26d3f415257c3067580518929e5bf942e74688bb8d9b902c",
            ),
            (
                CollectedKind::Disclosure,
                disclosure(),
                "6bd8ba9180c7652d2e1ffa3417dcd6b2ed8b7d5a553cbc309b263bd58a4cf07d",
            ),
        ] {
            let rows = repositories
                .collected_since(kind, moment(0))
                .await
                .expect("수집 항목");
            let row = rows.iter().find(|item| item.entry == entry).expect("대상");
            assert_eq!(
                repositories
                    .citation_digest(&row.entry)
                    .expect("원문 검증값")
                    .as_str(),
                expected
            );
        }
    });
}
