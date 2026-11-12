#[expect(clippy::missing_errors_doc, reason = "pedantic 소속이지만 개별 설정")]
pub fn load(path: &Source) -> Result<Loaded, LoadError> {
    path.load()
}
