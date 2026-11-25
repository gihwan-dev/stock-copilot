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

    pub async fn reclaim_space(&self) -> Result<application::WriteCompleted, SystemPortError> {
        self.write(|transaction| {
            let mut statement = transaction.prepare(sql::INCREMENTAL_VACUUM)?;
            let mut rows = statement.query([])?;
            while rows.next()?.is_some() {}
            Ok(application::WriteCompleted)
        })
        .await
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

#[cfg(test)]
mod consumption_tests;
#[cfg(test)]
mod validation_report_tests;

mod proposal_consumption;
pub use proposal_consumption::ProposalConsumptionStore;
mod validation_report;
pub use validation_report::ValidationReportStore;
#[cfg(test)]
mod proposal_boundary_tests;

mod instrument_mapping;
pub use instrument_mapping::{InstrumentIdIssuerStore, InstrumentMappingStore};
#[cfg(test)]
mod instrument_mapping_contract_tests;
#[cfg(test)]
mod instrument_mapping_correction_tests;
#[cfg(test)]
mod instrument_mapping_history_tests;
#[cfg(test)]
mod instrument_mapping_tests;

mod estimates;
pub use estimates::{EstimateStore, ValuationStore};
#[cfg(test)]
mod estimates_tests;

mod signals;
pub use signals::SignalStore;
#[cfg(test)]
mod signals_tests;

mod audit;
pub use audit::{AnchoredAuditStore, AuditStore};
#[cfg(test)]
mod audit_tests;

mod chat;
pub use chat::ChatStore;
#[cfg(test)]
mod chat_tests;

mod settings;
pub use settings::{SettingsStore, TossSpecHash, TossSpecHashError};
#[cfg(test)]
mod settings_tests;

#[cfg(test)]
mod maintenance_tests;

#[cfg(test)]
mod audit_anchor_tests;

mod backup;
pub use backup::{BackupDirectory, BackupOutcome, create_backup, restore_latest_backup};
#[cfg(test)]
mod backup_tests;

mod order_rate;
pub use order_rate::OrderRateStore;
mod order_ledger;
pub use order_ledger::{commit_prepared_submission, current_proposal, proposal_instrument};
#[cfg(test)]
mod order_rate_tests;

mod status;
pub use status::{NewsDataSettingsStore, SourceStatusStore, SpecStatusStore};
#[cfg(test)]
mod status_tests;

mod wake;
pub use wake::{WakeTriggerStore, WebItemStore};
#[cfg(test)]
mod wake_tests;

mod watchlist;
pub use watchlist::{SignalSkipStore, WatchlistStore};
#[cfg(test)]
mod watchlist_tests;

mod briefing;
pub use briefing::BriefingStore;

mod shared;
pub use shared::{SharedClosure, SharedDatabase, SqliteRepositories, not_opened};
mod shared_master;
pub use shared_master::SqliteMasterSession;
mod master_store;
pub use master_store::{MasterSnapshots, StoredMaster};
mod portfolio_store;
pub use portfolio_store::SqlitePortfolioStore;
#[cfg(test)]
mod briefing_tests;
#[cfg(test)]
mod portfolio_store_tests;
#[cfg(test)]
mod shared_tests;

#[cfg(test)]
mod jobs_tests;

#[cfg(test)]
mod signal_store_boundary_tests;

#[cfg(test)]
mod close_evidence_tests;

mod ai_jobs;
mod ai_preferences;
pub use ai_jobs::{AiJobStore, PrinciplesStore, Sha256PromptHasher};
pub use ai_preferences::AiPreferencesStore;
#[cfg(test)]
mod ai_jobs_tests;

#[cfg(test)]
mod scorecard_tests;

mod jobs;
pub use jobs::JobScheduleSqlStore;
