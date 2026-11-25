use std::future::Future;
use std::sync::{Arc, RwLock};

use application::{
    BrokerRead, EventBus, MarketToolReads, PortError, QuoteObservation, QuoteToolView, RetryDelay,
    SystemFailure, SystemPort, SystemPortError,
};
use domain::{
    AccountId, BrokerAccountSelection, Currency, DomainEvent, InstrumentId, KnownMarket,
    QuoteInstrument, SavedQuoteInstrument, Symbol, select_brokerage_account,
};

use crate::ipc::state::BoxFuture;
use crate::mcp_catalog::UnconnectedToolData;
use crate::toss_link::ConnectionListener;

#[derive(Debug, Clone, Default)]
pub struct AccountSlot(Arc<RwLock<Vec<AccountId>>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionChange {
    Changed,
    Unchanged,
}

impl AccountSlot {
    #[must_use]
    pub fn with(accounts: Vec<AccountId>) -> Self {
        Self(Arc::new(RwLock::new(accounts)))
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

    pub async fn select(&self) -> SelectionChange {
        let Ok(rows) = self.broker.accounts().await else {
            return SelectionChange::Unchanged;
        };
        let chosen = match select_brokerage_account(&rows) {
            BrokerAccountSelection::Selected(row) => vec![row.account().clone()],
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

pub trait ToolQuoteSource: Send + Sync {
    fn quotes(
        &self,
        instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>>;
}

impl<B: BrokerRead + 'static> ToolQuoteSource for B {
    fn quotes(
        &self,
        instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>> {
        Box::pin(async move { BrokerRead::quotes(self, &instruments).await })
    }
}

#[derive(Clone, Default)]
pub struct BrokerMarketTools {
    quotes: Option<Arc<dyn ToolQuoteSource>>,
}

impl std::fmt::Debug for BrokerMarketTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrokerMarketTools")
    }
}

const TOOL_INSTRUMENT_ID: &str = "tool-quote";

fn unavailable() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    )
}

fn tool_instrument(symbol: &Symbol, market: KnownMarket) -> Option<QuoteInstrument> {
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

impl BrokerMarketTools {
    #[must_use]
    pub fn connected(quotes: Arc<dyn ToolQuoteSource>) -> Self {
        Self {
            quotes: Some(quotes),
        }
    }

    #[must_use]
    pub const fn unconnected() -> Self {
        Self { quotes: None }
    }

    async fn read_quote(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> Result<Option<QuoteToolView>, SystemPortError> {
        let Some(source) = self.quotes.as_ref() else {
            return Err(unavailable());
        };
        let Some(instrument) = tool_instrument(symbol, market) else {
            return Ok(None);
        };
        let observations = source
            .quotes(vec![instrument])
            .await
            .map_err(|_error| unavailable())?;
        let Some(observation) = observations.into_iter().next() else {
            return Ok(None);
        };
        let Ok(quote) = observation.result else {
            return Err(unavailable());
        };
        Ok(quote.last_price().map(|price| QuoteToolView {
            symbol: symbol.clone(),
            market,
            price: price.as_money(),
            change_rate: None,
            collected_at: quote.collected_at(),
        }))
    }
}

impl MarketToolReads for BrokerMarketTools {
    fn quote(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<QuoteToolView>, SystemPortError>> + Send {
        self.read_quote(symbol, market)
    }

    fn candles(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
        window: application::CandleWindow,
    ) -> impl Future<Output = Result<Option<application::CandlesToolView>, SystemPortError>> + Send
    {
        UnconnectedToolData.candles(symbol, market, window)
    }

    fn market_overview(
        &self,
    ) -> impl Future<Output = Result<Option<application::MarketOverviewToolView>, SystemPortError>> + Send
    {
        UnconnectedToolData.market_overview()
    }

    fn rankings(
        &self,
        kind: application::RankingToolKind,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<application::RankingsToolView>, SystemPortError>> + Send
    {
        UnconnectedToolData.rankings(kind, market)
    }

    fn sectors(
        &self,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<application::SectorsToolView>, SystemPortError>> + Send
    {
        UnconnectedToolData.sectors(market)
    }

    fn investor_flow(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
    ) -> impl Future<Output = Result<Option<application::InvestorFlowToolView>, SystemPortError>> + Send
    {
        UnconnectedToolData.investor_flow(symbol, market)
    }
}

#[cfg(test)]
mod tests;
