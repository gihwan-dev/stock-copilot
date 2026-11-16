pub(crate) const CREATE_LEDGER: &str = "CREATE TABLE migration_ledger (
    number INTEGER PRIMARY KEY CHECK (number > 0),
    name TEXT NOT NULL UNIQUE,
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    applied_at INTEGER NOT NULL
) STRICT;
";
pub(crate) const LEDGER: &str =
    "SELECT number, name, sha256, applied_at FROM migration_ledger ORDER BY number";
pub(crate) const INSERT_LEDGER: &str =
    "INSERT INTO migration_ledger(number, name, sha256, applied_at) VALUES (?1, ?2, ?3, ?4)";
pub(crate) const TABLES: &str = "SELECT name, strict FROM pragma_table_list WHERE schema = 'main' AND type IN ('table', 'virtual', 'shadow') AND name NOT GLOB 'sqlite_*'";
pub(crate) const SCHEMA: &str = "SELECT name, sql FROM sqlite_schema WHERE type = 'table' AND name NOT GLOB 'sqlite_*' ORDER BY name";

pub(crate) const VERSION: &str = "PRAGMA user_version";
