mod open_contract;

use super::{Database, DatabasePath, sql};
use application::SecretValue;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Fixture(PathBuf);

impl Fixture {
    pub(crate) fn new() -> Self {
        let name = [
            String::from("stock-copilot-s01"),
            std::process::id().to_string(),
            NEXT.fetch_add(1, Ordering::Relaxed).to_string(),
        ]
        .join("-");
        let path = std::env::temp_dir().join(name);
        assert!(!path.exists(), "fixture directory already exists");
        std::fs::create_dir_all(&path).expect("test operation succeeds");
        Self(path)
    }
    pub(crate) fn path(&self) -> PathBuf {
        self.0.join("store.db")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("test operation succeeds");
    }
}

pub(crate) fn key() -> SecretValue {
    SecretValue::new((1..=32).collect())
}

pub(crate) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test operation succeeds")
}

#[test]
fn encrypted_reopen_and_pragma_contract() {
    let fixture = Fixture::new();
    let rt = runtime();
    for _ in 0_u8..2 {
        let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock())
            .expect("test operation succeeds");
        rt.block_on(db.check()).expect("test operation succeeds");
        rt.block_on(db.read(|connection| {
            assert_eq!(
                connection.query_row(sql::PROVIDER, [], |row| row.get::<_, String>(0))?,
                "commoncrypto"
            );
            assert_eq!(
                connection.query_row(sql::TEMP_STORE, [], |row| row.get::<_, i64>(0))?,
                2
            );
            assert_eq!(
                connection.query_row(sql::JOURNAL, [], |row| row.get::<_, String>(0))?,
                "wal"
            );
            assert_eq!(
                connection.query_row(sql::FOREIGN_KEYS, [], |row| row.get::<_, i64>(0))?,
                1
            );
            Ok(())
        }))
        .expect("test operation succeeds");
    }
}

#[test]
fn rejects_wrong_key_plaintext_and_invalid_length() {
    let fixture = Fixture::new();
    {
        let _database = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock())
            .expect("test operation succeeds");
    }
    let failure = Database::open(
        &DatabasePath::new(fixture.path()),
        &SecretValue::new(vec![99; 32]),
        &clock(),
    )
    .expect_err("operation must be rejected");
    assert_eq!(
        failure.failure(),
        application::SystemFailure::RepositoryIntegrity
    );
    assert_eq!(
        Database::open(
            &DatabasePath::new(fixture.path()),
            &SecretValue::new(vec![1; 31]),
            &clock()
        )
        .expect_err("operation must be rejected")
        .failure(),
        application::SystemFailure::SecretIntegrity
    );
    let plain = fixture.0.join("plain.db");
    {
        let connection = rusqlite::Connection::open(&plain).expect("test operation succeeds");
        connection
            .execute_batch(sql::tests::CREATE_TEST)
            .expect("test operation succeeds");
    };
    assert_eq!(
        Database::open(&DatabasePath::new(plain), &key(), &clock())
            .expect_err("operation must be rejected")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn encrypts_database_and_live_wal_and_rolls_back() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock())
        .expect("test operation succeeds");
    rt.block_on(db.write(|transaction| {
        transaction.execute_batch(sql::tests::CREATE_TEST)?;
        transaction.execute(sql::tests::INSERT_TEST, ["s01-confidential-marker-72641"])?;
        Ok(())
    }))
    .expect("test operation succeeds");
    for path in [fixture.path(), fixture.0.join("store.db-wal")] {
        let bytes = std::fs::read(path).expect("test operation succeeds");
        assert_ne!(bytes.len(), 0);
        assert!(!bytes.iter().enumerate().any(|(offset, _)| {
            bytes
                .get(offset..)
                .expect("valid offset")
                .starts_with(b"s01-confidential-marker-72641")
        }));
        assert!(!bytes.starts_with(b"SQLite format 3"));
    }
    assert_eq!(
        rt.block_on(db.write(|transaction| {
            transaction.execute(sql::tests::INSERT_TEST, ["rolled-back"])?;
            Err::<(), _>(rusqlite::Error::InvalidQuery)
        }))
        .expect_err("operation must be rejected")
        .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
    let rows = rt
        .block_on(db.read(|connection| {
            connection.query_row(sql::tests::COUNT_TEST, [], |row| row.get::<_, i64>(0))
        }))
        .expect("test operation succeeds");
    assert_eq!(rows, 1);
    assert_eq!(
        rt.block_on(
            db.read(|connection| connection.execute(sql::tests::INSERT_TEST, ["readonly"]))
        )
        .expect_err("operation must be rejected")
        .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn worker_panic_closes_current_queued_and_future_requests() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock())
        .expect("test operation succeeds");
    let failure = rt
        .block_on(db.write::<(), _>(|_| std::panic::resume_unwind(Box::new("test shutdown"))))
        .expect_err("operation must be rejected");
    assert_eq!(
        failure.failure(),
        application::SystemFailure::RepositoryIntegrity
    );
    assert_eq!(
        rt.block_on(db.write(|_| Ok(())))
            .expect_err("operation must be rejected"),
        failure
    );
}

