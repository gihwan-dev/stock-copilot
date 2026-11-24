use crate::types::{JobConfig, JobError, JobRunDetail, ScheduledJobId, StartupPolicy};
use domain::Timestamp;
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::future::Future;
use std::pin::Pin;

pub type JobFuture<'a> = Pin<Box<dyn Future<Output = Result<JobRunDetail, JobError>> + Send + 'a>>;

pub trait ScheduledJob: Send + Sync {
    fn id(&self) -> ScheduledJobId;
    fn default_config(&self) -> JobConfig;
    fn run(&self, now: Timestamp) -> JobFuture<'_>;
    fn startup_policy(&self) -> StartupPolicy {
        StartupPolicy::WaitForSchedule
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateJob(ScheduledJobId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegisteredJobMeta {
    pub id: ScheduledJobId,
    pub default_config: JobConfig,
    pub startup_policy: StartupPolicy,
}

#[derive(Default)]
pub struct JobRegistry {
    jobs: BTreeMap<ScheduledJobId, Box<dyn ScheduledJob>>,
}

impl Debug for JobRegistry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobRegistry")
            .field("job_ids", &self.jobs.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl JobRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<J: ScheduledJob + 'static>(&mut self, job: J) -> Result<(), RegistryError> {
        let id = job.id();
        if self.jobs.contains_key(&id) {
            return Err(RegistryError::DuplicateJob(id));
        }
        self.jobs.insert(id, Box::new(job));
        Ok(())
    }

    #[must_use]
    pub fn list(&self) -> Vec<RegisteredJobMeta> {
        self.jobs
            .values()
            .map(|job| RegisteredJobMeta {
                id: job.id(),
                default_config: job.default_config(),
                startup_policy: job.startup_policy(),
            })
            .collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }

    pub(crate) fn into_entries(self) -> BTreeMap<ScheduledJobId, Box<dyn ScheduledJob>> {
        self.jobs
    }
}
