use src_tauri::audit_queue::AuditEventQueue;
use src_tauri::ipc::briefing::BriefingIpcState;
use src_tauri::ipc::open_external::ExternalLinkRuntime;
use src_tauri::platform::notifier::{
    AsyncRuntimeNotificationDispatch, FilteredNotifier, NotificationDelivery,
    NotificationEventSubscriber, NotificationPreferences, UnavailableNotificationDelivery,
};
use src_tauri::platform::scheduling::SpecWatchInputs;
use src_tauri::platform::single_instance::{InstanceAcquisition, SocketPath, attach_to_app};
use src_tauri::product_events::{
    BridgedUiEvents, FlushingAuditSink, RecordedTriggerUrgency, UiEventSlot, ui_event_kinds,
};
use src_tauri::{build_app, run_app};
use src_tauri::{
    database::{DatabaseFactory, DatabaseKeyProvision},
    ports::Ports,
    product_ports::ProductPorts,
    unavailable::Unavailable,
};
use std::sync::Arc;
use store_sqlite::{SharedDatabase, SqliteRepositories};

fn product_context() -> tauri::Context<tauri::Wry> {
    #[expect(
        clippy::disallowed_macros,
        reason = "tauri::generate_context! expands to include_bytes! and env!"
    )]
    #[expect(
        clippy::disallowed_methods,
        reason = "tauri::generate_context! expands to std::process::exit"
    )]
    #[expect(
        clippy::disallowed_types,
        reason = "tauri::generate_context! expands to std::collections::HashMap"
    )]
    let context = tauri::generate_context!();
    context
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = product_context();
    let clock = platform_macos::SystemClock::new()
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let shared_clock = Arc::new(clock);
    let repositories = Arc::new(SqliteRepositories::new(SharedDatabase::new()));
    let notifier = Arc::new(FilteredNotifier::new(
        UnavailableNotificationDelivery,
        NotificationPreferences::default(),
    ));
    let ui_events = Arc::new(UiEventSlot::<tauri::Wry>::default());
    let (events, scheduler_events) = product_event_bus(ProductSubscribers {
        notifier: Arc::clone(&notifier),
        audit: Arc::clone(&repositories),
        clock: Arc::clone(&shared_clock),
        ui_events: Arc::clone(&ui_events),
    });
    let ports = product_ports(&repositories, notifier, &shared_clock, events);
    let app = build_app(
        product_builder(scheduler_events, &repositories, ui_events, &shared_clock),
        ports,
        context,
    )?;
    let socket_path = SocketPath::try_new(
        tauri::Manager::path(&app)
            .app_data_dir()?
            .join("instance.sock"),
    )
    .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let single_instance = attach_to_app(app.handle(), socket_path)
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    match single_instance {
        InstanceAcquisition::Primary(_guard) => {
            run_app(app, |handle, code| handle.exit(code.as_i32()));
            Ok(())
        }
        InstanceAcquisition::SecondaryNotified => Ok(()),
    }
}

fn product_ports(
    repositories: &Arc<SqliteRepositories>,
    notifier: Arc<ProductNotifier>,
    shared_clock: &Arc<platform_macos::SystemClock>,
    events: application::EventBus,
) -> Ports<ProductPorts<SqliteRepositories, ProductSecrets>> {
    Ports::<ProductPorts<SqliteRepositories, ProductSecrets>> {
        broker_read: Arc::new(Unavailable),
        market_info: Arc::new(Unavailable),
        macro_series: Arc::new(Unavailable),
        fundamentals: Arc::new(Unavailable),
        ai_provider: Arc::new(Unavailable),
        chat_repository: Arc::clone(repositories),
        chat_ledger: Arc::clone(repositories),
        chat_queue: Arc::new(Unavailable),
        warm_chat: Arc::new(Unavailable),
        portfolios: Arc::new(Unavailable),
        transactions: Arc::clone(repositories),
        market_data: Arc::clone(repositories),
        news: Arc::clone(repositories),
        theses: Arc::clone(repositories),
        estimates: Arc::clone(repositories),
        signals: Arc::clone(repositories),
        proposals: Arc::clone(repositories),
        valuations: Arc::clone(repositories),
        settings: Arc::clone(repositories),
        audit: Arc::clone(repositories),
        secrets: Arc::new(product_secrets()),
        notifier,
        news_feeds: Vec::new(),
        clock: Arc::clone(shared_clock),
        presence: Arc::new(platform_macos::MacOsRiskDialogPresence::unavailable()),
        events,
        saved_accounts: Vec::new(),
        database: Arc::new(database_factory(Arc::clone(shared_clock))),
    }
}

