mod fixture;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use application::OrderConfirmation;
use approval_command::service::OrderApprovalHandler;
use tauri::Manager;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use testkit_ports::memory::MemoryReply;
use testkit_ports::{RecordedOrderSummary, ScriptedPresence};
use trade_fakes::SentOrderSummary;

use crate::ipc::order_approval::OrderApprovalRuntime;

use fixture::{PROPOSAL, PathFixture, fixture};

fn assembled(
    presence: &Arc<ScriptedPresence>,
    runtime: OrderApprovalRuntime,
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
    crate::build_app(mock_builder().manage(runtime), ports, context).expect("제품 조립")
}

fn invoke(cmd: &str, origin: &str, body: serde_json::Value) -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: cmd.into(),
        callback: tauri::ipc::CallbackFn(13),
        error: tauri::ipc::CallbackFn(17),
        url: origin.parse().expect("출처"),
        body: tauri::ipc::InvokeBody::Json(body),
        headers: tauri::http::HeaderMap::new(),
        invoke_key: tauri::test::INVOKE_KEY.into(),
    }
}

const LOCAL: &str = "tauri://localhost";

fn approval(proposal_id: &str) -> serde_json::Value {
    serde_json::json!({"request": {"proposal_id": proposal_id}})
}

fn run_with<Check>(presence: &Arc<ScriptedPresence>, path: &Arc<PathFixture>, check: Check)
where
    Check: Fn(&tauri::AppHandle<MockRuntime>) + 'static,
{
    let handler: Arc<dyn OrderApprovalHandler> =
        Arc::<fixture::ApprovalServiceOf>::clone(&path.service);
    let app = assembled(presence, OrderApprovalRuntime::new(handler));
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
    assert_eq!(code, 0_i32, "경로 고정 앱 종료");
    assert_eq!(checked.load(Ordering::SeqCst), 1, "단언이 실제로 실행됨");
    assert_eq!(exits.load(Ordering::SeqCst), 1, "정리 뒤 종료 요청");
}

fn response_of(
    handle: &tauri::AppHandle<MockRuntime>,
    cmd: &str,
    origin: &str,
    body: serde_json::Value,
) -> Result<serde_json::Value, serde_json::Value> {
    let window = handle.get_webview_window("main").expect("주 창");
    tauri::test::get_ipc_response(&window, invoke(cmd, origin, body)).map(|response| {
        response
            .deserialize::<serde_json::Value>()
            .expect("응답 JSON")
    })
}

fn sent_matches_shown(shown: &RecordedOrderSummary, sent: &SentOrderSummary) -> bool {
    shown.instrument_name == sent.instrument_name
        && shown.symbol == sent.symbol
        && shown.market == sent.market
        && shown.side == sent.side
        && shown.quantity == sent.quantity
        && shown.limit_price == sent.limit_price
        && shown.expected_amount == sent.expected_amount
        && shown.expected_fee == sent.expected_fee
        && shown.account_suffix == sent.account_suffix
        && shown.client_order_prefix == sent.client_order_prefix
}

#[test]
fn f_and_g_confirmed_presence_sends_exactly_the_shown_order_once() {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
    let path = Arc::new(fixture(Arc::clone(&presence), "BRK-O13-1"));
    let observed_presence = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_with(&presence, &path, move |handle| {
        assert_eq!(
            response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL)),
            Ok(serde_json::json!({"status": "submitted", "reason": "none"})),
            "(f) 본인 확인 통과는 전송"
        );
        assert_eq!(
            response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL)),
            Ok(serde_json::json!({"status": "rejected", "reason": "already-consumed"})),
            "(g) 소비된 제안 재요청 거부"
        );
        let sent = observed_path.trade.sent();
        let shown = observed_presence.order_calls();
        assert_eq!(sent.len(), 1, "(f) 정확히 1건");
        assert_eq!(shown.len(), 1, "(g) 재요청에 본인 확인 0회");
        assert!(
            sent_matches_shown(&shown[0], &sent[0].summary),
            "(f) 보낸 주문 = 확인 창 요약"
        );
        assert_eq!(
            sent[0].client_order_id.as_str(),
            PROPOSAL,
            "(f) 코어가 정한 결정적 키"
        );
        assert_eq!(observed_path.tx.prepared().len(), 1, "전송 전 기록 1건");
        assert_eq!(
            observed_path.tx.prepared()[0].terms,
            *sent[0].intent.terms(),
            "(f) 보낸 주문 = 코어 계산값"
        );
    });
}

