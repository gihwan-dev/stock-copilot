pub(crate) const INSERT_EVENT: &str = "INSERT INTO instrument_mapping_events(seq, event_kind, instrument_id, market, prev_symbol, prev_isin, next_symbol, next_isin, disclosure_source, disclosure_receipt_no, action_kind, action_year, action_month, action_day, action_from_symbol, action_to_symbol, action_from_count, action_to_count, note, recorded_seconds, recorded_nanos, recorded_offset) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)";

pub(crate) const REPLACE_EVENT: &str = "INSERT OR REPLACE INTO instrument_mapping_events(seq, event_kind, instrument_id, market, prev_symbol, prev_isin, next_symbol, next_isin, disclosure_source, disclosure_receipt_no, action_kind, action_year, action_month, action_day, action_from_symbol, action_to_symbol, action_from_count, action_to_count, note, recorded_seconds, recorded_nanos, recorded_offset) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)";

pub(crate) const UPDATE_EVENT_NOTE: &str =
    "UPDATE instrument_mapping_events SET note = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_SEQ: &str =
    "UPDATE instrument_mapping_events SET seq = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_KIND: &str =
    "UPDATE instrument_mapping_events SET event_kind = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_NEXT_SYMBOL: &str =
    "UPDATE instrument_mapping_events SET next_symbol = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_NEXT_ISIN: &str =
    "UPDATE instrument_mapping_events SET next_isin = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_NEXT_PAIR: &str =
    "UPDATE instrument_mapping_events SET next_symbol = ?2, next_isin = ?3 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_ACTION_KIND: &str =
    "UPDATE instrument_mapping_events SET action_kind = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_ACTION_YEAR: &str =
    "UPDATE instrument_mapping_events SET action_year = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_ACTION_MONTH: &str =
    "UPDATE instrument_mapping_events SET action_month = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_ACTION_FROM_SYMBOL: &str =
    "UPDATE instrument_mapping_events SET action_from_symbol = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_ACTION_TO_SYMBOL: &str =
    "UPDATE instrument_mapping_events SET action_to_symbol = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_ACTION_FROM_COUNT: &str =
    "UPDATE instrument_mapping_events SET action_from_count = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_ACTION_TO_COUNT: &str =
    "UPDATE instrument_mapping_events SET action_to_count = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_ACTION_DAY: &str =
    "UPDATE instrument_mapping_events SET action_day = ?2 WHERE seq = ?1";

pub(crate) const CLEAR_EVENT_NEXT_SYMBOL: &str =
    "UPDATE instrument_mapping_events SET next_symbol = NULL WHERE seq = ?1";

pub(crate) const CLEAR_EVENT_ACTION_KIND: &str =
    "UPDATE instrument_mapping_events SET action_kind = NULL WHERE seq = ?1";

pub(crate) const CLEAR_EVENT_ACTION_FROM_SYMBOL: &str =
    "UPDATE instrument_mapping_events SET action_from_symbol = NULL WHERE seq = ?1";

pub(crate) const CLEAR_EVENT_ACTION_FROM_COUNT: &str =
    "UPDATE instrument_mapping_events SET action_from_count = NULL WHERE seq = ?1";

pub(crate) const RECREATE_UNCONSTRAINED_EVENTS: &str = "DROP TABLE instrument_mapping_events; CREATE TABLE instrument_mapping_events (seq INTEGER NOT NULL, event_kind TEXT NOT NULL, instrument_id TEXT NOT NULL, market TEXT NOT NULL, prev_symbol TEXT NOT NULL, prev_isin TEXT NOT NULL, next_symbol TEXT, next_isin TEXT, disclosure_source TEXT NOT NULL, disclosure_receipt_no TEXT NOT NULL, action_kind TEXT, action_year INTEGER, action_month INTEGER, action_day INTEGER, action_from_symbol TEXT, action_to_symbol TEXT, action_from_count TEXT, action_to_count TEXT, note TEXT NOT NULL, recorded_seconds INTEGER NOT NULL, recorded_nanos INTEGER NOT NULL, recorded_offset INTEGER NOT NULL) STRICT;";

