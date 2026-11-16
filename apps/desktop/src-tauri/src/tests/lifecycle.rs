use crate::{
    database::DatabaseState,
    lifecycle::{Lifecycle, RefreshOutcome},
};

#[test]
fn shutdown_cancels_and_joins_running_refresh_and_is_idempotent() {
    tauri::async_runtime::block_on(async {
        let refresh = tauri::async_runtime::spawn(std::future::pending());
        let lifecycle = Lifecycle::new(refresh, DatabaseState::NotConfigured);
        lifecycle.shutdown().await;
        lifecycle.shutdown().await;
        let guard = lifecycle.outcome().lock().await;
        let outcome = guard.as_ref().expect("종료 결과");
        match &outcome.refresh {
            RefreshOutcome::Cancelled => {}
            RefreshOutcome::Completed(_) | RefreshOutcome::Failed(_) => panic!("취소됨"),
        }
        assert_eq!(
            outcome.database,
            Ok(crate::lifecycle::DatabaseClosure::NotOpened)
        );
    });
}

#[test]
fn shutdown_preserves_database_failure() {
    tauri::async_runtime::block_on(async {
        let error = super::support::failure(application::SystemFailure::RepositoryIntegrity);
        let lifecycle = Lifecycle::new(
            tauri::async_runtime::spawn(std::future::pending()),
            DatabaseState::Failed(error),
        );
        lifecycle.shutdown().await;
        assert_eq!(
            lifecycle
                .outcome()
                .lock()
                .await
                .as_ref()
                .expect("결과")
                .database,
            Err(error)
        );
    });
}

#[test]
fn a_panicked_refresh_is_preserved_as_failure_instead_of_cancellation() {
    let refresh = tauri::async_runtime::spawn(async {
        std::panic::resume_unwind(Box::new("refresh failed"))
    });
    while !refresh.inner().is_finished() {
        std::thread::yield_now();
    }
    let lifecycle = Lifecycle::new(refresh, DatabaseState::NotConfigured);
    tauri::async_runtime::block_on(lifecycle.shutdown());
    let guard = lifecycle.outcome().try_lock().expect("종료 결과");
    match &guard.as_ref().expect("결과").refresh {
        RefreshOutcome::Failed(error) => assert!(error.to_string().contains("panic")),
        RefreshOutcome::Cancelled | RefreshOutcome::Completed(_) => panic!("실패 보존"),
    }
}
