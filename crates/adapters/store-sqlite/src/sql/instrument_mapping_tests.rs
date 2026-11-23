pub(crate) const DELETE_BY_ID: &str = "DELETE FROM instrument_mappings WHERE instrument_id = ?1";
pub(crate) const REACTIVATE_BY_ID: &str =
    "UPDATE instrument_mappings SET status = 'active' WHERE instrument_id = ?1";
pub(crate) const RETIRE_AGAIN_BY_ID: &str =
    "UPDATE instrument_mappings SET status = 'retired' WHERE instrument_id = ?1";
pub(crate) const CHANGE_ID: &str = "UPDATE instrument_mappings SET instrument_id = ?2, status = 'retired' WHERE instrument_id = ?1";
pub(crate) const CHANGE_MARKET: &str =
    "UPDATE instrument_mappings SET market = ?2, status = 'retired' WHERE instrument_id = ?1";
pub(crate) const CHANGE_SYMBOL: &str =
    "UPDATE instrument_mappings SET symbol = ?2, status = 'retired' WHERE instrument_id = ?1";
pub(crate) const CHANGE_ISIN: &str =
    "UPDATE instrument_mappings SET isin = ?2, status = 'retired' WHERE instrument_id = ?1";
pub(crate) const REPLACE_ROW: &str = "INSERT OR REPLACE INTO instrument_mappings(instrument_id, market, symbol, isin, status) VALUES (?1, ?2, ?3, ?4, ?5)";
pub(crate) const UPSERT_ROW: &str = "INSERT INTO instrument_mappings(instrument_id, market, symbol, isin, status) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(instrument_id) DO UPDATE SET status = excluded.status";
pub(crate) const INSERT_STRICT_BLOB_ID: &str = "INSERT INTO instrument_mappings(instrument_id, market, symbol, isin, status) VALUES (X'6964', 'KR', '005930', 'KR7005930003', 'active')";
pub(crate) const INSERT_STRICT_BLOB_SYMBOL: &str = "INSERT INTO instrument_mappings(instrument_id, market, symbol, isin, status) VALUES ('opaque-blob', 'KR', X'3030', 'KR7005930003', 'active')";
pub(crate) const TABLE_STRICT_FLAG: &str =
    "SELECT strict FROM pragma_table_list WHERE schema = 'main' AND name = 'instrument_mappings'";
pub(crate) const RECREATE_UNCONSTRAINED_STRICT: &str = "DROP TABLE instrument_mappings; CREATE TABLE instrument_mappings (instrument_id TEXT NOT NULL, market TEXT NOT NULL, symbol TEXT NOT NULL, isin TEXT NOT NULL, status TEXT NOT NULL) STRICT;";
pub(crate) const IGNORE_CHECKS_ON: &str = "PRAGMA ignore_check_constraints = ON";
pub(crate) const IGNORE_CHECKS_OFF: &str = "PRAGMA ignore_check_constraints = OFF";
pub(crate) const DROP_GUARD_TRIGGERS: &str = "DROP TRIGGER instrument_mappings_retire_only; DROP TRIGGER instrument_mappings_insert_active_only; DROP TRIGGER instrument_mappings_no_replace;";
pub(crate) const UPDATE_CORRUPT_ID: &str =
    "UPDATE instrument_mappings SET instrument_id = ?2 WHERE instrument_id = ?1";
pub(crate) const UPDATE_CORRUPT_MARKET: &str =
    "UPDATE instrument_mappings SET market = ?2 WHERE instrument_id = ?1";
pub(crate) const UPDATE_CORRUPT_SYMBOL: &str =
    "UPDATE instrument_mappings SET symbol = ?2 WHERE instrument_id = ?1";
pub(crate) const UPDATE_CORRUPT_ISIN: &str =
    "UPDATE instrument_mappings SET isin = ?2 WHERE instrument_id = ?1";
pub(crate) const UPDATE_CORRUPT_STATUS: &str =
    "UPDATE instrument_mappings SET status = ?2 WHERE instrument_id = ?1";
pub(crate) const FAIL_INSERT_ABORT: &str = "CREATE TRIGGER fail_mapping_insert BEFORE INSERT ON instrument_mappings BEGIN SELECT RAISE(ABORT, 'injected insert failure'); END";
pub(crate) const FAIL_RETIRE_ABORT: &str = "CREATE TRIGGER fail_mapping_retire BEFORE UPDATE ON instrument_mappings BEGIN SELECT RAISE(ABORT, 'injected retire failure'); END";
pub(crate) const FAIL_INSERT_IGNORE: &str = "CREATE TRIGGER ignore_mapping_insert BEFORE INSERT ON instrument_mappings BEGIN SELECT RAISE(IGNORE); END";
pub(crate) const FAIL_RETIRE_IGNORE: &str = "CREATE TRIGGER ignore_mapping_retire BEFORE UPDATE ON instrument_mappings BEGIN SELECT RAISE(IGNORE); END";
pub(crate) const ZERO_BUSY_TIMEOUT: &str = "PRAGMA busy_timeout = 0";
pub(crate) const COUNT_ROWS: &str = "SELECT count(*) FROM instrument_mappings";
