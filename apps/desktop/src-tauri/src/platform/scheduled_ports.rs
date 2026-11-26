use crate::ports::{PortTypes, Ports};
use application::MarketInfo as _;
use application::{
    BriefingOutcome, CheckTossSpec, FullInspectionRun, HoldingsBriefingContext,
    IntradayReviewOutcome, IntradayReviewSubject, RunBriefing, RunFullInspection,
    RunIntradayReview, SecretKey, SecretStore, SpecDriftStatus, TakeValuationSnapshot,
    TossSpecBaseline, TossSpecSource, TransactionSchedule,
};
use domain::{
    BriefingKind, CalendarDate, InspectionRangeDays, KnownMarket, SnapshotCalendar, Timestamp,
};
use scheduler::{
    BackupResult, BackupRetention, BriefingPort, BriefingRunResult, CompactionResult,
    DatabaseBackupPort, IntradayCompactionPort, JobError, SpecWatchPort, SpecWatchResult,
    TransactionSyncPort, TransactionSyncTally,
};
use scheduler::{
    DailyValuationPort, EstimateResolutionPort, IntradayReviewPort, ResolutionTally, WatchCause,
};
use std::marker::PhantomData;
use std::sync::Arc;
use store_sqlite::SqliteRepositories;
use tauri::{AppHandle, Manager, Runtime};

