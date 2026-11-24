use crate::ai_queue::{AiJobQueue, AiQueueAdmission};
use crate::market_hours::{DueBound, MarketCalendarSource, NoMarketCalendar, market_due};
use crate::registry::{JobRegistry, ScheduledJob};
use crate::schedule_time::{ScheduleTimeError, initial_due, is_within_skip_grace, next_due_after};
use crate::types::{
    JobConfig, JobEnablement, JobError, JobExecutionRecord, JobKind, JobRunOutcome,
    JobScheduleOverride, JobStatusSnapshot, LoopControl, MissedPolicy, ScheduleRule,
    ScheduledJobId, StartupPolicy, TickReport,
};
use application::{Clock, EventSubscriber, SubscriberFailure};
use domain::{DomainEvent, Timestamp};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify, mpsc, watch};

pub const DEFAULT_TICK_INTERVAL_SECONDS: u64 = 30;
const MAX_HISTORY_PER_JOB: usize = 32;

#[derive(Debug, Default)]
pub struct SchedulerWakeSignal {
    notify: Notify,
}

impl SchedulerWakeSignal {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wake(&self) {
        self.notify.notify_one();
    }

    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}

#[derive(Debug)]
pub struct JobTriggers {
    sender: mpsc::UnboundedSender<ScheduledJobId>,
    wake: Arc<SchedulerWakeSignal>,
}

impl JobTriggers {
    pub fn trigger(&self, id: ScheduledJobId) {
        if let Err(_closed) = self.sender.send(id) {
            return;
        }
        self.wake.wake();
    }
}

#[derive(Debug)]
pub struct SchedulerEventSubscriber {
    wake: Arc<SchedulerWakeSignal>,
    triggers: Option<Arc<JobTriggers>>,
}

impl SchedulerEventSubscriber {
    #[must_use]
    pub const fn new(wake: Arc<SchedulerWakeSignal>) -> Self {
        Self {
            wake,
            triggers: None,
        }
    }

    #[must_use]
    pub const fn with_triggers(triggers: Arc<JobTriggers>, wake: Arc<SchedulerWakeSignal>) -> Self {
        Self {
            wake,
            triggers: Some(triggers),
        }
    }
}

impl EventSubscriber for SchedulerEventSubscriber {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        match event {
            DomainEvent::OrderSubmitted(_) => {
                if let Some(triggers) = &self.triggers {
                    triggers.trigger(ScheduledJobId::PortfolioSync);
                }
                self.wake.wake();
                Ok(())
            }
            DomainEvent::SettingsChanged
            | DomainEvent::AiBudgetChanged
            | DomainEvent::MarketSessionChanged(_)
            | DomainEvent::WatchlistChanged
            | DomainEvent::BrokerConnectionChanged => {
                self.wake.wake();
                Ok(())
            }
            DomainEvent::PortfolioSyncRecorded(_)
            | DomainEvent::TransactionsSyncRecorded(_)
            | DomainEvent::PriceUpdated(_)
            | DomainEvent::PriceMoved(_)
            | DomainEvent::DisclosureArrived(_)
            | DomainEvent::NewsActivityIncreased(_)
            | DomainEvent::ProposalCreated(_)
            | DomainEvent::ProposalExpired(_)
            | DomainEvent::ProposalApproved(_)
            | DomainEvent::ProposalRejected(_)
            | DomainEvent::ProposalSkipped(_)
            | DomainEvent::OrderOutcomeUnknown(_)
            | DomainEvent::PolicyChanged
            | DomainEvent::OrderSwitchChanged
            | DomainEvent::ThesisUpdated(_)
            | DomainEvent::SelfCheckChanged
            | DomainEvent::SignalRaised(_)
            | DomainEvent::SignalCleared(_)
            | DomainEvent::EstimateSubmitted(_)
            | DomainEvent::EstimateResolved(_)
            | DomainEvent::ValuationSnapshotTaken(_)
            | DomainEvent::OrderRejected(_)
            | DomainEvent::OrderOutcomeResolved(_)
            | DomainEvent::OrderFilled(_)
            | DomainEvent::OrderCancelled(_)
            | DomainEvent::OrderModified(_)
            | DomainEvent::CollectionFinished(_)
            | DomainEvent::NewsClusterGrew(_)
            | DomainEvent::AccountSelectionChanged
            | DomainEvent::StoredDataChanged
            | DomainEvent::FxRateUpdated(_)
            | DomainEvent::DisclosureTriggerRecorded(_)
            | DomainEvent::SourceStatusChanged(_) => Ok(()),
        }
    }
}