type ProductSecrets = secrets_keychain::KeychainSecretStore<secrets_keychain::SystemKeychain>;

fn product_secrets() -> ProductSecrets {
    let build = if tauri::is_dev() {
        secrets_keychain::BuildKind::Development
    } else {
        secrets_keychain::BuildKind::Release
    };
    secrets_keychain::KeychainSecretStore::new(
        secrets_keychain::KeychainProfile::for_build(build),
        secrets_keychain::SystemKeychain,
    )
}

fn database_key_provision() -> DatabaseKeyProvision {
    DatabaseKeyProvision(Box::new(secrets_keychain::generate_database_key))
}

fn product_builder(
    scheduler_events: Arc<scheduler::SchedulerEventBridge>,
    repositories: &Arc<SqliteRepositories>,
    ui_events: Arc<UiEventSlot<tauri::Wry>>,
    clock: &Arc<platform_macos::SystemClock>,
) -> tauri::Builder<tauri::Wry> {
    let builder = tauri::Builder::default()
        .manage(ExternalLinkRuntime::new(
            Arc::new(Unavailable),
            Arc::new(Unavailable),
        ))
        .manage(scheduler_events)
        .manage(Arc::clone(repositories))
        .manage(BriefingIpcState::from_repository(Arc::clone(repositories)))
        .manage(database_key_provision())
        .manage(ui_events);
    match spec_watch_inputs(clock) {
        Some(inputs) => builder.manage(inputs),
        None => builder,
    }
}

fn spec_watch_inputs(clock: &Arc<platform_macos::SystemClock>) -> Option<SpecWatchInputs> {
    let shared: Arc<dyn application::Clock> = Arc::<platform_macos::SystemClock>::clone(clock);
    let http = broker_toss::TossHttpClient::new(
        &broker_toss::TossEndpoint::production(),
        shared,
        Arc::new(broker_toss::TokioRequestWait),
    )
    .ok()?;
    let baseline = broker_toss::pinned_spec_baseline().ok()?;
    Some(SpecWatchInputs {
        source: Arc::new(broker_toss::TossSpecClient::new(http)),
        baseline,
    })
}

type ProductNotifier = FilteredNotifier<UnavailableNotificationDelivery>;

struct ProductSubscribers<D> {
    notifier: Arc<FilteredNotifier<D>>,
    audit: Arc<SqliteRepositories>,
    clock: Arc<platform_macos::SystemClock>,
    ui_events: Arc<UiEventSlot<tauri::Wry>>,
}

fn product_event_bus<D: NotificationDelivery + 'static>(
    subscribers: ProductSubscribers<D>,
) -> (application::EventBus, Arc<scheduler::SchedulerEventBridge>) {
    let bridge = Arc::new(scheduler::SchedulerEventBridge::new());
    let mut builder = application::EventBusBuilder::default();
    builder.subscribe(
        scheduler::SchedulerEventBridge::subscribed_kinds(),
        scheduler::BridgedSchedulerEvents(Arc::clone(&bridge)),
    );
    builder.subscribe(ui_event_kinds(), BridgedUiEvents(subscribers.ui_events));
    builder.subscribe(
        NotificationEventSubscriber::<
            AsyncRuntimeNotificationDispatch<FilteredNotifier<D>>,
            RecordedTriggerUrgency,
        >::subscribed_kinds(),
        NotificationEventSubscriber::new(
            AsyncRuntimeNotificationDispatch::new(subscribers.notifier),
            RecordedTriggerUrgency,
        ),
    );
    let clock: Arc<dyn application::Clock> = subscribers.clock;
    builder.subscribe(
        application::AuditEventRecorder::<FlushingAuditSink<SqliteRepositories>>::kinds(),
        application::AuditEventRecorder::new(
            clock,
            FlushingAuditSink::new(AuditEventQueue::default(), subscribers.audit),
        ),
    );
    (builder.build(), bridge)
}

