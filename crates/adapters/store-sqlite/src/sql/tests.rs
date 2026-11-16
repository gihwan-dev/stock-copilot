pub(crate) const CREATE_TEST: &str =
    "CREATE TABLE samples (id INTEGER PRIMARY KEY, payload TEXT NOT NULL) STRICT";
pub(crate) const INSERT_TEST: &str = "INSERT INTO samples(payload) VALUES (?1)";
pub(crate) const COUNT_TEST: &str = "SELECT count(*) FROM samples";
pub(crate) const VALUE_TEST: &str = "SELECT payload FROM samples WHERE id = 1";
pub(crate) const RAW_KEY: &str =
    "PRAGMA key = \"x'0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20'\"";
pub(crate) const UNKNOWN: &str = "PRAGMA s01_unknown_setting";

pub(crate) const BAD_HASH: &str = "UPDATE migration_ledger SET sha256 = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'";
pub(crate) const BAD_NAME: &str = "UPDATE migration_ledger SET name = '0001_other.sql'";
pub(crate) const MISSING: &str = "DELETE FROM migration_ledger";
pub(crate) const GAP: &str = "UPDATE migration_ledger SET number = 2";
pub(crate) const FUTURE: &str = "PRAGMA user_version = 2";
pub(crate) const DROP_LEDGER: &str = "DROP TABLE migration_ledger";
pub(crate) const NONSTRICT: &str = "CREATE TABLE loose (id INTEGER)";
pub(crate) const BAD_MONEY: &str =
    "CREATE TABLE money_fixture (amount TEXT NOT NULL, currency TEXT NOT NULL) STRICT";
pub(crate) const BROKEN: &str =
    "CREATE TABLE partial (id INTEGER) STRICT; INSERT INTO absent VALUES (1)";
pub(crate) const PARTIAL: &str = "SELECT count(*) FROM sqlite_schema WHERE name = 'partial'";

pub(crate) const GOOD_MONEY: &str = "CREATE TABLE money_fixture (currency TEXT NOT NULL, amount TEXT NOT NULL CHECK (amount NOT GLOB '*[^0-9.-]*' AND amount GLOB '*[0-9]*' AND amount NOT GLOB '?*-*' AND amount NOT GLOB '*.*.*' AND amount NOT GLOB '.*' AND amount NOT GLOB '-.*' AND amount NOT GLOB '*.')) STRICT";
pub(crate) const INSERT_MONEY: &str =
    "INSERT INTO money_fixture(currency, amount) VALUES ('KRW', ?1)";

pub(crate) const LEDGER_COUNT: &str = "SELECT count(*) FROM migration_ledger";
pub(crate) const PREFIX_NONSTRICT: &str = "CREATE TABLE sqliteevil (id INTEGER)";

pub(crate) const ESCAPE_COMMIT: &str =
    "COMMIT; CREATE TABLE partial (id INTEGER) STRICT; INSERT INTO absent VALUES (1)";
pub(crate) const ESCAPE_ROLLBACK: &str =
    "ROLLBACK; CREATE TABLE partial (id INTEGER) STRICT; INSERT INTO absent VALUES (1)";
pub(crate) const ESCAPE_SAVEPOINT: &str =
    "SAVEPOINT escaped; CREATE TABLE partial (id INTEGER) STRICT";
