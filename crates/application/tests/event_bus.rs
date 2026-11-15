#[cfg(test)]
mod tests {
    use application::{
        DeliveryFailureCause, EventBus, EventDeliveryFailure, EventPublisher, EventSubscriber,
        SubscriberError,
    };
    use domain::{DomainEvent, DomainEventKind};
    use proptest::{prop_assert_eq, proptest, test_runner::Config};
    use std::{
        collections::{BTreeMap, BTreeSet},
        sync::mpsc::{Receiver, Sender, channel},
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
    #[derive(Debug)]
    struct RecordingSubscriber(Sender<DomainEvent>);
    impl EventSubscriber for RecordingSubscriber {
        fn on_event(
            &self,
            event: &DomainEvent,
            _publisher: &mut EventPublisher<'_>,
        ) -> Result<(), SubscriberError> {
            match self.0.send(event.clone()) {
                Ok(()) => Ok(()),
                Err(_closed) => Err(SubscriberError::Unavailable),
            }
        }
    }

    #[derive(Debug, Clone, Copy)]
    enum Reaction {
        Success,
        Reject,
        Panic,
    }
    #[derive(Debug)]
    struct Reactive {
        outgoing: BTreeMap<DomainEventKind, Vec<DomainEvent>>,
        reaction: Reaction,
    }
    impl EventSubscriber for Reactive {
        fn on_event(
            &self,
            event: &DomainEvent,
            publisher: &mut EventPublisher<'_>,
        ) -> Result<(), SubscriberError> {
            for next in self
                .outgoing
                .get(&expected_kind(event))
                .into_iter()
                .flatten()
            {
                publisher.publish(next.clone());
            }
            match self.reaction {
                Reaction::Success => Ok(()),
                Reaction::Reject => Err(SubscriberError::Unavailable),
                Reaction::Panic => unwind_subscriber(),
            }
        }
    }

    fn unwind_subscriber() -> ! {
        panic!("가짜 구독자 패닉");
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
        }
    }

    #[test]
    fn all_kinds_deliver_exact_payloads_only_to_matching_subscribers() {
        let mut bus = EventBus::default();
        let receivers: Vec<_> = kinds()
            .into_iter()
            .map(|kind| {
                let receiver = Recording::default();
                let _subscription = bus.subscribe(BTreeSet::from([kind]), receiver.subscriber());
                (kind, receiver)
            })
            .collect();
        let mut published = events();
        published.push(DomainEvent::PortfolioSyncRecorded(domain::AccountId::new(
            99,
        )));
        published.push(DomainEvent::PriceUpdated(
            domain::InstrumentId::try_new("another").expect("다른 가짜 ID"),
        ));
        for event in &published {
            bus.publish(event.clone());
        }
        assert_eq!(receivers.len(), 38);
        for (kind, receiver) in receivers {
            let expected: Vec<_> = published
                .iter()
                .filter(|event| expected_kind(event) == kind)
                .cloned()
                .collect();
            assert_eq!(receiver.received(), expected);
        }
        assert_eq!(bus.take_failures(), vec![]);
    }

    #[derive(Debug)]
    struct AdditionalSubscriber(RecordingSubscriber);
    impl EventSubscriber for AdditionalSubscriber {
        fn on_event(
            &self,
            event: &DomainEvent,
            publisher: &mut EventPublisher<'_>,
        ) -> Result<(), SubscriberError> {
            self.0.on_event(event, publisher)
        }
    }
    #[test]
    fn new_subscriber_requires_only_its_implementation_and_registration() {
        let mut bus = EventBus::default();
        let existing = Recording::default();
        let added = Recording::default();
        let first = bus.subscribe(kinds(), existing.subscriber());
        bus.publish(DomainEvent::SettingsChanged);
        let second = bus.subscribe(kinds(), AdditionalSubscriber(added.subscriber()));
        assert_ne!(first, second);
        bus.publish(DomainEvent::PolicyChanged);
        assert_eq!(
            existing.received(),
            vec![DomainEvent::SettingsChanged, DomainEvent::PolicyChanged]
        );
        assert_eq!(added.received(), vec![DomainEvent::PolicyChanged]);
    }

