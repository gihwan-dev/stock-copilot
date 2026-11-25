use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use application::{EventBusBuilder, EventSubscriber, SubscriberFailure};
use domain::{ClientOrderId, DomainEvent, DomainEventKind};

use super::forwarding_bus;

struct Counter(Arc<AtomicUsize>);

impl EventSubscriber for Counter {
    fn on_event(&self, _event: &DomainEvent) -> Result<(), SubscriberFailure> {
        let _previous = self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn order_events_of_the_approval_service_reach_the_product_bus() {
    let seen = Arc::new(AtomicUsize::new(0));
    let mut builder = EventBusBuilder::default();
    builder.subscribe(
        std::collections::BTreeSet::from([DomainEventKind::OrderSubmitted]),
        Counter(Arc::clone(&seen)),
    );
    let product = Arc::new(builder.build());
    let approvals = forwarding_bus(&product);
    let id = ClientOrderId::try_new("sc-ord-0001").expect("주문 번호");
    let _report = approvals.publish(DomainEvent::OrderSubmitted(id));
    assert_eq!(seen.load(Ordering::SeqCst), 1);
}
