#[expect(clippy::needless_pass_by_value, reason = "명령이 아닌 함수")]
pub fn app_version<R: Runtime>(app: AppHandle<R>) -> AppVersion {
    version(&app)
}
