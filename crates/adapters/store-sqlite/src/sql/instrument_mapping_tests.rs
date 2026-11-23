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
pub(crate) const ISSUER_TABLE_STRICT_FLAG: &str =
    "SELECT strict FROM pragma_table_list WHERE schema = 'main' AND name = 'instrument_id_issuer'";
pub(crate) const RECREATE_UNCONSTRAINED_STRICT: &str = "DROP TABLE instrument_mappings; CREATE TABLE instrument_mappings (instrument_id TEXT NOT NULL, market TEXT NOT NULL, symbol TEXT NOT NULL, isin TEXT NOT NULL, status TEXT NOT NULL) STRICT;";
pub(crate) const RECREATE_UNCONSTRAINED_ISSUER: &str = "DROP TABLE instrument_id_issuer; CREATE TABLE instrument_id_issuer (slot INTEGER NOT NULL, last_issued TEXT NOT NULL) STRICT;";
pub(crate) const IGNORE_CHECKS_ON: &str = "PRAGMA ignore_check_constraints = ON";
pub(crate) const IGNORE_CHECKS_OFF: &str = "PRAGMA ignore_check_constraints = OFF";
pub(crate) const DROP_GUARD_TRIGGERS: &str = "DROP TRIGGER instrument_mappings_retire_only; DROP TRIGGER instrument_mappings_insert_active_only; DROP TRIGGER instrument_mappings_no_replace;";
pub(crate) const RESTORE_GUARD_TRIGGERS: &str = "CREATE TRIGGER instrument_mappings_retire_only BEFORE UPDATE ON instrument_mappings WHEN OLD.status != 'active' OR NEW.status != 'retired' OR NEW.instrument_id != OLD.instrument_id OR NEW.market != OLD.market OR NEW.symbol != OLD.symbol OR NEW.isin != OLD.isin BEGIN SELECT RAISE(ABORT, 'instrument mapping can only transition from active to retired'); END; CREATE TRIGGER instrument_mappings_no_replace BEFORE INSERT ON instrument_mappings WHEN EXISTS (SELECT 1 FROM instrument_mappings WHERE instrument_id = NEW.instrument_id OR (market = NEW.market AND upper(symbol) = upper(NEW.symbol)) OR upper(isin) = upper(NEW.isin)) BEGIN SELECT RAISE(ABORT, 'instrument mapping cannot replace existing history'); END; CREATE TRIGGER instrument_mappings_insert_active_only BEFORE INSERT ON instrument_mappings WHEN NEW.status != 'active' BEGIN SELECT RAISE(ABORT, 'instrument mapping must start active'); END;";
pub(crate) const DROP_ISSUER_TRIGGERS: &str = "DROP TRIGGER instrument_id_issuer_no_delete; DROP TRIGGER instrument_id_issuer_monotonic; DROP TRIGGER instrument_id_issuer_single_insert;";
pub(crate) const RESTORE_ISSUER_TRIGGERS: &str = "CREATE TRIGGER instrument_id_issuer_no_delete BEFORE DELETE ON instrument_id_issuer BEGIN SELECT RAISE(ABORT, 'instrument id issuer cursor cannot be deleted'); END; CREATE TRIGGER instrument_id_issuer_monotonic BEFORE UPDATE ON instrument_id_issuer WHEN NEW.slot != OLD.slot OR NEW.last_issued <= OLD.last_issued BEGIN SELECT RAISE(ABORT, 'instrument id issuer cursor must advance monotonically'); END; CREATE TRIGGER instrument_id_issuer_single_insert BEFORE INSERT ON instrument_id_issuer WHEN EXISTS (SELECT 1 FROM instrument_id_issuer) BEGIN SELECT RAISE(ABORT, 'instrument id issuer cursor already exists'); END;";
pub(crate) const DROP_SYMBOL_INDEX: &str = "DROP INDEX instrument_mappings_symbol_key";
pub(crate) const DROP_ISIN_INDEX: &str = "DROP INDEX instrument_mappings_isin_key";
pub(crate) const DROP_NO_DELETE_TRIGGER: &str = "DROP TRIGGER instrument_mappings_no_delete";
pub(crate) const DROP_RETIRE_ONLY_TRIGGER: &str = "DROP TRIGGER instrument_mappings_retire_only";
pub(crate) const DROP_NO_REPLACE_TRIGGER: &str = "DROP TRIGGER instrument_mappings_no_replace";
pub(crate) const DROP_INSERT_ACTIVE_TRIGGER: &str =
    "DROP TRIGGER instrument_mappings_insert_active_only";
