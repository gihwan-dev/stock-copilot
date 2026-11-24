#[cfg(test)]
mod tests {
    use application::{
        DeliveryFailure, DeliveryReport, EventBus, EventBusBuilder, EventSubscriber,
        SubscriberFailure,
    };
    use domain::{DomainEvent, DomainEventKind};
    use proptest::{prop_assert_eq, proptest, test_runner::Config};
    use std::{
        collections::BTreeSet,
        sync::{
            Arc,
            mpsc::{Receiver, Sender, channel},
        },
        thread,
    };

    #[derive(Debug)]
    struct Recording {
        sender: Sender<DomainEvent>,
        receiver: Receiver<DomainEvent>,
    }
    impl Default for Recording {
        fn default() -> Self {
            let (sender, receiver) = channel();
            Self { sender, receiver }
        }
    }
    impl Recording {
        fn subscriber(&self) -> RecordingSubscriber {
            RecordingSubscriber(self.sender.clone())
        }
        fn received(&self) -> Vec<DomainEvent> {
            self.receiver.try_iter().collect()
        }
    }
    struct RecordingSubscriber(Sender<DomainEvent>);
    impl EventSubscriber for RecordingSubscriber {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            match self.0.send(event.clone()) {
                Ok(()) => Ok(()),
                Err(_closed) => Err(SubscriberFailure::Unavailable),
            }
        }
    }
    struct Unavailable;
    impl EventSubscriber for Unavailable {
        fn on_event(&self, _event: &DomainEvent) -> Result<(), SubscriberFailure> {
            Err(SubscriberFailure::Unavailable)
        }
    }
    struct OrderedSubscriber(usize, Sender<(usize, DomainEvent)>);
    impl EventSubscriber for OrderedSubscriber {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            match self.1.send((self.0, event.clone())) {
                Ok(()) => Ok(()),
                Err(_closed) => Err(SubscriberFailure::Unavailable),
            }
        }
    }
    fn globals() -> Vec<DomainEvent> {
        vec![
            DomainEvent::BrokerConnectionChanged,
            DomainEvent::SettingsChanged,
            DomainEvent::PolicyChanged,
            DomainEvent::OrderSwitchChanged,
            DomainEvent::WatchlistChanged,
            DomainEvent::SelfCheckChanged,
            DomainEvent::AiBudgetChanged,
            DomainEvent::AccountSelectionChanged,
            DomainEvent::StoredDataChanged,
        ]
    }
    fn instruments() -> Vec<DomainEvent> {
        let instrument = domain::InstrumentId::try_new("fake-instrument").expect("가짜 종목");
        vec![
            DomainEvent::PriceUpdated(instrument.clone()),
            DomainEvent::PriceMoved(instrument.clone()),
            DomainEvent::NewsActivityIncreased(instrument.clone()),
            DomainEvent::ThesisUpdated(instrument.clone()),
            DomainEvent::SignalRaised(instrument.clone()),
            DomainEvent::SignalCleared(instrument.clone()),
            DomainEvent::EstimateSubmitted(instrument.clone()),
            DomainEvent::EstimateResolved(instrument.clone()),
            DomainEvent::NewsClusterGrew(instrument),
        ]
    }
    fn accounts() -> Vec<DomainEvent> {
        let account = domain::AccountId::new(17);
        vec![
            DomainEvent::PortfolioSyncRecorded(account.clone()),
            DomainEvent::TransactionsSyncRecorded(account.clone()),
            DomainEvent::ValuationSnapshotTaken(account.clone()),
            DomainEvent::OrderFilled(account.clone()),
            DomainEvent::OrderCancelled(account.clone()),
            DomainEvent::OrderModified(account),
        ]
    }
    fn proposals() -> Vec<DomainEvent> {
        let proposal = domain::ProposalId::new(
            domain::ClientOrderId::try_new("fake-proposal").expect("가짜 제안"),
        );
        vec![
            DomainEvent::ProposalCreated(proposal.clone()),
            DomainEvent::ProposalExpired(proposal.clone()),
            DomainEvent::ProposalApproved(proposal.clone()),
            DomainEvent::ProposalRejected(proposal.clone()),
            DomainEvent::ProposalSkipped(proposal),
        ]
    }
    fn orders() -> Vec<DomainEvent> {
        let order = domain::ClientOrderId::try_new("fake-order").expect("가짜 주문");
        vec![
            DomainEvent::OrderSubmitted(order.clone()),
            DomainEvent::OrderOutcomeUnknown(order.clone()),
            DomainEvent::OrderRejected(order.clone()),
            DomainEvent::OrderOutcomeResolved(order),
        ]
    }
    fn others() -> Vec<DomainEvent> {
        let disclosure = domain::Disclosure::new(
            domain::ReceiptNo::Dart(
                domain::DartReceiptNo::try_new("20261114000001").expect("가짜 접수"),
            ),
            domain::NormalizedUrl::try_new("https://example.com/disclosure").expect("가짜 주소"),
            domain::UntrustedText::try_new("가짜 제목").expect("가짜 제목"),
            None,
        )
        .key()
        .clone();
        vec![
            DomainEvent::MarketSessionChanged(domain::KnownMarket::Kr),
            DomainEvent::CollectionFinished(domain::InformationSource::Web),
            DomainEvent::FxRateUpdated(domain::FxPair::UsdKrw),
            DomainEvent::DisclosureArrived(disclosure.clone()),
            DomainEvent::DisclosureTriggerRecorded(disclosure),
            DomainEvent::SourceStatusChanged(domain::DataSource::KindDisclosure),
            DomainEvent::BriefingFinished(domain::BriefingId::new(
                domain::BriefingKind::Morning,
                domain::CalendarDate::from_parts(
                    domain::CalendarYear::new(2026),
                    domain::MonthNumber::new(11),
                    domain::DayNumber::new(25),
                )
                .expect("가짜 날짜"),
            )),
        ]
    }
    fn events() -> Vec<DomainEvent> {
        [
            globals(),
            instruments(),
            accounts(),
            proposals(),
            orders(),
            others(),
        ]
        .concat()
    }
    fn kinds() -> BTreeSet<DomainEventKind> {
        events().iter().map(expected_kind).collect()
    }
    fn expected_kind(event: &DomainEvent) -> DomainEventKind {
        match event {
            DomainEvent::PortfolioSyncRecorded(_) => DomainEventKind::PortfolioSyncRecorded,
            DomainEvent::TransactionsSyncRecorded(_) => DomainEventKind::TransactionsSyncRecorded,
            DomainEvent::PriceUpdated(_) => DomainEventKind::PriceUpdated,
            DomainEvent::PriceMoved(_) => DomainEventKind::PriceMoved,
            DomainEvent::DisclosureArrived(_) => DomainEventKind::DisclosureArrived,
            DomainEvent::NewsActivityIncreased(_) => DomainEventKind::NewsActivityIncreased,
            DomainEvent::ProposalCreated(_) => DomainEventKind::ProposalCreated,
            DomainEvent::ProposalExpired(_) => DomainEventKind::ProposalExpired,
            DomainEvent::ProposalApproved(_) => DomainEventKind::ProposalApproved,
            DomainEvent::ProposalRejected(_) => DomainEventKind::ProposalRejected,
            DomainEvent::ProposalSkipped(_) => DomainEventKind::ProposalSkipped,
            DomainEvent::OrderSubmitted(_) => DomainEventKind::OrderSubmitted,
            DomainEvent::OrderOutcomeUnknown(_) => DomainEventKind::OrderOutcomeUnknown,
            DomainEvent::BrokerConnectionChanged => DomainEventKind::BrokerConnectionChanged,
            DomainEvent::SettingsChanged => DomainEventKind::SettingsChanged,
            DomainEvent::PolicyChanged => DomainEventKind::PolicyChanged,
            DomainEvent::OrderSwitchChanged => DomainEventKind::OrderSwitchChanged,
            DomainEvent::WatchlistChanged => DomainEventKind::WatchlistChanged,
            DomainEvent::ThesisUpdated(_) => DomainEventKind::ThesisUpdated,
            DomainEvent::SelfCheckChanged => DomainEventKind::SelfCheckChanged,
            DomainEvent::MarketSessionChanged(_) => DomainEventKind::MarketSessionChanged,
            DomainEvent::SignalRaised(_) => DomainEventKind::SignalRaised,
            DomainEvent::SignalCleared(_) => DomainEventKind::SignalCleared,
            DomainEvent::EstimateSubmitted(_) => DomainEventKind::EstimateSubmitted,
            DomainEvent::EstimateResolved(_) => DomainEventKind::EstimateResolved,
            DomainEvent::ValuationSnapshotTaken(_) => DomainEventKind::ValuationSnapshotTaken,
            DomainEvent::OrderRejected(_) => DomainEventKind::OrderRejected,
            DomainEvent::OrderOutcomeResolved(_) => DomainEventKind::OrderOutcomeResolved,
            DomainEvent::OrderFilled(_) => DomainEventKind::OrderFilled,
            DomainEvent::OrderCancelled(_) => DomainEventKind::OrderCancelled,
            DomainEvent::OrderModified(_) => DomainEventKind::OrderModified,
            DomainEvent::CollectionFinished(_) => DomainEventKind::CollectionFinished,
            DomainEvent::NewsClusterGrew(_) => DomainEventKind::NewsClusterGrew,
            DomainEvent::AiBudgetChanged => DomainEventKind::AiBudgetChanged,
            DomainEvent::AccountSelectionChanged => DomainEventKind::AccountSelectionChanged,
            DomainEvent::StoredDataChanged => DomainEventKind::StoredDataChanged,
            DomainEvent::FxRateUpdated(_) => DomainEventKind::FxRateUpdated,
            DomainEvent::DisclosureTriggerRecorded(_) => DomainEventKind::DisclosureTriggerRecorded,
            DomainEvent::SourceStatusChanged(_) => DomainEventKind::SourceStatusChanged,
            DomainEvent::BriefingFinished(_) => DomainEventKind::BriefingFinished,
        }
    }

    #[test]
    fn all_kinds_deliver_exact_payloads_only_to_matching_subscribers() {
        let mut builder = EventBusBuilder::default();
        let receivers: Vec<_> = kinds()
            .into_iter()
            .map(|kind| {
                let receiver = Recording::default();
                let _subscription =
                    builder.subscribe(BTreeSet::from([kind]), receiver.subscriber());
                (kind, receiver)
            })
            .collect();
        let bus = builder.build();
        let mut published = events();
        published.push(DomainEvent::PortfolioSyncRecorded(domain::AccountId::new(
            99,
        )));
        published.push(DomainEvent::PriceUpdated(
            domain::InstrumentId::try_new("another").expect("다른 가짜 ID"),
        ));
        for event in &published {
            assert_eq!(bus.publish(event.clone()), DeliveryReport::default());
        }
        assert_eq!(receivers.len(), 40);
        for (kind, receiver) in receivers {
            let expected: Vec<_> = published
                .iter()
                .filter(|event| expected_kind(event) == kind)
                .cloned()
                .collect();
            assert_eq!(receiver.received(), expected);
        }
    }
    struct AdditionalSubscriber(RecordingSubscriber);
    impl EventSubscriber for AdditionalSubscriber {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0.on_event(event)
        }
    }
    #[test]
    fn additional_subscriber_needs_only_implementation_and_builder_registration() {
        let mut builder = EventBusBuilder::default();
        let existing = Recording::default();
        let added = Recording::default();
        let first = builder.subscribe(kinds(), existing.subscriber());
        let second = builder.subscribe(kinds(), AdditionalSubscriber(added.subscriber()));
        assert_ne!(first, second);
        let bus = builder.build();
        assert_eq!(
            bus.publish(DomainEvent::SettingsChanged),
            DeliveryReport::default()
        );
        assert_eq!(existing.received(), vec![DomainEvent::SettingsChanged]);
        assert_eq!(added.received(), vec![DomainEvent::SettingsChanged]);
    }
    #[test]
    fn publication_and_registration_order_are_deterministic_without_coalescing() {
        let mut builder = EventBusBuilder::default();
        let (sender, trace) = channel();
        for tag in [3, 1, 2] {
            let _subscription = builder.subscribe(kinds(), OrderedSubscriber(tag, sender.clone()));
        }
        let bus = builder.build();
        let published = vec![
            DomainEvent::StoredDataChanged,
            DomainEvent::StoredDataChanged,
            DomainEvent::SettingsChanged,
        ];
        for event in &published {
            assert_eq!(bus.publish(event.clone()), DeliveryReport::default());
        }
        let expected: Vec<_> = published
            .into_iter()
            .flat_map(|event| [3, 1, 2].map(|tag| (tag, event.clone())))
            .collect();
        assert_eq!(trace.try_iter().collect::<Vec<_>>(), expected);
    }
    #[test]
    fn empty_subscriptions_receive_nothing_and_duplicate_registrations_remain_distinct() {
        assert_eq!(
            EventBusBuilder::default()
                .build()
                .publish(DomainEvent::SettingsChanged),
            DeliveryReport::default()
        );
        let mut builder = EventBusBuilder::default();
        let empty = Recording::default();
        let repeated = Recording::default();
        let _empty = builder.subscribe(BTreeSet::new(), empty.subscriber());
        let first = builder.subscribe(kinds(), repeated.subscriber());
        let second = builder.subscribe(kinds(), repeated.subscriber());
        assert_ne!(first, second);
        let bus = builder.build();
        assert_eq!(
            bus.publish(DomainEvent::SettingsChanged),
            DeliveryReport::default()
        );
        assert_eq!(empty.received(), vec![]);
        assert_eq!(
            repeated.received(),
            vec![DomainEvent::SettingsChanged, DomainEvent::SettingsChanged]
        );
    }
    #[test]
    fn report_is_per_publication_and_failures_do_not_stop_later_subscribers() {
        let mut builder = EventBusBuilder::default();
        let healthy = Recording::default();
        let first = builder.subscribe(kinds(), Unavailable);
        let _healthy = builder.subscribe(kinds(), healthy.subscriber());
        let second = builder.subscribe(
            BTreeSet::from([DomainEventKind::SettingsChanged]),
            Unavailable,
        );
        let bus = builder.build();
        let expected = DeliveryReport {
            failures: vec![
                DeliveryFailure {
                    subscription: first,
                    cause: SubscriberFailure::Unavailable,
                },
                DeliveryFailure {
                    subscription: second,
                    cause: SubscriberFailure::Unavailable,
                },
            ],
        };
        assert_eq!(bus.publish(DomainEvent::SettingsChanged), expected);
        assert_eq!(
            bus.publish(DomainEvent::PolicyChanged),
            DeliveryReport {
                failures: vec![DeliveryFailure {
                    subscription: first,
                    cause: SubscriberFailure::Unavailable
                }]
            }
        );
        assert_eq!(bus.publish(DomainEvent::SettingsChanged), expected);
        assert_eq!(
            healthy.received(),
            vec![
                DomainEvent::SettingsChanged,
                DomainEvent::PolicyChanged,
                DomainEvent::SettingsChanged
            ]
        );
    }
    fn assert_send_sync<Value: Send + Sync>() {}
    #[test]
    fn immutable_bus_and_arc_can_publish_on_another_thread() {
        assert_send_sync::<EventBus>();
        assert_send_sync::<Arc<EventBus>>();
        let mut builder = EventBusBuilder::default();
        let receiver = Recording::default();
        let _subscription = builder.subscribe(kinds(), receiver.subscriber());
        let bus = Arc::new(builder.build());
        let shared = Arc::clone(&bus);
        let report = thread::spawn(move || shared.publish(DomainEvent::SettingsChanged))
            .join()
            .expect("발행 스레드");
        assert_eq!(report, DeliveryReport::default());
        assert_eq!(
            bus.publish(DomainEvent::PolicyChanged),
            DeliveryReport::default()
        );
        assert_eq!(
            receiver.received(),
            vec![DomainEvent::SettingsChanged, DomainEvent::PolicyChanged]
        );
    }
    struct Panicking;
    impl EventSubscriber for Panicking {
        fn on_event(&self, _event: &DomainEvent) -> Result<(), SubscriberFailure> {
            subscriber_bug()
        }
    }
    fn subscriber_bug() -> ! {
        panic!("구독자 버그");
    }
    #[test]
    #[should_panic(expected = "구독자 버그")]
    fn subscriber_panics_propagate() {
        let mut builder = EventBusBuilder::default();
        let _subscription = builder.subscribe(kinds(), Panicking);
        let _report = builder.build().publish(DomainEvent::SettingsChanged);
    }
    proptest! {
        #![proptest_config(Config::with_cases(testkit::property_cases()))]
        #[test]
        fn arbitrary_publications_equal_each_subscribers_filtered_sequence(
            published in proptest::collection::vec(proptest::sample::select(events()),0..64),
            filters in proptest::collection::vec(proptest::collection::btree_set(proptest::sample::select(kinds().into_iter().collect::<Vec<_>>()),0..=40),0..8),
        ) {
            let mut builder=EventBusBuilder::default();
            let receivers:Vec<_>=filters.iter().map(|filter| {
                let receiver=Recording::default();
                let _subscription=builder.subscribe(filter.clone(),receiver.subscriber()); receiver
            }).collect();
            let bus=builder.build();
            for event in published.iter().flat_map(|event|[event,event]) { prop_assert_eq!(bus.publish(event.clone()),DeliveryReport::default()); }
            for (filter,receiver) in filters.iter().zip(receivers) {
                let expected:Vec<_>=published.iter().filter(|event|filter.contains(&expected_kind(event))).flat_map(|event|[event.clone(),event.clone()]).collect();
                prop_assert_eq!(receiver.received(),expected);
            }
        }
    }
}
