use super::{DatabaseClosure, RefreshOutcome, ShutdownOutcome, Work};
use crate::database::DatabaseState;
use application::SystemPortError;

pub(super) async fn finish<Close, Closing>(work: Work, close: Close) -> ShutdownOutcome
where
    Close: FnOnce(DatabaseState) -> Closing,
    Closing: Future<Output = Result<DatabaseClosure, SystemPortError>>,
{
    work.refresh.abort();
    let refresh = match work.refresh.await {
        Ok(result) => RefreshOutcome::Completed(result),
        Err(tauri::Error::JoinError(error)) if error.is_cancelled() => RefreshOutcome::Cancelled,
        Err(error) => RefreshOutcome::Failed(error),
    };
    let database = close(work.database).await;
    ShutdownOutcome { refresh, database }
}

pub(super) async fn close_database(
    state: DatabaseState,
) -> Result<DatabaseClosure, SystemPortError> {
    match state {
        DatabaseState::Ready(database) => database.close().await.map(DatabaseClosure::Closed),
        DatabaseState::Failed(error) => Err(error),
        DatabaseState::NotConfigured => Ok(DatabaseClosure::NotOpened),
    }
}

#[cfg(test)]
mod tests {
    use super::{DatabaseClosure, DatabaseState, RefreshOutcome, Work, finish};
    use std::sync::{Arc, RwLock};

    #[derive(Debug, PartialEq, Eq)]
    enum Stage {
        RefreshDropped,
        DatabaseClosing,
    }
    struct OnDrop(Arc<RwLock<Vec<Stage>>>);
    impl Drop for OnDrop {
        fn drop(&mut self) {
            self.0.write().expect("기록").push(Stage::RefreshDropped);
        }
    }

    #[test]
    fn database_closing_begins_only_after_refresh_future_has_been_dropped() {
        let stages = Arc::new(RwLock::new(Vec::new()));
        let capture = Arc::clone(&stages);
        let (send, receive) = std::sync::mpsc::channel();
        let refresh = tauri::async_runtime::spawn(async move {
            let _on_drop = OnDrop(capture);
            send.send(()).expect("갱신 시작");
            std::future::pending().await
        });
        receive
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("실행 중인 갱신");
        let work = Work {
            refresh,
            database: DatabaseState::NotConfigured,
        };
        let observed = Arc::clone(&stages);
        let outcome = tauri::async_runtime::block_on(finish(work, move |database| {
            let mut log = observed.write().expect("기록");
            assert_eq!(*log, vec![Stage::RefreshDropped]);
            log.push(Stage::DatabaseClosing);
            super::close_database(database)
        }));
        match outcome.refresh {
            RefreshOutcome::Cancelled => {}
            RefreshOutcome::Completed(_) | RefreshOutcome::Failed(_) => panic!("취소 완료"),
        }
        assert_eq!(outcome.database, Ok(DatabaseClosure::NotOpened));
        assert_eq!(
            *stages.read().expect("기록"),
            vec![Stage::RefreshDropped, Stage::DatabaseClosing]
        );
    }
}
