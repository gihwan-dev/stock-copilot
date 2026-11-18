pub(crate) const FTS: &str =
    "CREATE VIRTUAL TABLE information_fts USING fts5(title, content='', contentless_delete=1);";
pub(crate) const NONSTRICT: &str = "CREATE TABLE ordinary (payload TEXT);";
pub(crate) const FTS4: &str = "CREATE VIRTUAL TABLE legacy_fts USING fts4(title);";
pub(crate) const SPOOF: &str = "CREATE VIRTUAL TABLE information_fts USING fts5(title, content='', contentless_delete=1); DROP TABLE information_fts_data; CREATE TABLE 'information_fts_data'(fake TEXT);";
pub(crate) const SPOOF_KIND: &str =
    "SELECT type FROM pragma_table_list WHERE name = 'information_fts_data'";
pub(crate) const ORPHAN: &str = "CREATE TABLE 'information_fts_data'(fake TEXT);";
pub(crate) const FTS_DEFAULT: &str =
    "CREATE VIRTUAL TABLE information_fts USING fts5(title, body);";
pub(crate) const FAKE_DECLARATION: &str =
    "INSERT INTO notes(payload) VALUES ('CREATE VIRTUAL TABLE information_fts USING fts5(title);')";
pub(crate) const EXACT_SPOOF: &str = "CREATE VIRTUAL TABLE information_fts USING fts5(title, content='', contentless_delete=1); DROP TABLE information_fts_data; CREATE TABLE 'information_fts_data'(id INTEGER PRIMARY KEY, block BLOB);";
pub(crate) const COPIED_SPOOF: &str = "CREATE VIRTUAL TABLE information_fts USING fts5(title, content='', contentless_delete=1); CREATE TEMP TABLE saved AS SELECT * FROM information_fts_data; DROP TABLE information_fts_data; CREATE TABLE 'information_fts_data'(id INTEGER PRIMARY KEY, block BLOB); INSERT INTO information_fts_data SELECT * FROM saved; DROP TABLE saved;";
pub(crate) const ROGUE_FTS: &str = "CREATE VIRTUAL TABLE rogue_fts USING fts5(title);";
pub(crate) const ROGUE_DAMAGED: &str = "CREATE VIRTUAL TABLE rogue_fts USING fts5(title); DROP TABLE rogue_fts_data; CREATE TABLE 'rogue_fts_data'(id INTEGER PRIMARY KEY, block BLOB);";
pub(crate) const FTS_NO_DOCSIZE: &str =
    "CREATE VIRTUAL TABLE compact_fts USING fts5(title, columnsize=0);";
