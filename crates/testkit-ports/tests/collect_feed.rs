#[cfg(test)]
mod tests {
    use application::{
        AiReadPolicy, CollectFeed, CollectedEntry, CollectedObservation, ConditionalGet,
        DataAvailability, DisclosureRepository, EventBus, EventBusBuilder, EventSubscriber,
        FeedCursor, FeedPosition, FeedRoundOutcome, FeedSource, InformationError, NewsMetadata,
        NewsRepository, PublicationTime, ResetTime, Retention, SourceState, SourceStatus,
        SourceStatusRepository, SubscriberFailure, SystemFailure, SystemPort, SystemPortError,
        UserAgentRequirement, WriteCompleted,
    };
    use domain::{
        DataSource, Disclosure, DisclosureKey, DisclosureSource, DomainEvent, DomainEventKind,
        InformationSource, InstrumentId, KindReceiptNo, Nanosecond, NewsItem, NewsSource,
        NormalizedUrl, OffsetSeconds, ReceiptNo, Timestamp, UnixSeconds, UntrustedText,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::future::Future;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};
    use testkit_ports::SteppedClock;
    use testkit_ports::scripted::ScriptedNewsFeed;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("텍스트")
    }

    fn url(value: &str) -> NormalizedUrl {
        NormalizedUrl::try_new(value).expect("주소")
    }

    fn metadata() -> NewsMetadata {
        NewsMetadata::new(
            Retention::NotStored,
            AiReadPolicy::TitleOnly,
            ConditionalGet::Unsupported,
            UserAgentRequirement::Standard,
        )
    }

    fn news(path: &str) -> CollectedObservation {
        CollectedObservation::new(
            CollectedEntry::News(NewsItem::new(
                NewsSource::Yonhap,
                url(&format!("https://example.com/{path}")),
                text("기사 제목"),
                None,
            )),
            PublicationTime::NotProvided,
            at(100),
        )
    }

    fn disclosure() -> Disclosure {
        Disclosure::new(
            ReceiptNo::Kind(KindReceiptNo::try_new("20261113000001").expect("접수 번호")),
            url("https://example.com/disclosure"),
            text("공시 제목"),
            None,
        )
    }

    fn cursor(value: &str) -> FeedCursor {
        FeedCursor::Continue(FeedPosition::new(text(value)))
    }

    fn storage_failure() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
    }

    #[derive(Default)]
    struct Store {
        news: RwLock<Vec<NewsItem>>,
        disclosures: RwLock<Vec<Disclosure>>,
        fail_news: RwLock<Option<SystemPortError>>,
        statuses: RwLock<BTreeMap<DataSource, SourceStatus>>,
    }

    impl NewsRepository for Store {
        fn store(
            &self,
            item: &NewsItem,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            let failure = *self.fail_news.read().expect("잠금");
            if let Some(error) = failure {
                return std::future::ready(Err(error));
            }
            self.news.write().expect("잠금").push(item.clone());
            std::future::ready(Ok(WriteCompleted))
        }
        fn for_instrument(
            &self,
            _instrument: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<NewsItem>, SystemPortError>> + Send {
            std::future::ready(Ok(Vec::new()))
        }
    }

    impl DisclosureRepository for Store {
        fn store_disclosure(
            &self,
            item: &Disclosure,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            self.disclosures.write().expect("잠금").push(item.clone());
            std::future::ready(Ok(WriteCompleted))
        }
        fn disclosure(
            &self,
            _key: &DisclosureKey,
        ) -> impl Future<Output = Result<Option<Disclosure>, SystemPortError>> + Send {
            std::future::ready(Ok(None))
        }
        fn disclosures_for_instrument(
            &self,
            _instrument: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<Disclosure>, SystemPortError>> + Send {
            std::future::ready(Ok(Vec::new()))
        }
    }

    impl SourceStatusRepository for Store {
        fn find(
            &self,
            source: DataSource,
        ) -> impl Future<Output = Result<Option<SourceStatus>, SystemPortError>> + Send {
            std::future::ready(Ok(self
                .statuses
                .read()
                .expect("잠금")
                .get(&source)
                .copied()))
        }
        fn list_all(
            &self,
        ) -> impl Future<Output = Result<Vec<SourceStatus>, SystemPortError>> + Send {
            std::future::ready(Ok(self
                .statuses
                .read()
                .expect("잠금")
                .values()
                .copied()
                .collect()))
        }
        fn save(
            &self,
            status: &SourceStatus,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            self.statuses
                .write()
                .expect("잠금")
                .insert(status.source(), *status);
            std::future::ready(Ok(WriteCompleted))
        }
    }

    struct Recorder(Arc<RwLock<Vec<DomainEvent>>>);

    impl EventSubscriber for Recorder {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            match self.0.write() {
                Ok(mut guard) => guard.push(event.clone()),
                Err(poisoned) => poisoned.into_inner().push(event.clone()),
            }
            Ok(())
        }
    }

    fn recording_bus() -> (EventBus, Arc<RwLock<Vec<DomainEvent>>>) {
        let events = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([
                DomainEventKind::DisclosureArrived,
                DomainEventKind::CollectionFinished,
                DomainEventKind::SourceStatusChanged,
            ]),
            Recorder(Arc::clone(&events)),
        );
        (builder.build(), events)
    }

    fn state(store: &Store, source: DataSource) -> SourceState {
        store
            .statuses
            .read()
            .expect("잠금")
            .get(&source)
            .expect("상태")
            .state()
    }

    #[test]
    fn collected_news_and_disclosures_are_stored_with_events_status_and_next_cursor() {
        let store = Store::default();
        let clock = SteppedClock::new(at(200));
        let (events, recorded) = recording_bus();
        let feed =
            ScriptedNewsFeed::new(FeedSource::Disclosure(DisclosureSource::Kind), metadata());
        let item = disclosure();
        feed.script_new_items()
            .push(Ok(DataAvailability::Available((
                vec![
                    news("a"),
                    CollectedObservation::new(
                        CollectedEntry::Disclosure(item.clone()),
                        PublicationTime::Known(at(90)),
                        at(100),
                    ),
                ],
                cursor("next"),
            ))));
        let round =
            run(CollectFeed::new(&store, &store, &clock, &events).collect(&feed, cursor("prev")))
                .expect("수집");
        assert_eq!(round.outcome, FeedRoundOutcome::Collected);
        assert_eq!(round.source, DataSource::KindDisclosure);
        assert_eq!(round.cursor, cursor("next"));
        assert_eq!(*round.news.as_u32(), 1);
        assert_eq!(*round.disclosures.as_u32(), 1);
        assert_eq!(store.news.read().expect("잠금").len(), 1);
        assert_eq!(*store.disclosures.read().expect("잠금"), vec![item.clone()]);
        assert_eq!(
            state(&store, DataSource::KindDisclosure),
            SourceState::Healthy
        );
        assert_eq!(
            *recorded.read().expect("잠금"),
            vec![
                DomainEvent::DisclosureArrived(item.key().clone()),
                DomainEvent::CollectionFinished(InformationSource::Disclosure(
                    DisclosureSource::Kind
                )),
                DomainEvent::SourceStatusChanged(DataSource::KindDisclosure),
            ]
        );
    }

    #[test]
    fn source_failure_keeps_the_previous_cursor_and_records_failed_status_only() {
        let store = Store::default();
        let clock = SteppedClock::new(at(200));
        let (events, recorded) = recording_bus();
        let feed = ScriptedNewsFeed::new(FeedSource::News(NewsSource::Hankyung), metadata());
        let failure = InformationError::Unavailable(ResetTime::Known(at(300)));
        feed.script_new_items().push(Err(failure));
        let round =
            run(CollectFeed::new(&store, &store, &clock, &events).collect(&feed, cursor("prev")))
                .expect("상태 기록");
        assert_eq!(round.outcome, FeedRoundOutcome::SourceFailed);
        assert_eq!(round.cursor, cursor("prev"));
        assert_eq!(*round.news.as_u32(), 0);
        assert_eq!(
            state(&store, DataSource::Hankyung),
            SourceState::Failed(failure)
        );
        assert_eq!(
            *recorded.read().expect("잠금"),
            vec![DomainEvent::SourceStatusChanged(DataSource::Hankyung)]
        );
    }

    #[test]
    fn missing_configuration_and_empty_results_are_recorded_without_collection_events() {
        let store = Store::default();
        let clock = SteppedClock::new(at(200));
        let (events, recorded) = recording_bus();
        let feed =
            ScriptedNewsFeed::new(FeedSource::Disclosure(DisclosureSource::Dart), metadata());
        feed.script_new_items()
            .push(Ok(DataAvailability::NotConfigured));
        feed.script_new_items().push(Ok(DataAvailability::NoData));
        let use_case = CollectFeed::new(&store, &store, &clock, &events);
        let first = run(use_case.collect(&feed, FeedCursor::Start)).expect("키 없음");
        assert_eq!(first.outcome, FeedRoundOutcome::NotConfigured);
        assert_eq!(
            state(&store, DataSource::DartDisclosureApi),
            SourceState::NotConfigured
        );
        let second = run(use_case.collect(&feed, FeedCursor::Start)).expect("자료 없음");
        assert_eq!(second.outcome, FeedRoundOutcome::NoData);
        assert_eq!(second.cursor, FeedCursor::Start);
        assert_eq!(
            state(&store, DataSource::DartDisclosureApi),
            SourceState::NoData
        );
        assert_eq!(
            *recorded.read().expect("잠금"),
            vec![
                DomainEvent::SourceStatusChanged(DataSource::DartDisclosureApi),
                DomainEvent::SourceStatusChanged(DataSource::DartDisclosureApi),
            ]
        );
    }

    #[test]
    fn storage_failure_stops_the_round_before_cursor_advance_or_status_write() {
        let store = Store::default();
        *store.fail_news.write().expect("잠금") = Some(storage_failure());
        let clock = SteppedClock::new(at(200));
        let (events, recorded) = recording_bus();
        let feed = ScriptedNewsFeed::new(FeedSource::News(NewsSource::Yonhap), metadata());
        feed.script_new_items()
            .push(Ok(DataAvailability::Available((
                vec![news("a")],
                cursor("next"),
            ))));
        let result =
            run(CollectFeed::new(&store, &store, &clock, &events).collect(&feed, cursor("prev")));
        assert_eq!(result, Err(storage_failure()));
        assert!(store.statuses.read().expect("잠금").is_empty());
        assert!(recorded.read().expect("잠금").is_empty());
    }

    #[test]
    fn collection_due_follows_cadence_minutes_and_turns_off_for_non_interval_cadences() {
        use application::{CollectionCadence, CollectionDue, CollectionMinutes, collection_due};
        let every = CollectionCadence::Every(CollectionMinutes::try_new(10).expect("10분"));
        assert_eq!(collection_due(every, None, at(0)), CollectionDue::Now);
        assert_eq!(
            collection_due(every, Some(at(0)), at(599)),
            CollectionDue::At(at(600))
        );
        assert_eq!(
            collection_due(every, Some(at(0)), at(600)),
            CollectionDue::Now
        );
        assert_eq!(
            collection_due(CollectionCadence::Off, None, at(0)),
            CollectionDue::Off
        );
        assert_eq!(
            collection_due(CollectionCadence::AfterUsClose, None, at(0)),
            CollectionDue::Off
        );
    }
}