fn database_factory<C: application::Clock + 'static>(clock: Arc<C>) -> DatabaseFactory {
    DatabaseFactory(Box::new(move |path, key| {
        src_tauri::database::prepare_directory(&path)?;
        store_sqlite::Database::open(&path, &key, clock.as_ref())
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn product_acl_allows_only_main_window_event_subscription() {
        let mut context = super::product_context();
        let authority = context.runtime_authority_mut();
        let local = tauri::ipc::Origin::Local;
        let remote = tauri::ipc::Origin::Remote {
            url: "https://example.com".parse().expect("원격 출처"),
        };
        for command in ["plugin:event|listen", "plugin:event|unlisten"] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &local)
                    .is_some(),
                "{command}"
            );
            assert!(
                authority
                    .resolve_access(command, "other", "other", &local)
                    .is_none(),
                "{command}"
            );
            assert!(
                authority
                    .resolve_access(command, "main", "main", &remote)
                    .is_none(),
                "{command}"
            );
        }
        for command in ["plugin:event|emit", "plugin:event|emit_to"] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &local)
                    .is_none(),
                "{command}"
            );
        }
        let without_permissions = tauri::runtime_authority!(
            std::collections::BTreeMap::new(),
            tauri::utils::acl::resolved::Resolved::default()
        );
        for command in ["plugin:event|listen", "plugin:event|unlisten"] {
            assert!(
                without_permissions
                    .resolve_access(command, "main", "main", &local)
                    .is_none(),
                "{command}"
            );
        }
    }

    #[test]
    fn product_event_permissions_are_individually_required() {
        use std::collections::BTreeMap;
        use tauri::utils::acl::{capability::Capability, manifest::Manifest, resolved::Resolved};

        let manifests: BTreeMap<String, Manifest> = serde_json::from_str(
            &std::fs::read_to_string("gen/schemas/acl-manifests.json").expect("생성된 ACL"),
        )
        .expect("ACL 매니페스트");
        let capability: Capability = serde_json::from_str(
            &std::fs::read_to_string("capabilities/main.json").expect("제품 capability"),
        )
        .expect("제품 capability 형식");
        for (omitted, denied, retained) in [
            (
                "core:event:allow-listen",
                "plugin:event|listen",
                "plugin:event|unlisten",
            ),
            (
                "core:event:allow-unlisten",
                "plugin:event|unlisten",
                "plugin:event|listen",
            ),
        ] {
            let mut edited = capability.clone();
            edited
                .permissions
                .retain(|permission| permission.identifier().get() != omitted);
            assert_eq!(edited.permissions.len() + 1, capability.permissions.len());
            let capabilities = BTreeMap::from([(edited.identifier.clone(), edited)]);
            let resolved = Resolved::resolve(
                &manifests,
                capabilities,
                tauri::utils::platform::Target::MacOS,
            )
            .expect("제품 ACL 해석");
            let authority = tauri::runtime_authority!(BTreeMap::new(), resolved);
            assert!(
                authority
                    .resolve_access(denied, "main", "main", &tauri::ipc::Origin::Local)
                    .is_none(),
                "{omitted}: {denied}"
            );
            assert!(
                authority
                    .resolve_access(retained, "main", "main", &tauri::ipc::Origin::Local)
                    .is_some(),
                "{omitted}: {retained}"
            );
        }
    }

    #[test]
    fn composition_root_constructs_the_fixed_product_endpoint_without_sending_requests() {
        let endpoint = broker_toss::TossEndpoint::production();
        assert!(format!("{endpoint:?}").contains("Production"));
        let spec: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("spec"),
        )
        .expect("JSON");
        let expected_origin = spec["servers"][0]["url"].as_str().expect("origin");
        assert_eq!(
            format!("{endpoint:?}"),
            format!("TossEndpoint {{ origin: {expected_origin:?}, mode: Production }}")
        );
        let client = broker_toss::TossHttpClient::new(
            &endpoint,
            std::sync::Arc::new(testkit_ports::SteppedClock::new(
                broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
                    .expect("clock")
                    .into_domain(),
            )),
            std::sync::Arc::new(broker_toss::TokioRequestWait),
        )
        .expect("product configuration");
        assert_eq!(format!("{client:?}"), "TossHttpClient(<redacted>)");
    }

    #[test]
    fn configured_database_is_opened_and_explicitly_closed_after_refresh_cleanup() {
        use src_tauri::database::{DatabaseStatus, open_database};
        use src_tauri::lifecycle::Lifecycle;
        use std::sync::Arc;
        use testkit_ports::{memory::MemoryReply, system_fakes::MemorySecretStore};
        let directory = std::path::PathBuf::from("../../../target/U-01")
            .join(format!("database-lifecycle-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("테스트 폴더");
        let path = store_sqlite::DatabasePath::new(directory.join("nested").join("store.db"));
        let secrets = MemorySecretStore::default();
        secrets.script_load().push(MemoryReply::Return(Ok(Some(
            application::SecretValue::new(vec![83; 32]),
        ))));
        let clock = Arc::new(testkit_ports::SteppedClock::new(
            broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
                .expect("clock")
                .into_domain(),
        ));
        let factory = Arc::new(super::database_factory(clock));
        tauri::async_runtime::block_on(async {
            let state = open_database(&secrets, factory, || Ok(path)).await;
            assert_eq!(state.status(), DatabaseStatus::Ready);
            let lifecycle =
                Lifecycle::new(tauri::async_runtime::spawn(std::future::pending()), state);
            lifecycle.shutdown().await;
            let guard = lifecycle.outcome().lock().await;
            match &guard.as_ref().expect("종료 결과").database {
                Ok(src_tauri::lifecycle::DatabaseClosure::Closed(_receipt)) => {}
                Ok(src_tauri::lifecycle::DatabaseClosure::NotOpened) => {
                    panic!("열린 DB의 명시적 종료 증거 필요")
                }
                Err(error) => panic!("종료 실패: {error:?}"),
            }
        });
        std::fs::remove_dir_all(directory).expect("테스트 정리");
    }

    #[test]
    fn product_acl_allows_open_external_only_from_main_local_origin() {
        let mut context = super::product_context();
        let authority = context.runtime_authority_mut();
        let local = tauri::ipc::Origin::Local;
        let remote = tauri::ipc::Origin::Remote {
            url: "https://example.com".parse().expect("원격 출처"),
        };
        assert!(
            authority
                .resolve_access("open_external", "main", "main", &local)
                .is_some(),
            "main 로컬 출처 허용"
        );
        assert!(
            authority
                .resolve_access("open_external", "other", "other", &local)
                .is_none(),
            "다른 창 거부"
        );
        assert!(
            authority
                .resolve_access("open_external", "main", "main", &remote)
                .is_none(),
            "원격 출처 거부"
        );
    }

    #[test]
    fn composition_root_constructs_sources_client_without_broker_credentials_or_hosts() {
        let clock = std::sync::Arc::new(testkit_ports::SteppedClock::new(
            broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
                .expect("clock")
                .into_domain(),
        ));
        let client = sources::SourceHttpClient::new(
            sources::SourceEndpoint::live(),
            clock,
            std::sync::Arc::new(sources::TokioSourceWait),
            None,
        )
        .expect("live sources client");
        assert_eq!(format!("{client:?}"), "SourceHttpClient(<redacted>)");
        for spec in sources::SourceRegistry::all() {
            for host in spec.allowed_hosts() {
                assert!(!host.to_ascii_lowercase().contains("toss"), "{host}");
            }
            match spec.credential() {
                sources::CredentialRequirement::None
                | sources::CredentialRequirement::Optional(
                    application::CredentialSource::Dart | application::CredentialSource::Fred,
                ) => {}
            }
        }
    }
}

#[cfg(test)]
mod product_bus_tests {
    use super::{ProductSubscribers, product_event_bus};
    use application::{Notification, SystemPortError, WriteCompleted};
    use domain::{DomainEvent, InstrumentId};
    use src_tauri::platform::notifier::{
        FilteredNotifier, NotificationDelivery, NotificationMessage, NotificationPreferences,
        NotificationToggle,
    };
    use src_tauri::product_events::UiEventSlot;
    use std::sync::{Arc, mpsc};
    use std::time::Duration;
    use store_sqlite::{SharedDatabase, SqliteRepositories};

    type Sent = (Notification, NotificationMessage);

    struct ChannelDelivery(mpsc::Sender<Sent>);

    impl NotificationDelivery for ChannelDelivery {
        fn deliver(
            &self,
            notification: Notification,
            message: NotificationMessage,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            let _sent = self.0.send((notification, message));
            std::future::ready(Ok(WriteCompleted))
        }
    }

    fn instrument() -> InstrumentId {
        InstrumentId::try_new("005930").expect("종목")
    }

    #[test]
    fn product_event_bus_turns_each_new_signal_into_one_macos_notification() {
        let (sender, receiver) = mpsc::channel();
        let notifier = Arc::new(FilteredNotifier::new(
            ChannelDelivery(sender),
            NotificationPreferences::default(),
        ));
        let clock = Arc::new(platform_macos::SystemClock::new().expect("시계"));
        let (events, _bridge) = product_event_bus(ProductSubscribers {
            notifier: Arc::clone(&notifier),
            audit: Arc::new(SqliteRepositories::new(SharedDatabase::new())),
            clock,
            ui_events: Arc::new(UiEventSlot::<tauri::Wry>::default()),
        });
        let _cleared = events.publish(DomainEvent::SignalCleared(instrument()));
        let _raised = events.publish(DomainEvent::SignalRaised(instrument()));
        let (notification, message) = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("새 신호 알림");
        assert_eq!(notification, Notification::NewSignal);
        assert_eq!(message.title, "새 매매 신호");
        assert!(
            receiver.recv_timeout(Duration::from_millis(300)).is_err(),
            "신호 해제는 알리지 않는다"
        );
        notifier
            .set_toggle(Notification::NewSignal, NotificationToggle::Disabled)
            .expect("끄기");
        let _muted = events.publish(DomainEvent::SignalRaised(instrument()));
        assert!(
            receiver.recv_timeout(Duration::from_millis(300)).is_err(),
            "꺼 둔 알림은 보내지 않는다"
        );
    }
}

#[cfg(test)]
mod briefing_store_tests {
    use application::{BriefingOutcome, BriefingRecord, BriefingRepository, SecretValue};
    use domain::{
        BriefingId, BriefingKind, CalendarDate, CalendarYear, DayNumber, MonthNumber, UntrustedText,
    };
    use src_tauri::ipc::briefing::{BriefingIpcState, UiBriefingStatus, load_briefings_view};
    use std::sync::Arc;
    use store_sqlite::{SharedDatabase, SqliteRepositories};

    fn finished_at() -> domain::Timestamp {
        broker_toss::TossTimestamp::try_from("2026-11-24T23:20:00Z")
            .expect("clock")
            .into_domain()
    }

    fn clock() -> testkit_ports::SteppedClock {
        testkit_ports::SteppedClock::new(finished_at())
    }

    #[test]
    fn installed_database_serves_saved_briefings_to_the_briefing_ipc() {
        let directory = std::path::PathBuf::from("../../../target/chat2")
            .join(format!("sqlite-briefings-{}", std::process::id()));
        let shared = SharedDatabase::new();
        let repositories = Arc::new(SqliteRepositories::new(shared.clone()));
        let state = BriefingIpcState::from_repository(Arc::clone(&repositories));
        tauri::async_runtime::block_on(async {
            let missing = load_briefings_view(&state).await;
            assert!(missing.is_err(), "설치 전에는 읽지 못한다");
            let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
            src_tauri::database::prepare_directory(&path).expect("폴더");
            let database =
                store_sqlite::Database::open(&path, &SecretValue::new(vec![83; 32]), &clock())
                    .expect("열기");
            assert!(shared.install(database).await.is_none());
            let date = CalendarDate::from_parts(
                CalendarYear::new(2026),
                MonthNumber::new(11),
                DayNumber::new(25),
            )
            .expect("date");
            let record = BriefingRecord {
                id: BriefingId::new(BriefingKind::Morning, date),
                finished_at: finished_at(),
                outcome: BriefingOutcome::Completed {
                    body: UntrustedText::try_new("오늘 브리핑").expect("text"),
                    sources: Vec::new(),
                },
            };
            let _saved = repositories.save(&record).await.expect("저장");
            let view = load_briefings_view(&state).await.expect("조회");
            let latest = view.latest.expect("최근 브리핑");
            assert_eq!(latest.body, "오늘 브리핑");
            assert_eq!(latest.status, UiBriefingStatus::Completed);
            assert_eq!(latest.finished_at, "2026-11-25T08:20:00+09:00");
            let _closed = shared.close().await;
        });
        std::fs::remove_dir_all(directory).expect("정리");
    }
}

#[cfg(test)]
mod key_provision_tests {
    use application::SecretValue;
    use src_tauri::database::{DatabaseKeyProvision, DatabaseStatus, open_provisioned_database};
    use src_tauri::lifecycle::{DatabaseClosure, Lifecycle};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use testkit_ports::memory::MemoryReply;
    use testkit_ports::system_fakes::MemorySecretStore;

    const FAKE_KEY: [u8; 32] = *b"fake-db-key-9c41e7-never-on-disk";

    fn clock() -> Arc<testkit_ports::SteppedClock> {
        Arc::new(testkit_ports::SteppedClock::new(
            broker_toss::TossTimestamp::try_from("2026-11-25T00:00:00Z")
                .expect("clock")
                .into_domain(),
        ))
    }

    fn provision(count: Arc<AtomicUsize>) -> DatabaseKeyProvision {
        DatabaseKeyProvision(Box::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
            Ok(SecretValue::new(FAKE_KEY.to_vec()))
        }))
    }

    async fn launch(
        secrets: &MemorySecretStore,
        count: &Arc<AtomicUsize>,
        path: &std::path::Path,
    ) -> DatabaseStatus {
        let state = open_provisioned_database(
            secrets,
            &provision(Arc::clone(count)),
            Arc::new(super::database_factory(clock())),
            store_sqlite::DatabasePath::new(path.to_path_buf()),
        )
        .await;
        let status = state.status();
        let lifecycle = Lifecycle::new(tauri::async_runtime::spawn(std::future::pending()), state);
        lifecycle.shutdown().await;
        let guard = lifecycle.outcome().lock().await;
        match &guard.as_ref().expect("종료 결과").database {
            Ok(DatabaseClosure::Closed(_receipt)) => {}
            Ok(DatabaseClosure::NotOpened) => panic!("열린 DB의 명시적 종료 필요"),
            Err(error) => panic!("종료 실패: {error:?}"),
        }
        status
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        (0..=haystack.len().saturating_sub(needle.len()))
            .any(|start| haystack.get(start..start.saturating_add(needle.len())) == Some(needle))
    }

    #[test]
    fn first_launch_creates_the_key_and_the_next_launch_reopens_without_key_bytes_on_disk() {
        let directory = std::path::PathBuf::from("../../../target/fin-c")
            .join(format!("product-key-{}", std::process::id()));
        let path = directory.join("data").join("stock-copilot.db");
        let secrets = MemorySecretStore::default();
        for _ in 0..4_usize {
            secrets.script_load().push(MemoryReply::Memory);
        }
        secrets.script_store().push(MemoryReply::Memory);
        let count = Arc::new(AtomicUsize::new(0));
        tauri::async_runtime::block_on(async {
            assert_eq!(launch(&secrets, &count, &path).await, DatabaseStatus::Ready);
            assert_eq!(launch(&secrets, &count, &path).await, DatabaseStatus::Ready);
        });
        assert_eq!(count.load(Ordering::SeqCst), 1, "키는 첫 실행에만 만든다");
        let hex: Vec<u8> = FAKE_KEY
            .iter()
            .flat_map(|byte| format!("{byte:02x}").into_bytes())
            .collect();
        let mut scanned = 0_usize;
        for entry in std::fs::read_dir(path.parent().expect("폴더")).expect("목록") {
            let bytes = std::fs::read(entry.expect("항목").path()).expect("파일");
            assert!(!contains(&bytes, &FAKE_KEY), "원시 키가 파일에 있음");
            assert!(!contains(&bytes, &hex), "16진 키가 파일에 있음");
            scanned = scanned.saturating_add(1);
        }
        assert!(scanned >= 1, "DB 파일을 검사함");
        std::fs::remove_dir_all(directory).expect("정리");
    }

    #[test]
    fn product_secrets_use_the_development_keychain_profile_outside_tauri_build() {
        assert_eq!(
            super::product_secrets().profile(),
            &secrets_keychain::KeychainProfile::development()
        );
    }
}
