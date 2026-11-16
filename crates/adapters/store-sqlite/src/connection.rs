use crate::{DatabasePath, failure, sql};
use application::{SecretValue, SystemFailure, SystemPort, SystemPortError};
use rusqlite::{Connection, OpenFlags};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use zeroize::Zeroizing;

pub(crate) use crate::failure::integrity;

const RAW_KEY_PREFIX: &str = "x'";
const RAW_KEY_SUFFIX: &str = "'";
const FILE_NOT_FOUND: i32 = 2;

#[derive(Clone, Copy)]
pub(crate) enum Access {
    Read,
    Write,
}

fn verify_number(connection: &Connection, sql: &str, expected: i64) -> Result<(), SystemPortError> {
    let actual: i64 = connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(|error| failure::sqlite(&error))?;
    if actual == expected {
        Ok(())
    } else {
        Err(integrity())
    }
}

fn verify_text(connection: &Connection, sql: &str, expected: &str) -> Result<(), SystemPortError> {
    let actual: String = connection
        .query_row(sql, [], |row| row.get(0))
        .map_err(|error| failure::sqlite(&error))?;
    if actual == expected {
        Ok(())
    } else {
        Err(integrity())
    }
}

fn validate_key(key: &SecretValue) -> Result<(), SystemPortError> {
    if key.as_exposed_secret().len() != 32 {
        return Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::SecretIntegrity,
        ));
    }
    Ok(())
}

fn apply_key(connection: &Connection, key: &SecretValue) -> Result<(), SystemPortError> {
    validate_key(key)?;
    let mut statement = Zeroizing::new(String::with_capacity(67));
    statement.push_str(RAW_KEY_PREFIX);
    for byte in key.as_exposed_secret() {
        for nibble in [byte >> 4_u8, byte & 15] {
            statement.push(char::from_digit(u32::from(nibble), 16).ok_or_else(integrity)?);
        }
    }
    statement.push_str(RAW_KEY_SUFFIX);
    connection
        .key(None::<&str>, statement.as_bytes(), false)
        .map_err(|error| failure::sqlite(&error))
}

pub(crate) fn open(
    path: &DatabasePath,
    key: &SecretValue,
    access: Access,
) -> Result<Connection, SystemPortError> {
    validate_key(key)?;
    prepare_file(path, access)?;
    let flags = match access {
        Access::Read => OpenFlags::SQLITE_OPEN_READ_ONLY,
        Access::Write => OpenFlags::SQLITE_OPEN_READ_WRITE.union(OpenFlags::SQLITE_OPEN_CREATE),
    }
    .union(OpenFlags::SQLITE_OPEN_NO_MUTEX);
    let connection = Connection::open_with_flags(path.as_path(), flags)
        .map_err(|error| failure::sqlite(&error))?;
    verify_access(&connection, access)?;
    connection
        .execute_batch(sql::SET_CIPHER_MEMORY)
        .map_err(|error| failure::sqlite(&error))?;
    verify_text(&connection, sql::CIPHER_MEMORY, "1")?;
    apply_key(&connection, key)?;
    configure(&connection)?;
    let _: i64 = connection
        .query_row(sql::SCHEMA, [], |row| row.get(0))
        .map_err(|error| failure::sqlite(&error))?;
    match access {
        Access::Read => {
            connection
                .execute_batch(sql::SET_QUERY_ONLY)
                .map_err(|error| failure::sqlite(&error))?;
            verify_number(&connection, sql::QUERY_ONLY, 1)?;
        }
        Access::Write => {}
    }
    Ok(connection)
}

fn verify_parent(path: &DatabasePath) -> Result<(), SystemPortError> {
    let parent = path
        .as_path()
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let metadata = std::fs::metadata(parent).map_err(|error| failure::io(&error))?;
    if !metadata.is_dir() {
        return Err(integrity());
    }
    if metadata.permissions().readonly() {
        return Err(failure::permission());
    }
    Ok(())
}

fn prepare_file(path: &DatabasePath, access: Access) -> Result<(), SystemPortError> {
    verify_sidecars(path)?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    match access {
        Access::Write => {
            verify_parent(path)?;
            options.write(true).create(true).truncate(false).mode(0o600);
        }
        Access::Read => {}
    }
    let file = options
        .open(path.as_path())
        .map_err(|error| failure::io(&error))?;
    if file
        .metadata()
        .map_err(|error| failure::io(&error))?
        .is_file()
    {
        Ok(())
    } else {
        Err(integrity())
    }
}

fn verify_sidecars(path: &DatabasePath) -> Result<(), SystemPortError> {
    for suffix in ["-wal", "-shm"] {
        let mut sidecar = path.as_path().as_os_str().to_owned();
        sidecar.push(suffix);
        match std::fs::symlink_metadata(&sidecar) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    return Err(integrity());
                }
            }
            Err(error) => match error.raw_os_error() {
                Some(FILE_NOT_FOUND) => {}
                Some(_) | None => return Err(failure::io(&error)),
            },
        }
    }
    Ok(())
}

fn verify_access(connection: &Connection, access: Access) -> Result<(), SystemPortError> {
    match (
        access,
        connection
            .is_readonly("main")
            .map_err(|error| failure::sqlite(&error))?,
    ) {
        (Access::Read, true) | (Access::Write, false) => Ok(()),
        (Access::Write, true) => Err(failure::permission()),
        (Access::Read, false) => Err(integrity()),
    }
}

