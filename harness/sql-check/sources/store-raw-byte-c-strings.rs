pub const SUM: &[u8] = b"SELECT sum(\x70rice) FROM quotes";
pub const AVERAGE: &str = r#"SELECT avg(price) FROM "quotes""#;
pub const MAXIMUM: &core::ffi::CStr = c"SELECT max(\u{70}rice) FROM quotes";
pub const TOTAL: &[u8] = br"SELECT total(price) FROM quotes";
