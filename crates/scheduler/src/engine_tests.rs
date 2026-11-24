use crate::fixture_tests::{
    FixedCalendar, RecordingJob, code_config, kr_calendar_with_late_close, kst, runtime, span_secs,
};
use crate::{
    DisabledAiJobQueue, JobExecutionRecord, JobRecordSink, JobRegistry, KstTimeOfDay, MissedPolicy,
    NoMarketCalendar, ScheduleRule, ScheduledJobId, Scheduler, SchedulerOptions, StartupPolicy,
    trading_status_now,
};
use application::EventSubscriber;
use domain::{ClientOrderId, DomainEvent, KnownMarket, MarketTradingStatus};
use std::sync::Arc;
use testkit_ports::SteppedClock;
use tokio::sync::RwLock;

fn daily(hour: u8, minute: u8) -> ScheduleRule {
    ScheduleRule::DailyKst(KstTimeOfDay::try_new(hour, minute).expect("시각"))
}

#[test]
fn run_at_startup_jobs_run_on_first_tick_and_others_wait() {
    runtime().block_on(async {
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 25, 10, 0)));
        let (eager, eager_runs) = RecordingJob::new(
            ScheduledJobId::EstimateResolution,
            code_config(daily(16, 10), MissedPolicy::CatchUpOnce),
            StartupPolicy::RunAtStartup,
        );
        let (lazy, lazy_runs) = RecordingJob::new(
            ScheduledJobId::DailyBackup,
            code_config(daily(4, 0), MissedPolicy::CatchUpOnce),
            StartupPolicy::WaitForSchedule,
        );
        let mut registry = JobRegistry::new();
        registry.register(eager).expect("등록");
        registry.register(lazy).expect("등록");
        let scheduler = Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러");
        let report = scheduler.tick_once().await;
        assert_eq!(report.records.len(), 1);
        assert_eq!(eager_runs.read().await.len(), 1);
        assert_eq!(lazy_runs.read().await.len(), 0);
        let snapshot = scheduler.status_snapshots().await;
        let eager_state = snapshot
            .iter()
            .find(|entry| entry.id == ScheduledJobId::EstimateResolution)
            .expect("상태");
        assert_eq!(eager_state.next_due, Some(kst(2026, 11, 25, 16, 10)));
    });
}

#[test]
fn restore_catches_up_once_only_when_the_latest_occurrence_was_missed() {
    runtime().block_on(async {
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 25, 10, 0)));
        let (missed, missed_runs) = RecordingJob::new(
            ScheduledJobId::MorningBriefing,
            code_config(daily(8, 20), MissedPolicy::CatchUpOnce),
            StartupPolicy::WaitForSchedule,
        );
        let (fresh, fresh_runs) = RecordingJob::new(
            ScheduledJobId::DailyBackup,
            code_config(daily(4, 0), MissedPolicy::CatchUpOnce),
            StartupPolicy::WaitForSchedule,
        );
        let (skip, skip_runs) = RecordingJob::new(
            ScheduledJobId::PreUsMarketCheck,
            code_config(daily(8, 0), MissedPolicy::Skip),
            StartupPolicy::WaitForSchedule,
        );
        let mut registry = JobRegistry::new();
        registry.register(missed).expect("등록");
        registry.register(fresh).expect("등록");
        registry.register(skip).expect("등록");
        let scheduler = Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러");
        scheduler
            .restore_last_runs(&[
                (ScheduledJobId::MorningBriefing, kst(2026, 11, 21, 8, 20)),
                (ScheduledJobId::DailyBackup, kst(2026, 11, 25, 4, 0)),
                (ScheduledJobId::PreUsMarketCheck, kst(2026, 11, 20, 8, 0)),
            ])
            .await;
        let first = scheduler.tick_once().await;
        let second = scheduler.tick_once().await;
        assert_eq!(first.records.len(), 1);
        assert_eq!(first.records[0].scheduled_for, kst(2026, 11, 25, 8, 20));
        assert_eq!(second.records.len(), 0);
        assert_eq!(missed_runs.read().await.len(), 1);
        assert_eq!(fresh_runs.read().await.len(), 0);
        assert_eq!(skip_runs.read().await.len(), 0);
    });
}

