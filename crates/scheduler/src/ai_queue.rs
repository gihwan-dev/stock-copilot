use crate::ScheduledJobId;
use domain::Timestamp;
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiQueueAdmission {
    Admitted,
    PausedForChat,
    BudgetReached { resumes_at: Option<Timestamp> },
    RateLimited { resumes_at: Option<Timestamp> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiQueueError {
    Unavailable,
}

pub trait AiJobQueue: Send + Sync {
    fn request_background_slot(
        &self,
        job: ScheduledJobId,
        now: Timestamp,
    ) -> impl Future<Output = Result<AiQueueAdmission, AiQueueError>> + Send;

    fn release_background_slot(
        &self,
        job: ScheduledJobId,
        now: Timestamp,
    ) -> impl Future<Output = Result<(), AiQueueError>> + Send;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DisabledAiJobQueue;

impl AiJobQueue for DisabledAiJobQueue {
    fn request_background_slot(
        &self,
        _job: ScheduledJobId,
        _now: Timestamp,
    ) -> impl Future<Output = Result<AiQueueAdmission, AiQueueError>> + Send {
        std::future::ready(Ok(AiQueueAdmission::BudgetReached { resumes_at: None }))
    }

    fn release_background_slot(
        &self,
        _job: ScheduledJobId,
        _now: Timestamp,
    ) -> impl Future<Output = Result<(), AiQueueError>> + Send {
        std::future::ready(Ok(()))
    }
}
