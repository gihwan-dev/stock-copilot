pub(crate) const CREATE_TEST: &str =
    "CREATE TABLE samples (id INTEGER PRIMARY KEY, value TEXT NOT NULL) STRICT";
pub(crate) const INSERT_TEST: &str = "INSERT INTO samples(value) VALUES (?1)";
pub(crate) const COUNT_TEST: &str = "SELECT count(*) FROM samples";
pub(crate) const RAW_KEY: &str =
    "PRAGMA key = \"x'0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20'\"";
pub(crate) const UNKNOWN: &str = "PRAGMA s01_unknown_setting";
