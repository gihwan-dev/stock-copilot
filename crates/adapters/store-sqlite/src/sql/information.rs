pub(crate) const CREATE: &str = "CREATE TABLE information_items (\n    id INTEGER PRIMARY KEY,\n    category TEXT NOT NULL,\n    source TEXT NOT NULL,\n    external_id TEXT NOT NULL,\n    url TEXT NOT NULL,\n    title TEXT NOT NULL,\n    body TEXT,\n    UNIQUE (category, source, external_id)\n) STRICT;\nCREATE TABLE information_links (\n    category TEXT NOT NULL,\n    source TEXT NOT NULL,\n    external_id TEXT NOT NULL,\n    instrument TEXT NOT NULL,\n    PRIMARY KEY (category, source, external_id, instrument)\n) STRICT;\nCREATE VIRTUAL TABLE information_fts USING fts5(title_grams, body_grams, tokenize='unicode61', content='', contentless_delete=1);\n";
pub(crate) const BY_KEY: &str = "SELECT id, category, source, external_id, url, title, body FROM information_items WHERE category = ?1 AND source = ?2 AND external_id = ?3";
pub(crate) const BY_ID: &str = "SELECT id, category, source, external_id, url, title, body FROM information_items WHERE id = ?1";
pub(crate) const ALL_ITEMS: &str =
    "SELECT id, category, source, external_id, url, title, body FROM information_items ORDER BY id";
pub(crate) const INSERT_ITEM: &str = "INSERT INTO information_items(category, source, external_id, url, title, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
pub(crate) const UPDATE_ITEM: &str =
    "UPDATE information_items SET url = ?1, title = ?2, body = ?3 WHERE id = ?4";
pub(crate) const DELETE_LINKS: &str =
    "DELETE FROM information_links WHERE category = ?1 AND source = ?2 AND external_id = ?3";
pub(crate) const INSERT_LINK: &str = "INSERT INTO information_links(category, source, external_id, instrument) VALUES (?1, ?2, ?3, ?4)";
pub(crate) const BY_INSTRUMENT: &str = "SELECT i.id, i.category, i.source, i.external_id, i.url, i.title, i.body FROM information_items AS i INNER JOIN information_links AS l ON i.category = l.category AND i.source = l.source AND i.external_id = l.external_id WHERE l.instrument = ?1 ORDER BY i.id";
pub(crate) const DELETE_INDEX: &str = "DELETE FROM information_fts WHERE rowid = ?1";
pub(crate) const DELETE_ALL_INDEX: &str = "DELETE FROM information_fts";
pub(crate) const INSERT_INDEX: &str =
    "INSERT INTO information_fts(rowid, title_grams, body_grams) VALUES (?1, ?2, ?3)";
pub(crate) const SEARCH_INDEX: &str = "SELECT rowid FROM information_fts WHERE information_fts MATCH ?1 ORDER BY rank, rowid LIMIT ?2";