pub const FULL_INSPECTION_RANGE_DAYS: u32 = u32::MAX;

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

    fn estimate_flow(&self) -> Option<crate::estimate_flow::EstimateFlowSlot> {
        self.app
            .try_state::<crate::estimate_flow::EstimateFlowSlot>()
            .map(|state| state.inner().clone())
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

impl<R: Runtime, P: PortTypes> scheduler::NewsRetentionPort for AppStoredJobs<R, P> {
    async fn prune_expired_news(&self) -> Result<u32, JobError> {
        let ports = self.ports()?;
        let removed = application::PruneCollectedNews {
            store: &self.repositories,
            clock: ports.clock.as_ref(),
        }
        .run()
        .await
        .map_err(storage)?;
        Ok(*removed.as_u32())
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
        for account in &ports.saved_accounts.current() {
            let synced = recent.sync_recent(account).await;
            crate::diagnostics_log::record(&application::DiagnosticEvent::Sync(
                application::SyncDiagnosis {
                    job: application::SyncJob::Transactions,
                    result: match &synced {
                        Ok(_delivery) => application::SyncResult::Complete,
                        Err(_error) => application::SyncResult::Failed,
                    },
                    errors: Vec::new(),
                },
            ));
            match synced {
                Ok(_delivery) => tally.recent_synced = tally.recent_synced.saturating_add(1),
                Err(error) => first_failure = first_failure.or_else(|| Some(storage(error))),
            }
            let inspected = full.run_full_inspection(account, range_limit).await.outcome;
            record_full_inspection(&inspected);
            match inspected {
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

fn record_full_inspection<E>(inspected: &Result<FullInspectionRun, E>) {
    let result = match inspected {
        Ok(FullInspectionRun::Completed { .. }) => application::SyncResult::Complete,
        Ok(FullInspectionRun::AlreadyCompletedToday) => return,
        Err(_error) => application::SyncResult::Failed,
    };
    crate::diagnostics_log::record(&application::DiagnosticEvent::Sync(
        application::SyncDiagnosis {
            job: application::SyncJob::TransactionsFullInspection,
            result,
            errors: Vec::new(),
        },
    ));
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

impl<R: Runtime, P: PortTypes> BriefingPort for AppStoredJobs<R, P> {
    async fn run(
        &self,
        kind: BriefingKind,
        _now: Timestamp,
    ) -> Result<BriefingRunResult, JobError> {
        let ports = self.ports()?;
        let accounts = ports.saved_accounts.current();
        let context = HoldingsBriefingContext {
            portfolios: ports.portfolios.as_ref(),
            news: ports.news.as_ref(),
            accounts: &accounts,
        };
        let report = RunBriefing {
            ai: ports.ai_provider.as_ref(),
            repository: &self.repositories,
            context: &context,
            clock: ports.clock.as_ref(),
            events: &ports.events,
        }
        .execute(kind)
        .await
        .map_err(storage)?;
        match report.record.outcome {
            BriefingOutcome::Completed { .. } => Ok(BriefingRunResult::Completed),
            BriefingOutcome::Failed(_) => Ok(BriefingRunResult::Failed),
        }
    }
}

impl<R: Runtime, P: PortTypes> DailyValuationPort for AppStoredJobs<R, P> {
    async fn record_today(&self, _now: Timestamp) -> Result<(), JobError> {
        let ports = self.ports()?;
        let unavailable = |_error| JobError::PortUnavailable(String::from("market-calendar"));
        let kr = ports
            .market_info
            .calendar(KnownMarket::Kr)
            .await
            .map_err(unavailable)?;
        let us = ports
            .market_info
            .calendar(KnownMarket::Us)
            .await
            .map_err(unavailable)?;
        let calendar = SnapshotCalendar::new(&kr, &us)
            .map_err(|error| JobError::ExecutionFailed(format!("{error:?}")))?;
        let use_case = TakeValuationSnapshot::new(
            ports.portfolios.as_ref(),
            &self.repositories,
            &self.repositories,
            ports.clock.as_ref(),
            &ports.events,
        );
        let mut first_failure = None;
        for account in &ports.saved_accounts.current() {
            if let Err(error) = use_case.execute(account, &calendar).await {
                first_failure =
                    first_failure.or_else(|| Some(JobError::ExecutionFailed(format!("{error:?}"))));
            }
        }
        first_failure.map_or(Ok(()), Err)
    }

    fn missing_days(
        &self,
        _now: Timestamp,
    ) -> impl Future<Output = Result<Vec<CalendarDate>, JobError>> + Send {
        std::future::ready(Ok(Vec::new()))
    }

    fn recompute_day(
        &self,
        _day: CalendarDate,
    ) -> impl Future<Output = Result<(), JobError>> + Send {
        std::future::ready(Err(JobError::PortUnavailable(String::from(
            "valuation-backfill",
        ))))
    }
}

impl<R: Runtime, P: PortTypes> IntradayReviewPort for AppStoredJobs<R, P> {
    async fn review(&self, cause: WatchCause, _now: Timestamp) -> Result<(), JobError> {
        let ports = self.ports()?;
        let subject = match cause {
            WatchCause::UrgentDisclosure(key) => IntradayReviewSubject::UrgentDisclosure(key),
            WatchCause::PriceMove(instrument) => IntradayReviewSubject::PriceMove(instrument),
            WatchCause::NewsSurge(instrument) => IntradayReviewSubject::NewsSurge(instrument),
        };
        if let Some(flow) = self.estimate_flow() {
            match flow.0.intraday(&subject).await? {
                application::SingleEstimateOutcome::Submitted
                | application::SingleEstimateOutcome::NotSubmitted => return Ok(()),
                application::SingleEstimateOutcome::NoTask
                | application::SingleEstimateOutcome::Failed => {}
            }
        }
        let outcome = RunIntradayReview {
            ai: ports.ai_provider.as_ref(),
            clock: ports.clock.as_ref(),
        }
        .execute(&subject)
        .await;
        match outcome {
            IntradayReviewOutcome::Completed => Ok(()),
            IntradayReviewOutcome::IsolationRejected
            | IntradayReviewOutcome::ToolsUnavailable
            | IntradayReviewOutcome::RateLimited
            | IntradayReviewOutcome::ProviderUnavailable
            | IntradayReviewOutcome::PromptRejected => {
                Err(JobError::ExecutionFailed(format!("{outcome:?}")))
            }
        }
    }
}

impl<R: Runtime, P: PortTypes> EstimateResolutionPort for AppStoredJobs<R, P> {
    async fn resolve_due(&self, now: Timestamp) -> Result<ResolutionTally, JobError> {
        use application::resolve_estimates::{ResolutionStatus, ResolveEstimates};
        let ports = self.ports()?;
        let slot = self
            .app
            .try_state::<crate::platform::resolution::ResolutionDataSlot>()
            .map(|state| state.inner().clone())
            .ok_or_else(|| JobError::PortUnavailable(String::from("resolution-data")))?;
        let data = crate::platform::resolution::SlotData(slot.0.as_ref());
        let reports = ResolveEstimates::new(&self.repositories, &data, &ports.events)
            .execute(now)
            .await
            .map_err(storage)?;
        let mut tally = ResolutionTally::default();
        for report in reports {
            match report.status {
                ResolutionStatus::Resolved { brier, .. } => {
                    tally.resolved = tally.resolved.saturating_add(1);
                    tally.brier = brier
                        .and_then(|comparison| tally.brier.clone().with(comparison))
                        .unwrap_or_else(|| tally.brier.clone());
                }
                ResolutionStatus::AlreadyResolved => {
                    tally.resolved = tally.resolved.saturating_add(1);
                }
                ResolutionStatus::Ongoing => tally.ongoing = tally.ongoing.saturating_add(1),
                ResolutionStatus::Unavailable(_)
                | ResolutionStatus::Storage(_)
                | ResolutionStatus::Calibration(_)
                | ResolutionStatus::MissingFact
                | ResolutionStatus::Conflict => {
                    tally.unavailable = tally.unavailable.saturating_add(1);
                }
            }
        }
        Ok(tally)
    }
}

impl<R: Runtime, P: PortTypes> scheduler::EstimateCyclePort for AppStoredJobs<R, P> {
    async fn run_cycle(&self, _now: Timestamp) -> Result<scheduler::EstimateCycleTally, JobError> {
        let flow = self
            .estimate_flow()
            .ok_or_else(|| JobError::PortUnavailable(String::from("estimate-flow")))?;
        flow.0.cycle().await
    }
}
