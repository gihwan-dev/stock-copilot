#[expect(clippy::disallowed_macros, reason = "인라인 모듈")]
pub mod error {
    #[derive(Type)]
    pub enum IpcError {
        Bug,
    }
}
