use super::{
    MacOsRiskDialogPresence, RiskChangeDialogContent, RiskDialogBackend, RiskDialogResponse,
    UnavailableRiskDialogBackend, build_risk_dialog_content, ratio_text_to_percent,
};
use application::user_presence::UserPresence;
use application::{
    ChangeRiskSettings, DeliveryReport, EventBusBuilder, EventSubscriber, RiskChangeConfirmation,
    RiskSettingsChangeOutcome, SettingsRepository, SubscriberFailure,
};
use domain::{
    DomainEvent, DomainEventKind, HoursPermissionState, NumericInput, OrderPermission,
    ProfileWeight, RiskAppetite, RiskChange, RiskSettings, SavedMarketOrderSetting, WeightCaps,
};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use testkit_ports::memory::{MemoryReply, MemorySettingsRepository};

const FORBIDDEN_UI_TERMS: [&str; 8] = [
    "MCP",
    "세션",
    "프로필",
    "격리",
    "큐",
    "보정",
    "사전 점검",
    "토큰",
];

fn run<F: Future>(future: F) -> F::Output {
    let mut pinned = std::pin::pin!(future);
    match pinned
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("즉시 완료되어야 한다"),
    }
}

fn repository() -> MemorySettingsRepository {
    let repo = MemorySettingsRepository::default();
    for _ in 0_u8..16 {
        repo.script_store_risk_settings().push(MemoryReply::Memory);
        repo.script_risk_settings().push(MemoryReply::Memory);
        repo.script_profile().push(MemoryReply::Memory);
    }
    repo
}

fn safe_settings() -> RiskSettings {
    RiskSettings::new(
        OrderPermission::Disabled,
        SavedMarketOrderSetting::Disabled,
        HoursPermissionState::Disabled,
        WeightCaps::default(),
        RiskAppetite::Neutral,
    )
}

fn caps(cap: &str, warning: &str, sector: &str) -> WeightCaps {
    let parse = |text| {
        ProfileWeight::parse(&NumericInput::try_new(text).expect("입력 길이")).expect("비중")
    };
    WeightCaps::try_new(parse(cap), parse(warning), parse(sector)).expect("비중 상한")
}

#[derive(Debug)]
struct FakeRiskDialogBackend {
    responses: RwLock<VecDeque<RiskDialogResponse>>,
    presented: RwLock<Vec<RiskChangeDialogContent>>,
}

impl FakeRiskDialogBackend {
    fn new(responses: impl IntoIterator<Item = RiskDialogResponse>) -> Self {
        Self {
            responses: RwLock::new(responses.into_iter().collect()),
            presented: RwLock::new(Vec::new()),
        }
    }

    fn presented(&self) -> Vec<RiskChangeDialogContent> {
        self.presented.read().expect("기록 읽기").clone()
    }
}

impl RiskDialogBackend for FakeRiskDialogBackend {
    fn present(&self, content: &RiskChangeDialogContent) -> RiskDialogResponse {
        self.presented.write().expect("기록").push(content.clone());
        self.responses
            .write()
            .expect("응답 큐")
            .pop_front()
            .unwrap_or(RiskDialogResponse::Unavailable)
    }
}

#[derive(Clone, Default)]
struct RecordingSubscriber(Arc<RwLock<Vec<DomainEvent>>>);

impl EventSubscriber for RecordingSubscriber {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        let mut recorded = self
            .0
            .write()
            .map_err(|_error| SubscriberFailure::Unavailable)?;
        recorded.push(event.clone());
        Ok(())
    }
}

#[test]
fn confirming_all_five_risk_changes_updates_settings_with_zero_biometric_calls() {
    let repo = repository();
    run(repo.store_risk_settings(&safe_settings())).expect("초기 설정");
    let backend = FakeRiskDialogBackend::new([RiskDialogResponse::Confirmed; 5]);
    let presence = MacOsRiskDialogPresence::new(backend);
    let subscriber = RecordingSubscriber::default();
    let mut bus_builder = EventBusBuilder::default();
    let _sub = bus_builder.subscribe(
        std::iter::once(DomainEventKind::SettingsChanged).collect(),
        subscriber.clone(),
    );
    let events = bus_builder.build();
    let use_case = ChangeRiskSettings::new(&repo, &presence, &events);
    let requested = RiskSettings::new(
        OrderPermission::Enabled,
        SavedMarketOrderSetting::Enabled,
        HoursPermissionState::Enabled,
        caps("0.3", "0.15", "0.5"),
        RiskAppetite::Aggressive,
    );

    let outcome = run(use_case.execute(requested)).expect("위험 설정 변경 실행");
    assert_eq!(
        outcome,
        RiskSettingsChangeOutcome::Changed(DeliveryReport::default()),
        "네이티브 확인 창 확인 후 설정이 변경되어야 한다"
    );
    assert_eq!(
        run(repo.risk_settings()).expect("설정 조회"),
        Some(requested),
        "변경된 위험 설정이 저장되어야 한다"
    );
    assert_eq!(
        presence.backend().presented().len(),
        5,
        "위험 증가 5개 항목마다 네이티브 확인 창이 표시되어야 한다"
    );
    assert_eq!(
        presence.biometric_calls(),
        0,
        "위험 설정 변경 경로에서는 macOS 본인 확인 호출이 0회여야 한다"
    );
    assert_eq!(
        *subscriber.0.read().expect("이벤트 읽기"),
        vec![DomainEvent::SettingsChanged],
        "설정 저장 뒤 SettingsChanged가 발행되어야 한다"
    );
}

