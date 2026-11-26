use std::sync::{Arc, RwLock};

use application::{BrokerRead, EventBus, PortError};
use domain::{
    AccountId, BrokerAccountSelection, Currency, DomainEvent, InstrumentId, KnownMarket,
    MaskedAccountNumber, QuoteInstrument, SavedQuoteInstrument, Symbol, select_brokerage_account,
};

use crate::ipc::state::BoxFuture;
use crate::toss_link::ConnectionListener;

pub use crate::mcp_market::{BrokerMarketTools, ToolMarketSource, ToolQuoteSource};

const TOOL_INSTRUMENT_ID: &str = "tool-quote";

pub(crate) fn tool_instrument(symbol: &Symbol, market: KnownMarket) -> Option<QuoteInstrument> {
    let currency = match market {
        KnownMarket::Kr => Currency::Krw,
        KnownMarket::Us => Currency::Usd,
    };
    let instrument_id = InstrumentId::try_new(TOOL_INSTRUMENT_ID).ok()?;
    QuoteInstrument::restore_from_saved_state(SavedQuoteInstrument {
        market,
        currency,
        symbol: symbol.clone(),
        instrument_id,
    })
    .ok()
}

#[derive(Debug, Clone, Default)]
pub struct AccountSlot(Arc<RwLock<Vec<AccountId>>>, Arc<RwLock<Vec<AccountLabel>>>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountLabel {
    account: AccountId,
    masked: MaskedAccountNumber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionChange {
    Changed,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountChoice {
    Chosen(SelectionChange),
    NotAvailable,
}

impl AccountSlot {
    #[must_use]
    pub fn with(accounts: Vec<AccountId>) -> Self {
        Self(Arc::new(RwLock::new(accounts)), Arc::default())
    }

    pub fn label(&self, account: &AccountId, masked: &MaskedAccountNumber) {
        let Ok(mut labels) = self.1.write() else {
            return;
        };
        labels.retain(|label| label.account != *account);
        labels.push(AccountLabel {
            account: account.clone(),
            masked: masked.clone(),
        });
    }

    #[must_use]
    pub fn masked_number(&self, account: &AccountId) -> Option<MaskedAccountNumber> {
        let labels = self.1.read().ok()?;
        labels
            .iter()
            .find(|label| label.account == *account)
            .map(|label| label.masked.clone())
    }

    #[must_use]
    pub fn current(&self) -> Vec<AccountId> {
        self.0
            .read()
            .map_or_else(|_poisoned| Vec::new(), |accounts| accounts.clone())
    }

    #[must_use]
    pub fn replace(&self, accounts: Vec<AccountId>) -> SelectionChange {
        let Ok(mut current) = self.0.write() else {
            return SelectionChange::Unchanged;
        };
        if *current == accounts {
            SelectionChange::Unchanged
        } else {
            *current = accounts;
            SelectionChange::Changed
        }
    }
}

pub struct AccountSelector<B> {
    broker: Arc<B>,
    slot: AccountSlot,
    events: Arc<EventBus>,
}

impl<B> std::fmt::Debug for AccountSelector<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccountSelector(<redacted>)")
    }
}

impl<B: BrokerRead + 'static> AccountSelector<B> {
    #[must_use]
    pub const fn new(broker: Arc<B>, slot: AccountSlot, events: Arc<EventBus>) -> Self {
        Self {
            broker,
            slot,
            events,
        }
    }

    pub async fn brokerage_accounts(
        &self,
    ) -> Result<Vec<(AccountId, MaskedAccountNumber)>, PortError> {
        let rows = self.broker.accounts().await?;
        Ok(rows
            .rows()
            .iter()
            .filter(|row| *row.kind() == domain::BrokerAccountKind::Brokerage)
            .map(|row| (row.account().clone(), row.masked_number().clone()))
            .collect())
    }

    pub async fn choose(&self, account: &AccountId) -> Result<AccountChoice, PortError> {
        let rows = self.broker.accounts().await?;
        match domain::select_brokerage_account_by_id(&rows, account) {
            domain::BrokerAccountKeySelection::Selected(row) => {
                self.slot.label(row.account(), row.masked_number());
                let change = self.slot.replace(vec![row.account().clone()]);
                if change == SelectionChange::Changed {
                    let _delivery = self.events.publish(DomainEvent::AccountSelectionChanged);
                }
                Ok(AccountChoice::Chosen(change))
            }
            domain::BrokerAccountKeySelection::NotBrokerage
            | domain::BrokerAccountKeySelection::NotFound
            | domain::BrokerAccountKeySelection::Empty => Ok(AccountChoice::NotAvailable),
        }
    }

    pub async fn select(&self) -> SelectionChange {
        let Ok(rows) = self.broker.accounts().await else {
            return SelectionChange::Unchanged;
        };
        if let Some(current) = self.slot.current().first()
            && let domain::BrokerAccountKeySelection::Selected(row) =
                domain::select_brokerage_account_by_id(&rows, current)
        {
            self.slot.label(row.account(), row.masked_number());
            return SelectionChange::Unchanged;
        }
        let chosen = match select_brokerage_account(&rows) {
            BrokerAccountSelection::Selected(row) => {
                self.slot.label(row.account(), row.masked_number());
                vec![row.account().clone()]
            }
            BrokerAccountSelection::NoBrokerage | BrokerAccountSelection::Empty => Vec::new(),
        };
        let change = self.slot.replace(chosen);
        if change == SelectionChange::Changed {
            let _delivery = self.events.publish(DomainEvent::AccountSelectionChanged);
        }
        change
    }
}

impl<B: BrokerRead + 'static> ConnectionListener for AccountSelector<B> {
    fn connected(&self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let _change = self.select().await;
        })
    }
}

#[cfg(test)]
mod tests;
