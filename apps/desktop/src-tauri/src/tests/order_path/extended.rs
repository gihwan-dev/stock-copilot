use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::task::{Context, Poll, Waker};

use application::{OrderConfirmation, RiskChangeConfirmation, SettingsRepository};
use approval_command::execute::{ApprovalCommandOutcome, ApprovalCommandRejection};
use approval_command::prepare::PrepareRejection;
use domain::{ClientOrderId, OrderPermission, ProposalId};
use scheduler::{
    DisabledAiJobQueue, JobRegistry, NoMarketCalendar, PortfolioSyncJob, Scheduler, SyncDemand,
};
use tauri::test::MockRuntime;
use testkit_ports::ScriptedPresence;
use testkit_ports::memory::MemoryReply;

use super::fixture::{
    AuditMode, GATE_HELD, GATE_OPEN, HourlyState, LedgerMode, Options, PROPOSAL, fixture_with,
};
use super::{LOCAL, approval, response_of, run_with};
use crate::platform::scheduling::AppPortfolioSync;
use crate::tests::support::TestPorts;

fn proposal() -> ProposalId {
    ProposalId::new(ClientOrderId::try_new(PROPOSAL).expect("제안 키"))
}

fn poll_once<F: Future>(future: std::pin::Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

#[test]
fn i_risk_change_without_confirmation_keeps_settings_and_never_authenticates() {
    for refusal in [
        RiskChangeConfirmation::Denied,
        RiskChangeConfirmation::Unavailable,
    ] {
        let presence = Arc::new(ScriptedPresence::new([], [refusal]));
        let path = Arc::new(fixture_with(
            Arc::clone(&presence),
            "BRK-O20-I",
            Options::default(),
        ));
        let observed = Arc::clone(&presence);
        let observed_path = Arc::clone(&path);
        run_with(&presence, &path, move |handle| {
            let ports = tauri::Manager::state::<crate::ports::Ports<TestPorts>>(handle);
            for _ in 0_u8..8 {
                ports.settings.script_profile().push(MemoryReply::Memory);
                ports
                    .settings
                    .script_risk_settings()
                    .push(MemoryReply::Memory);
                ports
                    .settings
                    .script_store_risk_settings()
                    .push(MemoryReply::Memory);
                ports
                    .secrets
                    .script_load()
                    .push(MemoryReply::Return(Ok(None)));
            }
            let reply = response_of(
                handle,
                "set_order_permission",
                LOCAL,
                serde_json::json!({"request": {"permission": "on"}}),
            )
            .expect("스위치 응답");
            assert_ne!(
                reply["kind"],
                serde_json::json!("changed"),
                "(i) 바뀌지 않음"
            );
            let stored =
                tauri::async_runtime::block_on(ports.settings.risk_settings()).expect("설정 조회");
            assert_eq!(
                stored.map(domain::RiskSettings::orders),
                None,
                "(i) 저장된 스위치 없음(꺼짐 유지)"
            );
            assert_eq!(observed.order_calls(), vec![], "(i) macOS 본인 확인 0회");
            assert_eq!(
                observed.risk_change_calls().len(),
                1,
                "(i) 네이티브 확인 창 1회"
            );
            assert_eq!(observed_path.trade.sent(), vec![]);
        });
    }
}

#[test]
fn j_audit_write_failure_sends_nothing() {
    for (audit, ledger, presence_calls) in [
        (AuditMode::Broken, LedgerMode::Atomic, 0_usize),
        (AuditMode::Writable, LedgerMode::WriteFails, 1_usize),
    ] {
        let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
        let path = Arc::new(fixture_with(
            Arc::clone(&presence),
            "BRK-O20-J",
            Options {
                audit,
                ledger,
                ..Options::default()
            },
        ));
        let observed = Arc::clone(&presence);
        let observed_path = Arc::clone(&path);
        run_with(&presence, &path, move |handle| {
            let reply = response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL));
            assert!(reply.is_err(), "(j) 기록 실패는 오류: {reply:?}");
            assert_eq!(observed.order_calls().len(), presence_calls);
            assert_eq!(observed_path.trade.sent(), vec![], "(j) 주문 0건");
        });
    }
}

#[test]
fn k_switch_off_rejects_before_presence() {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
    let path = Arc::new(fixture_with(
        Arc::clone(&presence),
        "BRK-O20-K",
        Options {
            permission: OrderPermission::Disabled,
            ..Options::default()
        },
    ));
    let observed = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_with(&presence, &path, move |handle| {
        assert_eq!(
            response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL)),
            Ok(serde_json::json!({"status": "rejected", "reason": "switch-off"})),
        );
        assert_eq!(observed.calls(), vec![], "(k) 본인 확인 0회");
        assert_eq!(observed_path.trade.sent(), vec![], "(k) 주문 0건");
    });
}