pub(crate) const DROP_ISSUER_NO_DELETE_TRIGGER: &str =
    "DROP TRIGGER instrument_id_issuer_no_delete";
pub(crate) const DROP_ISSUER_MONOTONIC_TRIGGER: &str =
    "DROP TRIGGER instrument_id_issuer_monotonic";
pub(crate) const DROP_ISSUER_SINGLE_INSERT_TRIGGER: &str =
    "DROP TRIGGER instrument_id_issuer_single_insert";
pub(crate) const EXTRA_ROGUE_TRIGGER: &str = "CREATE TRIGGER rogue_mapping_trigger BEFORE DELETE ON instrument_mappings BEGIN SELECT RAISE(IGNORE); END";
pub(crate) const ROGUE_UPPERCASE_TRIGGER: &str = "CREATE TRIGGER rogue_upper_retire AFTER INSERT ON INSTRUMENT_MAPPINGS BEGIN UPDATE instrument_mappings SET status = 'retired' WHERE instrument_id = NEW.instrument_id; END";
pub(crate) const ROGUE_CROSS_TABLE_TRIGGER: &str = "CREATE TRIGGER rogue_cross_retire AFTER INSERT ON thesis_notes BEGIN UPDATE instrument_mappings SET status = 'retired'; END";
pub(crate) const WRITABLE_SCHEMA_MIXED_CASE_TRIGGER: &str = "PRAGMA writable_schema = ON; INSERT INTO sqlite_schema(type, name, tbl_name, rootpage, sql) VALUES ('Trigger', 'sqlite_rogue', 'thesis_notes', 0, 'CREATE TRIGGER sqlite_rogue AFTER INSERT ON thesis_notes BEGIN UPDATE instrument_mappings SET status = ''retired''; END'); PRAGMA writable_schema = OFF;";
pub(crate) const WRITABLE_SCHEMA_NUL_TEXT_TRIGGER: &str = "PRAGMA writable_schema = ON; INSERT INTO sqlite_schema(type, name, tbl_name, rootpage, sql) VALUES ('trigger' || char(0), 'sqlite_txtnul', 'thesis_notes', 0, 'CREATE TRIGGER sqlite_txtnul AFTER INSERT ON thesis_notes BEGIN UPDATE instrument_mappings SET status = ''retired''; END'); PRAGMA writable_schema = OFF;";
pub(crate) const WRITABLE_SCHEMA_NUL_BLOB_TRIGGER: &str = "PRAGMA writable_schema = ON; INSERT INTO sqlite_schema(type, name, tbl_name, rootpage, sql) VALUES (X'7472696767657200', 'sqlite_blobnul', 'thesis_notes', 0, 'CREATE TRIGGER sqlite_blobnul AFTER INSERT ON thesis_notes BEGIN UPDATE instrument_mappings SET status = ''retired''; END'); PRAGMA writable_schema = OFF;";
pub(crate) const REPLACE_WEAK_NO_DELETE_TRIGGER: &str = "DROP TRIGGER instrument_mappings_no_delete; CREATE TRIGGER instrument_mappings_no_delete BEFORE DELETE ON instrument_mappings BEGIN SELECT 1; END;";
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
pub(crate) const UPDATE_ISSUER_VALUE: &str =
    "UPDATE instrument_id_issuer SET last_issued = ?1 WHERE slot = 1";
