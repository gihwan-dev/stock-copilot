pub(crate) const CREATE: &str = "CREATE TABLE job_schedule_settings (\n    job TEXT PRIMARY KEY,\n    enabled TEXT NOT NULL CHECK (enabled IN ('enabled', 'disabled')),\n    hour INTEGER CHECK (hour IS NULL OR (hour >= 0 AND hour <= 23)),\n    minute INTEGER CHECK (minute IS NULL OR (minute >= 0 AND minute <= 59)),\n    CHECK ((hour IS NULL) = (minute IS NULL))\n) STRICT;\nCREATE TABLE job_execution_records (\n    seq INTEGER PRIMARY KEY,\n    job TEXT NOT NULL,\n    payload TEXT NOT NULL\n) STRICT;\nCREATE INDEX job_execution_records_by_job ON job_execution_records(job, seq);\n";
pub(crate) const UPSERT_SETTING: &str = "INSERT INTO job_schedule_settings(job, enabled, hour, minute) VALUES (?1, ?2, ?3, ?4) ON CONFLICT(job) DO UPDATE SET enabled = excluded.enabled, hour = excluded.hour, minute = excluded.minute";
pub(crate) const ALL_SETTINGS: &str =
    "SELECT job, enabled, hour, minute FROM job_schedule_settings ORDER BY job";
pub(crate) const APPEND_RECORD: &str =
    "INSERT INTO job_execution_records(job, payload) VALUES (?1, ?2)";
pub(crate) const TRIM_RECORDS: &str = "DELETE FROM job_execution_records WHERE job = ?1 AND seq NOT IN (SELECT seq FROM job_execution_records WHERE job = ?1 ORDER BY seq DESC LIMIT 50)";
pub(crate) const RECENT_RECORDS: &str =
    "SELECT payload FROM job_execution_records WHERE job = ?1 ORDER BY seq DESC LIMIT 50";
