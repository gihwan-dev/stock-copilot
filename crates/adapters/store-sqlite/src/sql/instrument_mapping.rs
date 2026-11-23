pub(crate) const CREATE: &str = "CREATE TABLE instrument_mappings (\n    instrument_id TEXT PRIMARY KEY CHECK (instr(instrument_id, char(0)) = 0 AND length(instrument_id) BETWEEN 1 AND 64 AND instrument_id NOT GLOB '*[^!-~]*'),\n    market TEXT NOT NULL CHECK (market IN ('KR', 'US')),\n    symbol TEXT NOT NULL CHECK (instr(symbol, char(0)) = 0 AND length(symbol) BETWEEN 1 AND 32 AND symbol GLOB '[A-Za-z0-9]*' AND symbol NOT GLOB '*[^A-Za-z0-9.-]*'),\n    isin TEXT NOT NULL CHECK (instr(isin, char(0)) = 0 AND length(isin) BETWEEN 1 AND 64 AND isin NOT GLOB '*[^!-~]*'),\n    status TEXT NOT NULL CHECK (status IN ('active', 'superseded', 'retired')),\n    UNIQUE (market, symbol, isin)\n) STRICT;\nCREATE UNIQUE INDEX instrument_mappings_symbol_key ON instrument_mappings(market, upper(symbol));\nCREATE UNIQUE INDEX instrument_mappings_isin_key ON instrument_mappings(upper(isin));\nCREATE TRIGGER instrument_mappings_no_delete BEFORE DELETE ON instrument_mappings BEGIN SELECT RAISE(ABORT, 'instrument mappings cannot be deleted'); END;\nCREATE TRIGGER instrument_mappings_retire_only BEFORE UPDATE ON instrument_mappings WHEN OLD.status != 'active' OR NEW.status != 'retired' OR NEW.instrument_id != OLD.instrument_id OR NEW.market != OLD.market OR NEW.symbol != OLD.symbol OR NEW.isin != OLD.isin BEGIN SELECT RAISE(ABORT, 'instrument mapping can only transition from active to retired'); END;\nCREATE TRIGGER instrument_mappings_no_replace BEFORE INSERT ON instrument_mappings WHEN EXISTS (SELECT 1 FROM instrument_mappings WHERE instrument_id = NEW.instrument_id OR (market = NEW.market AND upper(symbol) = upper(NEW.symbol)) OR upper(isin) = upper(NEW.isin)) BEGIN SELECT RAISE(ABORT, 'instrument mapping cannot replace existing history'); END;\nCREATE TRIGGER instrument_mappings_insert_active_only BEFORE INSERT ON instrument_mappings WHEN NEW.status != 'active' BEGIN SELECT RAISE(ABORT, 'instrument mapping must start active'); END;\nCREATE TABLE instrument_id_issuer (\n    slot INTEGER PRIMARY KEY CHECK (slot = 1),\n    last_issued TEXT NOT NULL CHECK (instr(last_issued, char(0)) = 0 AND length(last_issued) = 16 AND last_issued NOT GLOB '*[^0-9a-f]*' AND last_issued != '0000000000000000')\n) STRICT;\nCREATE TRIGGER instrument_id_issuer_no_delete BEFORE DELETE ON instrument_id_issuer BEGIN SELECT RAISE(ABORT, 'instrument id issuer cursor cannot be deleted'); END;\nCREATE TRIGGER instrument_id_issuer_monotonic BEFORE UPDATE ON instrument_id_issuer WHEN NEW.slot != OLD.slot OR NEW.last_issued <= OLD.last_issued BEGIN SELECT RAISE(ABORT, 'instrument id issuer cursor must advance monotonically'); END;\nCREATE TRIGGER instrument_id_issuer_single_insert BEFORE INSERT ON instrument_id_issuer WHEN EXISTS (SELECT 1 FROM instrument_id_issuer) BEGIN SELECT RAISE(ABORT, 'instrument id issuer cursor already exists'); END;\n";

pub(crate) const SCHEMA_OBJECTS: &str = "SELECT sql FROM sqlite_schema WHERE lower(tbl_name) IN ('instrument_mappings', 'instrument_id_issuer') AND sql IS NOT NULL ORDER BY type, name";

pub(crate) const ALL_TRIGGERS: &str = "SELECT sql FROM sqlite_schema WHERE typeof(type) != 'text' OR type NOT IN ('table', 'index', 'view') ORDER BY name";

pub(crate) const TRIGGER_PREFIX: &str = "CREATE TRIGGER ";

pub(crate) const ALL: &str = "SELECT instrument_id, market, symbol, isin, status FROM instrument_mappings ORDER BY instrument_id, market, symbol, isin, status";

pub(crate) const ALL_IDS: &str =
    "SELECT instrument_id FROM instrument_mappings ORDER BY instrument_id";

pub(crate) const ID_EXISTS: &str = "SELECT 1 FROM instrument_mappings WHERE instrument_id = ?1";

pub(crate) const BY_SOURCE: &str = "SELECT instrument_id, market, symbol, isin, status FROM instrument_mappings WHERE (market = ?1 AND upper(symbol) = upper(?2)) OR upper(isin) = upper(?3) ORDER BY instrument_id, market, symbol, isin, status";

pub(crate) const BY_SOURCE_OR_ID: &str = "SELECT instrument_id, market, symbol, isin, status FROM instrument_mappings WHERE (market = ?1 AND upper(symbol) = upper(?2)) OR upper(isin) = upper(?3) OR instrument_id = ?4 ORDER BY instrument_id, market, symbol, isin, status";

pub(crate) const BY_ID: &str = "SELECT instrument_id, market, symbol, isin, status FROM instrument_mappings WHERE instrument_id = ?1 ORDER BY instrument_id, market, symbol, isin, status";

pub(crate) const INSERT: &str = "INSERT INTO instrument_mappings(instrument_id, market, symbol, isin, status) VALUES (?1, ?2, ?3, ?4, ?5)";

pub(crate) const RETIRE: &str = "UPDATE instrument_mappings SET status = 'retired' WHERE instrument_id = ?1 AND market = ?2 AND symbol = ?3 AND isin = ?4 AND status = 'active'";

pub(crate) const ISSUER_CURSOR: &str =
    "SELECT slot, last_issued FROM instrument_id_issuer ORDER BY slot";

pub(crate) const INSERT_ISSUER_CURSOR: &str =
    "INSERT INTO instrument_id_issuer(slot, last_issued) VALUES (1, ?1)";

pub(crate) const UPDATE_ISSUER_CURSOR: &str =
    "UPDATE instrument_id_issuer SET last_issued = ?1 WHERE slot = 1 AND last_issued = ?2";