#[test]
fn concurrent_producers_keep_fifo_and_use_one_writer_thread() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::tests::CREATE_TEST)))
        .expect("schema");
    let batches = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for producer in 0_u8..4 {
            let writer = &db.writer;
            handles.push(scope.spawn(move || {
                (0_u8..16)
                    .map(|sequence| submit_sample(writer, producer, sequence))
                    .collect::<Vec<_>>()
            }));
        }
        handles
            .into_iter()
            .map(|handle| handle.join().expect("producer"))
            .collect::<Vec<_>>()
    });
    let mut writer_id = None;
    for batch in batches {
        let mut previous = 0_i64;
        for pending in batch {
            let (row, thread) = rt
                .block_on(pending)
                .expect("response")
                .expect("transaction");
            assert!(row > previous);
            previous = row;
            match writer_id {
                None => writer_id = Some(thread),
                Some(expected) => assert_eq!(thread, expected),
            }
            assert_ne!(thread, std::thread::current().id());
        }
    }
    assert_eq!(
        rt.block_on(db.read(|connection| {
            connection.query_row(sql::tests::COUNT_TEST, [], |row| row.get::<_, i64>(0))
        }))
        .expect("count"),
        64
    );
}

#[test]
fn queued_requests_are_released_when_worker_exits() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let (release, gate) = std::sync::mpsc::channel();
    let first = db
        .writer
        .submit::<(), _>(move |_| {
            gate.recv().expect("gate");
            std::panic::resume_unwind(Box::new("worker exit"))
        })
        .expect("first");
    let second = db.writer.submit(|_| Ok(23_u8)).expect("queued");
    release.send(()).expect("release");
    rt.block_on(first).expect_err("first sender closed");
    rt.block_on(second).expect_err("queued sender closed");
}

fn submit_sample(
    writer: &super::worker::Writer,
    producer: u8,
    sequence: u8,
) -> tokio::sync::oneshot::Receiver<
    Result<(i64, std::thread::ThreadId), application::SystemPortError>,
> {
    writer
        .submit(move |transaction| {
            transaction.execute(
                sql::tests::INSERT_TEST,
                [[producer.to_string(), sequence.to_string()].join(":")],
            )?;
            Ok((transaction.last_insert_rowid(), std::thread::current().id()))
        })
        .expect("submit")
}

#[test]
fn raw_key_is_independently_readable_and_debug_is_closed() {
    let fixture = Fixture::new();
    {
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
        runtime().block_on(database.check()).expect("check");
        runtime()
            .block_on(database.read(|connection| {
                assert!(connection.is_readonly("main")?);
                Ok(())
            }))
            .expect("readonly");
    };
    let connection = rusqlite::Connection::open(fixture.path()).expect("independent open");
    connection
        .execute_batch(sql::tests::RAW_KEY)
        .expect("raw key");
    assert_eq!(
        connection
            .query_row(sql::SCHEMA, [], |row| row.get::<_, i64>(0))
            .expect("schema"),
        13
    );
    for length in [0_usize, 31, 33, 64] {
        assert_eq!(
            Database::open(
                &DatabasePath::new(fixture.path()),
                &SecretValue::new(vec![7; length]),
                &clock()
            )
            .expect_err("invalid length")
            .failure(),
            application::SystemFailure::SecretIntegrity
        );
    }
}

#[derive(Debug)]
struct DebugView<T>(T);

impl<T: std::fmt::Debug> std::fmt::Display for DebugView<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self.0, f)
    }
}

