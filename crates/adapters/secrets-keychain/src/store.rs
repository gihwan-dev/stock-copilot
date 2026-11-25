use crate::{KeychainBackend, KeychainFailure, KeychainProfile};
use application::{
    RetryDelay, SecretKey, SecretStore, SecretValue, SystemFailure, SystemPort, SystemPortError,
    WriteCompleted,
};
use std::future::Future;

#[derive(Debug)]
pub struct KeychainSecretStore<B> {
    profile: KeychainProfile,
    backend: B,
}

impl<B: KeychainBackend> KeychainSecretStore<B> {
    #[must_use]
    pub const fn new(profile: KeychainProfile, backend: B) -> Self {
        Self { profile, backend }
    }

    #[must_use]
    pub const fn profile(&self) -> &KeychainProfile {
        &self.profile
    }

    #[must_use]
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    fn load_now(&self, key: SecretKey) -> Result<Option<SecretValue>, SystemPortError> {
        self.backend
            .read(&self.profile.item(key))
            .map(|value| value.map(SecretValue::new))
            .map_err(port_error)
    }

    fn store_now(
        &self,
        key: SecretKey,
        value: &SecretValue,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.backend
            .write(&self.profile.item(key), value.as_exposed_secret())
            .map(|()| WriteCompleted)
            .map_err(port_error)
    }

    fn remove_now(&self, key: SecretKey) -> Result<WriteCompleted, SystemPortError> {
        self.backend
            .delete(&self.profile.item(key))
            .map(|()| WriteCompleted)
            .map_err(port_error)
    }
}

const fn port_error(failure: KeychainFailure) -> SystemPortError {
    let mapped = match failure {
        KeychainFailure::MissingEntitlement | KeychainFailure::Denied => {
            SystemFailure::PermissionDenied
        }
        KeychainFailure::Locked | KeychainFailure::Unavailable => {
            SystemFailure::Unavailable(RetryDelay::Unknown)
        }
        KeychainFailure::Malformed => SystemFailure::SecretIntegrity,
    };
    SystemPortError::new(SystemPort::SecretStore, mapped)
}

impl<B: KeychainBackend> SecretStore for KeychainSecretStore<B> {
    fn load(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
        std::future::ready(self.load_now(key))
    }

    fn store(
        &self,
        key: SecretKey,
        value: SecretValue,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(self.store_now(key, &value))
    }

    fn remove(
        &self,
        key: SecretKey,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(self.remove_now(key))
    }
}
