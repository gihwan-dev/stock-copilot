pub(crate) const DOUBLE_REF: &str =
    "CREATE TABLE hidden (id INTEGER REFERENCES \"a'b\"(id), price REAL) STRICT";
pub(crate) const BACKTICK_REF: &str =
    "CREATE TABLE hidden (id INTEGER REFERENCES `a'b`(id), price REAL) STRICT";
pub(crate) const BRACKET_REF: &str =
    "CREATE TABLE hidden (id INTEGER REFERENCES [a'b](id), price REAL) STRICT";
pub(crate) const PAREN_REF: &str =
    "CREATE TABLE hidden (id INTEGER REFERENCES \"a(b\"(id), amount TEXT) STRICT";
pub(crate) const DOUBLE_DEFAULT: &str =
    "CREATE TABLE hidden (note TEXT DEFAULT \"it's\", price REAL) STRICT";
pub(crate) const QUOTED_COLUMN: &str = "CREATE TABLE hidden (\"price\" REAL) STRICT";
pub(crate) const BACKTICK_COLUMN: &str = "CREATE TABLE hidden (`price` REAL) STRICT";
pub(crate) const BRACKET_COLUMN: &str = "CREATE TABLE hidden ([price] REAL) STRICT";
pub(crate) const TEXT_MARKERS: &str =
    "CREATE TABLE safe_text (note TEXT DEFAULT '--x /*y*/ \"quoted\" `tag` [id] ((, it''s') STRICT";
pub(crate) const TEXT_HIDDEN_PRICE: &str = "CREATE TABLE hidden (note TEXT DEFAULT '--x /*y*/ \"quoted\" `tag` [id] ((, it''s', price REAL) STRICT";
pub(crate) const QUOTED_TABLE_BASE: &str =
    "CREATE TABLE hidden (id INTEGER REFERENCES \"a'b\"(id)) STRICT";
pub(crate) const ADD_PRICE: &str = "ALTER TABLE hidden ADD COLUMN price REAL";
pub(crate) const TAMPER_DELETE: &str =
    "CREATE TABLE partial (id INTEGER PRIMARY KEY) STRICT; DELETE FROM migration_ledger";
pub(crate) const TAMPER_HASH: &str = "CREATE TABLE partial (id INTEGER PRIMARY KEY) STRICT; UPDATE migration_ledger SET sha256 = '0000000000000000000000000000000000000000000000000000000000000000'";
pub(crate) const TAMPER_NAME: &str = "CREATE TABLE partial (id INTEGER PRIMARY KEY) STRICT; UPDATE migration_ledger SET name = '0001_changed.sql'";
pub(crate) const TAMPER_NUMBER: &str =
    "CREATE TABLE partial (id INTEGER PRIMARY KEY) STRICT; UPDATE migration_ledger SET number = 3";

pub(crate) const TAMPER_TIME: &str = "CREATE TABLE partial (id INTEGER PRIMARY KEY) STRICT; UPDATE migration_ledger SET applied_at = 0";
pub(crate) const SUPPRESS_NEW: &str = "CREATE TRIGGER suppress_new AFTER INSERT ON migration_ledger WHEN NEW.number = 2 BEGIN DELETE FROM migration_ledger WHERE number = 2; END";
pub(crate) const CHANGE_NEW_TIME: &str = "CREATE TRIGGER change_new_time AFTER INSERT ON migration_ledger WHEN NEW.number = 2 BEGIN UPDATE migration_ledger SET applied_at = 0 WHERE number = 2; END";

pub(crate) const VALID_DML: &str = "CREATE TABLE partial (id INTEGER PRIMARY KEY) STRICT; INSERT INTO partial VALUES (17); UPDATE partial SET id = 29 WHERE id = 17";
pub(crate) const PARTIAL_ID: &str = "SELECT id FROM partial";

pub(crate) const PLAIN_QUOTED_REF: &str =
    "CREATE TABLE plain_ref (id INTEGER REFERENCES \"parent\"(id)) STRICT";
pub(crate) const PLAIN_BACKTICK_REF: &str =
    "CREATE TABLE plain_ref (id INTEGER REFERENCES `parent`(id)) STRICT";
pub(crate) const PLAIN_BRACKET_REF: &str =
    "CREATE TABLE plain_ref (id INTEGER REFERENCES [parent](id)) STRICT";
pub(crate) const NEGATIVE_DEFAULT: &str =
    "CREATE TABLE negative_literal (id INTEGER DEFAULT -17) STRICT";
pub(crate) const DIVISION_DEFAULT: &str =
    "CREATE TABLE divided_literal (id INTEGER DEFAULT (21 / 3)) STRICT";
pub(crate) const REPAIR_LEDGER: &str = "UPDATE migration_ledger SET number = 1, name = '0001_migration_ledger.sql', sha256 = 'f059233e48dd3cd2f492b96f916d760c19a109861b52707d6901f2bc0c6b2501'; CREATE TABLE partial (id INTEGER) STRICT";

pub(crate) const HEADER_BLOCK: &str =
    "CREATE TABLE x /*(z'*/ (a INTEGER, price REAL /*'*/ , b INTEGER) STRICT";
pub(crate) const HEADER_LINE: &str =
    "CREATE TABLE x --(z'\n (a INTEGER, amount TEXT --'\n , b INTEGER) STRICT";
pub(crate) const HEADER_DEPTH: &str = "CREATE TABLE x /*(*/ (a INTEGER, price REAL) STRICT";
pub(crate) const HEADER_LITERAL: &str = "CREATE TABLE x /*('*/ (a INTEGER, price REAL) STRICT";
pub(crate) const HEADER_UNDERFLOW: &str =
    "CREATE TABLE x /*(z'*/ (price REAL, a INTEGER DEFAULT (1 /*'*/ )) STRICT";
pub(crate) const UNICODE_SPACE: &str = "CREATE TABLE x (a\u{3000}price REAL) STRICT";
pub(crate) const UNICODE_NBSP: &str = "CREATE TABLE x (a\u{00a0}price REAL) STRICT";
pub(crate) const UNICODE_TEXT: &str =
    "CREATE TABLE localized (note TEXT DEFAULT '한글　가격') STRICT";
pub(crate) const IGNORE_CHECKS: &str = "PRAGMA ignore_check_constraints = 1";
pub(crate) const CHECK_STATE: &str = "PRAGMA ignore_check_constraints";
