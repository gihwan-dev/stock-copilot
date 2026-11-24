use crate::{Database, DatabasePath, connection, failure, sql};
use application::{AuditIntegrity, RestoreOutcome, SecretValue, SystemPortError};
use domain::{CalendarDate, CalendarYear, DayNumber, MonthNumber};
use rusqlite::Connection;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const KEEP_DAYS: i64 = 7;
const SUFFIX: &str = ".db";
const TEMPORARY: &str = ".db.tmp";
const PAGES_PER_STEP: i32 = 256;

#[derive(Debug, Clone)]
pub struct BackupDirectory(PathBuf);

impl BackupDirectory {
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self(path)
    }

    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }

    fn file(&self, date: CalendarDate, suffix: &str) -> PathBuf {
        self.0
            .join([date_name(date), String::from(suffix)].concat())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupOutcome {
    Created,
    AlreadyCreated,
}

fn date_name(date: CalendarDate) -> String {
    let month = *date.month().as_u8();
    let day = *date.day().as_u8();
    [
        date.year().as_i32().to_string(),
        String::from(if month < 10 { "-0" } else { "-" }),
        month.to_string(),
        String::from(if day < 10 { "-0" } else { "-" }),
        day.to_string(),
    ]
    .concat()
}

fn parse_name(name: &str) -> Option<CalendarDate> {
    let stem = name.strip_suffix(SUFFIX)?;
    let mut parts = stem.split('-');
    let year = parts.next()?.parse::<i32>().ok()?;
    let month = parts.next()?.parse::<u8>().ok()?;
    let day = parts.next()?.parse::<u8>().ok()?;
    if parts.next().is_some() || stem.len() != 10 {
        return None;
    }
    let date = CalendarDate::from_parts(
        CalendarYear::new(year),
        MonthNumber::new(month),
        DayNumber::new(day),
    )
    .ok()?;
    (date_name(date) == stem).then_some(date)
}

fn verified(connection: &Connection) -> rusqlite::Result<AuditIntegrity> {
    let mut cipher = connection.prepare(sql::CIPHER_INTEGRITY)?;
    if cipher.query([])?.next()?.is_some() {
        return Ok(AuditIntegrity::Tampered);
    }
    let quick: String = connection.query_row(sql::QUICK_CHECK, [], |row| row.get(0))?;
    if quick != "ok" {
        return Ok(AuditIntegrity::Tampered);
    }
    crate::audit::verify_chain(connection)
}

fn open_keyed(path: &Path, key: &SecretValue) -> Result<Connection, SystemPortError> {
    let connection = Connection::open(path).map_err(|error| failure::sqlite(&error))?;
    connection::apply_key(&connection, key)?;
    Ok(connection)
}

fn verify_file(path: &Path, key: &SecretValue) -> Result<AuditIntegrity, SystemPortError> {
    let connection = open_keyed(path, key)?;
    let result = verified(&connection).map_err(|error| failure::sqlite(&error));
    connection
        .close()
        .map_err(|(_connection, error)| failure::sqlite(&error))?;
    result
}

fn create_target(path: &Path) -> Result<(), SystemPortError> {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map(|_file| ())
        .map_err(|error| failure::io(&error))
}

fn remove_if_present(path: &Path) -> Result<(), SystemPortError> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(failure::io(&error)),
    }
}

fn copy_into(
    source: &Connection,
    target: &Path,
    key: &SecretValue,
) -> Result<AuditIntegrity, SystemPortError> {
    let mut destination = open_keyed(target, key)?;
    rusqlite::backup::Backup::new(source, &mut destination)
        .and_then(|backup| {
            backup.run_to_completion(PAGES_PER_STEP, std::time::Duration::ZERO, None)
        })
        .map_err(|error| failure::sqlite(&error))?;
    destination
        .query_row(sql::SET_JOURNAL_DELETE, [], |row| row.get::<_, String>(0))
        .map_err(|error| failure::sqlite(&error))?;
    let integrity = verified(&destination).map_err(|error| failure::sqlite(&error));
    destination
        .close()
        .map_err(|(_connection, error)| failure::sqlite(&error))?;
    integrity
}