    #[derive(Debug)]
    struct OrderedSubscriber(usize, Sender<(usize, DomainEvent)>);
    impl EventSubscriber for OrderedSubscriber {
        fn on_event(
            &self,
            event: &DomainEvent,
            _publisher: &mut EventPublisher<'_>,
        ) -> Result<(), SubscriberError> {
            match self.1.send((self.0, event.clone())) {
                Ok(()) => Ok(()),
                Err(_closed) => Err(SubscriberError::Unavailable),
            }
        }
    }
    #[test]
    fn delivery_order_is_publication_then_registration_order() {
        let mut bus = EventBus::default();
        let (sender, trace) = channel();
        for tag in [3, 1, 2] {
            let _subscription = bus.subscribe(kinds(), OrderedSubscriber(tag, sender.clone()));
        }
        let published = vec![
            DomainEvent::StoredDataChanged,
            DomainEvent::SettingsChanged,
            DomainEvent::StoredDataChanged,
        ];
        for event in &published {
            bus.publish(event.clone());
        }
        let expected: Vec<_> = published
            .into_iter()
            .flat_map(|event| [3, 1, 2].map(|tag| (tag, event.clone())))
            .collect();
        assert_eq!(trace.try_iter().collect::<Vec<_>>(), expected);
    }
    #[test]
    fn absent_and_empty_subscriptions_are_noops_and_duplicate_registrations_are_distinct() {
        let mut bus = EventBus::default();
        bus.publish(DomainEvent::SettingsChanged);
        let empty = Recording::default();
        let repeated = Recording::default();
        let _empty_id = bus.subscribe(BTreeSet::new(), empty.subscriber());
        let first = bus.subscribe(kinds(), repeated.subscriber());
        let second = bus.subscribe(kinds(), repeated.subscriber());
        assert_ne!(first, second);
        bus.publish(DomainEvent::SettingsChanged);
        assert_eq!(empty.received(), vec![]);
        assert_eq!(
            repeated.received(),
            vec![DomainEvent::SettingsChanged, DomainEvent::SettingsChanged]
        );
        assert_eq!(bus.take_failures(), vec![]);
    }
    #[test]
    fn failures_are_isolated_ordered_drained_and_keep_already_queued_events() {
        let mut bus = EventBus::default();
        let first = bus.subscribe(
            BTreeSet::from([DomainEventKind::SettingsChanged]),
            Reactive {
                outgoing: BTreeMap::from([(
                    DomainEventKind::SettingsChanged,
                    vec![DomainEvent::PolicyChanged],
                )]),
                reaction: Reaction::Reject,
            },
        );
        let second = bus.subscribe(
            BTreeSet::from([DomainEventKind::SettingsChanged]),
            Reactive {
                outgoing: BTreeMap::from([(
                    DomainEventKind::SettingsChanged,
                    vec![DomainEvent::OrderSwitchChanged],
                )]),
                reaction: Reaction::Panic,
            },
        );
        let healthy = Recording::default();
        let _healthy_id = bus.subscribe(kinds(), healthy.subscriber());
        for _ in 0_usize..2 {
            bus.publish(DomainEvent::SettingsChanged);
            assert_eq!(
                bus.take_failures(),
                vec![
                    EventDeliveryFailure {
                        subscription: first,
                        event: DomainEvent::SettingsChanged,
                        cause: DeliveryFailureCause::Subscriber(SubscriberError::Unavailable)
                    },
                    EventDeliveryFailure {
                        subscription: second,
                        event: DomainEvent::SettingsChanged,
                        cause: DeliveryFailureCause::Panicked
                    },
                ]
            );
            assert_eq!(bus.take_failures(), vec![]);
        }
        assert_eq!(
            healthy.received(),
            vec![
                DomainEvent::SettingsChanged,
                DomainEvent::PolicyChanged,
                DomainEvent::OrderSwitchChanged,
                DomainEvent::SettingsChanged,
                DomainEvent::PolicyChanged,
                DomainEvent::OrderSwitchChanged
            ]
        );
    }
    #[test]
    fn failures_accumulate_across_publications_until_taken() {
        let mut bus = EventBus::default();
        let subscription = bus.subscribe(
            kinds(),
            Reactive {
                outgoing: BTreeMap::new(),
                reaction: Reaction::Reject,
            },
        );
        let published = vec![DomainEvent::SettingsChanged, DomainEvent::PolicyChanged];
        for event in &published {
            bus.publish(event.clone());
        }
        let expected: Vec<_> = published
            .into_iter()
            .map(|event| EventDeliveryFailure {
                subscription,
                event,
                cause: DeliveryFailureCause::Subscriber(SubscriberError::Unavailable),
            })
            .collect();
        assert_eq!(bus.take_failures(), expected);
        assert_eq!(bus.take_failures(), vec![]);
    }
    #[derive(Debug)]
    struct Chain(std::cell::Cell<usize>);
    impl EventSubscriber for Chain {
        fn on_event(
            &self,
            event: &DomainEvent,
            publisher: &mut EventPublisher<'_>,
        ) -> Result<(), SubscriberError> {
            if let Some(remaining) = self.0.get().checked_sub(1) {
                self.0.set(remaining);
                publisher.publish(event.clone());
            }
            Ok(())
        }
    }
    #[test]
    fn long_republication_chain_does_not_recurse() {
        let mut bus = EventBus::default();
        let _chain = bus.subscribe(kinds(), Chain(std::cell::Cell::new(8192)));
        let receiver = Recording::default();
        let _receiver = bus.subscribe(kinds(), receiver.subscriber());
        bus.publish(DomainEvent::SettingsChanged);
        assert_eq!(
            receiver.received(),
            vec![DomainEvent::SettingsChanged; 8193]
        );
        assert_eq!(bus.take_failures(), vec![]);
    }
    #[test]
    fn nested_publications_are_fifo_after_all_current_subscribers() {
        let mut bus = EventBus::default();
        let _relay = bus.subscribe(
            kinds(),
            Reactive {
                outgoing: BTreeMap::from([
                    (
                        DomainEventKind::SettingsChanged,
                        vec![DomainEvent::PolicyChanged, DomainEvent::OrderSwitchChanged],
                    ),
                    (
                        DomainEventKind::PolicyChanged,
                        vec![DomainEvent::StoredDataChanged],
                    ),
                ]),
                reaction: Reaction::Success,
            },
        );
        let _second_relay = bus.subscribe(
            kinds(),
            Reactive {
                outgoing: BTreeMap::from([(
                    DomainEventKind::SettingsChanged,
                    vec![DomainEvent::WatchlistChanged],
                )]),
                reaction: Reaction::Success,
            },
        );
        let receiver = Recording::default();
        let _receiver = bus.subscribe(kinds(), receiver.subscriber());
        bus.publish(DomainEvent::SettingsChanged);
        assert_eq!(
            receiver.received(),
            vec![
                DomainEvent::SettingsChanged,
                DomainEvent::PolicyChanged,
                DomainEvent::OrderSwitchChanged,
                DomainEvent::WatchlistChanged,
                DomainEvent::StoredDataChanged
            ]
        );
        assert_eq!(bus.take_failures(), vec![]);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FakeConnection {
        Connected,
        Blocked,
        Expired,
        ActionRequired,
    }
    fn expected_connection_events(
        previous: FakeConnection,
        current: FakeConnection,
    ) -> Vec<DomainEvent> {
        if previous == current {
            vec![]
        } else {
            vec![DomainEvent::BrokerConnectionChanged]
        }
    }
    #[test]
    fn broker_helper_emits_one_change_including_recovery_and_no_self_check() {
        let states = [
            FakeConnection::Connected,
            FakeConnection::Blocked,
            FakeConnection::Expired,
            FakeConnection::ActionRequired,
        ];
        for previous in states {
            for current in states {
                let mut bus = EventBus::default();
                let receiver = Recording::default();
                let _receiver = bus.subscribe(kinds(), receiver.subscriber());
                bus.publish_broker_connection_change(&previous, &current);
                let expected = expected_connection_events(previous, current);
                assert_eq!(receiver.received(), expected);
                assert_eq!(bus.take_failures(), vec![]);
            }
        }
    }
    #[test]
    fn ordinary_publications_are_not_coalesced() {
        let mut bus = EventBus::default();
        let receiver = Recording::default();
        let _receiver = bus.subscribe(kinds(), receiver.subscriber());
        bus.publish(DomainEvent::BrokerConnectionChanged);
        bus.publish(DomainEvent::BrokerConnectionChanged);
        assert_eq!(
            receiver.received(),
            vec![
                DomainEvent::BrokerConnectionChanged,
                DomainEvent::BrokerConnectionChanged
            ]
        );
    }
    proptest! {
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn arbitrary_publications_equal_each_subscribers_filtered_sequence(
            published in proptest::collection::vec(proptest::sample::select(events()), 0..64),
            filters in proptest::collection::vec(
                proptest::collection::btree_set(proptest::sample::select(kinds().into_iter().collect::<Vec<_>>()), 0..=38), 0..8),
        ) {
            let mut bus = EventBus::default();
            let receivers: Vec<_> = filters.iter().map(|filter| {
                let receiver = Recording::default();
                let _subscription = bus.subscribe(filter.clone(), receiver.subscriber());
                receiver
            }).collect();
            for event in &published { bus.publish(event.clone()); }
            for (filter, receiver) in filters.iter().zip(receivers) {
                let expected: Vec<_> = published.iter().filter(|event| filter.contains(&expected_kind(event))).cloned().collect();
                prop_assert_eq!(&receiver.received(), &expected);
            }
            prop_assert_eq!(bus.take_failures(), vec![]);
        }
    }
}
