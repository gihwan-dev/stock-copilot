use std::sync::mpsc;
use std::time::Duration;

use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use tauri::{Listener, Manager};
use tauri_specta::Event;
use testkit_ports::memory::MemoryReply;

use crate::ipc::event::UiEvent;

fn assembled_app() -> tauri::App<MockRuntime> {
    let ports = super::support::ports();
    ports
        .settings
        .script_profile()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .settings
        .script_risk_settings()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .secrets
        .script_load()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .broker_read
        .script_accounts()
        .push(Ok(domain::BrokerAccountRows::try_new(vec![]).expect("계좌")));
    let mut context = mock_context(noop_assets());
    context
        .config_mut()
        .app
        .windows
        .push(tauri::utils::config::WindowConfig::default());
    crate::build_app(mock_builder(), ports, context).expect("제품 조립")
}

fn assert_wire_event(handle: &tauri::AppHandle<MockRuntime>) {
    let (sender, receiver) = mpsc::channel();
    let listener = handle.listen_any(UiEvent::NAME, move |event| {
        sender
            .send(event.payload().to_owned())
            .expect("이벤트 관측");
    });
    UiEvent::AccountSelectionChanged
        .emit(handle)
        .expect("등록된 이벤트 발행");
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("이벤트 수신"),
        r#""AccountSelectionChanged""#,
        "제품 조립이 이벤트를 등록하고 종류만 전달한다"
    );
    handle.unlisten(listener);
}

#[test]
fn product_assembly_mounts_the_event_and_emits_only_its_wire_payload() {
    let code = assembled_app().run_return(|handle, event| match event {
        tauri::RunEvent::Ready => {
            assert_wire_event(handle);
            tauri::async_runtime::block_on(
                handle.state::<crate::lifecycle::Lifecycle>().shutdown(),
            );
            handle
                .get_webview_window("main")
                .expect("주 창")
                .close()
                .expect("창 닫기");
        }
        tauri::RunEvent::ExitRequested { .. } => {
            assert!(
                handle
                    .state::<crate::lifecycle::Lifecycle>()
                    .outcome()
                    .try_lock()
                    .expect("종료 기록")
                    .is_some(),
                "이벤트 관측 뒤 정리한 앱을 종료한다"
            );
        }
        tauri::RunEvent::Exit
        | tauri::RunEvent::WindowEvent { .. }
        | tauri::RunEvent::WebviewEvent { .. }
        | tauri::RunEvent::Resumed
        | tauri::RunEvent::MainEventsCleared
        | tauri::RunEvent::Opened { .. }
        | tauri::RunEvent::MenuEvent(_)
        | tauri::RunEvent::Reopen { .. }
        | _ => {}
    });
    assert_eq!(code, 0_i32, "제품 조립 종료");
}

fn version_request(body: serde_json::Value) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: "app_version".into(),
        callback: tauri::ipc::CallbackFn(41),
        error: tauri::ipc::CallbackFn(59),
        url: "tauri://localhost".parse().expect("로컬 출처"),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: tauri::http::HeaderMap::new(),
        invoke_key: tauri::test::INVOKE_KEY.into(),
    }
}

fn assert_command_input(handle: &tauri::AppHandle<MockRuntime>) {
    let window = handle.get_webview_window("main").expect("주 창");
    let response = tauri::test::get_ipc_response(
        &window,
        version_request(serde_json::json!({"request":{"query":"current"}})),
    )
    .expect("생성 바인딩 인자 성공");
    assert_eq!(
        response
            .deserialize::<serde_json::Value>()
            .expect("명령 응답 JSON"),
        serde_json::json!({"version":"0.1.0"}),
        "실제 IPC가 생성 바인딩의 인자 모양을 받는다"
    );
    let rejected = tauri::test::get_ipc_response(
        &window,
        version_request(serde_json::json!({"request":{"query":"current","extra":"probe"}})),
    )
    .expect_err("DTO 내부 추가 필드 거부");
    assert!(
        rejected
            .as_str()
            .expect("역직렬화 오류")
            .contains("unknown field `extra`"),
        "권한 실패가 아니라 DTO의 추가 필드 거부다"
    );
}

