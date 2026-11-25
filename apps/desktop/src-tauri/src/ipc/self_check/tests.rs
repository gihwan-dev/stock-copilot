use application::{SourceState, SourceStatus};
use domain::DataSource;

use super::{
    AiLimit, ClaudeInstall, GetSelfCheckRequest, UiAiRuntime, UiSelfCheckProblem, self_check_view,
    status_view,
};
use crate::toss_link::TossConnectionState;

fn at(text: &str) -> domain::Timestamp {
    broker_toss::TossTimestamp::try_from(text)
        .expect("시각")
        .into_domain()
}

#[test]
fn toss_problems_come_before_claude_and_only_one_shows() {
    assert_eq!(
        self_check_view(
            Some(TossConnectionState::IpNotAllowed),
            ClaudeInstall::Missing
        )
        .problem,
        UiSelfCheckProblem::IpNotAllowed
    );
    assert_eq!(
        self_check_view(
            Some(TossConnectionState::KeyInUse),
            ClaudeInstall::Installed
        )
        .problem,
        UiSelfCheckProblem::TossKeyShared
    );
    assert_eq!(
        self_check_view(
            Some(TossConnectionState::Forbidden),
            ClaudeInstall::Installed
        )
        .problem,
        UiSelfCheckProblem::TossKeyPermission
    );
    let missing = self_check_view(Some(TossConnectionState::Connected), ClaudeInstall::Missing);
    assert_eq!(missing.problem, UiSelfCheckProblem::ClaudeMissing);
    super::super::tests::assert_response_snapshot("get_self_check", &missing);
}

#[test]
fn healthy_or_unknown_state_shows_nothing() {
    for state in [
        None,
        Some(TossConnectionState::Connected),
        Some(TossConnectionState::NeedsKeys),
    ] {
        assert_eq!(
            self_check_view(state, ClaudeInstall::Installed).problem,
            UiSelfCheckProblem::None
        );
    }
}

#[test]
fn status_lists_sources_with_kst_success_time_and_ai_wait() {
    let success = at("2026-11-25T00:30:00Z");
    let sources = vec![
        SourceStatus::try_new(
            DataSource::Yonhap,
            SourceState::Healthy,
            Some(success),
            Some(success),
        )
        .expect("상태"),
        SourceStatus::initial(DataSource::KindDisclosure),
    ];
    let view = status_view(
        ClaudeInstall::Installed,
        AiLimit::Waiting(at("2026-11-25T06:00:00Z")),
        &sources,
    );
    assert_eq!(view.ai_runtime, UiAiRuntime::Ready);
    assert_eq!(
        view.ai_resumes_at.as_deref(),
        Some("2026-11-25T15:00:00+09:00")
    );
    assert_eq!(view.sources.len(), 2);
    super::super::tests::assert_response_snapshot("get_status", &view);
}

#[test]
fn request_refuses_unknown_fields() {
    let _ok: GetSelfCheckRequest = serde_json::from_str(r#"{"scope":"current"}"#).expect("요청");
    let _extra = serde_json::from_str::<GetSelfCheckRequest>(r#"{"scope":"current","x":1}"#)
        .expect_err("모르는 필드 거부");
}
