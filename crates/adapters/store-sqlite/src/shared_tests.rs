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
