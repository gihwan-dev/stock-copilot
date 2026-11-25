use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use application::{
    AuditEntry, AuditKind, AuditLog, ConsumptionOutcome, EventBusBuilder, EventSubscriber,
    OrderConfirmation, ProposalConsumptionRepository, SubscriberFailure, SystemPortError,
};
use approval_command::service::{
    ApprovalService, ApprovalServiceParts, ApprovalServiceTypes, OrderApprovalHandler,
};
use domain::{
    BrokerOrderId, ClientOrderId, DomainEvent, DomainEventKind, OrderPermission, ProposalId,
    Timestamp,
};
use tauri::Manager;
use testkit_ports::proposal_memory::{
    MemoryProposalConsumptionRepository, MemoryValidationReportRepository,
};
use testkit_ports::scripted::{ScriptedBrokerRead, ScriptedMarketInfo};
use testkit_ports::system_fakes::MemoryAuditLog;
use testkit_ports::{ScriptedPresence, SteppedClock};
use trade_fakes::{BrokerReply, RecordingBroker};

use super::fixture::{
    AuditMode, FixedContexts, FixedProposals, FixedRates, HourlyState, NeverFrozen, NoRedraft,
    OrderSize, PROPOSAL, context, draft, master, path_time, push_gate_round, quote, rates,
    settings_with, storage,
};
use super::{LOCAL, approval, assembled, response_of};
use crate::ipc::order_approval::OrderApprovalRuntime;
use crate::ipc::state::BoxFuture;
use crate::order_runtime::{LedgerTxPort, SubmissionLedger, forwarding_bus};

struct MemoryLedger {
    consumption: Arc<MemoryProposalConsumptionRepository>,
    audit: Arc<MemoryAuditLog>,
    trade: Arc<RecordingBroker>,
    sent_at_commit: AtomicUsize,
}

impl SubmissionLedger for MemoryLedger {
    fn commit_prepared<'a>(
        &'a self,
        proposal: &'a ProposalId,
        client_order_id: &'a ClientOrderId,
        prepared_at: Timestamp,
    ) -> BoxFuture<'a, Result<ConsumptionOutcome, SystemPortError>> {
        Box::pin(async move {
            self.sent_at_commit
                .store(self.trade.sent().len(), Ordering::SeqCst);
            let outcome = self.consumption.consume(proposal).await?;
            if outcome == ConsumptionOutcome::Recorded {
                let _recorded = self
                    .audit
                    .append(&AuditEntry::new(
                        prepared_at,
                        AuditKind::SubmissionPrepared(client_order_id.clone()),
                    ))
                    .await?;
            }
            Ok(outcome)
        })
    }
}

struct ProductPathTypes;

impl ApprovalServiceTypes for ProductPathTypes {
    type Broker = ScriptedBrokerRead;
    type Market = ScriptedMarketInfo;
    type Settings = testkit_ports::memory::MemorySettingsRepository;
    type Rates = FixedRates;
    type Proposals = FixedProposals;
    type Consumption = MemoryProposalConsumptionRepository;
    type Reports = MemoryValidationReportRepository;
    type Audit = MemoryAuditLog;
    type Contexts = FixedContexts;
    type Redrafter = NoRedraft;
    type Freeze = NeverFrozen;
    type Clock = SteppedClock;
    type Presence = ScriptedPresence;
    type TxPort = LedgerTxPort;
    type Trade = RecordingBroker;
}

struct Counter(Arc<AtomicUsize>);

