use std::sync::Arc;

use application::{GetScorecardError, ScorecardView};
use domain::ScorecardError;

use super::{GetScorecardRequest, ScorecardIpcState, ScorecardSource, load_scorecard_view};
use crate::ipc::error::IpcErrorKind;
use crate::ipc::state::BoxFuture;

struct Fixed(fn() -> Result<ScorecardView, GetScorecardError>);

impl ScorecardSource for Fixed {
    fn load(&self) -> BoxFuture<'_, Result<ScorecardView, GetScorecardError>> {
        Box::pin(std::future::ready((self.0)()))
    }
}

fn state(reply: fn() -> Result<ScorecardView, GetScorecardError>) -> ScorecardIpcState {
    ScorecardIpcState(Arc::new(Fixed(reply)))
}

#[test]
fn no_judgments_yet_is_an_empty_scorecard_not_an_error() {
    let view = tauri::async_runtime::block_on(load_scorecard_view(&state(|| {
        Ok(ScorecardView::NoObservations)
    })))
    .expect("빈 성적표");
    assert_eq!(view.combinations.len(), 0);
    super::super::tests::assert_response_snapshot("get_scorecard", &view);
}

#[test]
fn calculation_failure_is_retryable_and_storage_failure_keeps_its_class() {
    let calculation = tauri::async_runtime::block_on(load_scorecard_view(&state(|| {
        Err(GetScorecardError::Calculation(ScorecardError::Calculation))
    })))
    .expect_err("계산 실패");
    assert_eq!(calculation.kind, IpcErrorKind::Retryable);
    let storage = tauri::async_runtime::block_on(load_scorecard_view(&state(|| {
        Err(GetScorecardError::Storage(
            application::SystemPortError::new(
                application::SystemPort::Repository,
                application::SystemFailure::Unavailable(application::RetryDelay::Unknown),
            ),
        ))
    })))
    .expect_err("저장소 실패");
    assert_eq!(storage.kind, IpcErrorKind::Retryable);
}

#[test]
fn request_refuses_unknown_fields() {
    let _ok: GetScorecardRequest = serde_json::from_str(r#"{"scope":"all"}"#).expect("요청");
    let _extra = serde_json::from_str::<GetScorecardRequest>(r#"{"scope":"all","model":"x"}"#)
        .expect_err("모르는 필드 거부");
}