pub(crate) const UPDATE_EVENT_DISCLOSURE_SOURCE: &str =
    "UPDATE instrument_mapping_events SET disclosure_source = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_RECORDED_NANOS: &str =
    "UPDATE instrument_mapping_events SET recorded_nanos = ?2 WHERE seq = ?1";

pub(crate) const UPDATE_EVENT_RECORDED_OFFSET: &str =
    "UPDATE instrument_mapping_events SET recorded_offset = ?2 WHERE seq = ?1";

pub(crate) const DELETE_EVENT_BY_SEQ: &str = "DELETE FROM instrument_mapping_events WHERE seq = ?1";

pub(crate) const MARK_SUPERSEDED: &str = "UPDATE instrument_mappings SET status = 'superseded', superseded_event_seq = ?5 WHERE instrument_id = ?1 AND market = ?2 AND symbol = ?3 AND isin = ?4 AND status = 'active'";

pub(crate) const RESTORE_RETIRED_TO_ACTIVE: &str = "UPDATE instrument_mappings SET status = 'active' WHERE instrument_id = ?1 AND market = ?2 AND symbol = ?3 AND isin = ?4 AND status = 'retired'";

pub(crate) const DELETE_UNLINKED_ACTIVE: &str = "DELETE FROM instrument_mappings WHERE instrument_id = ?1 AND market = ?2 AND symbol = ?3 AND isin = ?4 AND status = 'active'";

pub(crate) const RESTORE_SUPERSEDED_TO_ACTIVE: &str = "UPDATE instrument_mappings SET status = 'active', superseded_event_seq = NULL WHERE instrument_id = ?1 AND market = ?2 AND symbol = ?3 AND isin = ?4 AND status = 'superseded'";

pub(crate) const UPSERT_COMPOSITE_PK: &str = "INSERT INTO instrument_mappings(instrument_id, market, symbol, isin, status) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(instrument_id, market, symbol, isin) DO UPDATE SET status = excluded.status";

pub(crate) const EVENTS_TABLE_STRICT_FLAG: &str = "SELECT strict FROM pragma_table_list WHERE schema = 'main' AND name = 'instrument_mapping_events'";

pub(crate) const INSERT_EVENT_STRICT_BLOB: &str = "INSERT INTO instrument_mapping_events(seq, event_kind, instrument_id, market, prev_symbol, prev_isin, next_symbol, next_isin, disclosure_source, disclosure_receipt_no, action_kind, action_year, action_month, action_day, action_from_symbol, action_to_symbol, action_from_count, action_to_count, note, recorded_seconds, recorded_nanos, recorded_offset) VALUES (1, 'retirement_corrected', X'6964', 'KR', '005930', 'KR7005930003', NULL, NULL, 'DART', '20261124000001', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, 'note', 1763942400, 0, 32400)";

pub(crate) const DROP_EVENT_TRIGGERS: &str = "DROP TRIGGER instrument_mapping_events_no_update; DROP TRIGGER instrument_mapping_events_no_delete; DROP TRIGGER instrument_mapping_events_monotonic_seq;";

pub(crate) const RESTORE_EVENT_TRIGGERS: &str = "CREATE TRIGGER instrument_mapping_events_no_update BEFORE UPDATE ON instrument_mapping_events BEGIN SELECT RAISE(ABORT, 'instrument mapping events are append only'); END; CREATE TRIGGER instrument_mapping_events_no_delete BEFORE DELETE ON instrument_mapping_events BEGIN SELECT RAISE(ABORT, 'instrument mapping events are append only'); END; CREATE TRIGGER instrument_mapping_events_monotonic_seq BEFORE INSERT ON instrument_mapping_events WHEN NEW.seq != coalesce((SELECT max(seq) FROM instrument_mapping_events), 0) + 1 BEGIN SELECT RAISE(ABORT, 'instrument mapping event sequence must advance monotonically without gaps'); END;";

pub(crate) const DROP_EVENTS_INDEX: &str = "DROP INDEX instrument_mapping_events_instrument_seq";

