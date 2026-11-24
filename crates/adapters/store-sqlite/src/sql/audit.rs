pub(crate) const CREATE: &str = "CREATE TABLE audit_log (\n    seq INTEGER PRIMARY KEY CHECK (seq >= 1),\n    at_payload TEXT NOT NULL,\n    kind TEXT NOT NULL,\n    payload TEXT NOT NULL,\n    prev_hash TEXT NOT NULL CHECK (length(prev_hash) = 64),\n    hash TEXT NOT NULL UNIQUE CHECK (length(hash) = 64)\n) STRICT;\nCREATE UNIQUE INDEX audit_log_entry ON audit_log(at_payload, kind, payload);\nCREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;\nCREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;\nCREATE TRIGGER audit_log_seq BEFORE INSERT ON audit_log WHEN NEW.seq IS NOT (SELECT coalesce(max(seq), 0) + 1 FROM audit_log) BEGIN SELECT RAISE(ABORT, 'audit_log seq must be contiguous'); END;\n";
pub(crate) const LAST: &str = "SELECT seq, hash FROM audit_log ORDER BY seq DESC LIMIT 1";
pub(crate) const FIND_ENTRY: &str =
    "SELECT seq FROM audit_log WHERE at_payload = ?1 AND kind = ?2 AND payload = ?3";
pub(crate) const INSERT: &str = "INSERT INTO audit_log(seq, at_payload, kind, payload, prev_hash, hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
pub(crate) const ALL: &str =
    "SELECT seq, at_payload, kind, payload, prev_hash, hash FROM audit_log ORDER BY seq";
pub(crate) const HASH_AT: &str = "SELECT hash FROM audit_log WHERE seq = ?1";
