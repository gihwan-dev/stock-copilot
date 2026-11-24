use crate::fixture_tests::{RecordingJob, code_config, kst, runtime};
use crate::{
    DisabledAiJobQueue, JobConfig, JobEnablement, JobError, JobKind, JobRegistry,
    JobScheduleChange, JobScheduleSettingsRepository, KstTimeOfDay, MissedPolicy, ScheduleRule,
    ScheduleSettings, ScheduleSettingsError, ScheduledJobId, Scheduler, StartupPolicy,
};
use application::{EventBus, EventBusBuilder, EventSubscriber, SubscriberFailure};
use domain::{DomainEvent, DomainEventKind};
use std::collections::BTreeSet;
use std::sync::Arc;
use testkit_ports::SteppedClock;
use tokio::sync::RwLock;

#[derive(Default)]
struct MemorySettings {
    saved: RwLock<Vec<JobScheduleChange>>,
    failure: Option<JobError>,
}

impl JobScheduleSettingsRepository for MemorySettings {
    async fn load(&self) -> Result<Vec<JobScheduleChange>, JobError> {
        Ok(self.saved.read().await.clone())
    }

    async fn save(&self, change: JobScheduleChange) -> Result<(), JobError> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        self.saved.write().await.push(change);
        Ok(())
    }
}

#[derive(Default)]
struct CountingSubscriber(Arc<std::sync::atomic::AtomicUsize>);

impl EventSubscriber for CountingSubscriber {
    fn on_event(&self, _event: &DomainEvent) -> Result<(), SubscriberFailure> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

fn time(hour: u8, minute: u8) -> KstTimeOfDay {
    KstTimeOfDay::try_new(hour, minute).expect("시각")
}

fn ai_config(schedule: ScheduleRule) -> JobConfig {
    JobConfig {
        kind: JobKind::AiBackground,
        schedule,
        missed_policy: MissedPolicy::Skip,
        enabled: JobEnablement::Enabled,
    }
}

type Settings = ScheduleSettings<SteppedClock, DisabledAiJobQueue, MemorySettings>;

fn settings_with(
    repository: Arc<MemorySettings>,
    events: EventBus,
) -> (Settings, Arc<SteppedClock>) {
    let mut registry = JobRegistry::new();
    let (briefing, _) = RecordingJob::new(
        ScheduledJobId::MorningBriefing,
        ai_config(ScheduleRule::DailyKst(time(8, 20))),
        StartupPolicy::WaitForSchedule,
    );
    let (watch, _) = RecordingJob::new(
        ScheduledJobId::IntradayWatch,
        ai_config(ScheduleRule::EventTriggered),
        StartupPolicy::WaitForSchedule,
    );
    let (backup, _) = RecordingJob::new(
        ScheduledJobId::DailyBackup,
        code_config(
            ScheduleRule::DailyKst(time(4, 0)),
            MissedPolicy::CatchUpOnce,
        ),
        StartupPolicy::WaitForSchedule,
    );
    registry.register(briefing).expect("등록");
    registry.register(watch).expect("등록");
    registry.register(backup).expect("등록");
    let clock = Arc::new(SteppedClock::new(kst(2026, 11, 25, 7, 0)));
    let scheduler = Arc::new(
        Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러"),
    );
    (ScheduleSettings::new(scheduler, repository, events), clock)
}

#[test]
fn changing_ai_job_time_saves_applies_and_announces() {
    runtime().block_on(async {
        let repository = Arc::new(MemorySettings::default());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::SettingsChanged]),
            CountingSubscriber(Arc::clone(&count)),
        );
        let (settings, _clock) = settings_with(Arc::clone(&repository), builder.build());
        let change = JobScheduleChange {
            id: ScheduledJobId::MorningBriefing,
            enabled: JobEnablement::Enabled,
            time: Some(time(7, 30)),
        };
        let snapshot = settings.change(change).await.expect("변경");
        assert_eq!(
            snapshot.config.schedule,
            ScheduleRule::DailyKst(time(7, 30))
        );
        assert_eq!(snapshot.next_due, Some(kst(2026, 11, 25, 7, 30)));
        assert_eq!(*repository.saved.read().await, vec![change]);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 1);
        let configurable: Vec<ScheduledJobId> = settings
            .configurable_jobs()
            .await
            .into_iter()
            .map(|entry| entry.id)
            .collect();
        assert_eq!(
            configurable,
            vec![
                ScheduledJobId::MorningBriefing,
                ScheduledJobId::IntradayWatch
            ]
        );
        assert_eq!(settings.job_status().await.len(), 3);
    });
}

