pub(crate) const CREATE: &str = "CREATE TABLE thesis_notes (\n    instrument TEXT PRIMARY KEY\n) STRICT;\nCREATE TABLE thesis_history (\n    instrument TEXT NOT NULL REFERENCES thesis_notes(instrument),\n    sequence INTEGER NOT NULL CHECK (sequence > 0),\n    payload TEXT NOT NULL,\n    PRIMARY KEY (instrument, sequence)\n) STRICT;\nCREATE TRIGGER thesis_history_no_update BEFORE UPDATE ON thesis_history BEGIN SELECT RAISE(ABORT, 'thesis history is append only'); END;\nCREATE TRIGGER thesis_history_no_delete BEFORE DELETE ON thesis_history BEGIN SELECT RAISE(ABORT, 'thesis history is append only'); END;\n";
pub(crate) const NOTE: &str = "SELECT instrument FROM thesis_notes WHERE instrument = ?1";
pub(crate) const INSERT_NOTE: &str = "INSERT INTO thesis_notes(instrument) VALUES (?1)";
pub(crate) const HISTORY: &str =
    "SELECT sequence, payload FROM thesis_history WHERE instrument = ?1 ORDER BY sequence";
pub(crate) const INSERT_HISTORY: &str =
    "INSERT INTO thesis_history(instrument, sequence, payload) VALUES (?1, ?2, ?3)";
