pub struct Index {
    #[expect(clippy::disallowed_types, reason = "허용 목록 밖")]
    entries: std::collections::HashMap<Key, Value>,
}
