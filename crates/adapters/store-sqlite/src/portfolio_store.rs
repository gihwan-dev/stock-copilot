use crate::{HoldingsSnapshot, HoldingsStore, SnapshotHolding, SqliteRepositories, not_opened};
use application::{PortfolioRepository, SystemPort, SystemPortError, WriteCompleted};
use domain::{
    AccountId, BuyingPower, Currency, Holding, HoldingObservation, MasterInstrument, Money,
    Portfolio, SyncOutcome, SyncRecord, Timestamp,
};
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

    async fn restore(
        &self,
        account: &AccountId,
    ) -> Result<Option<SyncRecord<Portfolio>>, SystemPortError> {
        let stored = {
            let guard = self.repositories.shared().read().await;
            let Some(database) = guard.as_ref() else {
                return Ok(None);
            };
            HoldingsStore::new(database)
                .latest_holdings(account.clone())
                .await?
        };
        let Some((snapshot, cash)) = stored else {
            return Ok(None);
        };
        let masters: Vec<MasterInstrument> = crate::MasterSnapshots(&self.repositories)
            .stored()
            .await?
            .into_iter()
            .flat_map(|market| market.instruments)
            .collect();
        Ok(
            restored_portfolio(&snapshot, &cash, &masters).map(|portfolio| {
                SyncRecord::new(snapshot.observed_at, SyncOutcome::Partial(portfolio))
            }),
        )
    }
}

fn cash_rows(record: &SyncRecord<Portfolio>) -> Vec<(Money, Timestamp)> {
    match record.outcome() {
        SyncOutcome::Complete(portfolio) | SyncOutcome::Partial(portfolio) => {
            [Currency::Krw, Currency::Usd]
                .into_iter()
                .filter_map(|currency| {
                    let power = portfolio.buying_power(currency);
                    power.money().ok().map(|money| (money, power.observed_at()))
                })
                .collect()
        }
        SyncOutcome::Failed | SyncOutcome::AccessBlocked => Vec::new(),
    }
}

fn restored_holding(item: &SnapshotHolding, masters: &[MasterInstrument]) -> Option<Holding> {
    let currency = item.acquisition_price.money().currency();
    let master = masters
        .iter()
        .find(|master| master.row().symbol == item.symbol && master.row().currency == currency)?;
    Holding::new(
        master.held_instrument().ok()?,
        item.quantity,
        item.acquisition_price,
        HoldingObservation::new(item.current_price, item.reported, item.observed_at),
    )
    .ok()
}

fn restored_portfolio(
    snapshot: &HoldingsSnapshot,
    cash: &[(Money, Timestamp)],
    masters: &[MasterInstrument],
) -> Option<Portfolio> {
    let holdings = snapshot
        .holdings
        .iter()
        .map(|item| restored_holding(item, masters))
        .collect::<Option<Vec<_>>>()?;
    let power = |currency| {
        cash.iter()
            .find(|(money, _at)| money.currency() == currency)
            .map(|(money, at)| BuyingPower::new(*money, *at))
    };
    Portfolio::new(holdings, power(Currency::Krw)?, power(Currency::Usd)?).ok()
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

async fn record_snapshot(
    database: &crate::Database,
    account: &AccountId,
    holdings: HoldingsSnapshot,
    cash: Vec<(Money, Timestamp)>,
) -> Result<WriteCompleted, SystemPortError> {
    let store = HoldingsStore::new(database);
    let observed_at = holdings.observed_at;
    let written = store.record_holdings(holdings).await?;
    if cash.is_empty() {
        return Ok(written);
    }
    store.record_cash(account.clone(), observed_at, cash).await
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
                record_snapshot(database, account, holdings, cash_rows(record)).await?;
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
        let cached = self.latest.read().await.get(account).cloned();
        if let Some(record) = cached {
            return Ok(Some(record));
        }
        let Some(restored) = self.restore(account).await? else {
            return Ok(None);
        };
        let mut latest = self.latest.write().await;
        Ok(Some(
            latest.entry(account.clone()).or_insert(restored).clone(),
        ))
    }
}
