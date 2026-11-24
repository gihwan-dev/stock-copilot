pub(crate) const DELETE_SOURCE: &str = "DELETE FROM source_statuses";
pub(crate) const UNKNOWN_SOURCE: &str =
    "INSERT INTO source_statuses(source, payload) VALUES ('other', 'x')";
pub(crate) const CORRUPT_SOURCE: &str = "UPDATE source_statuses SET payload = '3:bad'";
pub(crate) const UPDATE_SPEC: &str = "UPDATE spec_status_records SET payload = 'x'";
pub(crate) const DELETE_SPEC: &str = "DELETE FROM spec_status_records";
pub(crate) const SPEC_COUNT: &str = "SELECT count(*) FROM spec_status_records";
pub(crate) const DELETE_NEWS: &str = "DELETE FROM news_data_settings";
pub(crate) const SECOND_NEWS: &str =
    "INSERT INTO news_data_settings(id, schema_version, payload) VALUES (2, 1, 'x')";
pub(crate) const FUTURE_NEWS: &str = "UPDATE news_data_settings SET schema_version = 2";
