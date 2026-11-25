use crate::{HoldingsSnapshot, HoldingsStore, SnapshotHolding, SqliteRepositories, not_opened};
use application::{PortfolioRepository, SystemPort, SystemPortError, WriteCompleted};
use domain::{AccountId, Portfolio, SyncOutcome, SyncRecord};
use std::collections::BTreeMap;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct SqlitePortfolioStore {
    repositories: SqliteRepositories,
    latest: RwLock<BTreeMap<AccountId, SyncRecord<Portfolio>>>,
}

impl std::fmt::Debug for SqlitePortfolioStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SqlitePortfolioStore(<redacted>)")
    }
}

impl SqlitePortfolioStore {
    #[must_use]
    pub fn new(repositories: SqliteRepositories) -> Self {
        Self {
            repositories,
            latest: RwLock::new(BTreeMap::new()),
        }
    }
}

fn snapshot(account: &AccountId, record: &SyncRecord<Portfolio>) -> Option<HoldingsSnapshot> {
    match record.outcome() {
        SyncOutcome::Complete(portfolio) | SyncOutcome::Partial(portfolio) => {
            Some(HoldingsSnapshot {
                account: account.clone(),
                observed_at: record.recorded_at(),
                holdings: portfolio
                    .holdings()
                    .iter()
                    .map(SnapshotHolding::from)
                    .collect(),
            })
        }
        SyncOutcome::Failed | SyncOutcome::AccessBlocked => None,
    }
}

impl PortfolioRepository for SqlitePortfolioStore {
    async fn record(
        &self,
        account: &AccountId,
        record: &SyncRecord<Portfolio>,
    ) -> Result<WriteCompleted, SystemPortError> {
        {
            let guard = self.repositories.shared().read().await;
            let database = guard.as_ref().ok_or(not_opened(SystemPort::Repository))?;
            if let Some(holdings) = snapshot(account, record) {
                let _written = HoldingsStore::new(database)
                    .record_holdings(holdings)
                    .await?;
            }
        }
        let _previous = self
            .latest
            .write()
            .await
            .insert(account.clone(), record.clone());
        Ok(WriteCompleted)
    }

    async fn latest(
        &self,
        account: &AccountId,
    ) -> Result<Option<SyncRecord<Portfolio>>, SystemPortError> {
        Ok(self.latest.read().await.get(account).cloned())
    }
}
