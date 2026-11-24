use crate::registry::{JobFuture, ScheduledJob};
use crate::types::{
    JobConfig, JobEnablement, JobError, JobKind, JobRunDetail, KstTimeOfDay, MissedPolicy,
    ScheduleRule, ScheduledJobId,
};
use application::{
    AiProvider, BriefingContextSource, BriefingOutcome, BriefingRepository, Clock, EventBus,
    RunBriefing,
};
use domain::{BriefingKind, KnownMarket, Nanosecond, SpanSeconds, TimeSpan, Timestamp};
use std::future::Future;
use std::sync::Arc;

pub const PRE_US_MARKET_LEAD_MINUTES: u64 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BriefingRunResult {
    Completed,
    Failed,
}

pub trait BriefingPort: Send + Sync {
    fn run(
        &self,
        kind: BriefingKind,
        now: Timestamp,
    ) -> impl Future<Output = Result<BriefingRunResult, JobError>> + Send;
}

fn ai_job(schedule: ScheduleRule) -> JobConfig {
    JobConfig {
        kind: JobKind::AiBackground,
        schedule,
        missed_policy: MissedPolicy::Skip,
        enabled: JobEnablement::Enabled,
    }
}

fn daily(hour: u8, minute: u8) -> ScheduleRule {
    match KstTimeOfDay::try_new(hour, minute) {
        Ok(time) => ScheduleRule::DailyKst(time),
        Err(_) => ScheduleRule::EventTriggered,
    }
}

fn pre_us_rule() -> ScheduleRule {
    let lead_span = TimeSpan::try_new(
        SpanSeconds::new(PRE_US_MARKET_LEAD_MINUTES.saturating_mul(60)),
        Nanosecond::new(0),
    );
    match (lead_span, KstTimeOfDay::try_new(22, 50)) {
        (Ok(lead), Ok(fallback)) => ScheduleRule::BeforeMarketOpen {
            market: KnownMarket::Us,
            lead,
            fallback,
        },
        (Err(_), _) | (_, Err(_)) => ScheduleRule::EventTriggered,
    }
}

#[derive(Debug)]
pub struct BriefingJob<P> {
    kind: BriefingKind,
    port: Arc<P>,
}

impl<P: BriefingPort> BriefingJob<P> {
    #[must_use]
    pub const fn morning(port: Arc<P>) -> Self {
        Self {
            kind: BriefingKind::Morning,
            port,
        }
    }

    #[must_use]
    pub const fn closing_review(port: Arc<P>) -> Self {
        Self {
            kind: BriefingKind::ClosingReview,
            port,
        }
    }

    #[must_use]
    pub const fn pre_us_market(port: Arc<P>) -> Self {
        Self {
            kind: BriefingKind::PreUsMarket,
            port,
        }
    }
}

impl<P: BriefingPort> ScheduledJob for BriefingJob<P> {
    fn id(&self) -> ScheduledJobId {
        match self.kind {
            BriefingKind::Morning => ScheduledJobId::MorningBriefing,
            BriefingKind::ClosingReview => ScheduledJobId::ClosingReview,
            BriefingKind::PreUsMarket => ScheduledJobId::PreUsMarketCheck,
        }
    }

    fn default_config(&self) -> JobConfig {
        match self.kind {
            BriefingKind::Morning => ai_job(daily(8, 20)),
            BriefingKind::ClosingReview => ai_job(daily(15, 50)),
            BriefingKind::PreUsMarket => ai_job(pre_us_rule()),
        }
    }

    fn run(&self, now: Timestamp) -> JobFuture<'_> {
        Box::pin(async move {
            match self.port.run(self.kind, now).await? {
                BriefingRunResult::Completed => Ok(JobRunDetail::new("briefing_finished", 1)),
                BriefingRunResult::Failed => {
                    Err(JobError::ExecutionFailed(String::from("briefing_failed")))
                }
            }
        })
    }
}

#[derive(Debug)]
pub struct RunBriefingRunner<A, R, X, C> {
    ai: Arc<A>,
    repository: Arc<R>,
    context: Arc<X>,
    clock: Arc<C>,
    events: EventBus,
}

impl<A, R, X, C> RunBriefingRunner<A, R, X, C>
where
    A: AiProvider,
    R: BriefingRepository,
    X: BriefingContextSource,
    C: Clock,
{
    #[must_use]
    pub const fn new(
        ai: Arc<A>,
        repository: Arc<R>,
        context: Arc<X>,
        clock: Arc<C>,
        events: EventBus,
    ) -> Self {
        Self {
            ai,
            repository,
            context,
            clock,
            events,
        }
    }
}

impl<A, R, X, C> BriefingPort for RunBriefingRunner<A, R, X, C>
where
    A: AiProvider,
    R: BriefingRepository,
    X: BriefingContextSource,
    C: Clock,
{
    async fn run(
        &self,
        kind: BriefingKind,
        _now: Timestamp,
    ) -> Result<BriefingRunResult, JobError> {
        let report = RunBriefing {
            ai: self.ai.as_ref(),
            repository: self.repository.as_ref(),
            context: self.context.as_ref(),
            clock: self.clock.as_ref(),
            events: &self.events,
        }
        .execute(kind)
        .await
        .map_err(|error| JobError::StorageFailure(format!("{error:?}")))?;
        match report.record.outcome {
            BriefingOutcome::Completed { .. } => Ok(BriefingRunResult::Completed),
            BriefingOutcome::Failed(_) => Ok(BriefingRunResult::Failed),
        }
    }
}
