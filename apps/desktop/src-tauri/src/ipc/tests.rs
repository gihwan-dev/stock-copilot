use std::fs;
use std::path::Path;

use serde::Serialize;
use tauri::test::{mock_builder, mock_context, noop_assets};

use super::app_version::{AppVersionQuery, AppVersionRequest, app_version};
use super::error::{IpcError, IpcErrorKind};
use super::settings::{
    GetSettingsRequest, SetOrderPermissionRequest, SettingsQueryScope, UiOrderPermission,
    UiOrderSwitchOutcomeKind, apply_order_permission, load_settings_view,
};
use super::state::ScreenState;

const RESPONSE_DIRECTORY: &str = "ipc/responses";

pub(crate) fn assert_response_snapshot<T: Serialize>(name: &str, value: &T) {
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

fn script_empty_secret_loads(ports: &crate::ports::Ports<crate::tests::support::TestPorts>) {
    for _ in 0_u8..3_u8 {
        ports
            .secrets
            .script_load()
            .push(testkit_ports::memory::MemoryReply::Return(Ok(None)));
    }
}

#[test]
fn settings_responses_match_snapshots_and_order_switch_requires_confirmation() {
    let ports = crate::tests::support::ports();
    ports
        .settings
        .script_profile()
        .push(testkit_ports::memory::MemoryReply::Return(Ok(None)));
    ports
        .settings
        .script_risk_settings()
        .push(testkit_ports::memory::MemoryReply::Return(Ok(None)));
    script_empty_secret_loads(&ports);
    let state = ScreenState::from_ports(&ports);
    let view = tauri::async_runtime::block_on(load_settings_view(&state)).expect("기본 설정");
    assert_response_snapshot("get_settings", &view);

    let base_ports = crate::tests::support::ports();
    let denied_ports = crate::ports::Ports {
        presence: std::sync::Arc::new(testkit_ports::ScriptedPresence::new(
            [],
            [application::RiskChangeConfirmation::Denied],
        )),
        ..base_ports
    };
    for _ in 0_u8..2_u8 {
        denied_ports
            .settings
            .script_profile()
            .push(testkit_ports::memory::MemoryReply::Return(Ok(None)));
        denied_ports
            .settings
            .script_risk_settings()
            .push(testkit_ports::memory::MemoryReply::Return(Ok(None)));
    }
    script_empty_secret_loads(&denied_ports);
    let denied_state = ScreenState::from_ports(&denied_ports);
    let outcome = tauri::async_runtime::block_on(apply_order_permission(
        &denied_state,
        UiOrderPermission::On,
    ))
    .expect("거부 응답");
    assert_eq!(outcome.kind, UiOrderSwitchOutcomeKind::Denied);
    assert_eq!(outcome.order_enabled, UiOrderPermission::Off);
    assert_response_snapshot("set_order_permission", &outcome);

    assert_eq!(
        serde_json::from_str::<GetSettingsRequest>(r#"{"scope":"all"}"#).expect("설정 요청"),
        GetSettingsRequest {
            scope: SettingsQueryScope::All,
        }
    );
    let _extra = serde_json::from_str::<GetSettingsRequest>(r#"{"scope":"all","extra":1}"#)
        .expect_err("모르는 필드 거부");
    let _bypass =
        serde_json::from_str::<SetOrderPermissionRequest>(r#"{"permission":"on","bypass":true}"#)
            .expect_err("우회 필드 거부");
}
