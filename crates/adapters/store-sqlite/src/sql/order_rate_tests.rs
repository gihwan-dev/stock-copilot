pub(crate) const UPDATE_SUBMISSION: &str = "UPDATE order_submissions SET utc_seconds = 0";
pub(crate) const DELETE_SUBMISSION: &str = "DELETE FROM order_submissions";
pub(crate) const DELETE_LIMIT: &str = "DELETE FROM order_rate_limit";
pub(crate) const SECOND_LIMIT_ROW: &str =
    "INSERT INTO order_rate_limit(id, hourly_limit) VALUES (2, 5)";
pub(crate) const OUT_OF_RANGE_LIMIT: &str = "UPDATE order_rate_limit SET hourly_limit = 31";
pub(crate) const SUBMISSION_COUNT: &str = "SELECT count(*) FROM order_submissions";
