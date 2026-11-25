use tauri::test::{mock_builder, mock_context, noop_assets};

use crate::build_app;
mod autostart;
mod collector;
mod database;
mod e2e_smoke;
mod exit;
mod ipc_contract;
mod key_provision;
mod lifecycle;
mod notifier;
mod order_path;
mod process_cleanup;
mod residency;
mod responsiveness;
mod scheduling;
mod startup;

pub(crate) mod support;

#[test]
fn build_app_builds_empty_app_on_mock_runtime() {
    build_app(
        mock_builder(),
        support::ports(),
        mock_context(noop_assets()),
    )
    .expect("mock runtime builds empty app");
}
