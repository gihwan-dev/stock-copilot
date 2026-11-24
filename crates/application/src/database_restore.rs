use crate::{DeliveryReport, EventBus};
use domain::{CalendarDate, DomainEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestoreOutcome {
    Restored(CalendarDate),
    NoVerifiedBackup,
}

#[must_use]
pub fn announce_restore(outcome: RestoreOutcome, events: &EventBus) -> Option<DeliveryReport> {
    match outcome {
        RestoreOutcome::Restored(_) => Some(events.publish(DomainEvent::StoredDataChanged)),
        RestoreOutcome::NoVerifiedBackup => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{RestoreOutcome, announce_restore};
    use crate::{EventBusBuilder, EventSubscriber, SubscriberFailure};
    use domain::{
        CalendarDate, CalendarYear, DayNumber, DomainEvent, DomainEventKind, MonthNumber,
    };
    use std::collections::BTreeSet;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Recorder(Arc<AtomicUsize>);

    impl EventSubscriber for Recorder {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            if *event == DomainEvent::StoredDataChanged {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    #[test]
    fn only_a_completed_restore_announces_stored_data_changed() {
        let seen = Arc::new(AtomicUsize::new(0));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::StoredDataChanged]),
            Recorder(Arc::clone(&seen)),
        );
        let events = builder.build();
        let date = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(25),
        )
        .expect("date");
        assert_eq!(
            announce_restore(RestoreOutcome::NoVerifiedBackup, &events),
            None
        );
        assert_eq!(seen.load(Ordering::SeqCst), 0);
        assert!(announce_restore(RestoreOutcome::Restored(date), &events).is_some());
        assert_eq!(seen.load(Ordering::SeqCst), 1);
    }
}
