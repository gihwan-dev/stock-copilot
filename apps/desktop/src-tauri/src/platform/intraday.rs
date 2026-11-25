use application::{EventSubscriber, SubscriberFailure};
use domain::{DomainEvent, DomainEventKind};
use scheduler::IntradayWatchSubscriber;
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

#[derive(Debug, Default)]
pub struct IntradayWatchSlot {
    target: RwLock<Option<IntradayWatchSubscriber>>,
}

impl IntradayWatchSlot {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn subscribed_kinds() -> BTreeSet<DomainEventKind> {
        BTreeSet::from([
            DomainEventKind::DisclosureTriggerRecorded,
            DomainEventKind::PriceMoved,
            DomainEventKind::NewsActivityIncreased,
        ])
    }

    pub fn bind(&self, subscriber: IntradayWatchSubscriber) {
        match self.target.write() {
            Ok(mut guard) => *guard = Some(subscriber),
            Err(poisoned) => *poisoned.into_inner() = Some(subscriber),
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
pub struct BridgedIntradayWatch(pub Arc<IntradayWatchSlot>);

impl EventSubscriber for BridgedIntradayWatch {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        self.0.forward(event)
    }
}
