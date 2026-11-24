use super::{SharedClosure, SharedDatabase, SharedDatabaseShutdownHook, SqliteRepositories};
use crate::platform::process_cleanup::{ProcessRuntime, ShutdownHook};
use application::{
    SecretValue, SettingsRepository, SystemFailure, SystemPort, SystemPortError, ThesisRepository,
};

fn clock() -> testkit_ports::SteppedClock {
    testkit_ports::SteppedClock::new(
        broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
            .expect("clock")
            .into_domain(),
    )
}

fn open(directory: &std::path::Path) -> store_sqlite::Database {
    let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
    crate::database::prepare_directory(&path).expect("폴더");
    store_sqlite::Database::open(&path, &SecretValue::new(vec![83; 32]), &clock()).expect("열기")
}

#[test]
fn repositories_report_unavailable_until_the_database_is_installed() {
    let repositories = SqliteRepositories::new(SharedDatabase::new());
    tauri::async_runtime::block_on(async {
        let error = repositories.risk_settings().await.expect_err("미설치");
        assert_eq!(
            error,
            SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::Unavailable(application::RetryDelay::Unknown)
            )
        );
        let instrument = domain::InstrumentId::try_new("005930").expect("종목");
        let _missing = repositories.load(&instrument).await.expect_err("미설치");
    });
}

#[test]
fn installed_database_serves_repositories_and_closes_on_shutdown() {
    let directory = std::path::PathBuf::from("../../../target/native2")
        .join(format!("sqlite-ports-{}", std::process::id()));
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    tauri::async_runtime::block_on(async {
        assert!(shared.install(open(&directory)).await.is_none());
        assert_eq!(repositories.risk_settings().await, Ok(None));
        let hook = SharedDatabaseShutdownHook::new(
            ProcessRuntime::default_in_memory().shutdown_hook(),
            shared.clone(),
        );
        let _summary = hook.on_shutdown().await;
        match hook.closure().await {
            Some(Ok(SharedClosure::Closed(_receipt))) => {}
            other => panic!("명시적 종료 필요: {other:?}"),
        }
        let _closed = repositories.risk_settings().await.expect_err("닫힘");
        assert_eq!(shared.close().await, Ok(SharedClosure::NotOpened));
    });
    std::fs::remove_dir_all(directory).expect("정리");
}
