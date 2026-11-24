use crate::sql::audit_tests as sql;
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{
    AuditStore, BackupDirectory, BackupOutcome, Database, DatabasePath, create_backup,
    restore_latest_backup,
};
use application::{AuditEntry, AuditIntegrity, AuditKind, AuditLog, RestoreOutcome, SecretValue};
use domain::{CalendarDate, CalendarYear, DayNumber, InstrumentId, MonthNumber};

fn date(day: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(2024),
        MonthNumber::new(8),
        DayNumber::new(day),
    )
    .expect("date")
}

fn entry(id: &str) -> AuditEntry {
    AuditEntry::new(
        timestamp(),
        AuditKind::SignalRaised(InstrumentId::try_new(id).expect("id")),
    )
}

fn directory(fixture: &Fixture) -> BackupDirectory {
    let path = fixture.path().with_file_name("backups");
    std::fs::create_dir_all(&path).expect("backups");
    BackupDirectory::new(path)
}

fn names(directory: &BackupDirectory) -> Vec<String> {
    let mut found = std::fs::read_dir(directory.as_path())
        .expect("read")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("utf8")
        })
        .collect::<Vec<_>>();
    found.sort();
    found
}

fn audit_rows(db: &Database) -> i64 {
    runtime()
        .block_on(
            db.read(|connection| connection.query_row(sql::COUNT, [], |row| row.get::<_, i64>(0))),
        )
        .expect("count")
}

#[test]
fn daily_backup_is_verified_encrypted_and_created_once() {
    let fixture = Fixture::new();
    let backups = directory(&fixture);
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    rt.block_on(AuditStore::new(&db).append(&entry("KR-1")))
        .expect("append");
    assert_eq!(
        rt.block_on(create_backup(&db, &backups, &key(), &clock()))
            .expect("backup"),
        BackupOutcome::Created
    );
    assert_eq!(
        rt.block_on(create_backup(&db, &backups, &key(), &clock()))
            .expect("again"),
        BackupOutcome::AlreadyCreated
    );
    assert_eq!(names(&backups), vec![String::from("2024-08-12.db")]);
    rt.block_on(db.close()).expect("close");
    let copy = DatabasePath::new(backups.as_path().join("2024-08-12.db"));
    assert_ne!(
        Database::open(&copy, &SecretValue::new(vec![9; 32]), &clock())
            .map(|_database| ())
            .err(),
        None
    );
    let reopened = Database::open(&copy, &key(), &clock()).expect("open copy");
    assert_eq!(audit_rows(&reopened), 1);
    assert_eq!(
        rt.block_on(AuditStore::new(&reopened).verify())
            .expect("verify"),
        AuditIntegrity::Verified
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn backups_older_than_seven_days_are_removed() {
    let fixture = Fixture::new();
    let backups = directory(&fixture);
    for name in [
        "2024-08-05.db",
        "2024-08-06.db",
        "notes.txt",
        "2024-08-1.db",
    ] {
        std::fs::write(backups.as_path().join(name), b"old").expect("seed");
    }
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    rt.block_on(create_backup(&db, &backups, &key(), &clock()))
        .expect("backup");
    assert_eq!(
        names(&backups),
        [
            "2024-08-06.db",
            "2024-08-1.db",
            "2024-08-12.db",
            "notes.txt"
        ]
        .map(String::from)
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn restore_uses_the_newest_verified_backup_and_keeps_the_replaced_file() {
    let fixture = Fixture::new();
    let backups = directory(&fixture);
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    rt.block_on(AuditStore::new(&db).append(&entry("KR-1")))
        .expect("append");
    rt.block_on(create_backup(&db, &backups, &key(), &clock()))
        .expect("backup");
    rt.block_on(AuditStore::new(&db).append(&entry("KR-2")))
        .expect("append");
    std::fs::write(backups.as_path().join("2024-08-13.db"), b"forged").expect("forged");
    assert_eq!(audit_rows(&db), 2);
    let (restored, outcome) = rt
        .block_on(restore_latest_backup(db, &path, &backups, &key(), &clock()))
        .expect("restore");
    assert_eq!(outcome, RestoreOutcome::Restored(date(12)));
    assert_eq!(audit_rows(&restored), 1);
    assert!(
        fixture
            .path()
            .with_file_name("store.db.replaced-2024-08-12")
            .exists()
    );
    rt.block_on(restored.close()).expect("close");
}

#[test]
fn restore_without_a_verified_backup_keeps_the_current_database() {
    let fixture = Fixture::new();
    let backups = directory(&fixture);
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    rt.block_on(AuditStore::new(&db).append(&entry("KR-1")))
        .expect("append");
    std::fs::write(backups.as_path().join("2024-08-12.db"), b"forged").expect("forged");
    let (current, outcome) = rt
        .block_on(restore_latest_backup(db, &path, &backups, &key(), &clock()))
        .expect("restore");
    assert_eq!(outcome, RestoreOutcome::NoVerifiedBackup);
    assert_eq!(audit_rows(&current), 1);
    rt.block_on(current.close()).expect("close");
}
