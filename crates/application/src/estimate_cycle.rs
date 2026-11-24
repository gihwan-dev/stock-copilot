use std::future::Future;

use domain::{InstrumentId, Timestamp};

use crate::{Clock, SystemPortError, TrackedUniverse, TrackingTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CycleAdmission {
    Admitted,
    Paused { resumes_at: Option<Timestamp> },
}

pub trait BackgroundBudget: Send + Sync {
    fn admit(
        &self,
        now: Timestamp,
    ) -> impl Future<Output = Result<CycleAdmission, SystemPortError>> + Send;
    fn release(&self, now: Timestamp) -> impl Future<Output = Result<(), SystemPortError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleEstimateOutcome {
    Submitted,
    NotSubmitted,
    Failed,
}

pub trait SingleEstimateRunner: Send + Sync {
    fn estimate(
        &self,
        target: &TrackingTarget,
    ) -> impl Future<Output = Result<SingleEstimateOutcome, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CycleProgress {
    Finished,
    Paused {
        resume_after: Option<InstrumentId>,
        resumes_at: Option<Timestamp>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateCycleReport {
    pub attempted: Vec<(InstrumentId, SingleEstimateOutcome)>,
    pub progress: CycleProgress,
}

#[derive(Debug)]
pub struct RunEstimateCycle<'a, Runner, Budget, TimeSource> {
    runner: &'a Runner,
    budget: &'a Budget,
    clock: &'a TimeSource,
}

impl<'a, Runner, Budget, TimeSource> RunEstimateCycle<'a, Runner, Budget, TimeSource>
where
    Runner: SingleEstimateRunner,
    Budget: BackgroundBudget,
    TimeSource: Clock,
{
    #[must_use]
    pub const fn new(runner: &'a Runner, budget: &'a Budget, clock: &'a TimeSource) -> Self {
        Self {
            runner,
            budget,
            clock,
        }
    }

    pub async fn execute(
        &self,
        universe: &TrackedUniverse,
        resume_after: Option<&InstrumentId>,
    ) -> Result<EstimateCycleReport, SystemPortError> {
        let targets = universe.monitoring_targets();
        let start = resume_after
            .and_then(|last| {
                targets
                    .iter()
                    .position(|target| target.instrument() == last)
            })
            .map_or(0_usize, |index| index.saturating_add(1_usize));
        let mut attempted = Vec::new();
        let mut last: Option<InstrumentId> = resume_after.cloned();
        for target in targets.iter().skip(start) {
            match self.budget.admit(self.clock.now()).await? {
                CycleAdmission::Paused { resumes_at } => {
                    return Ok(EstimateCycleReport {
                        attempted,
                        progress: CycleProgress::Paused {
                            resume_after: last,
                            resumes_at,
                        },
                    });
                }
                CycleAdmission::Admitted => {}
            }
            let result = self.runner.estimate(target).await;
            self.budget.release(self.clock.now()).await?;
            let outcome = match result {
                Ok(outcome) => outcome,
                Err(_error) => SingleEstimateOutcome::Failed,
            };
            attempted.push((target.instrument().clone(), outcome));
            last = Some(target.instrument().clone());
        }
        Ok(EstimateCycleReport {
            attempted,
            progress: CycleProgress::Finished,
        })
    }
}
