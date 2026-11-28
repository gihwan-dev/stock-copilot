pub(crate) const CREATE: &str = "CREATE TABLE dashboard_research (\n    section TEXT PRIMARY KEY,\n    payload TEXT NOT NULL\n) STRICT;\n";
pub(crate) const LOAD: &str = "SELECT payload FROM dashboard_research WHERE section = ?1";
pub(crate) const SAVE: &str = "INSERT INTO dashboard_research(section, payload) VALUES (?1, ?2) ON CONFLICT(section) DO UPDATE SET payload = excluded.payload";
