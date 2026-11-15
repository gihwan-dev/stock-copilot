use crate::{SecretValue, SystemPortError, WriteCompleted};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKey {
    TossClientId,
    TossClientSecret,
    TossAccessToken,
    DatabaseEncryptionKey,
    AuditHmacKey,
    DartApiKey,
    FredApiKey,
}

pub trait SecretStore: Send + Sync {
    fn load(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send;
    fn store(
        &self,
        key: SecretKey,
        value: SecretValue,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn remove(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}
