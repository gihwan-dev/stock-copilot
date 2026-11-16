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