pub(crate) const DROP_ACTIVE_RETIRED_ID_INDEX: &str =
    "DROP INDEX instrument_mappings_active_retired_id_key";

pub(crate) const DROP_ACTIVE_RETIRED_SYMBOL_INDEX: &str =
    "DROP INDEX instrument_mappings_active_retired_symbol_key";

pub(crate) const DROP_ACTIVE_RETIRED_ISIN_INDEX: &str =
    "DROP INDEX instrument_mappings_active_retired_isin_key";

pub(crate) const DROP_SUPERSEDED_EVENT_INDEX: &str =
    "DROP INDEX instrument_mappings_superseded_event_key";

pub(crate) const FOREIGN_KEYS_OFF: &str = "PRAGMA foreign_keys = OFF";

pub(crate) const FOREIGN_KEYS_ON: &str = "PRAGMA foreign_keys = ON";

pub(crate) const DROP_EVENTS_NO_UPDATE_TRIGGER: &str =
    "DROP TRIGGER instrument_mapping_events_no_update";

pub(crate) const DROP_EVENTS_NO_DELETE_TRIGGER: &str =
    "DROP TRIGGER instrument_mapping_events_no_delete";

pub(crate) const DROP_EVENTS_MONOTONIC_TRIGGER: &str =
    "DROP TRIGGER instrument_mapping_events_monotonic_seq";

pub(crate) const EXPLAIN_EVENTS_FOR_ID: &str = "EXPLAIN QUERY PLAN SELECT seq, event_kind, instrument_id, market, prev_symbol, prev_isin, next_symbol, next_isin, disclosure_source, disclosure_receipt_no, action_kind, action_year, action_month, action_day, action_from_symbol, action_to_symbol, action_from_count, action_to_count, note, recorded_seconds, recorded_nanos, recorded_offset FROM instrument_mapping_events WHERE instrument_id = 'inst-0000000000000001' ORDER BY seq";

pub(crate) const EXPLAIN_MONOTONIC_MAX_SEQ: &str =
    "EXPLAIN QUERY PLAN SELECT max(seq) FROM instrument_mapping_events";

pub(crate) const EXPLAIN_TRIGGER_SYMBOL_CONFLICT: &str = "EXPLAIN QUERY PLAN SELECT 1 FROM instrument_mappings WHERE market = 'US' AND upper(symbol) = upper('AAPL') AND instrument_id != 'inst-0000000000000001'";

pub(crate) const EXPLAIN_TRIGGER_ISIN_CONFLICT: &str = "EXPLAIN QUERY PLAN SELECT 1 FROM instrument_mappings WHERE upper(isin) = upper('US0378331005') AND instrument_id != 'inst-0000000000000001'";

pub(crate) const EXPLAIN_TRIGGER_SAME_ID: &str = "EXPLAIN QUERY PLAN SELECT 1 FROM instrument_mappings WHERE instrument_id = 'inst-0000000000000001' AND status IN ('active', 'retired')";

pub(crate) const UPDATE_SUPERSEDED_SEQ_DIRECT: &str = "UPDATE instrument_mappings SET superseded_event_seq = ?5 WHERE instrument_id = ?1 AND market = ?2 AND symbol = ?3 AND isin = ?4";

pub(crate) const INSERT_WITH_SUPERSEDED_SEQ: &str = "INSERT INTO instrument_mappings(instrument_id, market, symbol, isin, status, superseded_event_seq) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

pub(crate) const SELECT_SUPERSEDED_SEQ: &str = "SELECT superseded_event_seq FROM instrument_mappings WHERE instrument_id = ?1 AND market = ?2 AND symbol = ?3 AND isin = ?4";

pub(crate) const TRACKER_RENAME_SCRIPT: &str = "CREATE TABLE instrument_id_issuer (slot INTEGER PRIMARY KEY) STRICT;\nCREATE INDEX legacy_only_idx ON instrument_id_issuer(slot);\nCREATE TRIGGER legacy_only_trg BEFORE DELETE ON instrument_id_issuer BEGIN SELECT 1; END;\nALTER TABLE instrument_id_issuer RENAME TO instrument_id_issuer_legacy;\n";

