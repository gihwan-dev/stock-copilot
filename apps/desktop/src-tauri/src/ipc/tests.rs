use std::fs;
use std::path::Path;

use serde::Serialize;
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::app_version::app_version;
use super::error::IpcError;

const RESPONSE_DIRECTORY: &str = "ipc/responses";

fn assert_response_snapshot<T: Serialize>(name: &str, value: &T) {
    let path = Path::new(RESPONSE_DIRECTORY).join(format!("{name}.json"));
    let mut actual = serde_json::to_string_pretty(value).expect("serialize response");
    actual.push('\n');
    let recorded = fs::read_to_string(&path).expect("read recorded response");
    assert_eq!(
        recorded,
        actual,
        "{} differs from the serialized response; update it and run gen-bindings",
        path.display()
    );
}

#[test]
fn app_version_response_is_recorded_for_the_typescript_fake() {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let response = app_version(app.handle().clone()).expect("app version");
    assert_response_snapshot("app_version", &response);
}

#[test]
fn every_ipc_error_is_recorded_with_its_tag() {
    assert_response_snapshot(
        "ipc_errors",
        &[IpcError::UserFixable, IpcError::Retryable, IpcError::Bug],
    );
}