#[test]
fn triggers_run_event_jobs_once_and_order_submission_triggers_portfolio_sync() {
    runtime().block_on(async {
        let clock = Arc::new(SteppedClock::new(kst(2026, 11, 25, 10, 0)));
        let (watch, watch_runs) = RecordingJob::new(
            ScheduledJobId::IntradayWatch,
            code_config(ScheduleRule::EventTriggered, MissedPolicy::CatchUpOnce),
            StartupPolicy::WaitForSchedule,
        );
        let (sync, sync_runs) = RecordingJob::new(
            ScheduledJobId::PortfolioSync,
            code_config(
                ScheduleRule::Interval(span_secs(300)),
                MissedPolicy::CatchUpOnce,
            ),
            StartupPolicy::WaitForSchedule,
        );
        let mut registry = JobRegistry::new();
        registry.register(watch).expect("등록");
        registry.register(sync).expect("등록");
        let scheduler = Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러");
        assert_eq!(scheduler.tick_once().await.records.len(), 0);
        let triggers = scheduler.triggers();
        triggers.trigger(ScheduledJobId::IntradayWatch);
        triggers.trigger(ScheduledJobId::IntradayWatch);
        assert_eq!(scheduler.tick_once().await.records.len(), 1);
        assert_eq!(scheduler.tick_once().await.records.len(), 0);
        assert_eq!(watch_runs.read().await.len(), 1);
        let order = ClientOrderId::try_new("order-1").expect("주문 ID");
        scheduler
            .event_subscriber()
            .on_event(&DomainEvent::OrderSubmitted(order))
            .expect("구독");
        let report = scheduler.tick_once().await;
        assert_eq!(report.records.len(), 1);
        assert_eq!(report.records[0].id, ScheduledJobId::PortfolioSync);
        assert_eq!(sync_runs.read().await.len(), 1);
    });
}

#[test]
fn market_rules_follow_the_calendar_and_fall_back_without_one() {
    runtime().block_on(async {
        let start = kst(2026, 11, 25, 10, 0);
        let rule = ScheduleRule::AfterMarketClose {
            market: KnownMarket::Kr,
            delay: span_secs(600),
            fallback: KstTimeOfDay::try_new(15, 40).expect("시각"),
        };
        let build = |calendar: bool| {
            let (job, _runs) = RecordingJob::new(
                ScheduledJobId::EstimateResolution,
                code_config(rule, MissedPolicy::CatchUpOnce),
                StartupPolicy::WaitForSchedule,
            );
            let mut registry = JobRegistry::new();
            registry.register(job).expect("등록");
            let options = SchedulerOptions {
                calendars: if calendar {
                    Arc::new(FixedCalendar(kr_calendar_with_late_close()))
                } else {
                    Arc::new(NoMarketCalendar)
                },
                ..SchedulerOptions::default()
            };
            Scheduler::with_options(
                Arc::new(SteppedClock::new(start)),
                Arc::new(DisabledAiJobQueue),
                registry,
                options,
            )
            .expect("스케줄러")
        };
        let with_calendar = build(true).status_snapshots().await;
        assert_eq!(with_calendar[0].next_due, Some(kst(2026, 11, 25, 16, 40)));
        let fallback = build(false).status_snapshots().await;
        assert_eq!(fallback[0].next_due, Some(kst(2026, 11, 25, 15, 40)));
    });
}

#[test]
fn trading_status_is_open_inside_regular_hours_and_unknown_without_calendar() {
    let source = FixedCalendar(kr_calendar_with_late_close());
    assert_eq!(
        trading_status_now(&source, kst(2026, 11, 25, 10, 0)),
        MarketTradingStatus::Open
    );
    assert_eq!(
        trading_status_now(&source, kst(2026, 11, 25, 20, 0)),
        MarketTradingStatus::Unknown
    );
    assert_eq!(
        trading_status_now(&NoMarketCalendar, kst(2026, 11, 25, 10, 0)),
        MarketTradingStatus::Unknown
    );
}

struct CollectingSink(RwLock<Vec<JobExecutionRecord>>);

impl JobRecordSink for CollectingSink {
    fn record(&self, record: &JobExecutionRecord) {
        if let Ok(mut guard) = self.0.try_write() {
            guard.push(record.clone());
        }
    }
}

#[test]
fn record_sink_receives_every_completed_run() {
    runtime().block_on(async {
        let sink = Arc::new(CollectingSink(RwLock::new(Vec::new())));
        let (job, _runs) = RecordingJob::new(
            ScheduledJobId::DailyBackup,
            code_config(daily(4, 0), MissedPolicy::CatchUpOnce),
            StartupPolicy::RunAtStartup,
        );
        let mut registry = JobRegistry::new();
        registry.register(job).expect("등록");
        let sink_handle: Arc<dyn JobRecordSink> = Arc::<CollectingSink>::clone(&sink);
        let scheduler = Scheduler::with_options(
            Arc::new(SteppedClock::new(kst(2026, 11, 25, 10, 0))),
            Arc::new(DisabledAiJobQueue),
            registry,
            SchedulerOptions {
                records: sink_handle,
                ..SchedulerOptions::default()
            },
        )
        .expect("스케줄러");
        scheduler.tick_once().await;
        assert_eq!(sink.0.read().await.len(), 1);
    });
}
