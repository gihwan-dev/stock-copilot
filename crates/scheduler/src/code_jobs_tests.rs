use crate::fixture_tests::{date, kst, runtime, span_secs};
use crate::{
    BACKUP_RETENTION_DAYS, BackupResult, BackupRetention, CompactionResult, DailyBackupJob,
    DailyValuationJob, DailyValuationPort, DatabaseBackupPort, DisabledAiJobQueue,
    EstimateResolutionJob, EstimateResolutionPort, IntradayCompactionJob, IntradayCompactionPort,
    JobError, JobRegistry, JobRunOutcome, KstTimeOfDay, ResolutionTally, ScheduleRule,
    ScheduledJob, ScheduledJobId, Scheduler, SpecWatchPort, SpecWatchResult, StartupPolicy,
    TossSpecWatchJob,
};
use domain::{CalendarDate, KnownMarket, Timestamp};
use std::sync::Arc;
use testkit_ports::SteppedClock;
use tokio::sync::RwLock;

struct FixedSpec(SpecWatchResult);

impl SpecWatchPort for FixedSpec {
    fn check(
        &self,
        _now: Timestamp,
    ) -> impl Future<Output = Result<SpecWatchResult, JobError>> + Send {
        std::future::ready(Ok(self.0))
    }
}

#[test]
fn spec_watch_reports_changes_and_runs_daily() {
    runtime().block_on(async {
        let changed = TossSpecWatchJob::new(Arc::new(FixedSpec(SpecWatchResult::Changed)));
        let detail = changed.run(kst(2026, 11, 25, 7, 0)).await.expect("감시");
        assert_eq!(detail.summary_code, "spec_changed");
        assert_eq!(
            changed.default_config().schedule,
            ScheduleRule::DailyKst(KstTimeOfDay::try_new(7, 0).expect("시각"))
        );
        let unchanged = TossSpecWatchJob::new(Arc::new(FixedSpec(SpecWatchResult::Unchanged)));
        let unchanged_detail = unchanged.run(kst(2026, 11, 25, 7, 0)).await.expect("감시");
        assert_eq!(unchanged_detail.summary_code, "spec_unchanged");
    });
}

struct ScriptedBackup {
    result: BackupResult,
    seen: RwLock<Vec<BackupRetention>>,
}

impl DatabaseBackupPort for ScriptedBackup {
    async fn back_up(
        &self,
        _now: Timestamp,
        retention: BackupRetention,
    ) -> Result<BackupResult, JobError> {
        self.seen.write().await.push(retention);
        Ok(self.result)
    }
}

#[test]
fn backup_keeps_seven_days_and_fails_when_verification_fails() {
    runtime().block_on(async {
        let confirmed = Arc::new(ScriptedBackup {
            result: BackupResult::Confirmed { retained_copies: 7 },
            seen: RwLock::new(Vec::new()),
        });
        let detail = DailyBackupJob::new(Arc::clone(&confirmed))
            .run(kst(2026, 11, 25, 4, 0))
            .await
            .expect("백업");
        assert_eq!(detail.processed_items, 7);
        assert_eq!(confirmed.seen.read().await[0].days(), BACKUP_RETENTION_DAYS);
        let rejected = Arc::new(ScriptedBackup {
            result: BackupResult::VerificationFailed,
            seen: RwLock::new(Vec::new()),
        });
        let mut registry = JobRegistry::new();
        registry
            .register(DailyBackupJob::new(rejected))
            .expect("등록");
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 25, 3, 59)));
        let scheduler = Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러");
        clock.advance_to(kst(2026, 11, 25, 4, 0)).expect("시계");
        let report = scheduler.tick_once().await;
        assert_eq!(
            report.records[0].outcome,
            JobRunOutcome::Failed(JobError::ExecutionFailed(String::from(
                "backup_verification_failed"
            )))
        );
    });
}

#[derive(Default)]
struct RecordingCompaction {
    calls: RwLock<Vec<CompactionCall>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactionCall {
    Compact(Timestamp),
    Vacuum,
}

impl IntradayCompactionPort for RecordingCompaction {
    async fn compact_before(&self, cutoff: Timestamp) -> Result<CompactionResult, JobError> {
        self.calls
            .write()
            .await
            .push(CompactionCall::Compact(cutoff));
        Ok(CompactionResult { compacted_days: 2 })
    }