impl EventSubscriber for Counter {
    fn on_event(&self, _event: &DomainEvent) -> Result<(), SubscriberFailure> {
        let _previous = self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

struct Product {
    handler: Arc<dyn OrderApprovalHandler>,
    trade: Arc<RecordingBroker>,
    audit: Arc<MemoryAuditLog>,
    ledger: Arc<MemoryLedger>,
    submitted_on_product_bus: Arc<AtomicUsize>,
}

fn product(presence: &Arc<ScriptedPresence>, permission: OrderPermission) -> Product {
    let master = master();
    let proposal = draft(&master, OrderSize::Small);
    let id = proposal.id().clone();
    let broker = Arc::new(ScriptedBrokerRead::default());
    let market = Arc::new(ScriptedMarketInfo::default());
    broker.script_quotes().push(Ok(vec![quote(&master)]));
    push_gate_round(&broker, &market, &master, "35000", OrderSize::Small);
    push_gate_round(&broker, &market, &master, "35000", OrderSize::Small);
    let (consumption, reports, audit) = storage(AuditMode::Writable);
    let trade = Arc::new(RecordingBroker::with_replies([BrokerReply::Accept(
        BrokerOrderId::try_new("BRK-PRODUCT-1").expect("주문번호"),
    )]));
    let ledger = Arc::new(MemoryLedger {
        consumption: Arc::clone(&consumption),
        audit: Arc::clone(&audit),
        trade: Arc::clone(&trade),
        sent_at_commit: AtomicUsize::new(usize::MAX),
    });
    let seen = Arc::new(AtomicUsize::new(0));
    let mut bus = EventBusBuilder::default();
    bus.subscribe(
        std::collections::BTreeSet::from([DomainEventKind::OrderSubmitted]),
        Counter(Arc::clone(&seen)),
    );
    let product_bus = Arc::new(bus.build());
    let ledger_port: Arc<dyn SubmissionLedger> = Arc::<MemoryLedger>::clone(&ledger);
    let service = ApprovalService::new(ApprovalServiceParts::<ProductPathTypes> {
        broker,
        market,
        settings: Arc::new(settings_with(permission)),
        rates: Arc::new(rates(HourlyState::Room)),
        proposals: Arc::new(FixedProposals::from_map(BTreeMap::from([(
            id.clone(),
            proposal,
        )]))),
        consumption,
        reports,
        audit: Arc::clone(&audit),
        contexts: Arc::new(FixedContexts::open(BTreeMap::from([(
            id,
            context(&master, OrderSize::Small),
        )]))),
        redrafter: Arc::new(NoRedraft),
        freeze: Arc::new(NeverFrozen),
        clock: Arc::new(SteppedClock::new(path_time())),
        presence: Arc::clone(presence),
        tx_port: Arc::new(LedgerTxPort(ledger_port)),
        trade: Arc::clone(&trade),
        events: forwarding_bus(&product_bus),
    });
    Product {
        handler: Arc::new(service),
        trade,
        audit,
        ledger,
        submitted_on_product_bus: seen,
    }
}

fn run<Check>(
    presence: &Arc<ScriptedPresence>,
    handler: Arc<dyn OrderApprovalHandler>,
    check: Check,
) where
    Check: Fn(&tauri::AppHandle<tauri::test::MockRuntime>) + 'static,
{
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
    assert_eq!(code, 0_i32, "앱 종료");
    assert_eq!(checked.load(Ordering::SeqCst), 1, "단언이 실제로 실행됨");
    assert_eq!(exits.load(Ordering::SeqCst), 1, "정리 뒤 종료 요청");
}

#[test]
fn product_ledger_records_the_audit_before_the_fake_toss_order_and_events_reach_the_app_bus() {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
    let path = product(&presence, OrderPermission::Enabled);
    let trade = Arc::clone(&path.trade);
    let audit = Arc::clone(&path.audit);
    let ledger = Arc::clone(&path.ledger);
    let seen = Arc::clone(&path.submitted_on_product_bus);
    let shown = Arc::clone(&presence);
    run(&presence, Arc::clone(&path.handler), move |handle| {
        assert_eq!(
            response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL)),
            Ok(serde_json::json!({"status": "submitted", "reason": "none"})),
            "확인·본인 확인 뒤 가짜 토스로 전송"
        );
        assert_eq!(shown.order_calls().len(), 1, "본인 확인 1회");
        assert_eq!(trade.sent().len(), 1, "주문 1건");
        assert_eq!(
            ledger.sent_at_commit.load(Ordering::SeqCst),
            0,
            "사전 기록은 전송보다 먼저"
        );
        let kinds: Vec<AuditKind> = audit
            .entries()
            .iter()
            .map(|entry| entry.kind().clone())
            .collect();
        let id = ClientOrderId::try_new(PROPOSAL).expect("주문 번호");
        let prepared = kinds
            .iter()
            .position(|kind| kind == &AuditKind::SubmissionPrepared(id.clone()))
            .expect("사전 기록 감사");
        let answered = kinds
            .iter()
            .position(|kind| kind == &AuditKind::SubmissionResponseRecorded(id.clone()))
            .expect("응답 감사");
        assert!(prepared < answered, "감사 순서: 사전 기록 → 응답");
        assert_eq!(seen.load(Ordering::SeqCst), 1, "주문 이벤트가 앱 버스로");
        assert_eq!(
            response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL)),
            Ok(serde_json::json!({"status": "rejected", "reason": "already-consumed"})),
            "같은 제안은 다시 보내지 않음"
        );
        assert_eq!(trade.sent().len(), 1, "재요청에 추가 주문 0건");
    });
}

#[test]
fn with_the_order_switch_off_nothing_reaches_presence_or_toss() {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Confirmed], []));
    let path = product(&presence, OrderPermission::Disabled);
    let trade = Arc::clone(&path.trade);
    let shown = Arc::clone(&presence);
    run(&presence, Arc::clone(&path.handler), move |handle| {
        assert_eq!(
            response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL)),
            Ok(serde_json::json!({"status": "rejected", "reason": "switch-off"})),
            "주문 켜기 꺼짐(기본)"
        );
        assert_eq!(shown.order_calls().len(), 0, "본인 확인 0회");
        assert_eq!(trade.sent().len(), 0, "주문 0건");
    });
}

#[test]
fn denied_presence_sends_nothing_and_leaves_the_proposal_unconsumed() {
    let presence = Arc::new(ScriptedPresence::new([OrderConfirmation::Denied], []));
    let path = product(&presence, OrderPermission::Enabled);
    let trade = Arc::clone(&path.trade);
    let audit = Arc::clone(&path.audit);
    run(&presence, Arc::clone(&path.handler), move |handle| {
        let response = response_of(handle, "request_order_approval", LOCAL, approval(PROPOSAL));
        assert_eq!(
            response,
            Ok(serde_json::json!({"status": "rejected", "reason": "presence-denied"})),
        );
        assert_eq!(trade.sent().len(), 0, "주문 0건");
        let id = ClientOrderId::try_new(PROPOSAL).expect("주문 번호");
        assert!(
            audit
                .entries()
                .iter()
                .all(|entry| entry.kind() != &AuditKind::SubmissionPrepared(id.clone())),
            "사전 기록 없음"
        );
    });
}
