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
            RefreshOutcome::Completed(_) => panic!("취소됨"),
        }
        assert_eq!(outcome.database, Ok(()));
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
