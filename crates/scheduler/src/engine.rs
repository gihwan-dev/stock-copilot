use crate::ai_queue::{AiJobQueue, AiQueueAdmission};
use crate::registry::{JobRegistry, ScheduledJob};
use crate::schedule_time::{ScheduleTimeError, initial_due, is_within_skip_grace, next_due_after};
use crate::types::{
    JobConfig, JobEnablement, JobError, JobExecutionRecord, JobKind, JobRunOutcome,
    JobScheduleOverride, JobStatusSnapshot, LoopControl, MissedPolicy, ScheduledJobId, TickReport,
};
use application::{Clock, EventSubscriber, SubscriberFailure};
use domain::{DomainEvent, Timestamp};
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, Notify, watch};

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
pub struct SchedulerEventSubscriber {
    wake: Arc<SchedulerWakeSignal>,
}

impl SchedulerEventSubscriber {
    #[must_use]
    pub const fn new(wake: Arc<SchedulerWakeSignal>) -> Self {
        Self { wake }
    }
}

impl EventSubscriber for SchedulerEventSubscriber {
    fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
        match event {
            DomainEvent::SettingsChanged
            | DomainEvent::AiBudgetChanged
            | DomainEvent::MarketSessionChanged(_)
            | DomainEvent::WatchlistChanged
            | DomainEvent::OrderSubmitted(_)
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
        let started_at = clock.now();
        let entries = registry.into_entries();
        let mut states = BTreeMap::new();
        for (id, job) in &entries {
            let config = job.default_config();
            let next_due = initial_due(started_at, config.schedule)?;
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
        Ok(Self {
            clock,
            ai_queue,
            jobs: entries,
            states: Mutex::new(states),
            wake: Arc::new(SchedulerWakeSignal::new()),
        })
    }

    #[must_use]
    pub fn wake_signal(&self) -> Arc<SchedulerWakeSignal> {
        Arc::clone(&self.wake)
    }

    #[must_use]
    pub fn event_subscriber(&self) -> SchedulerEventSubscriber {
        SchedulerEventSubscriber::new(Arc::clone(&self.wake))
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
        state.next_due = initial_due(now, state.config.schedule)?;
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

    async fn collect_due_jobs(
        &self,
        now: Timestamp,
    ) -> Vec<(ScheduledJobId, JobConfig, Timestamp)> {
        let states = self.states.lock().await;
        let mut due = Vec::new();
        for (id, state) in &*states {
            if state.config.enabled == JobEnablement::Disabled {
                continue;
            }
            if let Some(scheduled_for) = state.next_due
                && scheduled_for <= now
            {
                due.push((*id, state.config, scheduled_for));
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
        Some(record)
    }

    async fn complete_cycle(
        &self,
        config: JobConfig,
        record: JobExecutionRecord,
    ) -> JobExecutionRecord {
        self.finish_job_cycle(record.id, config, record.evaluated_at, record.clone())
            .await;
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
            if let Ok(next) = next_due_after(now, config.schedule) {
                state.next_due = next;
            } else {
                state.next_due = None;
            }
            state.record_execution(record);
        }
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
