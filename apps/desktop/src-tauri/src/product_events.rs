use crate::audit_queue::AuditEventQueue;
use crate::ipc::event_bridge::UiEventBridge;
use crate::platform::notifier::{DisclosureUrgency, DisclosureUrgencyLookup};
use application::{AuditEntry, AuditEventSink, AuditLog, EventSubscriber, SubscriberFailure};
use domain::{DisclosureKey, DomainEvent, DomainEventKind};
use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};
use tauri::Runtime;

#[must_use]
pub fn ui_event_kinds() -> BTreeSet<DomainEventKind> {
    BTreeSet::from([
        DomainEventKind::PortfolioSyncRecorded,
        DomainEventKind::TransactionsSyncRecorded,
        DomainEventKind::PriceUpdated,
        DomainEventKind::PriceMoved,
        DomainEventKind::DisclosureArrived,
        DomainEventKind::NewsActivityIncreased,
        DomainEventKind::ProposalCreated,
        DomainEventKind::ProposalExpired,
        DomainEventKind::ProposalApproved,
        DomainEventKind::ProposalRejected,
        DomainEventKind::ProposalSkipped,
        DomainEventKind::OrderSubmitted,
        DomainEventKind::OrderOutcomeUnknown,
        DomainEventKind::BrokerConnectionChanged,
        DomainEventKind::SettingsChanged,
        DomainEventKind::PolicyChanged,
        DomainEventKind::OrderSwitchChanged,
        DomainEventKind::WatchlistChanged,
        DomainEventKind::ThesisUpdated,
        DomainEventKind::SelfCheckChanged,
        DomainEventKind::MarketSessionChanged,
        DomainEventKind::SignalRaised,
        DomainEventKind::SignalCleared,
        DomainEventKind::EstimateSubmitted,
        DomainEventKind::EstimateResolved,
        DomainEventKind::ValuationSnapshotTaken,
        DomainEventKind::OrderRejected,
        DomainEventKind::OrderOutcomeResolved,
        DomainEventKind::OrderFilled,
        DomainEventKind::OrderCancelled,
        DomainEventKind::OrderModified,
        DomainEventKind::CollectionFinished,
        DomainEventKind::NewsClusterGrew,
        DomainEventKind::AiBudgetChanged,
        DomainEventKind::AccountSelectionChanged,
        DomainEventKind::StoredDataChanged,
        DomainEventKind::FxRateUpdated,
        DomainEventKind::DisclosureTriggerRecorded,
        DomainEventKind::SourceStatusChanged,
        DomainEventKind::AiJobFinished,
        DomainEventKind::BriefingFinished,
    ])
}

#[derive(Debug)]
pub struct UiEventSlot<R: Runtime>(OnceLock<UiEventBridge<R>>);

impl<R: Runtime> Default for UiEventSlot<R> {
    fn default() -> Self {
        Self(OnceLock::new())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEventBinding {
    Bound,
    AlreadyBound,
}

impl<R: Runtime> UiEventSlot<R> {
    pub fn bind(&self, bridge: UiEventBridge<R>) -> UiEventBinding {
        match self.0.set(bridge) {
            Ok(()) => UiEventBinding::Bound,
            Err(_existing) => UiEventBinding::AlreadyBound,
        }
    }
}

#[derive(Debug)]
pub struct BridgedUiEvents<R: Runtime>(pub Arc<UiEventSlot<R>>);

impl<R: Runtime> EventSubscriber for BridgedUiEvents<R> {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        match self.0.0.get() {
            Some(bridge) => bridge.on_event(event),
            None => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RecordedTriggerUrgency;

impl DisclosureUrgencyLookup for RecordedTriggerUrgency {
    fn urgency(&self, _key: &DisclosureKey) -> DisclosureUrgency {
        DisclosureUrgency::Urgent
    }
}

pub struct FlushingAuditSink<A> {
    queue: AuditEventQueue,
    audit: Arc<A>,
    order: Arc<tokio::sync::Mutex<()>>,
}

impl<A> std::fmt::Debug for FlushingAuditSink<A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FlushingAuditSink")
    }
}

impl<A: AuditLog + 'static> FlushingAuditSink<A> {
    #[must_use]
    pub fn new(queue: AuditEventQueue, audit: Arc<A>) -> Self {
        Self {
            queue,
            audit,
            order: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    pub async fn flush(&self) -> Result<usize, application::SystemPortError> {
        flush_in_order(&self.order, &self.queue, self.audit.as_ref()).await
    }
}

async fn flush_in_order<A: AuditLog>(
    order: &tokio::sync::Mutex<()>,
    queue: &AuditEventQueue,
    audit: &A,
) -> Result<usize, application::SystemPortError> {
    let _turn = order.lock().await;
    queue.flush(audit).await
}

impl<A: AuditLog + 'static> AuditEventSink for FlushingAuditSink<A> {
    fn enqueue(&self, entry: AuditEntry) -> Result<(), SubscriberFailure> {
        self.queue.enqueue(entry)?;
        let queue = self.queue.clone();
        let audit = Arc::clone(&self.audit);
        let order = Arc::clone(&self.order);
        let _task = tauri::async_runtime::spawn(async move {
            let _flushed = flush_in_order(&order, &queue, audit.as_ref()).await;
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests;
