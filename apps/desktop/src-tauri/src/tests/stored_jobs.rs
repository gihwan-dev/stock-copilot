use crate::platform::scheduled_ports::AppStoredJobs;
use crate::platform::scheduling::register_stored_jobs;
use crate::sqlite_ports::{SharedDatabase, SqliteRepositories};
use application::SecretValue;
use scheduler::{
    DailyBackupJob, IntradayCompactionPort, JobError, JobRegistry, ScheduledJob, ScheduledJobId,
    TransactionSyncPort,
};
use tauri::Manager;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use testkit_ports::memory::MemoryReply;

type Stored = AppStoredJobs<MockRuntime, super::support::TestPorts>;

fn app() -> tauri::App<MockRuntime> {
    mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock 앱")
}

fn now() -> domain::Timestamp {
    broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
        .expect("시각")
        .into_domain()
}

fn open(directory: &std::path::Path) -> store_sqlite::Database {
    let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
    crate::database::prepare_directory(&path).expect("폴더");
    store_sqlite::Database::open(
        &path,
        &SecretValue::new(vec![83; 32]),
        &testkit_ports::SteppedClock::new(now()),
    )
    .expect("열기")
}

#[test]
fn stored_jobs_are_registered_for_the_shared_database() {
    let app = app();
    let ports = super::support::ports();
    let mut registry = JobRegistry::new();
    register_stored_jobs::<MockRuntime, super::support::TestPorts>(
        &mut registry,
        app.handle(),
        &SqliteRepositories::new(SharedDatabase::new()),
        &ports,
    )
    .expect("등록");
    let ids: Vec<ScheduledJobId> = registry.list().into_iter().map(|meta| meta.id).collect();
    for expected in [
        ScheduledJobId::IntradayCompaction,
        ScheduledJobId::DailyBackup,
        ScheduledJobId::TransactionsSync,
        ScheduledJobId::TossSpecWatch,
    ] {
        assert!(ids.contains(&expected), "{expected:?}");
    }
}

#[test]
fn stored_jobs_fail_closed_without_database_or_key() {
    let app = app();
    let ports = super::support::ports();
    ports
        .secrets
        .script_load()
        .push(MemoryReply::Return(Ok(None)));
    assert!(app.manage(ports));
    let jobs = Stored::new(app.handle().clone(), SqliteRepositories::default());
    tauri::async_runtime::block_on(async {
        assert_eq!(kind(jobs.compact_before(now()).await), Kind::Storage);
        assert_eq!(kind(jobs.incremental_vacuum().await), Kind::Storage);
        assert_eq!(kind(jobs.sync_all(now()).await), Kind::Ok);
    });
    let backup = DailyBackupJob::new(std::sync::Arc::new(jobs));
    assert_eq!(
        kind(tauri::async_runtime::block_on(backup.run(now()))),
        Kind::Port
    );
}

#[test]
fn intraday_compaction_runs_on_the_installed_database() {
    let directory = std::path::PathBuf::from("../../../target/native2")
        .join(format!("stored-jobs-{}", std::process::id()));
    let app = app();
    assert!(app.manage(super::support::ports()));
    let shared = SharedDatabase::new();
    let jobs = Stored::new(
        app.handle().clone(),
        SqliteRepositories::new(shared.clone()),
    );
    tauri::async_runtime::block_on(async {
        assert!(shared.install(open(&directory)).await.is_none());
        assert_eq!(
            jobs.compact_before(now())
                .await
                .map(|result| result.compacted_days),
            Ok(1)
        );
        assert_eq!(jobs.incremental_vacuum().await, Ok(()));
        let _closed = shared.close().await.expect("닫기");
    });
    std::fs::remove_dir_all(directory).expect("정리");
}

#[derive(Debug, PartialEq, Eq)]
enum Kind {
    Ok,
    Port,
    Storage,
    Calendar,
    Execution,
}

fn kind<T>(result: Result<T, JobError>) -> Kind {
    match result {
        Ok(_value) => Kind::Ok,
        Err(JobError::PortUnavailable(_reason)) => Kind::Port,
        Err(JobError::StorageFailure(_reason)) => Kind::Storage,
        Err(JobError::CalendarUnavailable) => Kind::Calendar,
        Err(JobError::ExecutionFailed(_reason)) => Kind::Execution,
    }
}
