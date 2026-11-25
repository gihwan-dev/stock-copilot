use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::Manager;
use tauri::test::{mock_builder, mock_context, noop_assets};
use tokio::sync::Mutex;

use super::{
    RestartConfirm, RestartToUpdateRequest, UiUpdateFailure, UiUpdateState, UpdateStatusQuery,
    UpdateStatusRequest, UpdateStatusView, get_update_status, restart_to_update,
};
use crate::updates::{
    FoundUpdate, ReleaseNotes, SourceFuture, UpdateCenter, UpdateFailure, UpdateMode, UpdatePhase,
    UpdateSource, UpdateVersion,
};

const RESPONSE_DIRECTORY: &str = "ipc/responses";

fn assert_response_snapshot<T: Serialize>(name: &str, value: &T) {
    let path = Path::new(RESPONSE_DIRECTORY).join(format!("{name}.json"));
    let mut actual = serde_json::to_string_pretty(value).expect("serialize response");
    actual.push('\n');
    let recorded = fs::read_to_string(&path).expect("read recorded response");
    assert_eq!(recorded, actual, "{} differs", path.display());
}

#[derive(Debug, Default)]
struct OfferingSource {
    installs: Mutex<u8>,
}

impl OfferingSource {
    fn installs(&self) -> u8 {
        match self.installs.try_lock() {
            Ok(installs) => *installs,
            Err(_busy) => u8::MAX,
        }
    }
}

impl UpdateSource for OfferingSource {
    fn check(&self) -> SourceFuture<'_, Option<FoundUpdate>> {
        Box::pin(async {
            Ok(Some(FoundUpdate {
                version: UpdateVersion::new("0.2.0"),
                notes: ReleaseNotes::new("- 자동 업데이트"),
            }))
        })
    }

    fn download(&self) -> SourceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }

    fn install(&self) -> Result<(), UpdateFailure> {
        match self.installs.try_lock() {
            Ok(mut installs) => {
                *installs = installs.saturating_add(1);
                Ok(())
            }
            Err(_busy) => Err(UpdateFailure::Install),
        }
    }
}

fn status(query: UpdateStatusQuery) -> UpdateStatusRequest {
    UpdateStatusRequest { query }
}

fn restart() -> RestartToUpdateRequest {
    RestartToUpdateRequest {
        confirm: RestartConfirm::Restart,
    }
}

#[test]
fn status_without_an_update_center_is_off_and_recorded_for_the_typescript_fake() {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let current = tauri::async_runtime::block_on(get_update_status(
        app.handle().clone(),
        status(UpdateStatusQuery::Current),
    ))
    .expect("status");
    assert_eq!(current.state, UiUpdateState::Off);
    assert_response_snapshot("get_update_status", &current);
    let restarted =
        tauri::async_runtime::block_on(restart_to_update(app.handle().clone(), restart()))
            .expect("restart");
    assert_response_snapshot("restart_to_update", &restarted);
}

#[test]
fn check_now_starts_a_background_check_that_ends_ready() {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let source = Arc::new(OfferingSource::default());
    let shared: Arc<dyn UpdateSource> = Arc::<OfferingSource>::clone(&source);
    let center = Arc::new(UpdateCenter::new(UpdateMode::Enabled, shared));
    let _managed = app.manage(Arc::clone(&center));
    let first = tauri::async_runtime::block_on(get_update_status(
        app.handle().clone(),
        status(UpdateStatusQuery::CheckNow),
    ))
    .expect("status");
    assert_ne!(first.state, UiUpdateState::Idle);
    let ready = tauri::async_runtime::block_on(async {
        for _attempt in 0_u8..200 {
            if let UpdatePhase::Ready(found) = center.phase().await {
                return Some(found);
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        None
    });
    assert_eq!(
        ready.map(|found| found.version),
        Some(UpdateVersion::new("0.2.0"))
    );
    let shown = tauri::async_runtime::block_on(get_update_status(
        app.handle().clone(),
        status(UpdateStatusQuery::Current),
    ))
    .expect("status");
    assert_eq!(
        shown.state,
        UiUpdateState::Ready {
            version: String::from("0.2.0"),
            notes: String::from("- 자동 업데이트"),
        }
    );
    assert_eq!(source.installs(), 0);
}

#[test]
fn restart_without_a_ready_update_installs_nothing() {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let source = Arc::new(OfferingSource::default());
    let shared: Arc<dyn UpdateSource> = Arc::<OfferingSource>::clone(&source);
    let _managed = app.manage(Arc::new(UpdateCenter::new(UpdateMode::Enabled, shared)));
    let view: UpdateStatusView =
        tauri::async_runtime::block_on(restart_to_update(app.handle().clone(), restart()))
            .expect("restart");
    assert_eq!(view.state, UiUpdateState::Idle);
    assert_eq!(source.installs(), 0);
}

#[test]
fn requests_refuse_unknown_fields_and_values() {
    assert_eq!(
        serde_json::from_str::<UpdateStatusRequest>(r#"{"query":"check-now"}"#).expect("check"),
        status(UpdateStatusQuery::CheckNow)
    );
    assert_eq!(
        serde_json::from_str::<RestartToUpdateRequest>(r#"{"confirm":"restart"}"#)
            .expect("restart"),
        restart()
    );
    for input in [
        r#"{"query":"current","url":"https://example.invalid/latest.json"}"#,
        r#"{"query":"install"}"#,
    ] {
        assert!(
            serde_json::from_str::<UpdateStatusRequest>(input).is_err(),
            "{input}"
        );
    }
    for input in [
        r#"{"confirm":"restart","version":"9.9.9"}"#,
        r#"{"confirm":"now"}"#,
    ] {
        assert!(
            serde_json::from_str::<RestartToUpdateRequest>(input).is_err(),
            "{input}"
        );
    }
}

#[test]
fn update_states_serialize_with_kind_tags() {
    let ready = serde_json::to_value(UiUpdateState::from(UpdatePhase::Ready(FoundUpdate {
        version: UpdateVersion::new("0.2.0"),
        notes: ReleaseNotes::new("고침"),
    })))
    .expect("ready");
    assert_eq!(
        ready,
        serde_json::json!({"ready": {"version": "0.2.0", "notes": "고침"}})
    );
    let failed = serde_json::to_value(UiUpdateState::from(UpdatePhase::Failed(
        UpdateFailure::Download,
    )))
    .expect("failed");
    assert_eq!(
        failed,
        serde_json::json!({"failed": {"failure": "download"}})
    );
    assert_eq!(
        serde_json::to_value(UiUpdateFailure::Check).expect("check"),
        serde_json::json!("check")
    );
    assert_eq!(
        serde_json::to_value(UiUpdateState::from(UpdatePhase::UpToDate)).expect("up to date"),
        serde_json::json!("up-to-date")
    );
}
