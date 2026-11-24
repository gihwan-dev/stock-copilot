use crate::lifecycle::Lifecycle;
use crate::platform::background::{BackgroundRunState, BackgroundRuntime};
use scheduler::{BridgeBinding, SchedulerEventBridge};
use std::sync::Arc;
use tauri::Manager;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use testkit_ports::memory::MemoryReply;

fn app_with(builder: tauri::Builder<MockRuntime>) -> tauri::App<MockRuntime> {
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
    crate::build_app(builder, ports, context).expect("제품 조립")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EventPhase {
    Ready,
    Other,
}

fn phase(event: &tauri::RunEvent) -> EventPhase {
    match event {
        tauri::RunEvent::Ready => EventPhase::Ready,
        tauri::RunEvent::ExitRequested { .. }
        | tauri::RunEvent::Exit
        | tauri::RunEvent::WindowEvent { .. }
        | tauri::RunEvent::WebviewEvent { .. }
        | tauri::RunEvent::Resumed
        | tauri::RunEvent::MainEventsCleared
        | tauri::RunEvent::Opened { .. }
        | tauri::RunEvent::MenuEvent(_)
        | tauri::RunEvent::Reopen { .. }
        | _ => EventPhase::Other,
    }
}

fn assert_started_and_stopped_by_shutdown(
    handle: &tauri::AppHandle<MockRuntime>,
    bridge: &SchedulerEventBridge,
) {
    let runtime = handle
        .try_state::<Arc<BackgroundRuntime>>()
        .map(|state| Arc::clone(&state))
        .expect("백그라운드 실행");
    assert_eq!(runtime.state(), BackgroundRunState::Running);
    assert_eq!(bridge.binding(), BridgeBinding::Bound);
    tauri::async_runtime::block_on(handle.state::<Lifecycle>().shutdown());
    assert_eq!(runtime.state(), BackgroundRunState::Stopped);
    handle
        .get_webview_window("main")
        .expect("주 창")
        .close()
        .expect("창 닫기");
}

#[test]
fn build_app_starts_scheduler_and_shutdown_stops_it_when_configured() {
    let bridge = Arc::new(SchedulerEventBridge::new());
    let checked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = Arc::clone(&checked);
    let observed = Arc::clone(&bridge);
    let _code =
        app_with(mock_builder().manage(Arc::clone(&bridge))).run_return(move |handle, event| {
            if phase(&event) == EventPhase::Ready {
                assert_started_and_stopped_by_shutdown(handle, &observed);
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });
    assert_eq!(checked.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn build_app_without_bridge_does_not_start_scheduler() {
    let checked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = Arc::clone(&checked);
    let _code = app_with(mock_builder()).run_return(move |handle, event| {
        if phase(&event) == EventPhase::Ready {
            assert!(handle.try_state::<Arc<BackgroundRuntime>>().is_none());
            seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            handle
                .get_webview_window("main")
                .expect("주 창")
                .close()
                .expect("창 닫기");
        }
    });
    assert_eq!(checked.load(std::sync::atomic::Ordering::SeqCst), 1);
}