#[test]
fn cancelling_or_unavailable_dialog_preserves_existing_settings_without_biometrics() {
    for (response, expected_outcome) in [
        (
            RiskDialogResponse::Cancelled,
            RiskSettingsChangeOutcome::Denied,
        ),
        (
            RiskDialogResponse::Unavailable,
            RiskSettingsChangeOutcome::Unavailable,
        ),
    ] {
        let repo = repository();
        run(repo.store_risk_settings(&safe_settings())).expect("초기 설정");
        let presence = MacOsRiskDialogPresence::new(FakeRiskDialogBackend::new([response]));
        let events = EventBusBuilder::default().build();
        let use_case = ChangeRiskSettings::new(&repo, &presence, &events);
        let requested = RiskSettings::new(
            OrderPermission::Enabled,
            SavedMarketOrderSetting::Disabled,
            HoursPermissionState::Disabled,
            WeightCaps::default(),
            RiskAppetite::Neutral,
        );

        let outcome = run(use_case.execute(requested)).expect("실행");
        assert_eq!(outcome, expected_outcome, "거부/불가 결과 일치");
        assert_eq!(
            run(repo.risk_settings()).expect("조회"),
            Some(safe_settings()),
            "미확인 시 기존 설정이 유지되어야 한다"
        );
        assert_eq!(presence.backend().presented().len(), 1, "확인 창 1회 표시");
        assert_eq!(
            presence.biometric_calls(),
            0,
            "미확인 경로에서도 본인 확인 호출 0회"
        );
    }
}

#[test]
fn unavailable_default_presence_fails_closed_for_risk_changes_and_orders() {
    let presence = MacOsRiskDialogPresence::unavailable();
    let repo = repository();
    run(repo.store_risk_settings(&safe_settings())).expect("초기 설정");
    let events = EventBusBuilder::default().build();
    let use_case = ChangeRiskSettings::new(&repo, &presence, &events);
    let requested = RiskSettings::new(
        OrderPermission::Enabled,
        SavedMarketOrderSetting::Disabled,
        HoursPermissionState::Disabled,
        WeightCaps::default(),
        RiskAppetite::Neutral,
    );
    assert_eq!(
        run(use_case.execute(requested)),
        Ok(RiskSettingsChangeOutcome::Unavailable),
        "rfd 부재 시 닫힌 실패로 Unavailable 반환"
    );
    assert_eq!(
        run(repo.risk_settings()).expect("조회"),
        Some(safe_settings()),
        "설정 변경 없음"
    );
    assert_eq!(presence.biometric_calls(), 0, "본인 확인 호출 0회");
    let backend = UnavailableRiskDialogBackend;
    assert_eq!(
        backend.present(&build_risk_dialog_content(&RiskChange::EnableOrders)),
        RiskDialogResponse::Unavailable,
        "미지원 백엔드 직접 호출도 Unavailable"
    );
    assert_eq!(
        run(presence.confirm_risk_change(RiskChange::EnableOrders)),
        RiskChangeConfirmation::Unavailable,
        "직접 포트 호출도 Unavailable"
    );
}

#[test]
fn core_generated_korean_dialog_copy_covers_all_variants_without_ux6_forbidden_words() {
    let variants = [
        RiskChange::EnableOrders,
        RiskChange::AllowMarketOrders,
        RiskChange::AllowHours,
        RiskChange::RelaxWeightCaps(caps("0.25", "0.125", "0.45")),
        RiskChange::IncreaseRiskAppetite(RiskAppetite::Aggressive),
        RiskChange::IncreaseRiskAppetite(RiskAppetite::Neutral),
        RiskChange::IncreaseRiskAppetite(RiskAppetite::Conservative),
    ];
    for change in variants {
        let content = build_risk_dialog_content(&change);
        assert!(!content.title.is_empty(), "제목 필수");
        assert!(!content.body.is_empty(), "본문 필수");
        assert_eq!(content.confirm_label, "확인", "확인 버튼 라벨");
        assert_eq!(content.cancel_label, "취소", "취소 버튼 라벨");
        for term in FORBIDDEN_UI_TERMS {
            assert!(!content.title.contains(term), "제목에 금지어 포함: {term}");
            assert!(!content.body.contains(term), "본문에 금지어 포함: {term}");
        }
    }
    let relax_content =
        build_risk_dialog_content(&RiskChange::RelaxWeightCaps(caps("0.25", "0.125", "0.45")));
    assert!(
        relax_content.body.contains("25%") && relax_content.body.contains("45%"),
        "비중 상한 수치가 백분율로 포함되어야 한다: {}",
        relax_content.body
    );
}

#[test]
fn ratio_text_percent_formatting_handles_exact_decimals_without_decimal_arithmetic() {
    for (input, expected) in [
        ("0.2", "20%"),
        ("0.25", "25%"),
        ("0.125", "12.5%"),
        ("1", "100%"),
        ("1.0", "100%"),
        ("0.005", "0.5%"),
        ("0.0005", "0.05%"),
        ("0.200", "20%"),
    ] {
        assert_eq!(ratio_text_to_percent(input), expected, "입력 {input} 변환");
    }
}
