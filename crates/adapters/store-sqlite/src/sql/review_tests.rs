pub(crate) const ALIASED_CHECK: &str = "CREATE TABLE paired (currency TEXT NOT NULL, unitPrice TEXT NOT NULL CHECK (instr(unit_price,char(0))=0 AND unit_price NOT GLOB '*[^0-9.-]*' AND unit_price GLOB '*[0-9]*' AND unit_price NOT GLOB '?*-*' AND unit_price NOT GLOB '*.*.*' AND unit_price NOT GLOB '.*' AND unit_price NOT GLOB '-.*' AND unit_price NOT GLOB '*.'), unit_price TEXT NOT NULL CHECK (instr(unit_price,char(0))=0 AND unit_price NOT GLOB '*[^0-9.-]*' AND unit_price GLOB '*[0-9]*' AND unit_price NOT GLOB '?*-*' AND unit_price NOT GLOB '*.*.*' AND unit_price NOT GLOB '.*' AND unit_price NOT GLOB '-.*' AND unit_price NOT GLOB '*.')) STRICT";

pub(crate) const SNAPSHOT_TABLE: &str =
    "CREATE TABLE ledger_probe (id INTEGER PRIMARY KEY, note TEXT NOT NULL) STRICT";
pub(crate) const SNAPSHOT_INDEX: &str = "CREATE INDEX probe_index ON ledger_probe(note)";
pub(crate) const SNAPSHOT_VIEW: &str =
    "CREATE VIEW probe_view AS SELECT id, note FROM ledger_probe";
pub(crate) const SNAPSHOT_TRIGGER: &str =
    "CREATE TRIGGER probe_trigger AFTER INSERT ON ledger_probe BEGIN SELECT 1; END";
pub(crate) const RESET_LEDGER: &str = "DELETE FROM migration_ledger; PRAGMA user_version = 0";
pub(crate) const SECOND_VALID: &str = "CREATE TABLE partial (id INTEGER PRIMARY KEY) STRICT";
pub(crate) const THIRD_BROKEN: &str =
    "CREATE TABLE partial_third (payload TEXT NOT NULL) STRICT; INSERT INTO absent VALUES (1)";
pub(crate) const RENAME_TABLE: &str = "ALTER TABLE ledger_probe RENAME TO renamed_probe";

pub(crate) const EMPTY_TABLE_NAME: &str = "CREATE TABLE \"\" (note TEXT) STRICT";
pub(crate) const UNICODE_TABLE_NAME: &str = "CREATE TABLE \"빈표\" (note TEXT) STRICT";
pub(crate) const BLOCK_COMMENT: &str = "CREATE TABLE noted (note TEXT /* fixture */) STRICT";
pub(crate) const LINE_COMMENT: &str = "CREATE TABLE noted (note TEXT -- fixture
) STRICT";
