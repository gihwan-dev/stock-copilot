mod fixture;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use application::OrderConfirmation;
use approval_command::service::OrderApprovalHandler;
use tauri::Manager;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use testkit_ports::ScriptedPresence;
use testkit_ports::memory::MemoryReply;

use crate::ipc::order_approval::OrderApprovalRuntime;

use fixture::{PROPOSAL, fixture};

const LOCAL: &str = "tauri://localhost";
const PRESSES: usize = 3;

fn always_denied() -> Arc<ScriptedPresence> {
    Arc::new(ScriptedPresence::new(
        std::iter::repeat_n(OrderConfirmation::Denied, 16),
        [],
    ))
}

fn assembled(
    presence: &Arc<ScriptedPresence>,
    runtime: Option<OrderApprovalRuntime>,
) -> tauri::App<MockRuntime> {
    let mut ports = super::support::ports();
    ports.presence = Arc::clone(presence);
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
    let builder = match runtime {
        Some(approval) => mock_builder().manage(approval),
        None => mock_builder(),
    };
    crate::build_app(builder, ports, context).expect("연기 시험 조립")
}

fn press_approve(handle: &tauri::AppHandle<MockRuntime>) -> serde_json::Value {
    let window = handle.get_webview_window("main").expect("주 창");
    let request = tauri::webview::InvokeRequest {
        cmd: "request_order_approval".into(),
        callback: tauri::ipc::CallbackFn(21),
        error: tauri::ipc::CallbackFn(23),
        url: LOCAL.parse().expect("출처"),
        body: tauri::ipc::InvokeBody::Json(
            serde_json::json!({"request": {"proposal_id": PROPOSAL}}),
        ),
        headers: tauri::http::HeaderMap::new(),
        invoke_key: tauri::test::INVOKE_KEY.into(),
    };
    tauri::test::get_ipc_response(&window, request)
        .expect("승인 응답")
        .deserialize::<serde_json::Value>()
        .expect("응답 JSON")
}

fn run_smoke<Check>(app: tauri::App<MockRuntime>, check: Check)
where
    Check: Fn(&tauri::AppHandle<MockRuntime>) + 'static,
{
    let checked = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&checked);
    let exits = Arc::new(AtomicUsize::new(0));
    let observed_exits = Arc::clone(&exits);
    let code = app.run_return(move |handle, event| match event {
        tauri::RunEvent::Ready => {
            check(handle);
            let _previous = observed.fetch_add(1, Ordering::SeqCst);
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
            let _previous = observed_exits.fetch_add(1, Ordering::SeqCst);
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
    assert_eq!(code, 0_i32, "연기 시험 앱 종료");
    assert_eq!(checked.load(Ordering::SeqCst), 1, "단언이 실제로 실행됨");
    assert_eq!(exits.load(Ordering::SeqCst), 1, "정리 뒤 종료 요청");
}

#[test]
fn pressing_approve_repeatedly_with_always_denied_presence_sends_zero_orders() {
    let presence = always_denied();
    let path = Arc::new(fixture(Arc::clone(&presence), "BRK-O21-1"));
    let handler: Arc<dyn OrderApprovalHandler> =
        Arc::<fixture::ApprovalServiceOf>::clone(&path.service);
    let app = assembled(&presence, Some(OrderApprovalRuntime::new(handler)));
    let observed_presence = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_smoke(app, move |handle| {
        let responses: Vec<serde_json::Value> =
            (0..PRESSES).map(|_press| press_approve(handle)).collect();
        assert_eq!(
            responses,
            vec![
                serde_json::json!({"status": "rejected", "reason": "presence-denied"}),
                serde_json::json!({"status": "rejected", "reason": "cooldown"}),
                serde_json::json!({"status": "rejected", "reason": "cooldown"}),
            ],
            "첫 승인은 본인 확인 거부, 이어지는 승인은 재요청 제한"
        );
        assert_eq!(
            observed_presence.order_calls().len(),
            1,
            "재요청 제한 중에는 본인 확인을 다시 부르지 않음"
        );
        assert_eq!(observed_path.trade.sent(), vec![], "주문 0건");
        assert_eq!(observed_path.tx.prepared(), vec![], "전송 전 기록 0건");
    });
}

#[test]
fn product_assembly_without_approval_runtime_never_reaches_presence() {
    let presence = always_denied();
    let app = assembled(&presence, None);
    let observed_presence = Arc::clone(&presence);
    run_smoke(app, move |handle| {
        for _press in 0..PRESSES {
            assert_eq!(press_approve(handle)["status"], "unavailable");
        }
        assert_eq!(observed_presence.calls(), vec![], "본인 확인 0회");
    });
}
