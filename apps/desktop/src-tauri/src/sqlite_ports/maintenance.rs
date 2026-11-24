use super::{REPOSITORY, SqliteRepositories, opened};
use application::{
    Clock, FullInspectionRepository, SecretValue, SpecStatusRecord, SystemPortError,
    TossSpecStatusRepository, TransactionSyncRecord, TransactionSyncRepository, WriteCompleted,
};
use domain::{AccountId, SettledRowSet, Timestamp};
use store_sqlite::{
    BackupDirectory, BackupOutcome, HoldingsStore, SpecStatusStore, TransactionStore,
};

impl TransactionSyncRepository for SqliteRepositories {
    async fn state(&self, account: &AccountId) -> Result<SettledRowSet, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TransactionSyncRepository::state(&TransactionStore::new(database), account).await
    }
    async fn latest_sync(
        &self,
        account: &AccountId,
    ) -> Result<Option<TransactionSyncRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TransactionSyncRepository::latest_sync(&TransactionStore::new(database), account).await
    }
    async fn record_sync(
        &self,
        account: &AccountId,
        previous: &SettledRowSet,
        previous_recorded_at: Option<Timestamp>,
        record: &TransactionSyncRecord,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TransactionSyncRepository::record_sync(
            &TransactionStore::new(database),
            account,
            previous,
            previous_recorded_at,
            record,
        )
        .await
    }
}

impl FullInspectionRepository for SqliteRepositories {
    async fn last_completed(
        &self,
        account: &AccountId,
    ) -> Result<Option<Timestamp>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        FullInspectionRepository::last_completed(&TransactionStore::new(database), account).await
    }
    async fn record_completed(
        &self,
        account: &AccountId,
        previous: Option<Timestamp>,
        completed_at: Timestamp,
    ) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        FullInspectionRepository::record_completed(
            &TransactionStore::new(database),
            account,
            previous,
            completed_at,
        )
        .await
    }
}

impl TossSpecStatusRepository for SqliteRepositories {
    async fn record(&self, status: &SpecStatusRecord) -> Result<WriteCompleted, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TossSpecStatusRepository::record(&SpecStatusStore::new(database), status).await
    }
    async fn latest(&self) -> Result<Option<SpecStatusRecord>, SystemPortError> {
        let guard = self.shared.read().await;
        let database = opened(guard.as_ref(), REPOSITORY)?;
        TossSpecStatusRepository::latest(&SpecStatusStore::new(database)).await
    }
}

pub async fn compact_intraday(
    repositories: &SqliteRepositories,
    clock: &dyn Clock,
) -> Result<WriteCompleted, SystemPortError> {
    let guard = repositories.shared.read().await;
    let database = opened(guard.as_ref(), REPOSITORY)?;
    HoldingsStore::new(database).compact_holdings(clock).await
}

pub async fn reclaim_space(
    repositories: &SqliteRepositories,
) -> Result<WriteCompleted, SystemPortError> {
    let guard = repositories.shared.read().await;
    let database = opened(guard.as_ref(), REPOSITORY)?;
    database.reclaim_space().await
}

pub async fn back_up(
    repositories: &SqliteRepositories,
    directory: &BackupDirectory,
    key: &SecretValue,
    clock: &dyn Clock,
) -> Result<BackupOutcome, SystemPortError> {
    let guard = repositories.shared.read().await;
    let database = opened(guard.as_ref(), REPOSITORY)?;
    store_sqlite::create_backup(database, directory, key, clock).await
}