#[derive(Debug, Clone)]
struct JobRuntimeState {
    config: JobConfig,
    next_due: Option<Timestamp>,
    last_evaluated_at: Option<Timestamp>,
    last_succeeded_at: Option<Timestamp>,
    last_outcome: Option<JobRunOutcome>,
    recent_history: Vec<JobExecutionRecord>,
}

impl JobRuntimeState {
    fn record_execution(&mut self, record: JobExecutionRecord) {
        self.last_evaluated_at = Some(record.evaluated_at);
        if let JobRunOutcome::Executed(_) = record.outcome {
            self.last_succeeded_at = Some(record.evaluated_at);
        }
        self.last_outcome = Some(record.outcome.clone());
        self.recent_history.push(record);
        if self.recent_history.len() > MAX_HISTORY_PER_JOB {
            let excess = self
                .recent_history
                .len()
                .saturating_sub(MAX_HISTORY_PER_JOB);
            self.recent_history = self.recent_history.split_off_retained(excess);
        }
    }
}

trait RetainedTail<T> {
    fn split_off_retained(&self, skip: usize) -> Vec<T>;
}

impl<T: Clone> RetainedTail<T> for Vec<T> {
    fn split_off_retained(&self, skip: usize) -> Self {
        self.iter().skip(skip).cloned().collect()
    }
}

pub struct Scheduler<C: Clock, Q: AiJobQueue> {
    clock: Arc<C>,
    ai_queue: Arc<Q>,
    jobs: BTreeMap<ScheduledJobId, Box<dyn ScheduledJob>>,
    states: Mutex<BTreeMap<ScheduledJobId, JobRuntimeState>>,
    wake: Arc<SchedulerWakeSignal>,
    triggers: Arc<JobTriggers>,
    trigger_inbox: Mutex<mpsc::UnboundedReceiver<ScheduledJobId>>,
    calendars: Arc<dyn MarketCalendarSource>,
    records: Arc<dyn JobRecordSink>,
    startup: Timestamp,
}

