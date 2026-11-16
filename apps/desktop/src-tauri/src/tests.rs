use tauri::test::{mock_builder, mock_context, noop_assets};

use crate::build_app;
mod database;
mod lifecycle;
mod startup;
mod support;

#[test]
fn build_app_builds_empty_app_on_mock_runtime() {
    build_app(
        mock_builder(),
        support::ports(),
        mock_context(noop_assets()),
    )
    .expect("mock runtime builds empty app");
}