pub(crate) const INSERT_ISSUER_RAW: &str =
    "INSERT INTO instrument_id_issuer(slot, last_issued) VALUES (?1, ?2)";
pub(crate) const REPLACE_ISSUER_ROW: &str =
    "INSERT OR REPLACE INTO instrument_id_issuer(slot, last_issued) VALUES (1, ?1)";
pub(crate) const DELETE_ISSUER_ROW: &str = "DELETE FROM instrument_id_issuer WHERE slot = 1";
pub(crate) const INSERT_ISSUER_BLOB: &str = "INSERT INTO instrument_id_issuer(slot, last_issued) VALUES (1, X'30303030303030303030303030303031')";
pub(crate) const FAIL_INSERT_ABORT: &str = "CREATE TRIGGER fail_mapping_insert BEFORE INSERT ON instrument_mappings BEGIN SELECT RAISE(ABORT, 'injected insert failure'); END";
pub(crate) const FAIL_RETIRE_ABORT: &str = "CREATE TRIGGER fail_mapping_retire BEFORE UPDATE ON instrument_mappings BEGIN SELECT RAISE(ABORT, 'injected retire failure'); END";
pub(crate) const FAIL_INSERT_IGNORE: &str = "CREATE TRIGGER ignore_mapping_insert BEFORE INSERT ON instrument_mappings BEGIN SELECT RAISE(IGNORE); END";
pub(crate) const FAIL_RETIRE_IGNORE: &str = "CREATE TRIGGER ignore_mapping_retire BEFORE UPDATE ON instrument_mappings BEGIN SELECT RAISE(IGNORE); END";
pub(crate) const FAIL_ISSUER_INSERT_IGNORE: &str = "CREATE TRIGGER ignore_issuer_insert BEFORE INSERT ON instrument_id_issuer BEGIN SELECT RAISE(IGNORE); END";
pub(crate) const FAIL_ISSUER_UPDATE_IGNORE: &str = "CREATE TRIGGER ignore_issuer_update BEFORE UPDATE ON instrument_id_issuer BEGIN SELECT RAISE(IGNORE); END";
pub(crate) const ZERO_BUSY_TIMEOUT: &str = "PRAGMA busy_timeout = 0";
pub(crate) const COUNT_ROWS: &str = "SELECT count(*) FROM instrument_mappings";
pub(crate) const EXPLAIN_BY_SOURCE: &str = "EXPLAIN QUERY PLAN SELECT instrument_id, market, symbol, isin, status FROM instrument_mappings WHERE (market = 'US' AND upper(symbol) = upper('AAPL')) OR upper(isin) = upper('US0378331005') ORDER BY instrument_id, market, symbol, isin, status";
pub(crate) const EXPLAIN_BY_SOURCE_OR_ID: &str = "EXPLAIN QUERY PLAN SELECT instrument_id, market, symbol, isin, status FROM instrument_mappings WHERE (market = 'US' AND upper(symbol) = upper('AAPL')) OR upper(isin) = upper('US0378331005') OR instrument_id = 'inst-0000000000000001' ORDER BY instrument_id, market, symbol, isin, status";
pub(crate) const EXPLAIN_BY_ID: &str = "EXPLAIN QUERY PLAN SELECT instrument_id, market, symbol, isin, status FROM instrument_mappings WHERE instrument_id = 'inst-0000000000000001' ORDER BY instrument_id, market, symbol, isin, status";
pub(crate) const EXPLAIN_ID_EXISTS: &str = "EXPLAIN QUERY PLAN SELECT 1 FROM instrument_mappings WHERE instrument_id = 'inst-0000000000000001'";
pub(crate) const EXPLAIN_NO_REPLACE_SUBQUERY: &str = "EXPLAIN QUERY PLAN SELECT 1 FROM instrument_mappings WHERE instrument_id = 'inst-0000000000000001' OR (market = 'US' AND upper(symbol) = upper('AAPL')) OR upper(isin) = upper('US0378331005')";
