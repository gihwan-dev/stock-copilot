use domain::{DomainEvent, DomainEventKind};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Debug, Formatter},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubscriptionId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriberFailure {
    Unavailable,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryFailure {
    pub subscription: SubscriptionId,
    pub cause: SubscriberFailure,
}
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DeliveryReport {
    pub failures: Vec<DeliveryFailure>,
}

pub trait EventSubscriber: Send + Sync {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure>;
}
#[derive(Default)]
pub struct EventBus {
    subscribers: Vec<Box<dyn EventSubscriber>>,
    subscriptions: BTreeMap<DomainEventKind, BTreeSet<SubscriptionId>>,
}
impl Debug for EventBus {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.subscriptions.fmt(f)
    }
}
#[derive(Debug, Default)]
pub struct EventBusBuilder(EventBus);
impl EventBusBuilder {
    pub fn subscribe<Subscriber: EventSubscriber + 'static>(
        &mut self,
        kinds: BTreeSet<DomainEventKind>,
        subscriber: Subscriber,
    ) -> SubscriptionId {
        let id = SubscriptionId(self.0.subscribers.len());
        self.0.subscribers.push(Box::new(subscriber));
        for kind in kinds {
            self.0.subscriptions.entry(kind).or_default().extend([id]);
        }
        id
    }
    #[must_use]
    pub fn build(self) -> EventBus {
        self.0
    }
}
impl EventBus {
    #[must_use]
    pub fn publish(&self, event: DomainEvent) -> DeliveryReport {
        let Some(subscriptions) = self.subscriptions.get(&event.kind()) else {
            return DeliveryReport::default();
        };
        let failures = self
            .subscribers
            .iter()
            .enumerate()
            .filter(|(index, _)| subscriptions.contains(&SubscriptionId(*index)))
            .filter_map(move |(index, subscriber)| {
                subscriber
                    .on_event(&event)
                    .err()
                    .map(|cause| DeliveryFailure {
                        subscription: SubscriptionId(index),
                        cause,
                    })
            })
            .collect();
        DeliveryReport { failures }
    }
}