#[test]
fn product_command_accepts_binding_arguments_and_rejects_unknown_dto_fields() {
    let code = assembled_app().run_return(|handle, event| match event {
        tauri::RunEvent::Ready => {
            assert_command_input(handle);
            tauri::async_runtime::block_on(
                handle.state::<crate::lifecycle::Lifecycle>().shutdown(),
            );
            handle
                .get_webview_window("main")
                .expect("주 창")
                .close()
                .expect("창 닫기");
        }
        tauri::RunEvent::ExitRequested { .. } => {
            assert!(
                handle
                    .state::<crate::lifecycle::Lifecycle>()
                    .outcome()
                    .try_lock()
                    .expect("종료 기록")
                    .is_some(),
                "명령 검증 뒤 정리한 앱을 종료한다"
            );
        }
        tauri::RunEvent::Exit
        | tauri::RunEvent::WindowEvent { .. }
        | tauri::RunEvent::WebviewEvent { .. }
        | tauri::RunEvent::Resumed
        | tauri::RunEvent::MainEventsCleared
        | tauri::RunEvent::Opened { .. }
        | tauri::RunEvent::MenuEvent(_)
        | tauri::RunEvent::Reopen { .. }
        | _ => {}
    });
    assert_eq!(code, 0_i32, "제품 명령 검증 종료");
}

fn open_external_invoke(body: serde_json::Value) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: "open_external".into(),
        callback: tauri::ipc::CallbackFn(61),
        error: tauri::ipc::CallbackFn(79),
        url: "tauri://localhost".parse().expect("로컬 출처"),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: tauri::http::HeaderMap::new(),
        invoke_key: tauri::test::INVOKE_KEY.into(),
    }
}

fn assembled_app_with_external_runtime(
    runtime: crate::ipc::open_external::ExternalLinkRuntime,
) -> tauri::App<MockRuntime> {
    let ports = super::support::ports();
    ports
        .settings
        .script_profile()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .settings
        .script_risk_settings()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .secrets
        .script_load()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .broker_read
        .script_accounts()
        .push(Ok(domain::BrokerAccountRows::try_new(vec![]).expect("계좌")));
    let mut context = mock_context(noop_assets());
    context
        .config_mut()
        .app
        .windows
        .push(tauri::utils::config::WindowConfig::default());
    crate::build_app(mock_builder().manage(runtime), ports, context).expect("제품 조립")
}

fn assert_open_external_ipc_flow(
    handle: &tauri::AppHandle<MockRuntime>,
    catalog: &testkit_ports::system_fakes::MemoryCollectedUrlCatalog,
    opener: &testkit_ports::system_fakes::ScriptedExternalUrlOpener,
) {
    let window = handle.get_webview_window("main").expect("주 창");
    let fixed = "https://claude.ai/settings/data-privacy-controls";
    opener
        .script_open()
        .push(Ok(application::ExternalUrlOpened));
    let fixed_res = tauri::test::get_ipc_response(
        &window,
        open_external_invoke(serde_json::json!({"request":{"url":fixed}})),
    )
    .expect("고정 주소 IPC 성공");
    assert_eq!(
        fixed_res
            .deserialize::<serde_json::Value>()
            .expect("응답 JSON"),
        serde_json::json!({"status":"opened"})
    );

    let collected = "https://www.mk.co.kr/news/stock/12205485";
    catalog.insert(domain::NormalizedUrl::try_new(collected).expect("수집 주소"));
    catalog.script_find_collected().push(MemoryReply::Memory);
    opener
        .script_open()
        .push(Ok(application::ExternalUrlOpened));
    let collected_res = tauri::test::get_ipc_response(
        &window,
        open_external_invoke(serde_json::json!({"request":{"url":collected}})),
    )
    .expect("수집 주소 IPC 성공");
    assert_eq!(
        collected_res
            .deserialize::<serde_json::Value>()
            .expect("수집 응답 JSON"),
        serde_json::json!({"status":"opened"})
    );
    assert_open_external_rejections(handle, fixed, collected, catalog, opener);
}

