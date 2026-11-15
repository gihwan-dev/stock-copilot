use crate::{DatabasePath, failure, sql};
use application::{SecretValue, SystemFailure, SystemPort, SystemPortError};
use rusqlite::{Connection, OpenFlags};
use zeroize::Zeroizing;

pub(crate) use crate::failure::integrity;

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

fn apply_key(connection: &Connection, key: &SecretValue) -> Result<(), SystemPortError> {
    if key.as_exposed_secret().len() != 32 {
        return Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::SecretIntegrity,
        ));
    }
    let mut statement = Zeroizing::new(String::with_capacity(67));
    statement.push_str(sql::KEY_PREFIX);
    for byte in key.as_exposed_secret() {
        for nibble in [byte >> 4_u8, byte & 15] {
            statement.push(char::from_digit(u32::from(nibble), 16).ok_or_else(integrity)?);
        }
    }
    statement.push_str(sql::KEY_SUFFIX);
    connection
        .key(None::<&str>, statement.as_bytes(), false)
        .map_err(|error| failure::sqlite(&error))
}

pub(crate) fn open(
    path: &DatabasePath,
    key: &SecretValue,
    access: Access,
) -> Result<Connection, SystemPortError> {
    let flags = match access {
        Access::Read => OpenFlags::SQLITE_OPEN_READ_ONLY,
        Access::Write => OpenFlags::SQLITE_OPEN_READ_WRITE.union(OpenFlags::SQLITE_OPEN_CREATE),
    }
    .union(OpenFlags::SQLITE_OPEN_NO_MUTEX);
    let connection = Connection::open_with_flags(path.as_path(), flags).map_err(|error| {
        if error.sqlite_error_code() == Some(rusqlite::ErrorCode::CannotOpen) {
            let mut options = std::fs::OpenOptions::new();
            options.read(true);
            match access {
                Access::Write => {
                    options.write(true);
                }
                Access::Read => {}
            }
            match options.open(path.as_path()) {
                Err(probe) => failure::io(&probe),
                Ok(_) => failure::unavailable(),
            }
        } else {
            failure::sqlite(&error)
        }
    })?;
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
