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
pub(crate) const MIGRATE_0021: &str = "CREATE TABLE information_times (\n    item_id INTEGER PRIMARY KEY REFERENCES information_items(id),\n    published_payload TEXT,\n    collected_payload TEXT NOT NULL\n) STRICT;\n";
pub(crate) const RECORD_TIMES: &str = "INSERT INTO information_times(item_id, published_payload, collected_payload) SELECT id, ?1, ?2 FROM information_items WHERE category = ?3 AND source = ?4 AND external_id = ?5 ON CONFLICT(item_id) DO UPDATE SET published_payload = excluded.published_payload, collected_payload = excluded.collected_payload";
pub(crate) const OBSERVED_BY_CATEGORY: &str = "SELECT i.id, i.category, i.source, i.external_id, i.url, i.title, i.body, t.published_payload, t.collected_payload FROM information_items AS i INNER JOIN information_times AS t ON t.item_id = i.id WHERE i.category = ?1 ORDER BY i.id DESC LIMIT 5000";
pub(crate) const OBSERVED_BY_ID: &str = "SELECT i.id, i.category, i.source, i.external_id, i.url, i.title, i.body, t.published_payload, t.collected_payload FROM information_items AS i INNER JOIN information_times AS t ON t.item_id = i.id WHERE i.id = ?1 AND i.category = ?2";
pub(crate) const LINKED_INSTRUMENTS: &str = "SELECT instrument FROM information_links WHERE category = ?1 AND source = ?2 AND external_id = ?3 ORDER BY instrument";
pub(crate) const KNOWN_URL: &str = "SELECT url FROM information_items WHERE url = ?1 LIMIT 1";
pub(crate) const NEWS_COLLECTED_TIMES: &str = "SELECT i.id, i.source, i.external_id, t.collected_payload FROM information_items AS i INNER JOIN information_times AS t ON t.item_id = i.id WHERE i.category = ?1";
pub(crate) const DELETE_TIMES: &str = "DELETE FROM information_times WHERE item_id = ?1";
pub(crate) const DELETE_ITEM: &str = "DELETE FROM information_items WHERE id = ?1";

pub(crate) const CITED_RESULT_HANDLE: &str = "
SELECT EXISTS(
    SELECT 1
    FROM tool_call_ledger AS call,
         json_each(CASE WHEN json_valid(call.result_json) THEN call.result_json ELSE '{}' END, '$.items') AS item
    WHERE call.run_id = ?1 AND call.tool_name = ?2 AND call.outcome = 'succeeded'
      AND json_type(CASE WHEN json_valid(call.result_json) THEN call.result_json ELSE '{}' END, '$.items') = 'array'
      AND json_extract(CASE WHEN item.type = 'object' THEN item.value ELSE '{}' END, '$.handle') = ?3
      AND json_extract(CASE WHEN item.type = 'object' THEN item.value ELSE '{}' END, '$.citation_digest') = ?4
)";
