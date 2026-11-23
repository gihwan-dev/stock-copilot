pub(crate) const UPDATE_CONSUMPTION: &str =
    "UPDATE proposal_consumptions SET proposal_id = 'changed' WHERE proposal_id = ?1";
pub(crate) const DELETE_CONSUMPTION: &str =
    "DELETE FROM proposal_consumptions WHERE proposal_id = ?1";
pub(crate) const FAIL_REPORT: &str = "CREATE TRIGGER fail_report_insert BEFORE INSERT ON proposal_validation_reports WHEN NEW.c01 = 'failed' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END";
pub(crate) const IGNORE_CHECKS_ON: &str = "PRAGMA ignore_check_constraints = ON";
pub(crate) const IGNORE_CHECKS_OFF: &str = "PRAGMA ignore_check_constraints = OFF";
pub(crate) const CORRUPT_REPORT: &str =
    "UPDATE proposal_validation_reports SET c04 = 'bad' WHERE proposal_id = ?1";
pub(crate) const FAIL_CONSUMPTION: &str = "CREATE TRIGGER fail_consumption BEFORE INSERT ON proposal_consumptions BEGIN SELECT RAISE(ABORT, 'fixture failure'); END";
pub(crate) const OTHER_UNIQUE: &str =
    "CREATE UNIQUE INDEX fixture_other_unique ON proposal_consumptions(substr(proposal_id, 1, 1))";

pub(crate) const REPLACE_CONSUMPTION: &str =
    "INSERT OR REPLACE INTO proposal_consumptions(proposal_id) VALUES (?1)";