fn prune(directory: &BackupDirectory, today: CalendarDate) -> Result<(), SystemPortError> {
    let entries = std::fs::read_dir(directory.as_path()).map_err(|error| failure::io(&error))?;
    for listed in entries {
        let entry = listed.map_err(|error| failure::io(&error))?;
        let name = entry.file_name();
        let Some(date) = name.to_str().and_then(parse_name) else {
            continue;
        };
        let age = date
            .days_until(today)
            .map_err(|_error| failure::integrity())?;
        if *age.as_i64() >= KEEP_DAYS {
            remove_if_present(&entry.path())?;
        }
    }
    Ok(())
}

fn backup_today(
    source: &Connection,
    directory: &BackupDirectory,
    key: &SecretValue,
    today: CalendarDate,
) -> Result<BackupOutcome, SystemPortError> {
    let finished = directory.file(today, SUFFIX);
    if finished.exists() {
        prune(directory, today)?;
        return Ok(BackupOutcome::AlreadyCreated);
    }
    let temporary = directory.file(today, TEMPORARY);
    remove_if_present(&temporary)?;
    create_target(&temporary)?;
    match copy_into(source, &temporary, key) {
        Ok(AuditIntegrity::Verified) => {}
        Ok(AuditIntegrity::Tampered) => {
            remove_if_present(&temporary)?;
            return Err(failure::integrity());
        }
        Err(error) => {
            remove_if_present(&temporary)?;
            return Err(error);
        }
    }
    std::fs::rename(&temporary, &finished).map_err(|error| failure::io(&error))?;
    prune(directory, today)?;
    Ok(BackupOutcome::Created)
}

fn newest_verified(
    directory: &BackupDirectory,
    key: &SecretValue,
) -> Result<Option<(CalendarDate, PathBuf)>, SystemPortError> {
    let entries = std::fs::read_dir(directory.as_path()).map_err(|error| failure::io(&error))?;
    let mut candidates = Vec::new();
    for listed in entries {
        let entry = listed.map_err(|error| failure::io(&error))?;
        if let Some(date) = entry.file_name().to_str().and_then(parse_name) {
            candidates.push((date, entry.path()));
        }
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
    for (date, path) in candidates {
        if verify_file(&path, key) == Ok(AuditIntegrity::Verified) {
            return Ok(Some((date, path)));
        }
    }
    Ok(None)
}

fn set_aside(path: &DatabasePath, today: CalendarDate) -> Result<(), SystemPortError> {
    let mut aside_name = path.as_path().as_os_str().to_owned();
    aside_name.push(".replaced-");
    aside_name.push(date_name(today));
    let aside = PathBuf::from(aside_name);
    remove_if_present(&aside)?;
    std::fs::rename(path.as_path(), &aside).map_err(|error| failure::io(&error))?;
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_path().as_os_str().to_owned();
        sidecar.push(suffix);
        remove_if_present(Path::new(&sidecar))?;
    }
    Ok(())
}

fn today_in_korea(clock: &dyn application::Clock) -> Result<CalendarDate, SystemPortError> {
    clock
        .now()
        .date_at_offset(domain::OffsetSeconds::new(32_400))
        .map_err(|_error| failure::integrity())
}

pub async fn create_backup(
    database: &Database,
    directory: &BackupDirectory,
    key: &SecretValue,
    clock: &dyn application::Clock,
) -> Result<BackupOutcome, SystemPortError> {
    let today = today_in_korea(clock)?;
    let owned_directory = directory.clone();
    let owned_key = SecretValue::new(key.as_exposed_secret().to_vec());
    database
        .writer
        .execute_connection(move |connection| {
            backup_today(connection, &owned_directory, &owned_key, today)
        })
        .await
}

pub async fn restore_latest_backup(
    database: Database,
    path: &DatabasePath,
    directory: &BackupDirectory,
    key: &SecretValue,
    clock: &dyn application::Clock,
) -> Result<(Database, RestoreOutcome), SystemPortError> {
    let today = today_in_korea(clock)?;
    database.close().await?;
    let outcome = match newest_verified(directory, key)? {
        Some((date, backup)) => {
            set_aside(path, today)?;
            std::fs::copy(&backup, path.as_path()).map_err(|error| failure::io(&error))?;
            RestoreOutcome::Restored(date)
        }
        None => RestoreOutcome::NoVerifiedBackup,
    };
    Ok((Database::open(path, key, clock)?, outcome))
}
