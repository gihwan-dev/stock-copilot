use crate::{
    BackgroundBudget, Clock, CycleAdmission, SystemPortError, TrackedUniverse, TrackingTarget,
};
use domain::{DeepResearchWindow, InstrumentId, TimeError, Timestamp, deep_research_window};
use std::future::Future;

use super::{DeepResearchError, DeepResearchRun};

pub trait DeepResearchJob: Send + Sync {
    fn research(
        &self,
        target: &TrackingTarget,
    ) -> impl Future<Output = Result<DeepResearchRun, DeepResearchError>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeepResearchPause {
    OutsideNight,
    Budget,
    Chat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepResearchTargetOutcome {
    Finished,
    Failed(DeepResearchError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepResearchCycleProgress {
    Finished,
    Paused {
        next: InstrumentId,
        resumes_at: Option<Timestamp>,
        reason: DeepResearchPause,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeepResearchCycleReport {
    pub attempted: Vec<(InstrumentId, DeepResearchTargetOutcome)>,
    pub progress: DeepResearchCycleProgress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeepResearchCycleError {
    Budget(SystemPortError),
    Time(TimeError),
}

#[derive(Debug)]
pub struct RunDeepResearchCycle<'a, Job, Budget, Time> {
    pub job: &'a Job,
    pub budget: &'a Budget,
    pub clock: &'a Time,
}

impl<Job: DeepResearchJob, Budget: BackgroundBudget, Time: Clock>
    RunDeepResearchCycle<'_, Job, Budget, Time>
{
    async fn pause(
        &self,
    ) -> Result<Option<(Option<Timestamp>, DeepResearchPause)>, DeepResearchCycleError> {
        let now = self.clock.now();
        if let DeepResearchWindow::Closed { opens_at } =
            deep_research_window(now).map_err(DeepResearchCycleError::Time)?
        {
            return Ok(Some((Some(opens_at), DeepResearchPause::OutsideNight)));
        }
        match self
            .budget
            .admit(now)
            .await
            .map_err(DeepResearchCycleError::Budget)?
        {
            CycleAdmission::Admitted => Ok(None),
            CycleAdmission::Paused { resumes_at } => {
                Ok(Some((resumes_at, DeepResearchPause::Budget)))
            }
        }
    }

    pub async fn execute(
        &self,
        universe: &TrackedUniverse,
        start_at: Option<&InstrumentId>,
    ) -> Result<DeepResearchCycleReport, DeepResearchCycleError> {
        let targets = universe.monitoring_targets();
        let begins =
            start_at.filter(|first| targets.iter().any(|target| target.instrument() == *first));
        let mut attempted = Vec::new();
        for target in targets
            .iter()
            .skip_while(|target| begins.is_some_and(|first| target.instrument() != first))
        {
            if let Some((resumes_at, reason)) = self.pause().await? {
                return Ok(DeepResearchCycleReport {
                    attempted,
                    progress: DeepResearchCycleProgress::Paused {
                        next: target.instrument().clone(),
                        resumes_at,
                        reason,
                    },
                });
            }
            let result = self.job.research(target).await;
            self.budget
                .release(self.clock.now())
                .await
                .map_err(DeepResearchCycleError::Budget)?;
            let outcome = match result {
                Ok(DeepResearchRun::Finished(_outcome)) => DeepResearchTargetOutcome::Finished,
                Ok(DeepResearchRun::Paused(_stage)) => {
                    return Ok(DeepResearchCycleReport {
                        attempted,
                        progress: DeepResearchCycleProgress::Paused {
                            next: target.instrument().clone(),
                            resumes_at: None,
                            reason: DeepResearchPause::Chat,
                        },
                    });
                }
                Err(error) => DeepResearchTargetOutcome::Failed(error),
            };
            attempted.push((target.instrument().clone(), outcome));
        }
        Ok(DeepResearchCycleReport {
            attempted,
            progress: DeepResearchCycleProgress::Finished,
        })
    }
}
