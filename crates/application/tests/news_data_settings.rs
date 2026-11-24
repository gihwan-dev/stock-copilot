#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};

    use application::{
        AliasName, CollectionCadence, CollectionConditions, CollectionMinutes, CollectionPhase,
        CollectionPlan, CollectionTarget, DartCallCount, DartDailyUsage, DartKeyState,
        EventBusBuilder, EventSubscriber, FomcDay, GetNewsDataSettings, NewsDataSettings,
        NewsDataSettingsChange, NewsDataSettingsError, NewsDataSettingsRepository,
        NewsSettingValueError, RetryDelay, SecContactSetting, SourceSwitch, StockAlias,
        SubscriberFailure, SystemFailure, SystemPort, SystemPortError, UpdateNewsDataSettings,
        WriteCompleted,
    };
    use domain::{
        AiBodyReadPolicy, AiBodyReadToggle, DataSource, DomainEvent, DomainEventKind, InstrumentId,
        MovePercent, NewsBurstCount, NewsSource, WakeThresholdError, WakeThresholds,
    };

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut pinned = Box::pin(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }

    #[derive(Debug, Default)]
    struct Repo {
        stored: RwLock<Option<NewsDataSettings>>,
        writes: RwLock<Vec<NewsDataSettings>>,
        fail_store: RwLock<Option<SystemPortError>>,
    }

    impl NewsDataSettingsRepository for Repo {
        fn load(
            &self,
        ) -> impl Future<Output = Result<Option<NewsDataSettings>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.stored.read().expect("잠금").clone()))
        }

        fn store(
            &self,
            settings: &NewsDataSettings,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            let failure = *self.fail_store.read().expect("잠금");
            if failure.is_none() {
                *self.stored.write().expect("잠금") = Some(settings.clone());
                self.writes.write().expect("잠금").push(settings.clone());
            }
            std::future::ready(failure.map_or(Ok(WriteCompleted), Err))
        }
    }

    struct Recorder(Arc<RwLock<Vec<DomainEvent>>>);

    impl EventSubscriber for Recorder {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .write()
                .map_err(|_poison| SubscriberFailure::Unavailable)?
                .push(event.clone());
            Ok(())
        }
    }

    fn alias(instrument: &str, name: &str) -> StockAlias {
        StockAlias {
            instrument: InstrumentId::try_new(instrument).expect("종목"),
            alias: AliasName::try_new(name).expect("약칭"),
        }
    }

    #[test]
    fn defaults_keep_every_source_on_body_reads_off_and_five_five_thresholds() {
        let repo = Repo::default();
        let settings = block_on(GetNewsDataSettings { repository: &repo }.execute()).expect("설정");
        for source in DataSource::all() {
            assert_eq!(settings.source_switch(source), SourceSwitch::On);
        }
        assert_eq!(
            settings.body_read().policy_for(&NewsSource::Hankyung),
            AiBodyReadPolicy::TitleAndSummary
        );
        assert_eq!(settings.wake(), WakeThresholds::default());
        assert_eq!(settings.wake().daily_move.as_u8(), &5);
        assert_eq!(settings.sec_contact(), None);
        assert_eq!(settings.aliases(), &[]);
    }

    #[test]
    fn each_change_is_stored_then_announces_settings_changed() {
        let repo = Repo::default();
        let events = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        let _id = builder.subscribe(
            [DomainEventKind::SettingsChanged].into(),
            Recorder(Arc::clone(&events)),
        );
        let bus = builder.build();
        let update = UpdateNewsDataSettings {
            repository: &repo,
            events: &bus,
        };
        let contact = SecContactSetting::try_new("Stock Copilot user@example.com").expect("연락처");
        for change in [
            NewsDataSettingsChange::Source(DataSource::GoogleNews, SourceSwitch::Off),
            NewsDataSettingsChange::BodyRead(NewsSource::Edaily, AiBodyReadToggle::Enabled),
            NewsDataSettingsChange::SecContact(Some(contact.clone())),
            NewsDataSettingsChange::Cadence(
                CollectionTarget::KindDisclosure,
                Some(CollectionMinutes::try_new(5).expect("주기")),
            ),
            NewsDataSettingsChange::Wake(WakeThresholds {
                daily_move: MovePercent::try_new(3).expect("변동"),
                news_per_hour: NewsBurstCount::try_new(8).expect("건수"),
            }),
            NewsDataSettingsChange::AddAlias(alias("KR7000660001", "하이닉스")),
        ] {
            let (_settings, report) = block_on(update.execute(change)).expect("변경");
            assert_eq!(report.failures, vec![]);
        }
        let saved = repo.stored.read().expect("잠금").clone().expect("저장");
        assert_eq!(
            saved.source_switch(DataSource::GoogleNews),
            SourceSwitch::Off
        );
        assert_eq!(saved.source_switch(DataSource::Hankyung), SourceSwitch::On);
        assert_eq!(
            saved.body_read().policy_for(&NewsSource::Edaily),
            AiBodyReadPolicy::BodyAllowed
        );
        assert_eq!(saved.sec_contact(), Some(&contact));
        assert_eq!(saved.wake().news_per_hour.as_u8(), &8);
        assert_eq!(saved.aliases(), &[alias("KR7000660001", "하이닉스")]);
        assert_eq!(repo.writes.read().expect("잠금").len(), 6_usize);
        assert_eq!(events.read().expect("잠금").len(), 6_usize);
        let (back_on, _report) = block_on(update.execute(NewsDataSettingsChange::Source(
            DataSource::GoogleNews,
            SourceSwitch::On,
        )))
        .expect("다시 켜기");
        assert_eq!(
            back_on.source_switch(DataSource::GoogleNews),
            SourceSwitch::On
        );
    }

    #[test]
    fn rejected_changes_store_nothing_and_publish_nothing() {
        let repo = Repo::default();
        let events = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        let _id = builder.subscribe(
            [DomainEventKind::SettingsChanged].into(),
            Recorder(Arc::clone(&events)),
        );
        let bus = builder.build();
        let update = UpdateNewsDataSettings {
            repository: &repo,
            events: &bus,
        };
        assert_eq!(
            block_on(update.execute(NewsDataSettingsChange::BodyRead(
                NewsSource::Yonhap,
                AiBodyReadToggle::Enabled
            ))),
            Err(NewsDataSettingsError::BodyReadLocked(NewsSource::Yonhap))
        );
        assert_eq!(
            block_on(update.execute(NewsDataSettingsChange::Cadence(
                CollectionTarget::TreasuryAndFred,
                Some(CollectionMinutes::try_new(10).expect("주기"))
            ))),
            Err(NewsDataSettingsError::CadenceNotAdjustable(
                CollectionTarget::TreasuryAndFred
            ))
        );
        assert_eq!(
            block_on(update.execute(NewsDataSettingsChange::RemoveAlias(
                AliasName::try_new("없음").expect("약칭")
            ))),
            Err(NewsDataSettingsError::AliasMissing)
        );
        let _first = block_on(update.execute(NewsDataSettingsChange::AddAlias(alias(
            "KR7005380001",
            "현대차",
        ))))
        .expect("약칭");
        assert_eq!(
            block_on(update.execute(NewsDataSettingsChange::AddAlias(alias(
                "KR7005380002",
                "현대차"
            )))),
            Err(NewsDataSettingsError::DuplicateAlias)
        );
        let error = SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        );
        *repo.fail_store.write().expect("잠금") = Some(error);
        assert_eq!(
            block_on(update.execute(NewsDataSettingsChange::Source(
                DataSource::Yonhap,
                SourceSwitch::Off
            ))),
            Err(NewsDataSettingsError::Storage(error))
        );
        assert_eq!(repo.writes.read().expect("잠금").len(), 1_usize);
        assert_eq!(events.read().expect("잠금").len(), 1_usize);
    }

    #[test]
    fn value_types_validate_contact_alias_and_threshold_ranges() {
        assert_eq!(
            SecContactSetting::try_new(" "),
            Err(NewsSettingValueError::Empty)
        );
        assert_eq!(
            SecContactSetting::try_new("Stock Copilot"),
            Err(NewsSettingValueError::MissingEmail)
        );
        assert_eq!(
            SecContactSetting::try_new("이름 user@example.com"),
            Err(NewsSettingValueError::InvalidCharacter)
        );
        assert_eq!(
            SecContactSetting::try_new(&format!("a@b.co {}", "x".repeat(260))),
            Err(NewsSettingValueError::TooLong)
        );
        assert_eq!(
            AliasName::try_new(&"가".repeat(33)),
            Err(NewsSettingValueError::TooLong)
        );
        assert_eq!(
            AliasName::try_new("a\nb"),
            Err(NewsSettingValueError::InvalidCharacter)
        );
        assert_eq!(MovePercent::try_new(0), Err(WakeThresholdError::OutOfRange));
        assert_eq!(
            MovePercent::try_new(31),
            Err(WakeThresholdError::OutOfRange)
        );
        assert_eq!(
            NewsBurstCount::try_new(1),
            Err(WakeThresholdError::OutOfRange)
        );
        assert_eq!(
            NewsBurstCount::try_new(51),
            Err(WakeThresholdError::OutOfRange)
        );
    }

    #[test]
    fn cadence_overrides_apply_only_to_running_collections() {
        let mut settings = NewsDataSettings::initial();
        settings
            .apply(NewsDataSettingsChange::Cadence(
                CollectionTarget::KindDisclosure,
                Some(CollectionMinutes::try_new(5).expect("주기")),
            ))
            .expect("변경");
        settings
            .apply(NewsDataSettingsChange::Cadence(
                CollectionTarget::DartList,
                Some(CollectionMinutes::try_new(5).expect("주기")),
            ))
            .expect("변경");
        let plan = CollectionPlan::build(&CollectionConditions {
            phase: CollectionPhase::DomesticSession,
            fomc: FomcDay::Regular,
            dart_key: DartKeyState::Missing,
            dart_usage: DartDailyUsage::new(DartCallCount::new(0), DartCallCount::new(20_000)),
        });
        let adjusted = settings.adjust_plan(&plan);
        let cadence = |target| {
            adjusted
                .iter()
                .find(|entry| entry.target == target)
                .map(|entry| entry.cadence)
        };
        assert_eq!(
            cadence(CollectionTarget::KindDisclosure),
            Some(CollectionCadence::Every(
                CollectionMinutes::try_new(5).expect("주기")
            ))
        );
        assert_eq!(
            cadence(CollectionTarget::DartList),
            Some(CollectionCadence::Off)
        );
        assert_eq!(
            cadence(CollectionTarget::TreasuryAndFred),
            Some(CollectionCadence::AfterUsClose)
        );
        settings
            .apply(NewsDataSettingsChange::Cadence(
                CollectionTarget::KindDisclosure,
                None,
            ))
            .expect("되돌리기");
        assert_eq!(
            settings.cadence_override(CollectionTarget::KindDisclosure),
            None
        );
    }
}
