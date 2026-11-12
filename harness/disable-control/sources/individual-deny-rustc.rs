#[expect(unused_qualifications, reason = "개별 deny 린트")]
pub fn path() -> Path {
    crate::Path::root()
}
