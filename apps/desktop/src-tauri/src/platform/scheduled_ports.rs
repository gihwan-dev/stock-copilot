use crate::ports::{PortTypes, Ports};
use application::{
    CheckTossSpec, FullInspectionRun, RunFullInspection, SecretKey, SecretStore, SpecDriftStatus,
    TossSpecBaseline, TossSpecSource, TransactionSchedule,
};
use domain::{InspectionRangeDays, Timestamp};
use scheduler::{
    BackupResult, BackupRetention, CompactionResult, DatabaseBackupPort, IntradayCompactionPort,
    JobError, SpecWatchPort, SpecWatchResult, TransactionSyncPort, TransactionSyncTally,
};
use std::marker::PhantomData;
use std::sync::Arc;
use store_sqlite::SqliteRepositories;
use tauri::{AppHandle, Manager, Runtime};

pub const FULL_INSPECTION_RANGE_DAYS: u32 = 31;

pub struct AppStoredJobs<R: Runtime, P: PortTypes> {
    app: AppHandle<R>,
    repositories: SqliteRepositories,
    ports: PhantomData<fn() -> P>,
}

impl<R: Runtime, P: PortTypes> std::fmt::Debug for AppStoredJobs<R, P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppStoredJobs").finish_non_exhaustive()
    }
}

impl<R: Runtime, P: PortTypes> AppStoredJobs<R, P> {
    #[must_use]
    pub const fn new(app: AppHandle<R>, repositories: SqliteRepositories) -> Self {
        Self {
            app,
            repositories,
            ports: PhantomData,
        }
    }

    fn ports(&self) -> Result<tauri::State<'_, Ports<P>>, JobError> {
        self.app
            .try_state::<Ports<P>>()
            .ok_or_else(|| JobError::PortUnavailable(String::from("ports")))
    }
}

fn storage(error: impl std::fmt::Debug) -> JobError {
    JobError::StorageFailure(format!("{error:?}"))
}

impl<R: Runtime, P: PortTypes> IntradayCompactionPort for AppStoredJobs<R, P> {
    async fn compact_before(&self, _cutoff: Timestamp) -> Result<CompactionResult, JobError> {
        let ports = self.ports()?;
        let _written = self
            .repositories
            .compact_intraday(ports.clock.as_ref())
            .await
            .map_err(storage)?;
        Ok(CompactionResult { compacted_days: 1 })
    }

    async fn incremental_vacuum(&self) -> Result<(), JobError> {
        self.repositories
            .reclaim_space()
            .await
            .map(|_written| ())
            .map_err(storage)
    }
}

impl<R: Runtime, P: PortTypes> DatabaseBackupPort for AppStoredJobs<R, P> {
    async fn back_up(
        &self,
        _now: Timestamp,
        _retention: BackupRetention,
    ) -> Result<BackupResult, JobError> {
        let ports = self.ports()?;
        let key = ports
            .secrets
            .load(SecretKey::DatabaseEncryptionKey)
            .await
            .map_err(storage)?
            .ok_or_else(|| JobError::PortUnavailable(String::from("database-key")))?;
        let directory = self
            .app
            .path()
            .app_data_dir()
            .map_err(|_error| JobError::PortUnavailable(String::from("app-data-dir")))?
            .join("backups");
        match self
            .repositories
            .back_up(directory, &key, ports.clock.as_ref())
            .await
        {
            Ok(_outcome) => Ok(BackupResult::Confirmed { retained_copies: 1 }),
            Err(_error) => Ok(BackupResult::VerificationFailed),
        }
    }
}

impl<R: Runtime, P: PortTypes> TransactionSyncPort for AppStoredJobs<R, P> {
    async fn sync_all(&self, _now: Timestamp) -> Result<TransactionSyncTally, JobError> {
        let ports = self.ports()?;
        let range_limit = InspectionRangeDays::try_new(FULL_INSPECTION_RANGE_DAYS)
            .map_err(|error| JobError::ExecutionFailed(format!("{error:?}")))?;
        let recent = TransactionSchedule {
            broker: ports.broker_read.as_ref(),
            repository: &self.repositories,
            inspections: &self.repositories,
            clock: ports.clock.as_ref(),
            events: &ports.events,
        };
        let full = RunFullInspection {
            broker: ports.broker_read.as_ref(),
            repository: &self.repositories,
            inspections: &self.repositories,
            clock: ports.clock.as_ref(),
            events: &ports.events,
        };
        let mut tally = TransactionSyncTally::default();
        let mut first_failure = None;
        for account in &ports.saved_accounts {
            match recent.sync_recent(account).await {
                Ok(_delivery) => tally.recent_synced = tally.recent_synced.saturating_add(1),
                Err(error) => first_failure = first_failure.or_else(|| Some(storage(error))),
            }
            match full.run_full_inspection(account, range_limit).await.outcome {
                Ok(FullInspectionRun::Completed { .. }) => {
                    tally.full_inspections_completed =
                        tally.full_inspections_completed.saturating_add(1);
                }
                Ok(FullInspectionRun::AlreadyCompletedToday) => {}
                Err(error) => {
                    first_failure = first_failure
                        .or_else(|| Some(JobError::ExecutionFailed(format!("{error:?}"))));
                }
            }
        }
        first_failure.map_or(Ok(tally), Err)
    }
}

pub struct AppSpecWatch<R: Runtime, P: PortTypes, S> {
    stored: AppStoredJobs<R, P>,
    source: Arc<S>,
    baseline: TossSpecBaseline,
}

impl<R: Runtime, P: PortTypes, S> std::fmt::Debug for AppSpecWatch<R, P, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppSpecWatch").finish_non_exhaustive()
    }
}

impl<R: Runtime, P: PortTypes, S: TossSpecSource> AppSpecWatch<R, P, S> {
    #[must_use]
    pub const fn new(
        stored: AppStoredJobs<R, P>,
        source: Arc<S>,
        baseline: TossSpecBaseline,
    ) -> Self {
        Self {
            stored,
            source,
            baseline,
        }
    }
}

impl<R: Runtime, P: PortTypes, S: TossSpecSource> SpecWatchPort for AppSpecWatch<R, P, S> {
    async fn check(&self, _now: Timestamp) -> Result<SpecWatchResult, JobError> {
        let ports = self.stored.ports()?;
        let report = CheckTossSpec::new(
            self.source.as_ref(),
            &self.stored.repositories,
            ports.clock.as_ref(),
            &ports.events,
        )
        .check(&self.baseline)
        .await
        .map_err(storage)?;
        match report.record.status() {
            SpecDriftStatus::Aligned => Ok(SpecWatchResult::Unchanged),
            SpecDriftStatus::DriftDetected { .. } => Ok(SpecWatchResult::Changed),
            SpecDriftStatus::CheckFailed(error) => {
                Err(JobError::ExecutionFailed(format!("{error:?}")))
            }
        }
    }
}
