pub(crate) const UPDATE_JOB: &str = "UPDATE ai_job_records SET payload = 'x'";
pub(crate) const DELETE_JOB: &str = "DELETE FROM ai_job_records";
pub(crate) const DELETE_PRINCIPLES: &str = "DELETE FROM investment_principles";
pub(crate) const FUTURE_PRINCIPLES: &str = "UPDATE investment_principles SET schema_version = 2";
pub(crate) const CORRUPT_JOB: &str = "INSERT INTO ai_job_records(job_id, finished_seconds, finished_nanos, payload) VALUES ('corrupt', 0, 0, '3:bad')";
