CREATE TABLE audit_log (
    seq INTEGER PRIMARY KEY CHECK (seq >= 1),
    at_payload TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload TEXT NOT NULL,
    prev_hash TEXT NOT NULL CHECK (length(prev_hash) = 64),
    hash TEXT NOT NULL UNIQUE CHECK (length(hash) = 64)
) STRICT;
CREATE UNIQUE INDEX audit_log_entry ON audit_log(at_payload, kind, payload);
CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
CREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
CREATE TRIGGER audit_log_seq BEFORE INSERT ON audit_log WHEN NEW.seq IS NOT (SELECT coalesce(max(seq), 0) + 1 FROM audit_log) BEGIN SELECT RAISE(ABORT, 'audit_log seq must be contiguous'); END;