fn assert_open_external_rejections(
    handle: &tauri::AppHandle<MockRuntime>,
    fixed: &str,
    collected: &str,
    catalog: &testkit_ports::system_fakes::MemoryCollectedUrlCatalog,
    opener: &testkit_ports::system_fakes::ScriptedExternalUrlOpener,
) {
    let window = handle.get_webview_window("main").expect("주 창");
    let unknown_field = tauri::test::get_ipc_response(
        &window,
        open_external_invoke(serde_json::json!({"request":{"url":fixed,"extra":"x"}})),
    )
    .expect_err("알 수 없는 필드 거부");
    assert!(
        unknown_field
            .as_str()
            .expect("역직렬화 오류")
            .contains("unknown field `extra`")
    );

    catalog.script_find_collected().push(MemoryReply::Memory);
    for denied in [
        "https://evil.com/phish",
        "http://claude.ai/settings/data-privacy-controls",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "https://user:pass@claude.ai/settings/data-privacy-controls",
        "https://claude.ai./settings/data-privacy-controls",
        "https://xn--clade-xsa.ai/settings/data-privacy-controls",
        "https://claude.ai/redirect?url=https://evil.com",
    ] {
        let err = tauri::test::get_ipc_response(
            &window,
            open_external_invoke(serde_json::json!({"request":{"url":denied}})),
        )
        .expect_err("비허용 주소 거부");
        assert_eq!(err, serde_json::json!({"kind":"bug"}), "{denied}");
    }
    assert_eq!(
        opener.opened_urls(),
        vec![String::from(fixed), String::from(collected)]
    );
}

#[test]
fn open_external_ipc_enforces_allowlist_and_dto_contract_through_build_app() {
    let catalog =
        std::sync::Arc::new(testkit_ports::system_fakes::MemoryCollectedUrlCatalog::default());
    let opener =
        std::sync::Arc::new(testkit_ports::system_fakes::ScriptedExternalUrlOpener::default());
    let runtime = crate::ipc::open_external::ExternalLinkRuntime::new(
        std::sync::Arc::clone(&catalog),
        std::sync::Arc::clone(&opener),
    );
    let code =
        assembled_app_with_external_runtime(runtime).run_return(move |handle, event| match event {
            tauri::RunEvent::Ready => {
                assert_open_external_ipc_flow(handle, &catalog, &opener);
                tauri::async_runtime::block_on(
                    handle.state::<crate::lifecycle::Lifecycle>().shutdown(),
                );
                handle
                    .get_webview_window("main")
                    .expect("주 창")
                    .close()
                    .expect("창 닫기");
            }
            tauri::RunEvent::ExitRequested { .. } => {
                assert!(
                    handle
                        .state::<crate::lifecycle::Lifecycle>()
                        .outcome()
                        .try_lock()
                        .expect("종료 기록")
                        .is_some()
                );
            }
            tauri::RunEvent::Exit
            | tauri::RunEvent::WindowEvent { .. }
            | tauri::RunEvent::WebviewEvent { .. }
            | tauri::RunEvent::Resumed
            | tauri::RunEvent::MainEventsCleared
            | tauri::RunEvent::Opened { .. }
            | tauri::RunEvent::MenuEvent(_)
            | tauri::RunEvent::Reopen { .. }
            | _ => {}
        });
    assert_eq!(code, 0_i32);
}

#[test]
fn open_external_default_product_assembly_fails_closed_without_opening() {
    let code = assembled_app().run_return(|handle, event| match event {
        tauri::RunEvent::Ready => {
            let window = handle.get_webview_window("main").expect("주 창");
            let fixed_err = tauri::test::get_ipc_response(
                &window,
                open_external_invoke(serde_json::json!({
                    "request": {"url": "https://claude.ai/settings/data-privacy-controls"}
                })),
            )
            .expect_err("기본 제품 조립은 닫힌 실패");
            assert_eq!(fixed_err, serde_json::json!({"kind":"retryable"}));

            let denied_err = tauri::test::get_ipc_response(
                &window,
                open_external_invoke(serde_json::json!({
                    "request": {"url": "http://example.com/a"}
                })),
            )
            .expect_err("비-https 즉시 거부");
            assert_eq!(denied_err, serde_json::json!({"kind":"bug"}));

            tauri::async_runtime::block_on(
                handle.state::<crate::lifecycle::Lifecycle>().shutdown(),
            );
            handle
                .get_webview_window("main")
                .expect("주 창")
                .close()
                .expect("창 닫기");
        }
        tauri::RunEvent::ExitRequested { .. } => {
            assert!(
                handle
                    .state::<crate::lifecycle::Lifecycle>()
                    .outcome()
                    .try_lock()
                    .expect("종료 기록")
                    .is_some()
            );
        }
        tauri::RunEvent::Exit
        | tauri::RunEvent::WindowEvent { .. }
        | tauri::RunEvent::WebviewEvent { .. }
        | tauri::RunEvent::Resumed
        | tauri::RunEvent::MainEventsCleared
        | tauri::RunEvent::Opened { .. }
        | tauri::RunEvent::MenuEvent(_)
        | tauri::RunEvent::Reopen { .. }
        | _ => {}
    });
    assert_eq!(code, 0_i32);
}
