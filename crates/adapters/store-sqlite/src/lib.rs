mod connection;
mod failure;
mod migration;
mod schema;
mod sql;
mod worker;

#[cfg(test)]
mod tests;

use application::{SecretValue, SystemPortError};
use rusqlite::{Connection, Transaction};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use worker::Writer;

#[derive(Debug, Clone)]
pub struct DatabasePath(PathBuf);

impl DatabasePath {
    #[must_use]
    pub const fn new(path: PathBuf) -> Self {
        Self(path)
    }
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

pub struct Database {
    writer: Writer,
    reader: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for Database {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Database(<redacted>)")
    }
}

impl Database {
    pub fn open(
        path: &DatabasePath,
        key: &SecretValue,
        clock: &dyn application::Clock,
    ) -> Result<Self, SystemPortError> {
        let writer = Writer::open_migrated(path, key, clock.now())?;
        let reader = connection::open(path, key, connection::Access::Read)?;
        Ok(Self {
            writer,
            reader: Arc::new(Mutex::new(reader)),
        })
    }

    pub async fn close(self) -> Result<application::WriteCompleted, SystemPortError> {
        tokio::task::spawn_blocking(move || {
            let Self {
                writer,
                reader: _reader,
            } = self;
            writer.close()
        })
        .await
        .map_err(|error| failure::join(&error))?
    }

    pub async fn check(&self) -> Result<application::WriteCompleted, SystemPortError> {
        self.read(|connection| connection.query_row(sql::SCHEMA, [], |row| row.get::<_, i64>(0)))
            .await?;
        self.write(|_| Ok(application::WriteCompleted)).await
    }

    pub(crate) async fn read<T, F>(&self, operation: F) -> Result<T, SystemPortError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> rusqlite::Result<T> + Send + 'static,
    {
        let mut connection = Arc::clone(&self.reader).lock_owned().await;
        tokio::task::spawn_blocking(move || {
            let transaction = connection
                .transaction()
                .map_err(|error| failure::sqlite(&error))?;
            let value = operation(&transaction).map_err(|error| failure::read(&error))?;
            transaction
                .commit()
                .map_err(|error| failure::sqlite(&error))?;
            Ok(value)
        })
        .await
        .map_err(|error| failure::join(&error))?
    }

    pub(crate) async fn write<T, F>(&self, operation: F) -> Result<T, SystemPortError>
    where
        T: Send + 'static,
        F: FnOnce(&Transaction<'_>) -> rusqlite::Result<T> + Send + 'static,
    {
        self.writer.execute(operation).await
    }
}

#[cfg(test)]
mod migration_tests;

#[cfg(test)]
mod review_tests;

#[cfg(test)]
mod third_tests;

#[cfg(test)]
mod final_tests;

mod holdings;
pub use holdings::{HoldingsSnapshot, HoldingsStore, SnapshotHolding};
#[cfg(test)]
mod holdings_tests;

#[cfg(test)]
mod transactions_tests;

mod transactions;
pub use transactions::TransactionStore;

mod market;
pub use market::MarketStore;

#[cfg(test)]
mod market_tests;

#[cfg(test)]
mod fts_guard_tests;

mod fts_schema;

#[cfg(test)]
mod information_tests;

mod information;
pub use information::InformationStore;

#[cfg(test)]
mod thesis_tests;

mod thesis;
pub use thesis::ThesisStore;
