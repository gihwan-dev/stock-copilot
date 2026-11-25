use std::future::Future;
use std::sync::{Arc, RwLock};

use application::{
    BrokerRead, Clock, EventBus, MarketToolReads, PortError, QuoteObservation, QuoteToolView,
    RetryDelay, SystemFailure, SystemPort, SystemPortError,
};
use domain::{
    AccountId, AdjustmentBasis, BrokerAccountSelection, Candle, CandlePeriod, Currency,
    DomainEvent, InstrumentId, KnownMarket, MaskedAccountNumber, Nanosecond, PriceAdjustment,
    QuoteInstrument, SavedQuoteInstrument, SpanSeconds, Symbol, TimeInterval, TimeSpan,
    select_brokerage_account,
};

use crate::ipc::state::BoxFuture;
use crate::mcp_catalog::UnconnectedToolData;
use crate::toss_link::ConnectionListener;

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

pub trait ToolQuoteSource: Send + Sync {
    fn quotes(
        &self,
        instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>>;
    fn daily_candles(
        &self,
        instrument: QuoteInstrument,
        window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>>;
}

impl<B: BrokerRead + 'static> ToolQuoteSource for B {
    fn quotes(
        &self,
        instruments: Vec<QuoteInstrument>,
    ) -> BoxFuture<'_, Result<Vec<QuoteObservation>, PortError>> {
        Box::pin(async move { BrokerRead::quotes(self, &instruments).await })
    }

    fn daily_candles(
        &self,
        instrument: QuoteInstrument,
        window: TimeInterval,
    ) -> BoxFuture<'_, Result<Vec<Candle>, PortError>> {
        Box::pin(async move {
            BrokerRead::candles(
                self,
                &instrument,
                window,
                CandlePeriod::Daily,
                PriceAdjustment::Adjusted(AdjustmentBasis::new(window.end())),
            )
            .await
        })
    }
}

#[derive(Clone, Default)]
pub struct BrokerMarketTools {
    quotes: Option<Arc<dyn ToolQuoteSource>>,
    clock: Option<Arc<dyn Clock>>,
}

impl std::fmt::Debug for BrokerMarketTools {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrokerMarketTools")
    }
}

const TOOL_INSTRUMENT_ID: &str = "tool-quote";
const DAY_SECONDS: u64 = 86_400;
const CALENDAR_SPAN_FACTOR: u64 = 2;

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
            clock: None,
        }
    }

    #[must_use]
    pub fn with_clock(self, clock: Arc<dyn Clock>) -> Self {
        Self {
            clock: Some(clock),
            ..self
        }
    }

    #[must_use]
    pub const fn unconnected() -> Self {
        Self {
            quotes: None,
            clock: None,
        }
    }

    async fn read_candles(
        &self,
        symbol: &Symbol,
        market: KnownMarket,
        window: application::CandleWindow,
    ) -> Result<Option<application::CandlesToolView>, SystemPortError> {
        let (Some(source), Some(clock)) = (self.quotes.as_ref(), self.clock.as_ref()) else {
            return Err(unavailable());
        };
        let Some(instrument) = tool_instrument(symbol, market) else {
            return Ok(None);
        };
        let now = clock.now();
        let days = u64::from(window.as_u16());
        let span = TimeSpan::try_new(
            SpanSeconds::new(
                days.saturating_mul(CALENDAR_SPAN_FACTOR)
                    .saturating_mul(DAY_SECONDS),
            ),
            Nanosecond::new(0),
        )
        .map_err(|_error| unavailable())?;
        let interval = now
            .checked_sub(span)
            .ok()
            .and_then(|start| TimeInterval::try_new(start, now).ok())
            .ok_or_else(unavailable)?;
        let candles = source
            .daily_candles(instrument, interval)
            .await
            .map_err(|_error| unavailable())?;
        let keep = usize::from(window.as_u16());
        let skip = candles.len().saturating_sub(keep);
        let lines = candles
            .iter()
            .skip(skip)
            .map(|candle| application::CandleToolLine {
                opened_at: candle.time().timestamp(),
                open: candle.values().open().as_money(),
                high: candle.values().high().as_money(),
                low: candle.values().low().as_money(),
                close: candle.values().close().as_money(),
                volume: Some(candle.values().volume()),
            })
            .collect();
        Ok(Some(application::CandlesToolView {
            symbol: symbol.clone(),
            market,
            collected_at: now,
            lines,
        }))
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
        self.read_candles(symbol, market, window)
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
