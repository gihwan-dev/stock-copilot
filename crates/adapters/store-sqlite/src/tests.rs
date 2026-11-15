use super::{Database, DatabasePath, sql};
use application::SecretValue;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

pub(crate) struct Fixture(PathBuf);

impl Fixture {
    pub(crate) fn new() -> Self {
        let path = std::env::temp_dir()
            .join("stock-copilot-s01")
            .join(std::process::id().to_string())
            .join(NEXT.fetch_add(1, Ordering::Relaxed).to_string());
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

fn key() -> SecretValue {
    SecretValue::new((1..=32).collect())
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test operation succeeds")
}

#[test]
fn encrypted_reopen_and_pragma_contract() {
    let fixture = Fixture::new();
    let rt = runtime();
    for _ in 0_u8..2 {
        let db = Database::open(&DatabasePath::new(fixture.path()), &key())
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
        let _database = Database::open(&DatabasePath::new(fixture.path()), &key())
            .expect("test operation succeeds");
    }
    let failure = Database::open(
        &DatabasePath::new(fixture.path()),
        &SecretValue::new(vec![99; 32]),
    )
    .expect_err("operation must be rejected");
    assert_eq!(
        failure.failure(),
        application::SystemFailure::RepositoryIntegrity
    );
    assert_eq!(
        Database::open(
            &DatabasePath::new(fixture.path()),
            &SecretValue::new(vec![1; 31])
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
        Database::open(&DatabasePath::new(plain), &key())
            .expect_err("operation must be rejected")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn encrypts_database_and_live_wal_and_rolls_back() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key())
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
    let db = Database::open(&DatabasePath::new(fixture.path()), &key())
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
    let db = Database::open(&DatabasePath::new(fixture.path()), &key()).expect("open");
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
    let db = Database::open(&DatabasePath::new(fixture.path()), &key()).expect("open");
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
        let database = Database::open(&DatabasePath::new(fixture.path()), &key()).expect("open");
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
        0
    );
    for length in [0_usize, 31, 33, 64] {
        assert_eq!(
            Database::open(
                &DatabasePath::new(fixture.path()),
                &SecretValue::new(vec![7; length])
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
        let database = Database::open(&DatabasePath::new(fixture.path()), &secret).expect("open");
        assert_eq!(DebugView(&database).to_string(), "Database(<redacted>)");
    };
    assert_eq!(DebugView(&secret).to_string(), "SecretValue(<redacted>)");
    let failure = Database::open(
        &DatabasePath::new(fixture.path()),
        &SecretValue::new(vec![99; 32]),
    )
    .expect_err("wrong key");
    assert_eq!(
        DebugView(&failure).to_string(),
        "SystemPortError { port: Repository, failure: RepositoryIntegrity }"
    );
}
