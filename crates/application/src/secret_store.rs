use crate::{SystemPortError, WriteCompleted};
use secrecy::{ExposeSecret, SecretSlice};
use std::future::Future;

pub struct SecretValue(SecretSlice<u8>);

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretValue(<redacted>)")
    }
}

impl SecretValue {
    #[must_use]
    pub const fn new(value: SecretSlice<u8>) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_exposed_secret(&self) -> &[u8] {
        self.0.expose_secret()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretKey {
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
