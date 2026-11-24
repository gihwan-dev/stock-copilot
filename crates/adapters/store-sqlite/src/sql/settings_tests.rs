pub(crate) const FUTURE_VERSION: &str =
    "UPDATE settings SET schema_version = 2 WHERE name = 'risk_settings'";
pub(crate) const CORRUPT_PROFILE: &str =
    "UPDATE settings SET payload = '3:bad' WHERE name = 'investor_profile'";
pub(crate) const DELETE_ALL: &str = "DELETE FROM settings";
pub(crate) const UNKNOWN_NAME: &str =
    "INSERT INTO settings(name, schema_version, payload) VALUES ('other', 1, 'x')";