impl<C: Clock, Q: AiJobQueue> Debug for Scheduler<C, Q> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler")
            .field("jobs", &self.jobs.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

impl<C: Clock, Q: AiJobQueue> Scheduler<C, Q> {
    pub fn new(
        clock: Arc<C>,
        ai_queue: Arc<Q>,
        registry: JobRegistry,
    ) -> Result<Self, ScheduleTimeError> {
        Self::with_market_calendar(clock, ai_queue, registry, Arc::new(NoMarketCalendar))
    }

    pub fn with_market_calendar(
        clock: Arc<C>,
        ai_queue: Arc<Q>,
        registry: JobRegistry,
        calendars: Arc<dyn MarketCalendarSource>,
    ) -> Result<Self, ScheduleTimeError> {
        Self::with_options(
            clock,
            ai_queue,
            registry,
            SchedulerOptions {
                calendars,
                records: Arc::new(DiscardJobRecords),
            },
        )
    }

    pub fn with_options(
        clock: Arc<C>,
        ai_queue: Arc<Q>,
        registry: JobRegistry,
        options: SchedulerOptions,
    ) -> Result<Self, ScheduleTimeError> {
        let SchedulerOptions { calendars, records } = options;
        let started_at = clock.now();
        let entries = registry.into_entries();
        let mut states = BTreeMap::new();
        for (id, job) in &entries {
            let config = job.default_config();
            let next_due = match job.startup_policy() {
                StartupPolicy::RunAtStartup => Some(started_at),
                StartupPolicy::WaitForSchedule => {
                    first_due(calendars.as_ref(), started_at, config.schedule)?
                }
            };
            states.insert(
                *id,
                JobRuntimeState {
                    config,
                    next_due,
                    last_evaluated_at: None,
                    last_succeeded_at: None,
                    last_outcome: None,
                    recent_history: Vec::new(),
                },
            );
        }
        let wake = Arc::new(SchedulerWakeSignal::new());
        let (sender, receiver) = mpsc::unbounded_channel();
        Ok(Self {
            clock,
            ai_queue,
            jobs: entries,
            states: Mutex::new(states),
            triggers: Arc::new(JobTriggers {
                sender,
                wake: Arc::clone(&wake),
            }),
            trigger_inbox: Mutex::new(receiver),
            wake,
            calendars,
            records,
            startup: started_at,
        })
    }

    pub async fn restore_last_runs(&self, last_runs: &[(ScheduledJobId, Timestamp)]) {
        let mut states = self.states.lock().await;
        for (id, last_scheduled_for) in last_runs {
            let Some(state) = states.get_mut(id) else {
                continue;
            };
            if state.config.missed_policy != MissedPolicy::CatchUpOnce {
                continue;
            }
            if state.next_due.is_some_and(|due| due <= self.startup) {
                continue;
            }
            let Some(missed) = latest_due_at_or_before(
                self.calendars.as_ref(),
                self.startup,
                state.config.schedule,
            ) else {
                continue;
            };
            if *last_scheduled_for < missed {
                state.next_due = Some(missed);
            }
        }
        self.wake.wake();
    }

    #[must_use]
    pub fn triggers(&self) -> Arc<JobTriggers> {
        Arc::clone(&self.triggers)
    }

    #[must_use]
    pub fn wake_signal(&self) -> Arc<SchedulerWakeSignal> {
        Arc::clone(&self.wake)
    }

    #[must_use]
    pub fn event_subscriber(&self) -> SchedulerEventSubscriber {
        SchedulerEventSubscriber::with_triggers(Arc::clone(&self.triggers), Arc::clone(&self.wake))
    }

    pub async fn apply_override(
        &self,
        override_entry: JobScheduleOverride,
    ) -> Result<Option<JobStatusSnapshot>, ScheduleTimeError> {
        let now = self.clock.now();
        let mut states = self.states.lock().await;
        let Some(state) = states.get_mut(&override_entry.id) else {
            return Ok(None);
        };
        state.config.enabled = override_entry.enabled;
        state.config.schedule = override_entry.schedule;
        state.next_due = first_due(self.calendars.as_ref(), now, state.config.schedule)?;
        let snapshot = build_snapshot(override_entry.id, state);
        self.wake.wake();
        Ok(Some(snapshot))
    }

    pub async fn status_snapshots(&self) -> Vec<JobStatusSnapshot> {
        let states = self.states.lock().await;
        states
            .iter()
            .map(|(id, state)| build_snapshot(*id, state))
            .collect()
    }

    pub async fn tick_once(&self) -> TickReport {
        let now = self.clock.now();
        let due_jobs = self.collect_due_jobs(now).await;
        let mut records = Vec::new();
        for (id, config, scheduled_for) in due_jobs {
            let record = self.process_due_job(id, config, scheduled_for, now).await;
            records.push(record);
        }
        TickReport {
            evaluated_at: now,
            records,
        }
    }

    pub async fn run_loop(
        self: Arc<Self>,
        tick_interval: Duration,
        mut shutdown: watch::Receiver<LoopControl>,
    ) {
        let effective_interval = if tick_interval.is_zero() {
            Duration::from_secs(DEFAULT_TICK_INTERVAL_SECONDS)
        } else {
            tick_interval
        };
        loop {
            if *shutdown.borrow() == LoopControl::Stop {
                break;
            }
            let _report = self.tick_once().await;
            let wait = wait_wake_or_shutdown(&self.wake, &mut shutdown);
            match tokio::time::timeout(effective_interval, wait).await {
                Ok(WaitOutcome::Stopped) => break,
                Ok(WaitOutcome::Woken) | Err(_) => {}
            }
        }
    }

    async fn drain_triggers(&self) -> BTreeSet<ScheduledJobId> {
        let mut inbox = self.trigger_inbox.lock().await;
        let mut triggered = BTreeSet::new();
        while let Ok(id) = inbox.try_recv() {
            triggered.insert(id);
        }
        triggered
    }

    async fn collect_due_jobs(
        &self,
        now: Timestamp,
    ) -> Vec<(ScheduledJobId, JobConfig, Timestamp)> {
        let triggered = self.drain_triggers().await;
        let states = self.states.lock().await;
        let mut due = Vec::new();
        for (id, state) in &*states {
            if state.config.enabled == JobEnablement::Disabled {
                continue;
            }
            let scheduled = state.next_due.filter(|scheduled_for| *scheduled_for <= now);
            match (scheduled, triggered.contains(id)) {
                (Some(scheduled_for), _) => due.push((*id, state.config, scheduled_for)),
                (None, true) => due.push((*id, state.config, now)),
                (None, false) => {}
            }
        }
        due
    }

    async fn process_due_job(
        &self,
        id: ScheduledJobId,
        config: JobConfig,
        scheduled_for: Timestamp,
        now: Timestamp,
    ) -> JobExecutionRecord {
        if config.missed_policy == MissedPolicy::Skip && !is_within_skip_grace(scheduled_for, now) {
            return self
                .complete_cycle(
                    config,
                    JobExecutionRecord {
                        id,
                        scheduled_for,
                        evaluated_at: now,
                        outcome: JobRunOutcome::SkippedMissedWindow,
                    },
                )
                .await;
        }
        if config.kind == JobKind::AiBackground
            && let Some(deferred) = self.check_ai_admission(id, scheduled_for, now).await
        {
            return deferred;
        }
        let outcome = self.execute_registered_job(id, now).await;
        if config.kind == JobKind::AiBackground {
            let released = self.ai_queue.release_background_slot(id, now).await;
            if let Err(_error) = released {}
        }
        self.complete_cycle(
            config,
            JobExecutionRecord {
                id,
                scheduled_for,
                evaluated_at: now,
                outcome,
            },
        )
        .await
    }

    async fn check_ai_admission(
        &self,
        id: ScheduledJobId,
        scheduled_for: Timestamp,
        now: Timestamp,
    ) -> Option<JobExecutionRecord> {
        let resumes_at = match self.ai_queue.request_background_slot(id, now).await {
            Ok(AiQueueAdmission::Admitted) => return None,
            Ok(AiQueueAdmission::PausedForChat) | Err(_) => None,
            Ok(
                AiQueueAdmission::BudgetReached { resumes_at }
                | AiQueueAdmission::RateLimited { resumes_at },
            ) => resumes_at,
        };
        let record = JobExecutionRecord {
            id,
            scheduled_for,
            evaluated_at: now,
            outcome: JobRunOutcome::DeferredByAiQueue { resumes_at },
        };
        self.record_deferred(id, resumes_at, record.clone()).await;
        self.records.record(&record);
        Some(record)
    }

    async fn complete_cycle(
        &self,
        config: JobConfig,
        record: JobExecutionRecord,
    ) -> JobExecutionRecord {
        self.finish_job_cycle(record.id, config, record.evaluated_at, record.clone())
            .await;
        self.records.record(&record);
        record
    }

    async fn execute_registered_job(&self, id: ScheduledJobId, now: Timestamp) -> JobRunOutcome {
        let Some(job) = self.jobs.get(&id) else {
            return JobRunOutcome::Failed(JobError::ExecutionFailed(String::from("job-not-found")));
        };
        match job.run(now).await {
            Ok(detail) => JobRunOutcome::Executed(detail),
            Err(error) => JobRunOutcome::Failed(error),
        }
    }

    async fn record_deferred(
        &self,
        id: ScheduledJobId,
        resumes_at: Option<Timestamp>,
        record: JobExecutionRecord,
    ) {
        let mut states = self.states.lock().await;
        if let Some(state) = states.get_mut(&id) {
            if let Some(resume) = resumes_at {
                state.next_due = Some(resume);
            }
            state.record_execution(record);
        }
    }

    async fn finish_job_cycle(
        &self,
        id: ScheduledJobId,
        config: JobConfig,
        now: Timestamp,
        record: JobExecutionRecord,
    ) {
        let mut states = self.states.lock().await;
        if let Some(state) = states.get_mut(&id) {
            if let Ok(next) = following_due(self.calendars.as_ref(), now, config.schedule) {
                state.next_due = next;
            } else {
                state.next_due = None;
            }
            state.record_execution(record);
        }
    }
}

pub trait JobRecordSink: Send + Sync {
    fn record(&self, record: &JobExecutionRecord);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DiscardJobRecords;

impl JobRecordSink for DiscardJobRecords {
    fn record(&self, _record: &JobExecutionRecord) {}
}

pub struct SchedulerOptions {
    pub calendars: Arc<dyn MarketCalendarSource>,
    pub records: Arc<dyn JobRecordSink>,
}

impl Debug for SchedulerOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchedulerOptions").finish_non_exhaustive()
    }
}

