#[cfg(test)]
mod tests {
    use application::{
        EventBusBuilder, EventSubscriber, SubscriberFailure, SystemFailure, SystemPort,
        SystemPortError,
    };
    use domain::{
        DomainEvent, DomainEventKind, FxPair, FxRate, FxRateKind, FxSource, Nanosecond,
        NumericInput, OffsetSeconds, TimeInterval, Timestamp, UnixSeconds,
    };
    use std::{
        future::Future,
        sync::{Arc, mpsc},
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        memory::{MemoryMarketDataRepository, MemoryReply},
        scripted::MarketDataRepositoryCall,
    };

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
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("time")
    }
    fn rate() -> FxRate {
        FxRate::parse(
            &NumericInput::try_new("1374.2").expect("input"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(at(10), at(70)).expect("window"),
            FxSource::TossOpenApi,
        )
        .expect("rate")
    }
    struct Observer {
        repository: Arc<MemoryMarketDataRepository>,
        events: mpsc::Sender<(DomainEvent, Vec<MarketDataRepositoryCall>)>,
    }
    impl EventSubscriber for Observer {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.events
                .send((event.clone(), self.repository.calls()))
                .map_err(|_error| SubscriberFailure::Unavailable)
        }
    }
    #[test]
    fn event_follows_successful_storage_and_is_absent_on_failure() {
        let repository = Arc::new(MemoryMarketDataRepository::default());
        let (sender, received) = mpsc::channel();
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            std::iter::once(DomainEventKind::FxRateUpdated).collect(),
            Observer {
                repository: Arc::clone(&repository),
                events: sender,
            },
        );
        let events = builder.build();
        repository.script_store_fx_rate().push(MemoryReply::Memory);
        let delivery =
            run(application::StoreFxRate::new(repository.as_ref(), &events).execute(rate()))
                .expect("stored");
        assert_eq!(delivery.failures.len(), 0);
        assert_eq!(
            received.try_recv().expect("event"),
            (
                DomainEvent::FxRateUpdated(FxPair::UsdKrw),
                vec![MarketDataRepositoryCall::StoreFxRate(rate())],
            )
        );
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        repository
            .script_store_fx_rate()
            .push(MemoryReply::Return(Err(error)));
        assert_eq!(
            run(application::StoreFxRate::new(repository.as_ref(), &events).execute(rate())),
            Err(error)
        );
        assert_eq!(received.try_recv(), Err(mpsc::TryRecvError::Empty));
    }
    #[test]
    fn refresh_reads_matching_rate_then_stores_before_publishing() {
        use application::{
            HttpStatus, PortError, PortErrorKind, ReadEndpoint, RefreshFxRate, RefreshFxRateError,
            ResponseMismatch,
        };
        use testkit_ports::scripted::{MarketInfoCall, ScriptedMarketInfo};
        let market = ScriptedMarketInfo::default();
        let repository = Arc::new(MemoryMarketDataRepository::default());
        let (sender, received) = mpsc::channel();
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            std::iter::once(DomainEventKind::FxRateUpdated).collect(),
            Observer {
                repository: Arc::clone(&repository),
                events: sender,
            },
        );
        let events = builder.build();
        let refresh = RefreshFxRate::new(&market, repository.as_ref(), &events);
        market.script_fx_rate().push(Ok(rate()));
        repository.script_store_fx_rate().push(MemoryReply::Memory);
        let report =
            run(refresh.execute(FxPair::UsdKrw, FxRateKind::Mid, Some(at(20)))).expect("refreshed");
        assert_eq!(report.rate, rate());
        assert_eq!(report.delivery.failures.len(), 0);
        assert_eq!(
            market.calls(),
            vec![MarketInfoCall::FxRate(
                FxPair::UsdKrw,
                FxRateKind::Mid,
                Some(at(20))
            )]
        );
        assert_eq!(
            received.try_recv().expect("event").1,
            vec![MarketDataRepositoryCall::StoreFxRate(rate())]
        );
        let broker_error = PortError::new(
            ReadEndpoint::FxRate,
            HttpStatus::Success,
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
        );
        market.script_fx_rate().push(Err(broker_error));
        assert_eq!(
            run(refresh.execute(FxPair::UsdKrw, FxRateKind::Mid, None)),
            Err(RefreshFxRateError::Broker(broker_error))
        );
        market.script_fx_rate().push(Ok(rate()));
        assert_eq!(
            run(refresh.execute(FxPair::UsdKrw, FxRateKind::Buy, None)),
            Err(RefreshFxRateError::MismatchedRate)
        );
        assert_eq!(repository.calls().len(), 1);
        assert_eq!(received.try_recv(), Err(mpsc::TryRecvError::Empty));
    }
}
