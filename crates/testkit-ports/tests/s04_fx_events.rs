#[cfg(test)]
mod tests {
    use application::{EventBusBuilder, EventSubscriber, SubscriberFailure};
    use domain::{
        DomainEvent, DomainEventKind, FxPair, FxRate, FxRateKind, FxSource, Nanosecond,
        NumericInput, OffsetSeconds, TimeInterval, Timestamp, UnixSeconds,
    };
    use std::{
        future::Future,
        sync::mpsc,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::{MemoryMarketDataRepository, MemoryReply};

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("expected ready"),
        }
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(13),
            OffsetSeconds::new(0),
        )
        .expect("time")
    }
    fn rate(value: &str, start: i64) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new(value).expect("input"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(at(start), at(start.checked_add(60).expect("end")))
                .expect("window"),
            FxSource::TossOpenApi,
        )
        .expect("rate")
    }
    struct Observer(mpsc::Sender<DomainEvent>);
    impl EventSubscriber for Observer {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .send(event.clone())
                .map_err(|_error| SubscriberFailure::Unavailable)
        }
    }
    #[test]
    fn repeated_and_older_rates_do_not_publish_but_changed_rates_do() {
        let repository = MemoryMarketDataRepository::default();
        let (sender, receiver) = mpsc::channel();
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            std::iter::once(DomainEventKind::FxRateUpdated).collect(),
            Observer(sender),
        );
        let events = builder.build();
        let store = application::StoreFxRate::new(&repository, &events);
        for (value, seconds, expected) in [
            ("1374.2", 10, 1),
            ("1374.2", 10, 0),
            ("1401.5", 9, 0),
            ("1399.9", 10, 1),
            ("1399.9", 11, 1),
        ] {
            repository.script_store_fx_rate().push(MemoryReply::Memory);
            run(store.execute(rate(value, seconds))).expect("store");
            assert_eq!(
                receiver.try_iter().count(),
                expected,
                "{value} at {seconds}"
            );
        }
    }
}
