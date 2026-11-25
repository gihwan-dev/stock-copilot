pub(crate) const CREATE: &str = "CREATE TABLE ai_preferences (\n    id INTEGER PRIMARY KEY CHECK (id = 1),\n    payload TEXT NOT NULL\n) STRICT;\n";
pub(crate) const LOAD: &str = "SELECT payload FROM ai_preferences WHERE id = 1";
pub(crate) const STORE: &str = "INSERT INTO ai_preferences(id, payload) VALUES (1, ?1) ON CONFLICT(id) DO UPDATE SET payload = excluded.payload";
