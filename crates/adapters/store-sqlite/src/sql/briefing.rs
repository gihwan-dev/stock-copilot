pub(crate) const CREATE: &str = "CREATE TABLE briefings (\n    id_key TEXT PRIMARY KEY,\n    seq INTEGER NOT NULL UNIQUE CHECK (seq >= 1),\n    payload TEXT NOT NULL\n) STRICT;\nCREATE TRIGGER briefings_no_delete BEFORE DELETE ON briefings BEGIN SELECT RAISE(ABORT, 'briefings are replaced, not deleted'); END;\n";
pub(crate) const UPSERT: &str = "INSERT INTO briefings(id_key, seq, payload) VALUES (?1, (SELECT coalesce(max(seq), 0) + 1 FROM briefings), ?2) ON CONFLICT(id_key) DO UPDATE SET seq = excluded.seq, payload = excluded.payload";
pub(crate) const FIND: &str = "SELECT payload FROM briefings WHERE id_key = ?1";
pub(crate) const RECENT: &str = "SELECT payload FROM briefings ORDER BY seq";
