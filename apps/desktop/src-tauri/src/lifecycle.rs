use crate::database::DatabaseState;
use application::{PortError, SystemPortError};
use domain::BrokerAccountRows;
use tauri::async_runtime::{JoinHandle, Mutex};

#[derive(Debug)]
pub enum RefreshOutcome {
    Completed(Result<BrokerAccountRows, PortError>),
    Cancelled,
}

#[derive(Debug)]
pub struct ShutdownOutcome {
    pub refresh: RefreshOutcome,
    pub database: Result<(), SystemPortError>,
}

#[derive(Debug)]
pub struct Lifecycle {
    work: Mutex<Option<Work>>,
    outcome: Mutex<Option<ShutdownOutcome>>,
}

#[derive(Debug)]
struct Work {
    refresh: JoinHandle<Result<BrokerAccountRows, PortError>>,
    database: DatabaseState,
}

impl Lifecycle {
    #[must_use]
    pub fn new(
        refresh: JoinHandle<Result<BrokerAccountRows, PortError>>,
        database: DatabaseState,
    ) -> Self {
        Self {
            work: Mutex::new(Some(Work { refresh, database })),
            outcome: Mutex::new(None),
        }
    }

    pub fn outcome(&self) -> &Mutex<Option<ShutdownOutcome>> {
        &self.outcome
    }

    pub async fn shutdown(&self) {
        let mut outcome = self.outcome.lock().await;
        let pending = self.work.lock().await.take();
        if let Some(work) = pending {
            work.refresh.abort();
            let refresh = match work.refresh.await {
                Ok(result) => RefreshOutcome::Completed(result),
                Err(_error) => RefreshOutcome::Cancelled,
            };
            let database = match work.database {
                DatabaseState::Ready(database) => {
                    match tauri::async_runtime::spawn_blocking(move || database.shutdown()).await {
                        Ok(result) => result,
                        Err(_error) => Err(SystemPortError::new(
                            application::SystemPort::Repository,
                            application::SystemFailure::RepositoryIntegrity,
                        )),
                    }
                }
                DatabaseState::Failed(error) => Err(error),
                DatabaseState::NotConfigured => Ok(()),
            };
            *outcome = Some(ShutdownOutcome { refresh, database });
        }
    }
}
