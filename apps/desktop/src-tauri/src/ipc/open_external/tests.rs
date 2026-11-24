use std::fs;
use std::path::Path;
use std::sync::Arc;

use application::{ExternalUrlOpened, ExternalUrlOpenerError};
use domain::NormalizedUrl;
use tauri::Manager;
use tauri::test::{mock_builder, mock_context, noop_assets};
use testkit_ports::memory::MemoryReply;
use testkit_ports::system_fakes::{MemoryCollectedUrlCatalog, ScriptedExternalUrlOpener};

use crate::ipc::error::{IpcError, IpcErrorKind};

use super::{
    ExternalLinkRuntime, OpenExternalRequest, OpenExternalResponse, OpenExternalStatus,
    open_external,
};

#[test]
fn open_external_response_is_recorded_for_the_typescript_fake() {
    let catalog = Arc::new(MemoryCollectedUrlCatalog::default());
    let opener = Arc::new(ScriptedExternalUrlOpener::default());
    opener.script_open().push(Ok(ExternalUrlOpened));
    let app = mock_builder()
        .manage(ExternalLinkRuntime::new(catalog, Arc::clone(&opener)))
        .build(mock_context(noop_assets()))
        .expect("mock app");

    let response = tauri::async_runtime::block_on(open_external(
        app.handle().clone(),
        OpenExternalRequest {
            url: String::from("https://claude.ai/settings/data-privacy-controls"),
        },
    ))
    .expect("고정 주소 열기");
    assert_eq!(
        response,
        OpenExternalResponse {
            status: OpenExternalStatus::Opened,
        }
    );
    assert_eq!(
        opener.opened_urls(),
        vec![String::from(
            "https://claude.ai/settings/data-privacy-controls"
        )]
    );

    let path = Path::new("ipc/responses/open_external.json");
    let mut actual = serde_json::to_string_pretty(&response).expect("직렬화");
    actual.push('\n');
    let recorded = fs::read_to_string(path).expect("기록된 응답");
    assert_eq!(recorded, actual, "응답 스냅샷 불일치");
}

#[test]
fn open_external_request_refuses_unknown_fields() {
    assert_eq!(
        serde_json::from_str::<OpenExternalRequest>(
            r#"{"url":"https://claude.ai/settings/data-privacy-controls"}"#
        )
        .expect("정상 요청"),
        OpenExternalRequest {
            url: String::from("https://claude.ai/settings/data-privacy-controls"),
        }
    );
    for input in [
        r#"{"url":"https://claude.ai/settings/data-privacy-controls","extra":"probe"}"#,
        r#"{"url":"https://claude.ai/settings/data-privacy-controls","allow":true}"#,
        "{}",
    ] {
        assert!(
            serde_json::from_str::<OpenExternalRequest>(input).is_err(),
            "거부돼야 할 입력: {input}"
        );
    }
}

#[test]
fn open_external_command_enforces_allowlist_and_fails_closed_without_state() {
    let bare_app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("빈 앱");
    let unavailable = tauri::async_runtime::block_on(open_external(
        bare_app.handle().clone(),
        OpenExternalRequest {
            url: String::from("https://claude.ai/settings/data-privacy-controls"),
        },
    ))
    .expect_err("상태 미등록 닫힌 실패");
    assert_eq!(
        unavailable,
        IpcError {
            kind: IpcErrorKind::Retryable
        }
    );
    let bad_scheme = tauri::async_runtime::block_on(open_external(
        bare_app.handle().clone(),
        OpenExternalRequest {
            url: String::from("javascript:alert(1)"),
        },
    ))
    .expect_err("위험 스킴 거부");
    assert_eq!(
        bad_scheme,
        IpcError {
            kind: IpcErrorKind::Bug
        }
    );
}

#[test]
fn open_external_command_opens_collected_url_and_maps_opener_errors() {
    let catalog = Arc::new(MemoryCollectedUrlCatalog::default());
    let opener = Arc::new(ScriptedExternalUrlOpener::default());
    let news = "https://www.mk.co.kr/news/stock/12205485";
    catalog.insert(NormalizedUrl::try_new(news).expect("뉴스 주소"));
    catalog.script_find_collected().push(MemoryReply::Memory);
    opener.script_open().push(Ok(ExternalUrlOpened));
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("앱");
    app.manage(ExternalLinkRuntime::new(
        Arc::clone(&catalog),
        Arc::clone(&opener),
    ));
    let runtime_debug = format!("{:?}", app.state::<ExternalLinkRuntime>().inner());
    assert_eq!(runtime_debug, "ExternalLinkRuntime(<redacted>)");

    let ok = tauri::async_runtime::block_on(open_external(
        app.handle().clone(),
        OpenExternalRequest {
            url: String::from(news),
        },
    ))
    .expect("수집 뉴스 열기");
    assert_eq!(ok.status, OpenExternalStatus::Opened);
    assert_eq!(opener.opened_urls(), vec![String::from(news)]);

    opener
        .script_open()
        .push(Err(ExternalUrlOpenerError::PermissionDenied));
    let perm_err = tauri::async_runtime::block_on(open_external(
        app.handle().clone(),
        OpenExternalRequest {
            url: String::from("https://claude.ai/settings/data-privacy-controls"),
        },
    ))
    .expect_err("권한 오류");
    assert_eq!(
        perm_err,
        IpcError {
            kind: IpcErrorKind::UserFixable
        }
    );
}
