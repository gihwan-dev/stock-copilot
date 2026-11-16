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
