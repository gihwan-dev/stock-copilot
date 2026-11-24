CREATE TABLE ai_job_records (
    job_id TEXT PRIMARY KEY,
    finished_seconds INTEGER NOT NULL,
    finished_nanos INTEGER NOT NULL CHECK (finished_nanos BETWEEN 0 AND 999999999),
    payload TEXT NOT NULL
) STRICT;
CREATE INDEX ai_job_records_finished ON ai_job_records(finished_seconds, finished_nanos);
CREATE TRIGGER ai_job_records_no_update BEFORE UPDATE ON ai_job_records BEGIN SELECT RAISE(ABORT, 'ai job records are append only'); END;
CREATE TRIGGER ai_job_records_no_delete BEFORE DELETE ON ai_job_records BEGIN SELECT RAISE(ABORT, 'ai job records are append only'); END;
CREATE TABLE investment_principles (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    schema_version INTEGER NOT NULL CHECK (schema_version >= 1),
    payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER investment_principles_no_delete BEFORE DELETE ON investment_principles BEGIN SELECT RAISE(ABORT, 'investment principles are replaced, not deleted'); END;
