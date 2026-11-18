pub(crate) const DROP_INDEX: &str = "DROP TABLE information_fts";
pub(crate) const CORRUPT_URL: &str =
    "UPDATE information_items SET url = 'invalid' WHERE category = 'news' AND source = 'yonhap'";
pub(crate) const STRAY_INDEX: &str = "INSERT INTO information_fts(rowid, title_grams, body_grams) VALUES (999999, 'u103v104 u104v111 u111v115 u115v116', '')";