#[test]
fn database_secret_and_failure_debug_remain_fixed() {
    let fixture = Fixture::new();
    let secret = key();
    {
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &secret, &clock()).expect("open");
        assert_eq!(DebugView(&database).to_string(), "Database(<redacted>)");
    };
    assert_eq!(DebugView(&secret).to_string(), "SecretValue(<redacted>)");
    let failure = Database::open(
        &DatabasePath::new(fixture.path()),
        &SecretValue::new(vec![99; 32]),
        &clock(),
    )
    .expect_err("wrong key");
    assert_eq!(
        DebugView(&failure).to_string(),
        "SystemPortError { port: Repository, failure: RepositoryIntegrity }"
    );
}

#[test]
fn permission_failures_require_permission_action() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    let denied = fixture.0.join("denied");
    std::fs::create_dir_all(&denied).expect("directory");
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o0)).expect("deny");
    let result = Database::open(
        &DatabasePath::new(denied.join("store.db")),
        &key(),
        &clock(),
    );
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o700)).expect("restore");
    assert_eq!(
        result.expect_err("permission denied").failure(),
        application::SystemFailure::PermissionDenied
    );
}

#[test]
fn one_read_callback_observes_one_snapshot_across_a_writer_commit() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::tests::CREATE_TEST)))
        .expect("schema");
    let (observed, after_first) = std::sync::mpsc::channel();
    let (committed, after_commit) = std::sync::mpsc::channel();
    let pending = db.read(move |connection| {
        let first = connection.query_row(sql::tests::COUNT_TEST, [], |row| row.get::<_, i64>(0))?;
        observed.send(()).expect("first query");
        after_commit.recv().expect("commit finished");
        let second =
            connection.query_row(sql::tests::COUNT_TEST, [], |row| row.get::<_, i64>(0))?;
        Ok((first, second))
    });
    let counts = std::thread::scope(|scope| {
        let reader = scope.spawn(|| rt.block_on(pending));
        after_first.recv().expect("first query finished");
        rt.block_on(db.write(|transaction| {
            transaction.execute(sql::tests::INSERT_TEST, ["between-queries"])
        }))
        .expect("commit while read snapshot remains open");
        committed.send(()).expect("release second query");
        reader.join().expect("reader").expect("snapshot")
    });
    assert_eq!(counts, (0, 0));
    assert_eq!(
        rt.block_on(db.read(|connection| {
            connection.query_row(sql::tests::COUNT_TEST, [], |row| row.get::<_, i64>(0))
        }))
        .expect("new snapshot"),
        1
    );
}

#[test]
fn writer_connection_applies_and_verifies_the_complete_pragma_contract() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    runtime()
        .block_on(db.write(|transaction| {
            for (statement, expected) in [
                (sql::TEMP_STORE, 2_i64),
                (sql::FOREIGN_KEYS, 1),
                (sql::SYNCHRONOUS, 2),
                (sql::QUERY_ONLY, 0),
            ] {
                let actual: i64 = transaction.query_row(statement, [], |row| row.get(0))?;
                assert_eq!(actual, expected);
            }
            for (statement, expected) in [
                (sql::PROVIDER, "commoncrypto"),
                (sql::JOURNAL, "wal"),
                (sql::CIPHER_MEMORY, "1"),
                (sql::CIPHER_HMAC, "1"),
                (sql::PAGE_SIZE, "4096"),
            ] {
                let actual: String = transaction.query_row(statement, [], |row| row.get(0))?;
                assert_eq!(actual, expected);
            }
            Ok(())
        }))
        .expect("writer contract");
}

fn sqlite_failure_cases() -> [(i32, application::SystemFailure); 15] {
    use application::{RetryDelay, SystemFailure};
    use rusqlite::ffi;
    [
        (ffi::SQLITE_PERM, SystemFailure::PermissionDenied),
        (ffi::SQLITE_READONLY, SystemFailure::PermissionDenied),
        (
            ffi::SQLITE_READONLY_ROLLBACK,
            SystemFailure::PermissionDenied,
        ),
        (ffi::SQLITE_AUTH, SystemFailure::PermissionDenied),
        (
            ffi::SQLITE_BUSY,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ),
        (
            ffi::SQLITE_LOCKED,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ),
        (
            ffi::SQLITE_NOMEM,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ),
        (
            ffi::SQLITE_INTERRUPT,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ),
        (
            ffi::SQLITE_IOERR,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ),
        (
            ffi::SQLITE_FULL,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ),
        (
            ffi::SQLITE_CANTOPEN,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ),
        (
            ffi::SQLITE_PROTOCOL,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        ),
        (ffi::SQLITE_CORRUPT, SystemFailure::RepositoryIntegrity),
        (ffi::SQLITE_NOTADB, SystemFailure::RepositoryIntegrity),
        (ffi::SQLITE_MISUSE, SystemFailure::RepositoryIntegrity),
    ]
}