pub(crate) const TRACKER_DROP_SCRIPT: &str = "DROP TABLE instrument_id_issuer_legacy;\n";

pub(crate) const COUNT_EVENTS: &str = "SELECT count(*) FROM instrument_mapping_events";

pub(crate) const FAULT_ON_MARK_SUPERSEDED: &str = "CREATE TRIGGER test_fault_trigger BEFORE UPDATE ON instrument_mappings WHEN NEW.status = 'superseded' BEGIN SELECT RAISE(ABORT, 'injected fault on mark superseded'); END;";

pub(crate) const FAULT_ON_INSERT_NEXT_ACTIVE: &str = "CREATE TRIGGER test_fault_trigger BEFORE INSERT ON instrument_mappings WHEN NEW.symbol = 'FAILNEXT' BEGIN SELECT RAISE(ABORT, 'injected fault on insert next'); END;";

pub(crate) const FAULT_ON_RESTORE_RETIRED: &str = "CREATE TRIGGER test_fault_trigger BEFORE UPDATE ON instrument_mappings WHEN OLD.status = 'retired' AND NEW.status = 'active' BEGIN SELECT RAISE(ABORT, 'injected fault on restore retired'); END;";

pub(crate) const FAULT_ON_DELETE_UNLINKED: &str = "CREATE TRIGGER test_fault_trigger BEFORE DELETE ON instrument_mappings BEGIN SELECT RAISE(ABORT, 'injected fault on delete unlinked'); END;";

pub(crate) const FAULT_ON_RESTORE_SUPERSEDED: &str = "CREATE TRIGGER test_fault_trigger BEFORE UPDATE ON instrument_mappings WHEN OLD.status = 'superseded' AND NEW.status = 'active' BEGIN SELECT RAISE(ABORT, 'injected fault on restore superseded'); END;";

pub(crate) const DROP_FAULT_TRIGGER: &str = "DROP TRIGGER test_fault_trigger";

pub(crate) const EXPLAIN_BY_TWO_SOURCES_OR_ID: &str = "EXPLAIN QUERY PLAN SELECT instrument_id, market, symbol, isin, status, superseded_event_seq FROM instrument_mappings WHERE (market = 'US' AND upper(symbol) = upper('FB')) OR upper(isin) = upper('US30303M1027') OR (market = 'US' AND upper(symbol) = upper('META')) OR upper(isin) = upper('US30303M9999') OR instrument_id = 'inst-0000000000000001' ORDER BY instrument_id, market, symbol, isin, status";

pub(crate) const EXPLAIN_MARK_SUPERSEDED: &str = "EXPLAIN QUERY PLAN UPDATE instrument_mappings SET status = 'superseded', superseded_event_seq = 1 WHERE instrument_id = 'inst-0000000000000001' AND market = 'US' AND symbol = 'FB' AND isin = 'US30303M1027' AND status = 'active' AND superseded_event_seq IS NULL";

pub(crate) const EXPLAIN_RESTORE_RETIRED: &str = "EXPLAIN QUERY PLAN UPDATE instrument_mappings SET status = 'active' WHERE instrument_id = 'inst-0000000000000001' AND market = 'KR' AND symbol = '005930' AND isin = 'KR7005930003' AND status = 'retired' AND superseded_event_seq IS NULL";

pub(crate) const EXPLAIN_DELETE_UNLINKED_ACTIVE: &str = "EXPLAIN QUERY PLAN DELETE FROM instrument_mappings WHERE instrument_id = 'inst-0000000000000001' AND market = 'US' AND symbol = 'META' AND isin = 'US30303M1027' AND status = 'active' AND superseded_event_seq IS NULL";

pub(crate) const EXPLAIN_RESTORE_SUPERSEDED: &str = "EXPLAIN QUERY PLAN UPDATE instrument_mappings SET status = 'active', superseded_event_seq = NULL WHERE instrument_id = 'inst-0000000000000001' AND market = 'US' AND symbol = 'FB' AND isin = 'US30303M1027' AND status = 'superseded' AND superseded_event_seq = 1";