impl Default for SchedulerOptions {
    fn default() -> Self {
        Self {
            calendars: Arc::new(NoMarketCalendar),
            records: Arc::new(DiscardJobRecords),
        }
    }
}

const CATCH_UP_LOOKBACK_SECONDS: u64 = 8 * 86_400;
const MAX_CATCH_UP_STEPS: usize = 32;

fn latest_due_at_or_before(
    calendars: &dyn MarketCalendarSource,
    at: Timestamp,
    rule: ScheduleRule,
) -> Option<Timestamp> {
    match rule {
        ScheduleRule::Interval(_) | ScheduleRule::EventTriggered => return None,
        ScheduleRule::DailyKst(_)
        | ScheduleRule::WeeklyKst { .. }
        | ScheduleRule::AfterMarketClose { .. }
        | ScheduleRule::BeforeMarketOpen { .. } => {}
    }
    let lookback = domain::TimeSpan::try_new(
        domain::SpanSeconds::new(CATCH_UP_LOOKBACK_SECONDS),
        domain::Nanosecond::new(0),
    )
    .ok()?;
    let mut cursor = first_due(calendars, at.checked_sub(lookback).ok()?, rule).ok()??;
    if cursor > at {
        return None;
    }
    for _ in 0..MAX_CATCH_UP_STEPS {
        match following_due(calendars, cursor, rule) {
            Ok(Some(next)) if next <= at => cursor = next,
            Ok(_) | Err(_) => break,
        }
    }
    Some(cursor)
}

