#[expect(clippy::disallowed_methods, reason = "허용 목록 밖")]
pub fn stop() {
    std::process::exit(1);
}
