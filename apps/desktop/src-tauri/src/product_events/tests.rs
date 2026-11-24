use super::{
    BridgedUiEvents, FlushingAuditSink, RecordedTriggerUrgency, UiEventBinding, UiEventSlot,
    ui_event_kinds,
};
use crate::audit_queue::AuditEventQueue;
use crate::ipc::event_bridge::UiEventBridge;
use crate::platform::notifier::{DisclosureUrgency, DisclosureUrgencyLookup};
use application::{AuditEntry, AuditEventSink, AuditKind, EventSubscriber};
use domain::{DomainEvent, DomainEventKind, InstrumentId, Timestamp};
use std::sync::Arc;
use tauri::test::{mock_builder, mock_context, noop_assets};
use testkit_ports::memory::MemoryReply;
use testkit_ports::system_fakes::MemoryAuditLog;

fn entry(symbol: &str) -> AuditEntry {
    AuditEntry::new(
        Timestamp::from_parts(
            domain::UnixSeconds::new(1_775_091_600),
            domain::Nanosecond::new(0),
            domain::OffsetSeconds::new(32_400),
        )
        .expect("시각"),
        AuditKind::SignalRaised(InstrumentId::try_new(symbol).expect("종목")),
    )
}

#[test]
fn ui_events_cover_every_domain_event_kind_and_bind_once() {
    let kinds = ui_event_kinds();
    assert_eq!(kinds.len(), 41);
    assert!(kinds.contains(&DomainEventKind::BriefingFinished));
    assert!(kinds.contains(&DomainEventKind::PortfolioSyncRecorded));
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock 앱");
    let slot = Arc::new(UiEventSlot::default());
    let subscriber = BridgedUiEvents(Arc::clone(&slot));
    assert_eq!(subscriber.on_event(&DomainEvent::SettingsChanged), Ok(()));
    assert_eq!(
        slot.bind(UiEventBridge::new(app.handle().clone())),
        UiEventBinding::Bound
    );
    assert_eq!(
        slot.bind(UiEventBridge::new(app.handle().clone())),
        UiEventBinding::AlreadyBound
    );
}

#[test]
fn recorded_disclosure_triggers_are_treated_as_urgent() {
    let key = domain::DisclosureKey::from_receipt(domain::ReceiptNo::Kind(
        domain::KindReceiptNo::try_new("20261116000001").expect("접수번호"),
    ));
    assert_eq!(
        RecordedTriggerUrgency.urgency(&key),
        DisclosureUrgency::Urgent
    );
}

#[test]
fn audit_entries_reach_the_log_in_enqueue_order() {
    let audit = Arc::new(MemoryAuditLog::default());
    for _ in ["kr-1", "kr-2", "kr-3"] {
        audit.script_append().push(MemoryReply::Memory);
    }
    let sink = FlushingAuditSink::new(AuditEventQueue::default(), Arc::clone(&audit));
    for symbol in ["kr-1", "kr-2", "kr-3"] {
        sink.enqueue(entry(symbol)).expect("큐");
    }
    let _flushed = tauri::async_runtime::block_on(sink.flush()).expect("기록");
    assert_eq!(
        audit.entries(),
        vec![entry("kr-1"), entry("kr-2"), entry("kr-3")]
    );
}
