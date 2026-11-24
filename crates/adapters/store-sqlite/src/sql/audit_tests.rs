pub(crate) const COUNT: &str = "SELECT count(*) FROM audit_log";
pub(crate) const UPDATE_PAYLOAD: &str = "UPDATE audit_log SET payload = 'forged' WHERE seq = 2";
pub(crate) const DELETE_MIDDLE: &str = "DELETE FROM audit_log WHERE seq = 2";
pub(crate) const DROP_UPDATE_TRIGGER: &str = "DROP TRIGGER audit_log_no_update";
pub(crate) const DROP_TABLE: &str = "DROP TABLE audit_log";
pub(crate) const ALTER_TABLE: &str = "ALTER TABLE audit_log RENAME TO audit_log_old";
pub(crate) const CREATE_TEMP_TRIGGER: &str =
    "CREATE TEMP TRIGGER audit_log_rogue BEFORE INSERT ON audit_log BEGIN SELECT 1; END";
pub(crate) const DROP_INDEX: &str = "DROP INDEX audit_log_entry";
pub(crate) const WRITABLE_SCHEMA: &str = "PRAGMA writable_schema = ON; UPDATE sqlite_schema SET sql = replace(sql, 'RAISE', 'SELECT') WHERE name = 'audit_log_no_update'; PRAGMA writable_schema = OFF;";
pub(crate) const INSERT_GAP: &str = "INSERT INTO audit_log(seq, at_payload, kind, payload, prev_hash, hash) VALUES (9, 'a', 'k', 'p', '0000000000000000000000000000000000000000000000000000000000000000', '1111111111111111111111111111111111111111111111111111111111111111')";
pub(crate) const DROP_GUARDS: &str =
    "DROP TRIGGER audit_log_no_update; DROP TRIGGER audit_log_no_delete;";
pub(crate) const UPDATE_HASH: &str = "UPDATE audit_log SET hash = '2222222222222222222222222222222222222222222222222222222222222222' WHERE seq = 3";
pub(crate) const ROWS: &str = "SELECT seq, prev_hash, hash FROM audit_log ORDER BY seq";
pub(crate) const INSERT_FIRST_FORGED: &str = "INSERT INTO audit_log(seq, at_payload, kind, payload, prev_hash, hash) VALUES (1, 'a', 'k', 'p', '0000000000000000000000000000000000000000000000000000000000000000', '1111111111111111111111111111111111111111111111111111111111111111')";
pub(crate) const RESTORE_GUARDS: &str = "CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;\nCREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;";
pub(crate) const DELETE_TAIL: &str = "DELETE FROM audit_log WHERE seq = 4";
pub(crate) const DELETE_ALL: &str = "DELETE FROM audit_log";
