use crate::platform::notifier::{
    AsyncRuntimeNotificationDispatch, DisclosureUrgency, DisclosureUrgencyLookup, FilteredNotifier,
    NotificationDelivery, NotificationDispatch, NotificationEventSubscriber, NotificationMessage,
    NotificationPreferences, NotificationToggle, UnavailableNotificationDelivery, message_for,
};
use application::{
    EventBusBuilder, Notification, Notifier, RetryDelay, SubscriberFailure, SystemFailure,
    SystemPort, SystemPortError, WriteCompleted,
};
use domain::{
    DartReceiptNo, Disclosure, DisclosureKey, DomainEvent, InstrumentId, NormalizedUrl, ReceiptNo,
    UntrustedText,
};
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock, mpsc};
use std::time::Duration;

#[derive(Default)]
struct RecordingDelivery {
    sent: RwLock<Vec<(Notification, NotificationMessage)>>,
    channel: Option<mpsc::Sender<Notification>>,
}

impl RecordingDelivery {
    fn with_channel(sender: mpsc::Sender<Notification>) -> Self {
        Self {
            sent: RwLock::new(Vec::new()),
            channel: Some(sender),
        }
    }

    fn sent(&self) -> Vec<(Notification, NotificationMessage)> {
        self.sent.read().expect("기록 조회").clone()
    }
}

impl NotificationDelivery for RecordingDelivery {
    fn deliver(
        &self,
        notification: Notification,
        message: NotificationMessage,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let result = match self.sent.write() {
            Ok(mut guard) => {
                guard.push((notification, message));
                if let Some(sender) = &self.channel
                    && let Err(_error) = sender.send(notification)
                {}
                Ok(WriteCompleted)
            }
            Err(_error) => Err(SystemPortError::new(
                SystemPort::Notifier,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            )),
        };
        std::future::ready(result)
    }
}

struct FixedUrgencyLookup {
    urgent_keys: BTreeSet<String>,
}

impl DisclosureUrgencyLookup for FixedUrgencyLookup {
    fn urgency(&self, key: &DisclosureKey) -> DisclosureUrgency {
        let receipt = match key.receipt_no() {
            ReceiptNo::Kind(value) => value.as_str(),
            ReceiptNo::Dart(value) => value.as_str(),
            ReceiptNo::SecEdgar(value) => value.as_str(),
        };
        if self.urgent_keys.contains(receipt) {
            DisclosureUrgency::Urgent
        } else {
            DisclosureUrgency::NonUrgent
        }
    }
}

struct SyncRecordingDispatch {
    recorded: Arc<RwLock<Vec<Notification>>>,
}

impl NotificationDispatch for SyncRecordingDispatch {
    fn dispatch(&self, notification: Notification) -> Result<(), SubscriberFailure> {
        let mut guard = self
            .recorded
            .write()
            .map_err(|_error| SubscriberFailure::Unavailable)?;
        guard.push(notification);
        Ok(())
    }
}

fn sample_disclosure_key(receipt: &str) -> DisclosureKey {
    Disclosure::new(
        ReceiptNo::Dart(DartReceiptNo::try_new(receipt).expect("접수 번호")),
        NormalizedUrl::try_new("https://example.com/disclosure").expect("주소"),
        UntrustedText::try_new("샘플 공시").expect("제목"),
        None,
    )
    .key()
    .clone()
}

#[test]
fn notification_messages_are_korean_and_free_of_internal_ux6_terms() {
    let banned_terms = [
        "MCP",
        "세션",
        "프로필",
        "격리",
        "큐",
        "보정",
        "사전 점검",
        "토큰",
    ];
    for kind in [
        Notification::NewSignal,
        Notification::UrgentDisclosure,
        Notification::BriefingFinished,
    ] {
        let message = message_for(kind);
        assert!(!message.title.is_empty(), "알림 제목 필수");
        assert!(!message.body.is_empty(), "알림 본문 필수");
        for term in banned_terms {
            assert!(
                !message.title.contains(term) && !message.body.contains(term),
                "UX-6 금지어 포함 금지: {term}"
            );
        }
    }
}

#[test]
fn filtered_notifier_respects_per_kind_toggles_and_unavailable_delivery() {
    tauri::async_runtime::block_on(async {
        let notifier = FilteredNotifier::new(
            RecordingDelivery::default(),
            NotificationPreferences::default(),
        );
        for kind in [
            Notification::NewSignal,
            Notification::UrgentDisclosure,
            Notification::BriefingFinished,
        ] {
            notifier.notify(kind).await.expect("기본 활성 전달");
        }
        assert_eq!(notifier.delivery().sent().len(), 3, "기본 3종 모두 전달");

        notifier
            .set_toggle(Notification::NewSignal, NotificationToggle::Disabled)
            .expect("신호 알림 끄기");
        notifier
            .set_toggle(Notification::BriefingFinished, NotificationToggle::Disabled)
            .expect("브리핑 알림 끄기");
        notifier
            .notify(Notification::NewSignal)
            .await
            .expect("꺼진 신호 무시");
        notifier
            .notify(Notification::BriefingFinished)
            .await
            .expect("꺼진 브리핑 무시");
        notifier
            .notify(Notification::UrgentDisclosure)
            .await
            .expect("켜진 긴급 공시 전달");
        assert_eq!(
            notifier.delivery().sent().len(),
            4,
            "꺼진 두 종류는 전송 0회, 켜진 긴급 공시만 1회 추가"
        );

        let unavailable = FilteredNotifier::new(
            UnavailableNotificationDelivery,
            NotificationPreferences::default(),
        );
        let error = unavailable
            .notify(Notification::NewSignal)
            .await
            .expect_err("OS 알림 플러그인 부재 시 Unavailable 오류");
        assert_eq!(error.port(), SystemPort::Notifier, "포트 분류 일치");
        unavailable
            .set_toggle(Notification::NewSignal, NotificationToggle::Disabled)
            .expect("끄기");
        assert_eq!(
            unavailable.notify(Notification::NewSignal).await,
            Ok(WriteCompleted),
            "꺼진 종류는 전송 어댑터를 호출하지 않는다"
        );
    });
}

