use crate::engine::SchedulerEventSubscriber;
use application::{EventSubscriber, SubscriberFailure};
use domain::{DomainEvent, DomainEventKind};
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeBinding {
    Unbound,
    Bound,
}

#[derive(Debug, Default)]
pub struct SchedulerEventBridge {
    target: RwLock<Option<SchedulerEventSubscriber>>,
}

impl SchedulerEventBridge {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn subscribed_kinds() -> BTreeSet<DomainEventKind> {
        BTreeSet::from([
            DomainEventKind::OrderSubmitted,
            DomainEventKind::SettingsChanged,
            DomainEventKind::AiBudgetChanged,
            DomainEventKind::MarketSessionChanged,
            DomainEventKind::WatchlistChanged,
            DomainEventKind::BrokerConnectionChanged,
        ])
    }

    pub fn bind(&self, subscriber: SchedulerEventSubscriber) {
        match self.target.write() {
            Ok(mut guard) => *guard = Some(subscriber),
            Err(poisoned) => *poisoned.into_inner() = Some(subscriber),
        }
    }

    #[must_use]
    pub fn binding(&self) -> BridgeBinding {
        let bound = match self.target.read() {
            Ok(guard) => guard.is_some(),
            Err(poisoned) => poisoned.into_inner().is_some(),
        };
        if bound {
            BridgeBinding::Bound
        } else {
            BridgeBinding::Unbound
        }
    }

    fn forward(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        let guard = match self.target.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        match guard.as_ref() {
            Some(subscriber) => subscriber.on_event(event),
            None => Ok(()),
        }
    }
}

#[derive(Debug)]
pub struct BridgedSchedulerEvents(pub Arc<SchedulerEventBridge>);

impl EventSubscriber for BridgedSchedulerEvents {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        self.0.forward(event)
    }
}
