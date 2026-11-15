use crate::SystemPortError;
use domain::Timestamp;
use std::future::Future;

pub trait Clock: Send + Sync {
    fn now(&self) -> impl Future<Output = Result<Timestamp, SystemPortError>> + Send;
}
