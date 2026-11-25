CREATE TABLE job_schedule_settings (
    job TEXT PRIMARY KEY,
    enabled TEXT NOT NULL CHECK (enabled IN ('enabled', 'disabled')),
    hour INTEGER CHECK (hour IS NULL OR (hour >= 0 AND hour <= 23)),
    minute INTEGER CHECK (minute IS NULL OR (minute >= 0 AND minute <= 59)),
    CHECK ((hour IS NULL) = (minute IS NULL))
) STRICT;
CREATE TABLE job_execution_records (
    seq INTEGER PRIMARY KEY,
    job TEXT NOT NULL,
    payload TEXT NOT NULL
) STRICT;
CREATE INDEX job_execution_records_by_job ON job_execution_records(job, seq);