#[test]
fn event_subscriber_dispatches_only_new_signals_and_urgent_disclosures() {
    let recorded = Arc::new(RwLock::new(Vec::new()));
    let subscriber = NotificationEventSubscriber::new(
        SyncRecordingDispatch {
            recorded: Arc::clone(&recorded),
        },
        FixedUrgencyLookup {
            urgent_keys: BTreeSet::from([String::from("20261124000001")]),
        },
    );
    let mut builder = EventBusBuilder::default();
    let _subscription = builder.subscribe(
        NotificationEventSubscriber::<SyncRecordingDispatch, FixedUrgencyLookup>::subscribed_kinds(
        ),
        subscriber,
    );
    let bus = builder.build();

    let urgent_key = sample_disclosure_key("20261124000001");
    let normal_key = sample_disclosure_key("20261124000002");
    let signal_report = bus.publish(DomainEvent::SignalRaised(
        InstrumentId::try_new("005930").expect("종목 ID"),
    ));
    let normal_report = bus.publish(DomainEvent::DisclosureTriggerRecorded(normal_key));
    let urgent_report = bus.publish(DomainEvent::DisclosureTriggerRecorded(urgent_key));
    let settings_report = bus.publish(DomainEvent::SettingsChanged);

    assert!(signal_report.failures.is_empty(), "신호 이벤트 전달 성공");
    assert!(
        normal_report.failures.is_empty(),
        "비긴급 공시 이벤트 처리 성공"
    );
    assert!(
        urgent_report.failures.is_empty(),
        "긴급 공시 이벤트 전달 성공"
    );
    assert!(
        settings_report.failures.is_empty(),
        "무관한 이벤트 무시 성공"
    );
    assert_eq!(
        *recorded.read().expect("수신 기록"),
        vec![Notification::NewSignal, Notification::UrgentDisclosure],
        "새 신호와 긴급 공시만 알림 대상으로 전달되어야 한다"
    );
}

#[test]
fn async_runtime_dispatch_delivers_through_filtered_notifier_without_blocking() {
    let (sender, receiver) = mpsc::channel();
    let notifier = Arc::new(FilteredNotifier::new(
        RecordingDelivery::with_channel(sender),
        NotificationPreferences::default(),
    ));
    let dispatch = AsyncRuntimeNotificationDispatch::new(Arc::clone(&notifier));
    let subscriber = NotificationEventSubscriber::new(
        dispatch,
        FixedUrgencyLookup {
            urgent_keys: BTreeSet::new(),
        },
    );
    let mut builder = EventBusBuilder::default();
    let _subscription = builder.subscribe(
        NotificationEventSubscriber::<
            AsyncRuntimeNotificationDispatch<FilteredNotifier<RecordingDelivery>>,
            FixedUrgencyLookup,
        >::subscribed_kinds(),
        subscriber,
    );
    let bus = builder.build();
    let report = bus.publish(DomainEvent::SignalRaised(
        InstrumentId::try_new("AAPL").expect("종목 ID"),
    ));
    assert!(report.failures.is_empty(), "비동기 디스패치 성공");
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("비동기 알림 수신"),
        Notification::NewSignal,
        "비동기 런타임에서 FilteredNotifier까지 전달 완료"
    );
}

#[test]
fn briefing_finished_event_dispatches_briefing_notification() {
    let recorded = Arc::new(RwLock::new(Vec::new()));
    let subscriber = NotificationEventSubscriber::new(
        SyncRecordingDispatch {
            recorded: Arc::clone(&recorded),
        },
        FixedUrgencyLookup {
            urgent_keys: BTreeSet::new(),
        },
    );
    let mut builder = EventBusBuilder::default();
    let _subscription = builder.subscribe(
        NotificationEventSubscriber::<SyncRecordingDispatch, FixedUrgencyLookup>::subscribed_kinds(
        ),
        subscriber,
    );
    let bus = builder.build();
    let date = domain::CalendarDate::from_parts(
        domain::CalendarYear::new(2026),
        domain::MonthNumber::new(11),
        domain::DayNumber::new(25),
    )
    .expect("날짜");
    let report = bus.publish(DomainEvent::BriefingFinished(domain::BriefingId::new(
        domain::BriefingKind::Morning,
        date,
    )));
    assert!(report.failures.is_empty(), "브리핑 완료 이벤트 전달 성공");
    assert_eq!(
        *recorded.read().expect("수신 기록"),
        vec![Notification::BriefingFinished]
    );
}