#[test]
fn code_jobs_and_unknown_jobs_are_rejected_without_saving() {
    runtime().block_on(async {
        let repository = Arc::new(MemorySettings::default());
        let (settings, _clock) = settings_with(Arc::clone(&repository), EventBus::default());
        let backup = settings
            .change(JobScheduleChange {
                id: ScheduledJobId::DailyBackup,
                enabled: JobEnablement::Disabled,
                time: None,
            })
            .await;
        assert_eq!(
            backup,
            Err(ScheduleSettingsError::NotConfigurable(
                ScheduledJobId::DailyBackup
            ))
        );
        let unknown = settings
            .change(JobScheduleChange {
                id: ScheduledJobId::WeeklyReview,
                enabled: JobEnablement::Enabled,
                time: None,
            })
            .await;
        assert_eq!(
            unknown,
            Err(ScheduleSettingsError::UnknownJob(
                ScheduledJobId::WeeklyReview
            ))
        );
        assert!(repository.saved.read().await.is_empty());
    });
}

#[test]
fn event_jobs_can_be_switched_off_but_not_retimed() {
    runtime().block_on(async {
        let repository = Arc::new(MemorySettings::default());
        let (settings, _clock) = settings_with(Arc::clone(&repository), EventBus::default());
        let retime = settings
            .change(JobScheduleChange {
                id: ScheduledJobId::IntradayWatch,
                enabled: JobEnablement::Enabled,
                time: Some(time(9, 0)),
            })
            .await;
        assert_eq!(
            retime,
            Err(ScheduleSettingsError::TimeNotAdjustable(
                ScheduledJobId::IntradayWatch
            ))
        );
        let off = settings
            .change(JobScheduleChange {
                id: ScheduledJobId::IntradayWatch,
                enabled: JobEnablement::Disabled,
                time: None,
            })
            .await
            .expect("끄기");
        assert_eq!(off.config.enabled, JobEnablement::Disabled);
        assert_eq!(repository.saved.read().await.len(), 1);
    });
}

#[test]
fn storage_failure_leaves_schedule_unchanged() {
    runtime().block_on(async {
        let repository = Arc::new(MemorySettings {
            saved: RwLock::new(Vec::new()),
            failure: Some(JobError::StorageFailure(String::from("disk"))),
        });
        let (settings, _clock) = settings_with(repository, EventBus::default());
        let failed = settings
            .change(JobScheduleChange {
                id: ScheduledJobId::MorningBriefing,
                enabled: JobEnablement::Disabled,
                time: None,
            })
            .await;
        assert_eq!(
            failed,
            Err(ScheduleSettingsError::Storage(JobError::StorageFailure(
                String::from("disk")
            )))
        );
        let briefing = settings
            .job_status()
            .await
            .into_iter()
            .find(|snapshot| snapshot.id == ScheduledJobId::MorningBriefing)
            .expect("브리핑");
        assert_eq!(briefing.config.enabled, JobEnablement::Enabled);
    });
}

#[test]
fn restore_applies_saved_changes_and_reports_rejected_ones() {
    runtime().block_on(async {
        let saved = vec![
            JobScheduleChange {
                id: ScheduledJobId::MorningBriefing,
                enabled: JobEnablement::Disabled,
                time: Some(time(9, 10)),
            },
            JobScheduleChange {
                id: ScheduledJobId::DailyBackup,
                enabled: JobEnablement::Disabled,
                time: None,
            },
        ];
        let repository = Arc::new(MemorySettings {
            saved: RwLock::new(saved),
            failure: None,
        });
        let (settings, _clock) = settings_with(repository, EventBus::default());
        let report = settings.restore().await.expect("복원");
        assert_eq!(report.applied, vec![ScheduledJobId::MorningBriefing]);
        assert_eq!(
            report.rejected,
            vec![ScheduleSettingsError::NotConfigurable(
                ScheduledJobId::DailyBackup
            )]
        );
        let briefing = settings
            .job_status()
            .await
            .into_iter()
            .find(|snapshot| snapshot.id == ScheduledJobId::MorningBriefing)
            .expect("브리핑");
        assert_eq!(briefing.config.enabled, JobEnablement::Disabled);
        assert_eq!(
            briefing.config.schedule,
            ScheduleRule::DailyKst(time(9, 10))
        );
    });
}
