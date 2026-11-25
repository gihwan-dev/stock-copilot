use application::{SecretKey, SecretStore, SecretValue, SystemPortError};
use std::sync::Arc;
use store_sqlite::{Database, DatabasePath};

pub struct DatabaseFactory(
    pub Box<dyn Fn(DatabasePath, SecretValue) -> Result<Database, SystemPortError> + Send + Sync>,
);
impl std::fmt::Debug for DatabaseFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DatabaseFactory")
    }
}

pub struct DatabaseKeyProvision(
    pub Box<dyn Fn() -> Result<SecretValue, SystemPortError> + Send + Sync>,
);
impl std::fmt::Debug for DatabaseKeyProvision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DatabaseKeyProvision")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyProvisioning {
    Existing,
    Created,
}

pub async fn provision_database_key<S: SecretStore>(
    secrets: &S,
    provision: &DatabaseKeyProvision,
    path: &DatabasePath,
) -> Result<KeyProvisioning, SystemPortError> {
    if let Some(_existing) = secrets.load(SecretKey::DatabaseEncryptionKey).await? {
        return Ok(KeyProvisioning::Existing);
    }
    match path.as_path().try_exists() {
        Ok(false) => {}
        Ok(true) => {
            return Err(SystemPortError::new(
                application::SystemPort::SecretStore,
                application::SystemFailure::SecretIntegrity,
            ));
        }
        Err(_error) => {
            return Err(SystemPortError::new(
                application::SystemPort::Repository,
                application::SystemFailure::Unavailable(application::RetryDelay::Unknown),
            ));
        }
    }
    let key = (provision.0)()?;
    secrets.store(SecretKey::DatabaseEncryptionKey, key).await?;
    Ok(KeyProvisioning::Created)
}

pub async fn open_provisioned_database<S: SecretStore>(
    secrets: &S,
    provision: &DatabaseKeyProvision,
    factory: Arc<DatabaseFactory>,
    path: DatabasePath,
) -> DatabaseState {
    match provision_database_key(secrets, provision, &path).await {
        Ok(KeyProvisioning::Existing | KeyProvisioning::Created) => {
            open_database(secrets, factory, move || Ok(path)).await
        }
        Err(error) => DatabaseState::Failed(error),
    }
}

#[derive(Debug)]
pub enum DatabaseState {
    NotConfigured,
    Ready(Database),
    Failed(SystemPortError),
}

pub async fn open_database<
    S: SecretStore,
    Resolve: FnOnce() -> Result<DatabasePath, SystemPortError>,
>(
    secrets: &S,
    factory: Arc<DatabaseFactory>,
    resolve: Resolve,
) -> DatabaseState {
    match secrets.load(SecretKey::DatabaseEncryptionKey).await {
        Ok(None) => DatabaseState::NotConfigured,
        Err(error) => DatabaseState::Failed(error),
        Ok(Some(key)) => {
            let path = match resolve() {
                Ok(path) => path,
                Err(error) => return DatabaseState::Failed(error),
            };
            match tauri::async_runtime::spawn_blocking(move || (factory.0)(path, key)).await {
                Ok(Ok(database)) => DatabaseState::Ready(database),
                Ok(Err(error)) => DatabaseState::Failed(error),
                Err(_error) => DatabaseState::Failed(SystemPortError::new(
                    application::SystemPort::Repository,
                    application::SystemFailure::RepositoryIntegrity,
                )),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseStatus {
    NotConfigured,
    Ready,
    Failed(SystemPortError),
}
impl DatabaseState {
    #[must_use]
    pub const fn status(&self) -> DatabaseStatus {
        match self {
            Self::NotConfigured => DatabaseStatus::NotConfigured,
            Self::Ready(_) => DatabaseStatus::Ready,
            Self::Failed(error) => DatabaseStatus::Failed(*error),
        }
    }
}

pub fn prepare_directory(path: &DatabasePath) -> Result<(), SystemPortError> {
    if let Some(parent) = path.as_path().parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            let failure = if error.kind() == std::io::ErrorKind::PermissionDenied {
                application::SystemFailure::PermissionDenied
            } else {
                application::SystemFailure::Unavailable(application::RetryDelay::Unknown)
            };
            SystemPortError::new(application::SystemPort::Repository, failure)
        })?;
    }
    Ok(())
}
