use tauri::test::{mock_builder, mock_context, noop_assets};

use crate::build_app;
mod autostart;
mod database;
mod exit;
mod ipc_contract;
mod lifecycle;
mod notifier;
mod process_cleanup;
mod residency;
mod responsiveness;
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
