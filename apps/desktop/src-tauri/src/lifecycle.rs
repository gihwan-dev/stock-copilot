use crate::database::DatabaseState;
use application::{PortError, SystemPortError};
use domain::BrokerAccountRows;
use tauri::async_runtime::{JoinHandle, Mutex};

mod cleanup;

#[derive(Debug)]
pub enum RefreshOutcome {
    Completed(Result<BrokerAccountRows, PortError>),
    Cancelled,
    Failed(tauri::Error),
}

#[derive(Debug)]
pub struct ShutdownOutcome {
    pub refresh: RefreshOutcome,
    pub database: Result<DatabaseClosure, SystemPortError>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DatabaseClosure {
    NotOpened,
    Closed(application::WriteCompleted),
}

#[derive(Debug)]
pub struct Lifecycle {
    phase: Mutex<LifecyclePhase>,
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
            phase: Mutex::new(LifecyclePhase::Running),
            work: Mutex::new(Some(Work { refresh, database })),
            outcome: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn phase(&self) -> LifecyclePhase {
        match self.phase.try_lock() {
            Ok(phase) => *phase,
            Err(_error) => LifecyclePhase::Stopping,
        }
    }

    #[must_use]
    pub fn exit_decision(&self) -> ExitDecision {
        match self.phase.try_lock() {
            Ok(mut phase) => match *phase {
                LifecyclePhase::Running => {
                    *phase = LifecyclePhase::Stopping;
                    ExitDecision::Start
                }
                LifecyclePhase::Stopping => ExitDecision::Defer,
                LifecyclePhase::Stopped => ExitDecision::Allow,
            },
            Err(_error) => ExitDecision::Defer,
        }
    }

    pub fn outcome(&self) -> &Mutex<Option<ShutdownOutcome>> {
        &self.outcome
    }

    pub async fn shutdown(&self) {
        let mut outcome = self.outcome.lock().await;
        let pending = self.work.lock().await.take();
        if let Some(work) = pending {
            *outcome = Some(cleanup::finish(work, cleanup::close_database).await);
        }
        *self.phase.lock().await = LifecyclePhase::Stopped;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecyclePhase {
    Running,
    Stopping,
    Stopped,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitDecision {
    Start,
    Defer,
    Allow,
}

#[cfg(test)]
mod tests {
    use super::{ExitDecision, Lifecycle, LifecyclePhase};
    use crate::database::DatabaseState;
    #[test]
    fn a_busy_phase_defers_instead_of_blocking_the_event_loop() {
        let lifecycle = Lifecycle::new(
            tauri::async_runtime::spawn(std::future::pending()),
            DatabaseState::NotConfigured,
        );
        assert_busy_phase(&lifecycle);
        tauri::async_runtime::block_on(lifecycle.shutdown());
    }
    fn assert_busy_phase(lifecycle: &Lifecycle) {
        let guard = lifecycle.phase.try_lock().expect("진입점 잠금");
        assert_eq!(lifecycle.exit_decision(), ExitDecision::Defer);
        assert_eq!(*guard, LifecyclePhase::Running);
    }
}
