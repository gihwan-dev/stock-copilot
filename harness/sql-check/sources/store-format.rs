pub fn query(table: &str) -> String {
    format!("SELECT id FROM {table}")
}
