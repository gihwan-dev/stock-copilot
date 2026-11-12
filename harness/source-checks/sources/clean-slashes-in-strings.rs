pub fn address() -> &'static str {
    "https://example.invalid//path /* not a comment */"
}

pub fn raw() -> &'static str {
    r#"// still a string"#
}

pub fn slash() -> char {
    '/'
}
