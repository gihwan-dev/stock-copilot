use domain::{DomainEvent, DomainEventKind};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt::Debug,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SubscriptionId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriberError {
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryFailureCause {
    Subscriber(SubscriberError),
    Panicked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDeliveryFailure {
    pub subscription: SubscriptionId,
    pub event: DomainEvent,
    pub cause: DeliveryFailureCause,
}

pub trait EventSubscriber: Debug {
    fn on_event(
        &self,
        event: &DomainEvent,
        publisher: &mut EventPublisher<'_>,
    ) -> Result<(), SubscriberError>;
}

#[derive(Debug)]
pub struct EventPublisher<'queue> {
    pending: &'queue mut Vec<DomainEvent>,
}

impl EventPublisher<'_> {
    pub fn publish(&mut self, event: DomainEvent) {
        self.pending.push(event);
    }
}

#[derive(Debug)]
struct Registration {
    id: SubscriptionId,
    subscriber: Box<dyn EventSubscriber>,
}

#[derive(Debug, Default)]
pub struct EventBus {
    registrations: Vec<Registration>,
    subscriptions: BTreeMap<DomainEventKind, BTreeSet<SubscriptionId>>,
    failures: Vec<EventDeliveryFailure>,
}

impl EventBus {
    pub fn subscribe<Subscriber: EventSubscriber + 'static>(
        &mut self,
        kinds: BTreeSet<DomainEventKind>,
        subscriber: Subscriber,
    ) -> SubscriptionId {
        let id = SubscriptionId(self.registrations.len());
        let registration = Registration {
            id,
            subscriber: Box::new(subscriber),
        };
        for kind in kinds {
            self.subscriptions.entry(kind).or_default().extend([id]);
        }
        self.registrations.push(registration);
        id
    }

    pub fn publish(&mut self, event: DomainEvent) {
        let mut pending = VecDeque::from([event]);
        while let Some(next) = pending.pop_front() {
            let mut followups = Vec::new();
            self.deliver(&next, &mut followups);
            pending.extend(followups);
        }
    }

    fn deliver(&mut self, event: &DomainEvent, pending: &mut Vec<DomainEvent>) {
        let Some(subscriptions) = self.subscriptions.get(&event.kind()) else {
            return;
        };
        for registration in &self.registrations {
            if !subscriptions.contains(&registration.id) {
                continue;
            }
            let mut publisher = EventPublisher { pending };
            let result = catch_unwind(AssertUnwindSafe(|| {
                registration.subscriber.on_event(event, &mut publisher)
            }));
            let cause = match result {
                Ok(Ok(())) => continue,
                Ok(Err(error)) => DeliveryFailureCause::Subscriber(error),
                Err(_) => DeliveryFailureCause::Panicked,
            };
            self.failures.push(EventDeliveryFailure {
                subscription: registration.id,
                event: event.clone(),
                cause,
            });
        }
    }

    pub fn publish_broker_connection_change<State: Eq>(
        &mut self,
        previous: &State,
        current: &State,
    ) {
        if previous != current {
            self.publish(DomainEvent::BrokerConnectionChanged);
        }
    }

    #[must_use]
    pub fn take_failures(&mut self) -> Vec<EventDeliveryFailure> {
        let mut failures = Vec::new();
        while let Some(failure) = self.failures.pop() {
            failures.push(failure);
        }
        failures.reverse();
        failures
    }
}
