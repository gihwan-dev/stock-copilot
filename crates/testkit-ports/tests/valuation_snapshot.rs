#[cfg(test)]
mod tests {
    use application::{
        EventBusBuilder, EventSubscriber, FxRateQuery, RecordOutcome, SubscriberFailure,
        SystemFailure, SystemPort, SystemPortError, TakeValuationSnapshot, ValuationSnapshotError,
    };
    use domain::{
        AccountId, BuyingPower, Currency, DomainEvent, DomainEventKind, FxPair, FxRateKind, Market,
        MarketCalendar, MarketSession, Money, Nanosecond, NumericInput, OffsetSeconds, Portfolio,
        SessionData, SessionInformation, SnapshotCalendar, SnapshotError, SnapshotSource,
        SyncOutcome, SyncRecord, TimeInterval, Timestamp, UnixSeconds, ValuationSnapshot,
    };
    use std::{
        collections::BTreeSet,
        future::Future,
        sync::{
            Arc,
            mpsc::{Sender, channel},
        },
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
        memory::{
            MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
            MemoryValuationRepository,
        },
    };
    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future)
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn at(day: i64, hour: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                day.checked_mul(86400)
                    .expect("일")
                    .checked_add(hour.checked_mul(3600).expect("시"))
                    .expect("초"),
            ),
            Nanosecond::new(37),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn session(day: i64, market: &Market) -> MarketSession {
        let data = Some(
            SessionData::try_new(
                TimeInterval::try_new(at(day, 0), at(day, 1)).expect("구간"),
                None,
            )
            .expect("장"),
        );
        let info = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
        };
        MarketSession::try_new(at(day, 0).offset_calendar_date(), info).expect("세션")
    }
    fn calendar(day: i64) -> SnapshotCalendar {
        let make = |market: &Market| {
            MarketCalendar::try_new(
                session(day.checked_sub(1).expect("전일"), market),
                session(day, market),
                session(day.checked_add(1).expect("익일"), market),
            )
            .expect("달력")
        };
        SnapshotCalendar::new(&make(&Market::Kr), &make(&Market::Us)).expect("평가 달력")
    }
    fn portfolio() -> Portfolio {
        let power = |text, currency| {
            BuyingPower::new(
                Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액"),
                at(3, 2),
            )
        };
        Portfolio::new(
            vec![],
            power("137", Currency::Krw),
            power("29", Currency::Usd),
        )
        .expect("포트")
    }
    fn error() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied)
    }
    struct Observer {
        repository: Arc<MemoryValuationRepository>,
        events: Sender<DomainEvent>,
        order: Sender<usize>,
    }
    impl EventSubscriber for Observer {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.events
                .send(event.clone())
                .map_err(|_error| SubscriberFailure::Unavailable)?;
            self.order
                .send(self.repository.calls().len())
                .map_err(|_error| SubscriberFailure::Unavailable)?;
            Err(SubscriberFailure::Unavailable)
        }
    }
    fn assert_report(
        report: &application::ValuationSnapshotReport,
        dates: &SnapshotCalendar,
        index: usize,
    ) {
        assert_eq!(
            report.snapshot,
            ValuationSnapshot::observe(dates, &portfolio().evaluate(None), at(3, 2), None)
                .expect("도메인 위임")
        );
        assert_eq!(
            report.recorded,
            if index == 0 {
                RecordOutcome::Recorded
            } else {
                RecordOutcome::AlreadyRecorded
            }
        );
        assert_eq!(report.snapshot.source(), SnapshotSource::Observed);
        assert_eq!(report.snapshot.recorded_at(), at(3, 2));
        assert_eq!(report.snapshot.valuation_at(), at(3, 2));
        assert_eq!(report.delivery.failures.len(), 1);
        assert_eq!(
            report.delivery.failures[0].cause,
            SubscriberFailure::Unavailable
        );
    }
    fn assert_calls(repository: &MemoryValuationRepository, account: &AccountId) {
        let calls = repository.script_append().calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(&calls[0].0, account);
        assert_eq!(calls[0].1, calls[1].1);
    }
    fn assert_market(market: &MemoryMarketDataRepository) {
        assert_eq!(
            market.script_fx_rate().calls(),
            vec![(FxPair::UsdKrw, FxRateKind::Mid, FxRateQuery::At(at(3, 2))); 2]
        );
    }
    #[test]
    fn records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure() {
        let portfolio_store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let repository = Arc::new(MemoryValuationRepository::default());
        let (sender, events) = channel();
        let (order, stored) = channel();
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::ValuationSnapshotTaken]),
            Observer {
                repository: Arc::clone(&repository),
                events: sender,
                order,
            },
        );
        let bus = builder.build();
        let clock = SteppedClock::new(at(3, 2));
        let account = AccountId::new(71);
        let dates = calendar(3);
        for (index, outcome) in [
            SyncOutcome::Complete(portfolio()),
            SyncOutcome::Partial(portfolio()),
        ]
        .into_iter()
        .enumerate()
        {
            portfolio_store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    at(3, 1),
                    outcome,
                )))));
            market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
            repository.script_append().push(MemoryReply::Memory);
            let report = run(TakeValuationSnapshot::new(
                &portfolio_store,
                &market,
                repository.as_ref(),
                &clock,
                &bus,
            )
            .execute(&account, &dates))
            .expect("기록");
            assert_report(&report, &dates, index);
        }
        assert_eq!(
            events.try_iter().collect::<Vec<_>>().as_slice(),
            &[
                DomainEvent::ValuationSnapshotTaken(account.clone()),
                DomainEvent::ValuationSnapshotTaken(account.clone())
            ]
        );
        assert_eq!(stored.try_iter().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(clock.calls(), vec![at(3, 2); 2]);
        assert_eq!(
            portfolio_store.script_latest().calls(),
            vec![(account.clone(),); 2]
        );
        assert_market(&market);
        assert_calls(repository.as_ref(), &account);
    }
    #[test]
    fn missing_failed_blocked_and_repository_errors_do_not_write_or_publish() {
        for (input, expected) in [
            (Ok(None), ValuationSnapshotError::MissingPortfolio),
            (Err(error()), ValuationSnapshotError::Repository(error())),
            (
                Ok(Some(SyncRecord::new(at(3, 1), SyncOutcome::Failed))),
                ValuationSnapshotError::UnavailablePortfolio,
            ),
            (
                Ok(Some(SyncRecord::new(at(3, 1), SyncOutcome::AccessBlocked))),
                ValuationSnapshotError::UnavailablePortfolio,
            ),
        ] {
            let store = MemoryPortfolioRepository::default();
            store.script_latest().push(MemoryReply::Return(input));
            let market = MemoryMarketDataRepository::default();
            let repository = MemoryValuationRepository::default();
            let clock = SteppedClock::new(at(3, 2));
            assert_eq!(
                run(TakeValuationSnapshot::new(
                    &store,
                    &market,
                    &repository,
                    &clock,
                    &application::EventBus::default()
                )
                .execute(&AccountId::new(73), &calendar(3))),
                Err(expected)
            );
            assert_eq!(repository.calls().len(), 0);
            assert_eq!(market.script_fx_rate().calls().len(), 0);
            assert_eq!(clock.calls().len(), 0);
        }
    }
    #[test]
    fn market_snapshot_and_append_errors_are_preserved() {
        for stage in 0_usize..3 {
            let store = MemoryPortfolioRepository::default();
            store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    at(3, 1),
                    SyncOutcome::Complete(portfolio()),
                )))));
            let market = MemoryMarketDataRepository::default();
            market
                .script_fx_rate()
                .push(MemoryReply::Return(if stage == 0 {
                    Err(error())
                } else {
                    Ok(None)
                }));
            let repository = Arc::new(MemoryValuationRepository::default());
            repository
                .script_append()
                .push(MemoryReply::Return(Err(error())));
            let (sender, events) = channel();
            let (order, stored) = channel();
            let mut builder = EventBusBuilder::default();
            builder.subscribe(
                BTreeSet::from([DomainEventKind::ValuationSnapshotTaken]),
                Observer {
                    repository: Arc::clone(&repository),
                    events: sender,
                    order,
                },
            );
            let bus = builder.build();
            let clock = SteppedClock::new(at(3, 2));
            let dates = calendar(if stage == 1 { 4 } else { 3 });
            let result =
                run(
                    TakeValuationSnapshot::new(&store, &market, repository.as_ref(), &clock, &bus)
                        .execute(&AccountId::new(79), &dates),
                );
            assert_eq!(
                result,
                Err(if stage == 1 {
                    ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod)
                } else {
                    ValuationSnapshotError::Repository(error())
                })
            );
            assert_eq!(repository.calls().len(), usize::from(stage == 2));
            assert_eq!(events.try_iter().count(), 0);
            assert_eq!(stored.try_iter().count(), 0);
        }
    }
}