fn configure(connection: &Connection) -> Result<(), SystemPortError> {
    verify_text(connection, sql::PROVIDER, "commoncrypto")?;
    for statement in [
        sql::SET_TEMP_STORE,
        sql::SET_FOREIGN_KEYS,
        sql::SET_SYNCHRONOUS,
    ] {
        connection
            .execute_batch(statement)
            .map_err(|error| failure::sqlite(&error))?;
    }
    verify_text(connection, sql::CIPHER_MEMORY, "1")?;
    verify_number(connection, sql::TEMP_STORE, 2)?;
    verify_number(connection, sql::FOREIGN_KEYS, 1)?;
    verify_number(connection, sql::SYNCHRONOUS, 2)?;
    verify_text(connection, sql::CIPHER_HMAC, "1")?;
    verify_text(connection, sql::PAGE_SIZE, "4096")?;
    verify_text(connection, sql::SET_JOURNAL, "wal")?;
    verify_text(connection, sql::JOURNAL, "wal")
}

#[cfg(test)]
mod tests {
    use super::{apply_key, integrity, verify_number, verify_text};
    use crate::sql;
    use application::SecretValue;
    use rusqlite::Connection;

    #[test]
    fn sidecar_paths_must_be_owned_regular_files() {
        let fixture = crate::tests::Fixture::new();
        let path = crate::DatabasePath::new(fixture.path());
        let target = fixture.path().with_file_name("regular-target");
        std::fs::write(&target, b"sidecar target").expect("regular file");
        for filename in ["store.db-wal", "store.db-shm"] {
            let sidecar = fixture.path().with_file_name(filename);
            std::os::unix::fs::symlink(&target, &sidecar).expect("symlink");
            let result = super::verify_sidecars(&path);
            std::fs::remove_file(&sidecar).expect("remove symlink");
            assert_eq!(result, Err(integrity()));
        }
        assert_eq!(super::verify_sidecars(&path), Ok(()));
    }

    #[test]
    fn parent_verification_requires_a_directory_with_write_permission() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = crate::tests::Fixture::new();
        let path = crate::DatabasePath::new(fixture.path());
        assert_eq!(super::verify_parent(&path), Ok(()));
        let relative = crate::DatabasePath::new(std::path::PathBuf::from("s01-parent-check.db"));
        assert_eq!(super::verify_parent(&relative), Ok(()));
        let parent = fixture.path().parent().expect("parent").to_path_buf();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o500)).expect("deny");
        let result = super::verify_parent(&path);
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).expect("restore");
        assert_eq!(result, Err(crate::failure::permission()));
        let component = parent.join("file-parent");
        std::fs::write(&component, b"not a directory").expect("file");
        assert_eq!(
            super::verify_parent(&crate::DatabasePath::new(component.join("store.db"))),
            Err(integrity())
        );
    }

    #[test]
    fn access_verification_rejects_sqlite_readonly_fallback_and_wrong_reader_mode() {
        let fixture = crate::tests::Fixture::new();
        let path = crate::DatabasePath::new(fixture.path());
        let secret = SecretValue::new(vec![71; 32]);
        let writer = super::open(&path, &secret, super::Access::Write).expect("writer");
        let reader = super::open(&path, &secret, super::Access::Read).expect("reader");
        assert_eq!(super::verify_access(&writer, super::Access::Write), Ok(()));
        assert_eq!(super::verify_access(&reader, super::Access::Read), Ok(()));
        assert_eq!(
            super::verify_access(&reader, super::Access::Write),
            Err(crate::failure::permission())
        );
        assert_eq!(
            super::verify_access(&writer, super::Access::Read),
            Err(integrity())
        );
    }

    #[test]
    fn key_material_never_enters_the_sql_parser() {
        let mut connection = Connection::open_in_memory().expect("connection");
        connection
            .authorizer(Some(|_context: rusqlite::hooks::AuthContext<'_>| {
                rusqlite::hooks::Authorization::Deny
            }))
            .expect("authorizer");
        apply_key(&connection, &SecretValue::new(vec![71; 32]))
            .expect("raw key bypasses SQL parser and authorizer");
    }

    #[test]
    fn numeric_setting_verification_rejects_mismatch_and_missing_rows() {
        let connection = Connection::open_in_memory().expect("connection");
        connection
            .execute_batch(sql::SET_TEMP_STORE)
            .expect("setting");
        verify_number(&connection, sql::TEMP_STORE, 2).expect("matching setting");
        assert_eq!(
            verify_number(&connection, sql::TEMP_STORE, 99),
            Err(integrity())
        );
        assert_eq!(
            verify_number(&connection, sql::tests::UNKNOWN, 2),
            Err(integrity())
        );
    }

    #[test]
    fn text_setting_verification_rejects_mismatch_and_missing_rows() {
        let connection = Connection::open_in_memory().expect("connection");
        apply_key(&connection, &SecretValue::new(vec![71; 32])).expect("key");
        verify_text(&connection, sql::PROVIDER, "commoncrypto").expect("matching provider");
        assert_eq!(
            verify_text(&connection, sql::PROVIDER, "openssl"),
            Err(integrity())
        );
        assert_eq!(
            verify_text(&connection, sql::tests::UNKNOWN, "commoncrypto"),
            Err(integrity())
        );
    }
}