fn first_due(
    calendars: &dyn MarketCalendarSource,
    at: Timestamp,
    rule: ScheduleRule,
) -> Result<Option<Timestamp>, ScheduleTimeError> {
    match market_due(calendars, rule, at, DueBound::AtOrAfter) {
        Some(due) => Ok(Some(due)),
        None => initial_due(at, rule),
    }
}

fn following_due(
    calendars: &dyn MarketCalendarSource,
    after: Timestamp,
    rule: ScheduleRule,
) -> Result<Option<Timestamp>, ScheduleTimeError> {
    match market_due(calendars, rule, after, DueBound::StrictlyAfter) {
        Some(due) => Ok(Some(due)),
        None => next_due_after(after, rule),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WaitOutcome {
    Woken,
    Stopped,
}

async fn wait_wake_or_shutdown(
    wake: &SchedulerWakeSignal,
    shutdown: &mut watch::Receiver<LoopControl>,
) -> WaitOutcome {
    let mut notified = std::pin::pin!(wake.notified());
    let mut changed = std::pin::pin!(shutdown.changed());
    std::future::poll_fn(|cx| {
        if notified.as_mut().poll(cx).is_ready() {
            return std::task::Poll::Ready(WaitOutcome::Woken);
        }
        match changed.as_mut().poll(cx) {
            std::task::Poll::Ready(Ok(())) => std::task::Poll::Ready(WaitOutcome::Woken),
            std::task::Poll::Ready(Err(_)) => std::task::Poll::Ready(WaitOutcome::Stopped),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    })
    .await
}

fn build_snapshot(id: ScheduledJobId, state: &JobRuntimeState) -> JobStatusSnapshot {
    JobStatusSnapshot {
        id,
        config: state.config,
        next_due: state.next_due,
        last_evaluated_at: state.last_evaluated_at,
        last_succeeded_at: state.last_succeeded_at,
        last_outcome: state.last_outcome.clone(),
        recent_history: state.recent_history.clone(),
    }
}
