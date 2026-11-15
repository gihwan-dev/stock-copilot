use crate::{SystemPortError, WriteCompleted};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notification {
    NewSignal,
    UrgentDisclosure,
    BriefingFinished,
}

pub trait Notifier: Send + Sync {
    fn notify(
        &self,
        notification: Notification,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}
