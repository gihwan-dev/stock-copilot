pub(crate) const UPDATE_TASK_PAYLOAD: &str =
    "UPDATE estimate_tasks SET task_payload = 'tampered' WHERE task_id = ?1";
pub(crate) const REVERT_TASK_STATUS: &str =
    "UPDATE estimate_tasks SET status = 'available' WHERE task_id = ?1";
pub(crate) const DELETE_TASK: &str = "DELETE FROM estimate_tasks WHERE task_id = ?1";
pub(crate) const UPDATE_SUBMISSION: &str =
    "UPDATE estimate_submissions SET submission_payload = 'tampered'";
pub(crate) const DELETE_SUBMISSION: &str = "DELETE FROM estimate_submissions";
pub(crate) const UPDATE_SUBMISSION_RECORD: &str =
    "UPDATE estimate_submission_records SET record_payload = 'tampered'";
pub(crate) const DELETE_SUBMISSION_RECORD: &str = "DELETE FROM estimate_submission_records";
pub(crate) const UPDATE_RESOLUTION: &str =
    "UPDATE estimate_resolutions SET outcome_payload = 'tampered'";
pub(crate) const DELETE_RESOLUTION: &str = "DELETE FROM estimate_resolutions";
pub(crate) const UPDATE_TASK_RESOLUTION: &str =
    "UPDATE estimate_task_resolutions SET outcome_payload = 'tampered'";
pub(crate) const DELETE_TASK_RESOLUTION: &str = "DELETE FROM estimate_task_resolutions";
pub(crate) const UPDATE_CALIBRATION: &str =
    "UPDATE estimate_calibrations SET table_payload = 'tampered'";
pub(crate) const DELETE_CALIBRATION: &str = "DELETE FROM estimate_calibrations";
pub(crate) const UPDATE_VALUATION: &str =
    "UPDATE daily_valuation_snapshots SET snapshot_payload = 'tampered'";
pub(crate) const DELETE_VALUATION: &str = "DELETE FROM daily_valuation_snapshots";
pub(crate) const UPDATE_VALUATION_DAY: &str =
    "UPDATE daily_valuation_days SET result_payload = 'tampered'";
pub(crate) const DELETE_VALUATION_DAY: &str = "DELETE FROM daily_valuation_days";
pub(crate) const INSERT_CORRUPT_SUBMISSION: &str = "INSERT INTO estimate_submissions(instrument, submission_hash, submission_payload) VALUES ('corrupt', 'corrupt-hash', 'not-valid')";
pub(crate) const INSERT_CORRUPT_VALUATION: &str = "INSERT INTO daily_valuation_snapshots(account, as_of_seconds, as_of_nanos, as_of_offset, snapshot_hash, snapshot_payload) VALUES (99, 1000, 0, 0, 'corrupt-hash', 'not-valid')";
