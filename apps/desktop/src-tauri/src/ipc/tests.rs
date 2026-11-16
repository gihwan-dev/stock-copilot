use std::fs;
use std::path::Path;

use serde::Serialize;
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::app_version::{AppVersionQuery, AppVersionRequest, app_version};
use super::error::{IpcError, IpcErrorKind};

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
    let response = tauri::async_runtime::block_on(app_version(
        app.handle().clone(),
        AppVersionRequest {
            query: AppVersionQuery::Current,
        },
    ))
    .expect("app version");
    assert_response_snapshot("app_version", &response);
}

#[test]
fn every_ipc_error_is_recorded_with_its_tag() {
    assert_response_snapshot(
        "ipc_errors",
        &[
            IpcErrorKind::UserFixable,
            IpcErrorKind::Retryable,
            IpcErrorKind::Bug,
            IpcErrorKind::NoData,
        ]
        .map(|kind| IpcError { kind }),
    );
}

#[test]
fn version_request_refuses_unknown_fields() {
    assert_eq!(
        serde_json::from_str::<AppVersionRequest>(r#"{"query":"current"}"#).expect("empty request"),
        AppVersionRequest {
            query: AppVersionQuery::Current
        }
    );
    for input in [
        r#"{"query":"current","accountSeq":12}"#,
        r#"{"query":"current","proposal_id":"p"}"#,
        r#"{"query":"current","quantity":"1.25"}"#,
    ] {
        assert!(
            serde_json::from_str::<AppVersionRequest>(input).is_err(),
            "unknown input: {input}"
        );
    }
}

#[test]
fn error_classification_exports_only_the_closed_kind() {
    use application::{
        BugDiagnostic, ErrorClass, MissingData, ResponseMismatch, RetryDelay, UserAction,
    };
    for (class, kind) in [
        (
            ErrorClass::UserAction(UserAction::CheckCredentials),
            "user-fixable",
        ),
        (ErrorClass::Wait(RetryDelay::Unknown), "retryable"),
        (
            ErrorClass::Bug(BugDiagnostic::Response(ResponseMismatch::Schema)),
            "bug",
        ),
        (ErrorClass::NoData(MissingData::Stock), "no-data"),
    ] {
        assert_eq!(
            serde_json::to_value(IpcError::from(class)).expect("error"),
            serde_json::json!({"kind":kind}),
            "classification and redaction"
        );
    }
}