    async fn incremental_vacuum(&self) -> Result<(), JobError> {
        self.calls.write().await.push(CompactionCall::Vacuum);
        Ok(())
    }
}

#[test]
fn compaction_cuts_thirty_days_back_and_vacuums_afterwards() {
    runtime().block_on(async {
        let port = Arc::new(RecordingCompaction::default());
        let job = IntradayCompactionJob::new(Arc::clone(&port));
        let detail = job.run(kst(2026, 11, 25, 3, 30)).await.expect("축소");
        assert_eq!(detail.processed_items, 2);
        let calls = port.calls.read().await.clone();
        assert_eq!(
            calls,
            vec![
                CompactionCall::Compact(kst(2026, 10, 26, 3, 30)),
                CompactionCall::Vacuum
            ]
        );
    });
}

struct ScriptedValuation {
    missing: Vec<CalendarDate>,
    failing: Option<CalendarDate>,
    recorded: RwLock<Vec<Timestamp>>,
    recomputed: RwLock<Vec<CalendarDate>>,
}

impl ScriptedValuation {
    fn new(missing: Vec<CalendarDate>, failing: Option<CalendarDate>) -> Self {
        Self {
            missing,
            failing,
            recorded: RwLock::new(Vec::new()),
            recomputed: RwLock::new(Vec::new()),
        }
    }
}

impl DailyValuationPort for ScriptedValuation {
    async fn record_today(&self, now: Timestamp) -> Result<(), JobError> {
        self.recorded.write().await.push(now);
        Ok(())
    }

    fn missing_days(
        &self,
        _now: Timestamp,
    ) -> impl Future<Output = Result<Vec<CalendarDate>, JobError>> + Send {
        std::future::ready(Ok(self.missing.clone()))
    }

    async fn recompute_day(&self, day: CalendarDate) -> Result<(), JobError> {
        self.recomputed.write().await.push(day);
        if self.failing == Some(day) {
            return Err(JobError::StorageFailure(String::from("recompute")));
        }
        Ok(())
    }
}

#[test]
fn valuation_recomputes_missing_days_and_records_today_only_after_sixteen() {
    runtime().block_on(async {
        let missing = vec![date(2026, 11, 20), date(2026, 11, 21)];
        let morning = Arc::new(ScriptedValuation::new(missing.clone(), None));
        let job = DailyValuationJob::new(Arc::clone(&morning));
        assert_eq!(job.startup_policy(), StartupPolicy::RunAtStartup);
        let detail = job.run(kst(2026, 11, 25, 9, 0)).await.expect("평가");
        assert_eq!(detail.processed_items, 2);
        assert_eq!(morning.recorded.read().await.len(), 0);
        assert_eq!(*morning.recomputed.read().await, missing);
        let evening = Arc::new(ScriptedValuation::new(Vec::new(), None));
        DailyValuationJob::new(Arc::clone(&evening))
            .run(kst(2026, 11, 25, 16, 0))
            .await
            .expect("평가");
        assert_eq!(evening.recorded.read().await.len(), 1);
    });
}

#[test]
fn valuation_keeps_going_after_a_failed_day_and_reports_the_failure() {
    runtime().block_on(async {
        let missing = vec![date(2026, 11, 20), date(2026, 11, 21)];
        let port = Arc::new(ScriptedValuation::new(
            missing.clone(),
            Some(date(2026, 11, 20)),
        ));
        let result = DailyValuationJob::new(Arc::clone(&port))
            .run(kst(2026, 11, 25, 9, 0))
            .await;
        assert_eq!(
            result,
            Err(JobError::StorageFailure(String::from("recompute")))
        );
        assert_eq!(*port.recomputed.read().await, missing);
    });
}

struct CountingResolution(RwLock<Vec<Timestamp>>);

impl EstimateResolutionPort for CountingResolution {
    async fn resolve_due(&self, now: Timestamp) -> Result<ResolutionTally, JobError> {
        self.0.write().await.push(now);
        Ok(ResolutionTally {
            resolved: 3,
            ongoing: 1,
            unavailable: 0,
        })
    }
}

#[test]
fn estimate_resolution_runs_after_each_close_and_catches_up_at_startup() {
    runtime().block_on(async {
        let port = Arc::new(CountingResolution(RwLock::new(Vec::new())));
        let domestic = EstimateResolutionJob::domestic(Arc::clone(&port));
        let us = EstimateResolutionJob::us(Arc::clone(&port));
        assert_eq!(domestic.id(), ScheduledJobId::EstimateResolution);
        assert_eq!(us.id(), ScheduledJobId::UsEstimateResolution);
        assert_eq!(
            domestic.default_config().schedule,
            ScheduleRule::AfterMarketClose {
                market: KnownMarket::Kr,
                delay: span_secs(2_400),
                fallback: KstTimeOfDay::try_new(16, 10).expect("시각"),
            }
        );
        assert_eq!(
            us.default_config().schedule,
            ScheduleRule::AfterMarketClose {
                market: KnownMarket::Us,
                delay: span_secs(1_800),
                fallback: KstTimeOfDay::try_new(6, 30).expect("시각"),
            }
        );
        let mut registry = JobRegistry::new();
        registry.register(domestic).expect("등록");
        registry.register(us).expect("등록");
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 25, 12, 0)));
        let scheduler = Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러");
        let report = scheduler.tick_once().await;
        assert_eq!(report.records.len(), 1);
        assert_eq!(report.records[0].id, ScheduledJobId::EstimateResolution);
        clock.advance_by(span_secs(4 * 3_600 + 600)).expect("시계");
        let after_close = scheduler.tick_once().await;
        assert_eq!(after_close.records.len(), 1);
        assert_eq!(port.0.read().await.len(), 2);
    });
}
