CREATE TABLE estimate_tasks (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL UNIQUE,
    run_id TEXT NOT NULL,
    instrument TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('available', 'consumed')),
    task_payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER estimate_tasks_no_delete BEFORE DELETE ON estimate_tasks BEGIN SELECT RAISE(ABORT, 'estimate tasks are append only'); END;
CREATE TRIGGER estimate_tasks_consume_only BEFORE UPDATE ON estimate_tasks WHEN OLD.status != 'available' OR NEW.status != 'consumed' OR NEW.id != OLD.id OR NEW.task_id != OLD.task_id OR NEW.run_id != OLD.run_id OR NEW.instrument != OLD.instrument OR NEW.task_payload != OLD.task_payload BEGIN SELECT RAISE(ABORT, 'estimate task can only transition from available to consumed'); END;
CREATE TRIGGER estimate_tasks_no_replace BEFORE INSERT ON estimate_tasks WHEN EXISTS (SELECT 1 FROM estimate_tasks WHERE task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'estimate task cannot be replaced'); END;
CREATE TRIGGER estimate_tasks_insert_available_only BEFORE INSERT ON estimate_tasks WHEN NEW.status != 'available' BEGIN SELECT RAISE(ABORT, 'estimate task must start available'); END;
CREATE TABLE estimate_submission_records (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL UNIQUE REFERENCES estimate_tasks(task_id),
    run_id TEXT NOT NULL,
    instrument TEXT NOT NULL,
    record_payload TEXT NOT NULL
) STRICT;
CREATE INDEX estimate_submission_records_instrument ON estimate_submission_records(instrument, id);
CREATE TRIGGER estimate_submission_records_no_update BEFORE UPDATE ON estimate_submission_records BEGIN SELECT RAISE(ABORT, 'estimate submission records are append only'); END;
CREATE TRIGGER estimate_submission_records_no_delete BEFORE DELETE ON estimate_submission_records BEGIN SELECT RAISE(ABORT, 'estimate submission records are append only'); END;
CREATE TRIGGER estimate_submission_records_no_replace BEFORE INSERT ON estimate_submission_records WHEN EXISTS (SELECT 1 FROM estimate_submission_records WHERE task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'estimate submission records are append only'); END;
CREATE TABLE estimate_submissions (
    id INTEGER PRIMARY KEY,
    instrument TEXT NOT NULL,
    submission_hash TEXT NOT NULL UNIQUE,
    submission_payload TEXT NOT NULL
) STRICT;
CREATE INDEX estimate_submissions_instrument ON estimate_submissions(instrument, id);
CREATE TRIGGER estimate_submissions_no_update BEFORE UPDATE ON estimate_submissions BEGIN SELECT RAISE(ABORT, 'estimate submissions are append only'); END;
CREATE TRIGGER estimate_submissions_no_delete BEFORE DELETE ON estimate_submissions BEGIN SELECT RAISE(ABORT, 'estimate submissions are append only'); END;
CREATE TRIGGER estimate_submissions_no_replace BEFORE INSERT ON estimate_submissions WHEN EXISTS (SELECT 1 FROM estimate_submissions WHERE submission_hash = NEW.submission_hash) BEGIN SELECT RAISE(ABORT, 'estimate submissions are append only'); END;
CREATE TABLE estimate_resolutions (
    id INTEGER PRIMARY KEY,
    task_key_hash TEXT NOT NULL UNIQUE,
    task_payload TEXT NOT NULL,
    outcome_payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER estimate_resolutions_no_update BEFORE UPDATE ON estimate_resolutions BEGIN SELECT RAISE(ABORT, 'estimate resolutions are append only'); END;
CREATE TRIGGER estimate_resolutions_no_delete BEFORE DELETE ON estimate_resolutions BEGIN SELECT RAISE(ABORT, 'estimate resolutions are append only'); END;
CREATE TRIGGER estimate_resolutions_no_replace BEFORE INSERT ON estimate_resolutions WHEN EXISTS (SELECT 1 FROM estimate_resolutions WHERE task_key_hash = NEW.task_key_hash) BEGIN SELECT RAISE(ABORT, 'estimate resolutions are append only'); END;
CREATE TABLE estimate_task_resolutions (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL UNIQUE REFERENCES estimate_submission_records(task_id),
    outcome_payload TEXT NOT NULL
) STRICT;
CREATE TRIGGER estimate_task_resolutions_no_update BEFORE UPDATE ON estimate_task_resolutions BEGIN SELECT RAISE(ABORT, 'estimate task resolutions are append only'); END;
CREATE TRIGGER estimate_task_resolutions_no_delete BEFORE DELETE ON estimate_task_resolutions BEGIN SELECT RAISE(ABORT, 'estimate task resolutions are append only'); END;
CREATE TRIGGER estimate_task_resolutions_no_replace BEFORE INSERT ON estimate_task_resolutions WHEN EXISTS (SELECT 1 FROM estimate_task_resolutions WHERE task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'estimate task resolutions are append only'); END;
CREATE TABLE estimate_resolution_unavailable (
    id INTEGER PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES estimate_submission_records(task_id),
    reason_payload TEXT NOT NULL
) STRICT;
CREATE INDEX estimate_resolution_unavailable_task ON estimate_resolution_unavailable(task_id, id);
CREATE TRIGGER estimate_resolution_unavailable_no_update BEFORE UPDATE ON estimate_resolution_unavailable BEGIN SELECT RAISE(ABORT, 'estimate unavailable records are append only'); END;
CREATE TRIGGER estimate_resolution_unavailable_no_delete BEFORE DELETE ON estimate_resolution_unavailable BEGIN SELECT RAISE(ABORT, 'estimate unavailable records are append only'); END;
CREATE TRIGGER estimate_resolution_unavailable_after_resolved BEFORE INSERT ON estimate_resolution_unavailable WHEN EXISTS (SELECT 1 FROM estimate_task_resolutions WHERE task_id = NEW.task_id) BEGIN SELECT RAISE(ABORT, 'resolved estimate cannot record unavailable'); END;
CREATE TABLE estimate_calibrations (
    id INTEGER PRIMARY KEY,
    key_hash TEXT NOT NULL,
    position INTEGER NOT NULL CHECK (position >= 0),
    key_payload TEXT NOT NULL,
    table_hash TEXT NOT NULL,
    table_payload TEXT NOT NULL,
    UNIQUE (key_hash, position)
) STRICT;
CREATE INDEX estimate_calibrations_lookup ON estimate_calibrations(key_hash, position DESC);
CREATE TRIGGER estimate_calibrations_no_update BEFORE UPDATE ON estimate_calibrations BEGIN SELECT RAISE(ABORT, 'estimate calibrations are append only'); END;
CREATE TRIGGER estimate_calibrations_no_delete BEFORE DELETE ON estimate_calibrations BEGIN SELECT RAISE(ABORT, 'estimate calibrations are append only'); END;
CREATE TRIGGER estimate_calibrations_no_replace BEFORE INSERT ON estimate_calibrations WHEN EXISTS (SELECT 1 FROM estimate_calibrations WHERE key_hash = NEW.key_hash AND position = NEW.position) BEGIN SELECT RAISE(ABORT, 'estimate calibrations are append only'); END;
CREATE TABLE daily_valuation_snapshots (
    id INTEGER PRIMARY KEY,
    account INTEGER NOT NULL,
    as_of_seconds INTEGER NOT NULL,
    as_of_nanos INTEGER NOT NULL CHECK (as_of_nanos >= 0 AND as_of_nanos < 1000000000),
    as_of_offset INTEGER NOT NULL CHECK (as_of_offset BETWEEN -86340 AND 86340 AND as_of_offset % 60 = 0),
    snapshot_hash TEXT NOT NULL,
    snapshot_payload TEXT NOT NULL,
    UNIQUE (account, snapshot_hash)
) STRICT;
CREATE INDEX daily_valuation_snapshots_lookup ON daily_valuation_snapshots(account, as_of_seconds, as_of_nanos, id);
CREATE TRIGGER daily_valuation_snapshots_no_update BEFORE UPDATE ON daily_valuation_snapshots BEGIN SELECT RAISE(ABORT, 'daily valuation snapshots are append only'); END;
CREATE TRIGGER daily_valuation_snapshots_no_delete BEFORE DELETE ON daily_valuation_snapshots BEGIN SELECT RAISE(ABORT, 'daily valuation snapshots are append only'); END;
CREATE TRIGGER daily_valuation_snapshots_no_replace BEFORE INSERT ON daily_valuation_snapshots WHEN EXISTS (SELECT 1 FROM daily_valuation_snapshots WHERE account = NEW.account AND snapshot_hash = NEW.snapshot_hash) BEGIN SELECT RAISE(ABORT, 'daily valuation snapshots are append only'); END;
CREATE TABLE daily_valuation_days (
    id INTEGER PRIMARY KEY,
    account INTEGER NOT NULL,
    day_year INTEGER NOT NULL CHECK (day_year BETWEEN 1 AND 9999),
    day_month INTEGER NOT NULL CHECK (day_month BETWEEN 1 AND 12),
    day_number INTEGER NOT NULL CHECK (day_number BETWEEN 1 AND 31),
    result_hash TEXT NOT NULL,
    result_payload TEXT NOT NULL,
    UNIQUE (account, day_year, day_month, day_number)
) STRICT;
CREATE TRIGGER daily_valuation_days_no_update BEFORE UPDATE ON daily_valuation_days BEGIN SELECT RAISE(ABORT, 'daily valuation days are append only'); END;
CREATE TRIGGER daily_valuation_days_no_delete BEFORE DELETE ON daily_valuation_days BEGIN SELECT RAISE(ABORT, 'daily valuation days are append only'); END;
CREATE TRIGGER daily_valuation_days_no_replace BEFORE INSERT ON daily_valuation_days WHEN EXISTS (SELECT 1 FROM daily_valuation_days WHERE account = NEW.account AND day_year = NEW.day_year AND day_month = NEW.day_month AND day_number = NEW.day_number) BEGIN SELECT RAISE(ABORT, 'daily valuation days are append only'); END;
