use crate::{AuditEntry, AuditKind, Clock, EventSubscriber, SubscriberFailure};
use domain::{DomainEvent, DomainEventKind};
use std::collections::BTreeSet;
use std::sync::Arc;

pub trait AuditEventSink: Send + Sync {
    fn enqueue(&self, entry: AuditEntry) -> Result<(), SubscriberFailure>;
}

pub struct AuditEventRecorder<Sink> {
    clock: Arc<dyn Clock>,
    sink: Sink,
}

impl<Sink> std::fmt::Debug for AuditEventRecorder<Sink> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuditEventRecorder").finish_non_exhaustive()
    }
}

impl<Sink: AuditEventSink> AuditEventRecorder<Sink> {
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>, sink: Sink) -> Self {
        Self { clock, sink }
    }

    #[must_use]
    pub fn kinds() -> BTreeSet<DomainEventKind> {
        BTreeSet::from([
            DomainEventKind::EstimateSubmitted,
            DomainEventKind::SignalRaised,
            DomainEventKind::ProposalSkipped,
        ])
    }
}

fn audit_kind(event: &DomainEvent) -> Option<AuditKind> {
    match event {
        DomainEvent::EstimateSubmitted(instrument) => {
            Some(AuditKind::EstimateSubmitted(instrument.clone()))
        }
        DomainEvent::SignalRaised(instrument) => Some(AuditKind::SignalRaised(instrument.clone())),
        DomainEvent::ProposalSkipped(proposal) => {
            Some(AuditKind::ProposalSkipped(proposal.clone()))
        }
        DomainEvent::PortfolioSyncRecorded(_)
        | DomainEvent::TransactionsSyncRecorded(_)
        | DomainEvent::PriceUpdated(_)
        | DomainEvent::PriceMoved(_)
        | DomainEvent::DisclosureArrived(_)
        | DomainEvent::NewsActivityIncreased(_)
        | DomainEvent::ProposalCreated(_)
        | DomainEvent::ProposalExpired(_)
        | DomainEvent::ProposalApproved(_)
        | DomainEvent::ProposalRejected(_)
        | DomainEvent::OrderSubmitted(_)
        | DomainEvent::OrderOutcomeUnknown(_)
        | DomainEvent::BrokerConnectionChanged
        | DomainEvent::SettingsChanged
        | DomainEvent::PolicyChanged
        | DomainEvent::OrderSwitchChanged
        | DomainEvent::WatchlistChanged
        | DomainEvent::ThesisUpdated(_)
        | DomainEvent::SelfCheckChanged
        | DomainEvent::MarketSessionChanged(_)
        | DomainEvent::SignalCleared(_)
        | DomainEvent::EstimateResolved(_)
        | DomainEvent::ValuationSnapshotTaken(_)
        | DomainEvent::OrderRejected(_)
        | DomainEvent::OrderOutcomeResolved(_)
        | DomainEvent::OrderFilled(_)
        | DomainEvent::OrderCancelled(_)
        | DomainEvent::OrderModified(_)
        | DomainEvent::CollectionFinished(_)
        | DomainEvent::NewsClusterGrew(_)
        | DomainEvent::AiBudgetChanged
        | DomainEvent::AccountSelectionChanged
        | DomainEvent::StoredDataChanged
        | DomainEvent::FxRateUpdated(_)
        | DomainEvent::DisclosureTriggerRecorded(_)
        | DomainEvent::AiJobFinished(_)
        | DomainEvent::SourceStatusChanged(_) => None,
    }
}

impl<Sink: AuditEventSink> EventSubscriber for AuditEventRecorder<Sink> {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        match audit_kind(event) {
            Some(kind) => self.sink.enqueue(AuditEntry::new(self.clock.now(), kind)),
            None => Ok(()),
        }
    }
}
