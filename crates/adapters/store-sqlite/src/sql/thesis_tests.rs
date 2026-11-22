pub(crate) const INSERT_BROKEN: &str =
    "INSERT INTO thesis_history(instrument, sequence, payload) VALUES (?1, 2, 'broken')";
pub(crate) const UPDATE_HISTORY: &str =
    "UPDATE thesis_history SET payload = 'changed' WHERE instrument = ?1";
pub(crate) const DELETE_HISTORY: &str = "DELETE FROM thesis_history WHERE instrument = ?1";
pub(crate) const FAIL_SECOND: &str = "CREATE TRIGGER thesis_fixture_failure BEFORE INSERT ON thesis_history WHEN NEW.sequence = 2 BEGIN SELECT RAISE(ABORT, 'fixture failure'); END";
pub(crate) const INSERT_ORPHAN: &str =
    "INSERT INTO thesis_history(instrument, sequence, payload) VALUES (?1, 1, 'broken')";
pub(crate) const DISABLE_FK_FIXTURE: &str = "PRAGMA foreign_keys = OFF";

pub(crate) const REPLACE_HISTORY: &str = "INSERT OR REPLACE INTO thesis_history(instrument, sequence, payload) VALUES (?1, 1, 'changed')";
pub(crate) const UPSERT_HISTORY: &str = "INSERT INTO thesis_history(instrument, sequence, payload) VALUES (?1, 1, 'changed') ON CONFLICT(instrument, sequence) DO UPDATE SET payload = excluded.payload";