#[test]
fn a_denied_presence_sends_nothing() {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Denied], []));
    let path = Arc::new(fixture(Arc::clone(&presence), "BRK-O13-2"));
    let observed_presence = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_with(&presence, &path, move |handle| {
        assert_eq!(
            response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL)),
            Ok(serde_json::json!({"status": "rejected", "reason": "presence-denied"})),
        );
        assert_eq!(observed_presence.order_calls().len(), 1);
        assert_eq!(observed_path.trade.sent(), vec![], "(a) 주문 0건");
    });
}

#[test]
fn b_extra_fields_are_rejected_before_presence() {
    let presence = Arc::new(ScriptedPresence::new([], []));
    let path = Arc::new(fixture(Arc::clone(&presence), "BRK-O13-3"));
    let observed_presence = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_with(&presence, &path, move |handle| {
        for extra in [
            serde_json::json!({"request": {"proposal_id": PROPOSAL, "quantity": "100"}}),
            serde_json::json!({"request": {"proposal_id": PROPOSAL, "price": "1"}}),
            serde_json::json!({"request": {"proposal_id": PROPOSAL, "approved": true}}),
        ] {
            let rejected = response_of(handle, "request_order_approval", LOCAL, extra)
                .expect_err("추가 필드 거부");
            assert!(
                rejected
                    .as_str()
                    .is_some_and(|text| text.contains("unknown field")),
                "(b) 역직렬화 거부: {rejected}"
            );
        }
        assert_eq!(observed_presence.calls(), vec![], "(b) 본인 확인 0회");
        assert_eq!(observed_path.trade.sent(), vec![], "(b) 주문 0건");
    });
}

#[test]
fn c_every_other_registered_command_never_reaches_presence_or_broker() {
    let presence = Arc::new(ScriptedPresence::new([], []));
    let path = Arc::new(fixture(Arc::clone(&presence), "BRK-O13-4"));
    let observed_presence = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_with(&presence, &path, move |handle| {
        let listed = std::fs::read_to_string("ipc/commands.txt").expect("생성된 명령 목록");
        assert!(listed.lines().any(|name| name == "request_order_approval"));
        for name in listed
            .lines()
            .filter(|name| !name.is_empty() && *name != "request_order_approval")
        {
            for body in [
                serde_json::json!({}),
                serde_json::json!({"request": {}}),
                approval(PROPOSAL),
            ] {
                let _ignored = response_of(handle, name, LOCAL, body);
            }
        }
        assert_eq!(observed_presence.calls(), vec![], "(c) 본인 확인 0회");
        assert_eq!(observed_path.trade.sent(), vec![], "(c) 주문 0건");
    });
}

#[test]
fn h_remote_origin_cannot_request_approval() {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
    let path = Arc::new(fixture(Arc::clone(&presence), "BRK-O13-5"));
    let observed_presence = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_with(&presence, &path, move |handle| {
        for origin in ["https://evil.example", "http://tauri.localhost"] {
            let rejected =
                response_of(handle, "request_order_approval", origin, approval(PROPOSAL));
            assert!(rejected.is_err(), "(h) 원격 출처 거부: {origin}");
        }
        assert_eq!(observed_presence.calls(), vec![], "(h) 본인 확인 0회");
        assert_eq!(observed_path.trade.sent(), vec![], "(h) 주문 0건");
    });
}