#[test]
fn l_hourly_limit_rejects_before_presence() {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
    let path = Arc::new(fixture_with(
        Arc::clone(&presence),
        "BRK-O20-L",
        Options {
            hourly_full: HourlyState::Full,
            ..Options::default()
        },
    ));
    let observed = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_with(&presence, &path, move |handle| {
        assert_eq!(
            response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL)),
            Ok(serde_json::json!({"status": "rejected", "reason": "hourly-limit"})),
        );
        assert_eq!(observed.calls(), vec![], "(l) 본인 확인 0회");
        assert_eq!(observed_path.trade.sent(), vec![], "(l) 주문 0건");
    });
}

#[test]
fn m_price_move_during_dialog_sends_nothing() {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
    let path = Arc::new(fixture_with(
        Arc::clone(&presence),
        "BRK-O20-M",
        Options {
            post_presence_price: "44000",
            ..Options::default()
        },
    ));
    let observed = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_with(&presence, &path, move |handle| {
        assert_eq!(
            response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL)),
            Ok(serde_json::json!({"status": "rejected", "reason": "conditions-changed"})),
        );
        assert_eq!(observed.order_calls().len(), 1, "(m) 확인 창은 떴음");
        assert_eq!(observed_path.trade.sent(), vec![], "(m) 주문 0건");
        assert_eq!(observed_path.tx.prepared(), vec![], "(m) 소비 0건");
    });
}

#[test]
fn n_second_request_while_dialog_is_held_is_rejected() {
    let gate = Arc::new(AtomicU8::new(GATE_HELD));
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
    let path = fixture_with(
        Arc::clone(&presence),
        "BRK-O20-N",
        Options {
            gate: Arc::clone(&gate),
            ..Options::default()
        },
    );
    let id = proposal();
    let mut first = pin!(path.service.request(&id));
    assert!(
        poll_once(first.as_mut()).is_pending(),
        "첫 요청이 창 슬롯을 잡고 대기"
    );
    let second = tauri::async_runtime::block_on(path.service.request(&id));
    assert_eq!(
        second,
        Ok(ApprovalCommandOutcome::Rejected(
            ApprovalCommandRejection::Prepare(PrepareRejection::DialogInProgress)
        )),
        "(n) 두 번째 요청 거부"
    );
    assert_eq!(
        presence.order_calls().len(),
        0,
        "(n) 두 번째 요청은 창을 띄우지 않음"
    );
    gate.store(GATE_OPEN, Ordering::SeqCst);
    let completed = match poll_once(first.as_mut()) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("첫 요청 완료"),
    };
    match completed {
        Ok(ApprovalCommandOutcome::Submitted { .. }) => {}
        other @ (Ok(
            ApprovalCommandOutcome::Rejected(_)
            | ApprovalCommandOutcome::BrokerRejected { .. }
            | ApprovalCommandOutcome::OutcomeUnknown { .. },
        )
        | Err(_)) => panic!("첫 요청은 정상 처리: {other:?}"),
    }
    assert_eq!(presence.order_calls().len(), 1, "(n) 확인 창 1개");
    assert_eq!(path.trade.sent().len(), 1);
}

#[test]
fn e_every_registered_scheduler_job_runs_without_presence_or_orders() {
    let presence = Arc::new(ScriptedPresence::new([], []));
    let path = Arc::new(fixture_with(
        Arc::clone(&presence),
        "BRK-O20-E",
        Options::default(),
    ));
    let observed = Arc::clone(&presence);
    let observed_path = Arc::clone(&path);
    run_with(
        &presence,
        &path,
        move |handle: &tauri::AppHandle<MockRuntime>| {
            let ports = tauri::Manager::state::<crate::ports::Ports<TestPorts>>(handle);
            let mut registry = JobRegistry::new();
            registry
                .register(PortfolioSyncJob::new(
                    Arc::new(AppPortfolioSync::<MockRuntime, TestPorts>::new(
                        handle.clone(),
                    )),
                    Arc::new(NoMarketCalendar),
                    Arc::new(SyncDemand::new()),
                ))
                .expect("제품 등록 작업");
            let ids: Vec<_> = registry.list().into_iter().map(|meta| meta.id).collect();
            assert!(!ids.is_empty(), "등록된 작업이 있음");
            let scheduler = Scheduler::new(
                Arc::clone(&ports.clock),
                Arc::new(DisabledAiJobQueue),
                registry,
            )
            .expect("스케줄러");
            for id in ids {
                scheduler.triggers().trigger(id);
            }
            let _report = tauri::async_runtime::block_on(scheduler.tick_once());
            assert_eq!(observed.calls(), vec![], "(e) 본인 확인 0회");
            assert_eq!(observed_path.trade.sent(), vec![], "(e) 주문 0건");
        },
    );
}