#[test]
fn sqlite_failures_preserve_permission_availability_and_integrity() {
    use application::{RetryDelay, SystemFailure};
    use rusqlite::ffi;

    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    for (code, expected) in sqlite_failure_cases() {
        let error = move || {
            rusqlite::Error::SqliteFailure(
                ffi::Error::new(code),
                Some(String::from("sensitive failure detail")),
            )
        };
        assert_eq!(
            rt.block_on(db.write(move |_| Err::<(), _>(error())))
                .expect_err("write failure")
                .failure(),
            expected
        );
        assert_eq!(
            rt.block_on(db.read(move |_| Err::<(), _>(error())))
                .expect_err("read failure")
                .failure(),
            if code == ffi::SQLITE_READONLY {
                SystemFailure::RepositoryIntegrity
            } else {
                expected
            }
        );
    }
    assert_eq!(
        Database::open(
            &DatabasePath::new(fixture.0.join("absent/store.db")),
            &key(),
            &clock()
        )
        .expect_err("missing parent")
        .failure(),
        SystemFailure::Unavailable(RetryDelay::Unknown)
    );
}

#[test]
fn cancelling_a_response_does_not_cancel_an_accepted_write() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::tests::CREATE_TEST)))
        .expect("schema");
    {
        let _pending = db
            .writer
            .submit(|transaction| {
                transaction.execute(sql::tests::INSERT_TEST, ["accepted-before-cancellation"])
            })
            .expect("accepted");
    };
    rt.block_on(db.write(|_| Ok(()))).expect("FIFO barrier");
    assert_eq!(
        rt.block_on(db.read(|connection| {
            connection.query_row(sql::tests::COUNT_TEST, [], |row| row.get::<_, i64>(0))
        }))
        .expect("count"),
        1
    );
}

pub(crate) struct TestClock;
impl application::Clock for TestClock {
    fn now(&self) -> domain::Timestamp {
        timestamp()
    }
}
pub(crate) const fn clock() -> TestClock {
    TestClock
}

pub(crate) fn timestamp() -> domain::Timestamp {
    domain::Timestamp::from_parts(
        domain::UnixSeconds::new(1_723_456_789),
        domain::Nanosecond::new(0),
        domain::OffsetSeconds::new(0),
    )
    .expect("timestamp")
}

#[test]
fn explicit_shutdown_drains_accepted_writes_on_a_blocking_worker() {
    let fixture = Fixture::new();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("database");
    let (release, gate) = std::sync::mpsc::channel();
    let (entered, entry) = tokio::sync::oneshot::channel();
    let first = database
        .writer
        .submit(move |_transaction| {
            entered.send(()).expect("entered");
            gate.recv().expect("release");
            Ok(17_i64)
        })
        .expect("accepted first");
    let second = database
        .writer
        .submit(|_transaction| Ok(29_i64))
        .expect("accepted second");
    runtime().block_on(async move {
        entry.await.expect("writer entered");
        let shutdown = tokio::spawn(async move { database.close().await });
        assert_eq!(
            tokio::spawn(async { 43_i64 })
                .await
                .expect("responsive runtime"),
            43
        );
        release.send(()).expect("release writer");
        shutdown.await.expect("shutdown worker").expect("shutdown");
        assert_eq!(first.await.expect("first reply"), Ok(17));
        assert_eq!(second.await.expect("second reply"), Ok(29));
    });
}

#[test]
fn explicit_shutdown_reports_a_failed_writer_instead_of_silent_drop() {
    let fixture = Fixture::new();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("database");
    runtime()
        .block_on(
            database.write::<(), _>(|_transaction| {
                std::panic::resume_unwind(Box::new("shutdown failure"))
            }),
        )
        .expect_err("writer failed");
    assert_eq!(
        runtime().block_on(database.close()),
        Err(application::SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::RepositoryIntegrity
        ))
    );
}

#[test]
fn fixture_owns_and_removes_its_complete_temporary_directory() {
    let directory = {
        let fixture = Fixture::new();
        let directory = fixture.0.clone();
        assert_eq!(directory.parent(), Some(std::env::temp_dir().as_path()));
        assert!(directory.is_dir());
        directory
    };
    assert!(!directory.exists());
}
