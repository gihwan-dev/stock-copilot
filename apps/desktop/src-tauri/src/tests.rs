use tauri::test::{mock_builder, mock_context, noop_assets};

use crate::build_app;

#[test]
fn build_app_builds_empty_app_on_mock_runtime() {
    let built = build_app(mock_builder(), mock_context(noop_assets()));
    assert!(built.is_ok());
}
