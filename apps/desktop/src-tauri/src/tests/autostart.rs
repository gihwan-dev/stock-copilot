use crate::platform::autostart::{
    AutostartController, AutostartError, AutostartPort, AutostartState, AutostartSyncOutcome,
    UnavailableAutostartPort,
};
use application::{EventBusBuilder, EventSubscriber, SubscriberFailure};
use domain::{DomainEvent, DomainEventKind};
use std::collections::BTreeSet;
use std::sync::{Arc, RwLock};

struct FakeAutostartPort {
    state: RwLock<AutostartState>,
    set_calls: RwLock<Vec<AutostartState>>,
    fail_on_set: RwLock<Option<AutostartError>>,
}

impl FakeAutostartPort {
    fn new(initial: AutostartState) -> Self {
        Self {
            state: RwLock::new(initial),
            set_calls: RwLock::new(Vec::new()),
            fail_on_set: RwLock::new(None),
        }
    }

    fn set_failure(&self, error: Option<AutostartError>) {
        *self.fail_on_set.write().expect("오류 주입") = error;
    }

    fn set_calls(&self) -> Vec<AutostartState> {
        self.set_calls.read().expect("호출 기록").clone()
    }
}

impl AutostartPort for FakeAutostartPort {
    fn current(&self) -> Result<AutostartState, AutostartError> {
        self.state
            .read()
            .map(|guard| *guard)
            .map_err(|_error| AutostartError::Unavailable)
    }

    fn set_state(&self, desired: AutostartState) -> Result<AutostartState, AutostartError> {
        let injected_error = self
            .fail_on_set
            .read()
            .map_err(|_error| AutostartError::Unavailable)?
            .clone();
        if let Some(error) = injected_error {
            return Err(error);
        }
        self.set_calls
            .write()
            .map_err(|_error| AutostartError::Unavailable)?
            .push(desired);
        let mut guard = self
            .state
            .write()
            .map_err(|_error| AutostartError::Unavailable)?;
        *guard = desired;
        Ok(desired)
    }
}

struct RecordingSubscriber {
    events: Arc<RwLock<Vec<DomainEventKind>>>,
}

impl EventSubscriber for RecordingSubscriber {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        self.events
            .write()
            .map_err(|_error| SubscriberFailure::Unavailable)?
            .push(event.kind());
        Ok(())
    }
}

#[test]
fn default_state_is_enabled_and_startup_reconcile_enables_autostart() {
    assert_eq!(
        AutostartState::default(),
        AutostartState::Enabled,
        "로그인 시 자동 시작 기본값은 켬이어야 한다 (P-4)"
    );
    let controller = AutostartController::new(FakeAutostartPort::new(AutostartState::Disabled));
    assert_eq!(
        controller.desired_state(),
        AutostartState::Enabled,
        "초기 목표 상태는 켬"
    );
    let first = controller.reconcile_startup().expect("첫 실행 동기화");
    assert_eq!(
        first,
        AutostartSyncOutcome::Synchronized(AutostartState::Enabled),
        "첫 실행에서 OS 상태를 기본 켬으로 맞춘다"
    );
    assert_eq!(
        controller.port().set_calls(),
        vec![AutostartState::Enabled],
        "set_state 1회 호출"
    );
    let second = controller.reconcile_startup().expect("재동기화");
    assert_eq!(
        second,
        AutostartSyncOutcome::Unchanged(AutostartState::Enabled),
        "이미 같으면 다시 등록하지 않는다"
    );
    assert_eq!(controller.port().set_calls().len(), 1, "중복 호출 없음");
}

#[test]
fn switching_autostart_updates_state_and_publishes_settings_changed() {
    let observed = Arc::new(RwLock::new(Vec::new()));
    let mut builder = EventBusBuilder::default();
    let _sub = builder.subscribe(
        BTreeSet::from([DomainEventKind::SettingsChanged]),
        RecordingSubscriber {
            events: Arc::clone(&observed),
        },
    );
    let events = builder.build();
    let controller = AutostartController::new(FakeAutostartPort::new(AutostartState::Enabled));

    let disabled = controller
        .apply(AutostartState::Disabled, &events)
        .expect("자동 시작 끄기");
    assert_eq!(disabled.state, AutostartState::Disabled, "꺼짐 반영");
    assert!(disabled.delivery.failures.is_empty(), "이벤트 전달 성공");
    assert_eq!(
        controller.desired_state(),
        AutostartState::Disabled,
        "내부 상태 갱신"
    );
    assert_eq!(
        observed.read().expect("기록").as_slice(),
        &[DomainEventKind::SettingsChanged],
        "설정 변경 이벤트 1회 발행"
    );

    let unchanged = controller
        .apply(AutostartState::Disabled, &events)
        .expect("같은 상태 재적용");
    assert_eq!(unchanged.state, AutostartState::Disabled, "상태 유지");
    assert_eq!(
        observed.read().expect("기록").len(),
        1,
        "변화가 없으면 중복 이벤트 미발행"
    );

    let enabled = controller
        .apply(AutostartState::Enabled, &events)
        .expect("자동 시작 다시 켜기");
    assert_eq!(enabled.state, AutostartState::Enabled, "켜짐 반영");
    assert_eq!(
        observed.read().expect("기록").len(),
        2,
        "다시 켤 때 설정 변경 이벤트 발행"
    );
}

#[test]
fn backend_failure_preserves_previous_state_and_emits_no_event() {
    let observed = Arc::new(RwLock::new(Vec::new()));
    let mut builder = EventBusBuilder::default();
    let _sub = builder.subscribe(
        BTreeSet::from([DomainEventKind::SettingsChanged]),
        RecordingSubscriber {
            events: Arc::clone(&observed),
        },
    );
    let events = builder.build();

    let unavailable = AutostartController::new(UnavailableAutostartPort);
    assert_eq!(
        unavailable.reconcile_startup(),
        Err(AutostartError::Unavailable),
        "잠금 파일에 autostart 플러그인 부재 시 명시적 불가"
    );
    assert_eq!(
        unavailable.apply(AutostartState::Disabled, &events),
        Err(AutostartError::Unavailable),
        "적용도 불가 반환"
    );
    assert_eq!(
        unavailable.desired_state(),
        AutostartState::Enabled,
        "실패 시 이전 상태 유지"
    );

    let fake = AutostartController::new(FakeAutostartPort::new(AutostartState::Enabled));
    fake.port()
        .set_failure(Some(AutostartError::SystemFailed(String::from("denied"))));
    let error = fake
        .apply(AutostartState::Disabled, &events)
        .expect_err("시스템 오류 전파");
    assert_eq!(
        error,
        AutostartError::SystemFailed(String::from("denied")),
        "오류 내용 보존"
    );
    assert_eq!(
        fake.desired_state(),
        AutostartState::Enabled,
        "시스템 오류 시 이전 상태 유지"
    );
    assert!(
        observed.read().expect("기록").is_empty(),
        "실패 시 SettingsChanged 미발행"
    );
}
