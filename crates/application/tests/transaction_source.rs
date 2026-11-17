#[cfg(test)]
mod tests {
    #[test]
    fn source_is_opaque_and_preserves_every_byte_without_debug_disclosure() {
        let spelling =
            "{ \n\t\r\"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789\":\"\\uAC00 가 \\\" \\\\ / : , [ ]\" }";
        let source = application::RawTransactionRow::new(spelling.to_owned());
        assert_eq!(source.as_str().as_bytes(), spelling.as_bytes());
        assert_eq!(source.clone(), source);
        assert_eq!(format!("{source:?}"), "RawTransactionRow([redacted])");
    }
}
