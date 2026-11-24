use crate::{TokenRenewal, TossAccountRead};
use application::{
    BrokerRead, Clock, HoldingsObservation, InstrumentIdIssuer, InstrumentMasterAuthority,
    PortError, QuoteObservation, SecretStore, TransactionRead,
};
use domain::{
    AccountId, BrokerAccountRows, BrokerOrder, BrokerOrderId, BrokerOrderSet, BuyingPower, Candle,
    CandlePeriod, CommissionRateFact, Currency, InstrumentMaster, KnownMarket, Orderbook,
    PriceAdjustment, PriceLimits, ProvenClosedOrderSet, QuoteInstrument, ResolvedOrderInstrument,
    SellableQuantityFact, SettlementWindow, StockWarnings, TimeInterval, TransactionFilter,
};

pub struct TossInstrumentRead<Store, Time, Renewal, Authority, Issuer> {
    pub(crate) reader: TossAccountRead<Store, Time, Renewal>,
    pub(crate) authority: Authority,
    pub(crate) issuer: Issuer,
}

impl<Store, Time, Renewal, Authority, Issuer> std::fmt::Debug
    for TossInstrumentRead<Store, Time, Renewal, Authority, Issuer>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TossInstrumentRead(<redacted>)")
    }
}

impl<Store: SecretStore, Time: Clock, Renewal: TokenRenewal, Authority, Issuer>
    TossInstrumentRead<Store, Time, Renewal, Authority, Issuer>
{
    #[must_use]
    pub fn with_market_master(mut self, master: domain::MasterInstrumentFacts) -> Self {
        self.reader = self.reader.with_market_master(master);
        self
    }

    #[must_use]
    pub fn capabilities() -> application::BrokerCapabilities {
        TossAccountRead::<Store, Time, Renewal>::capabilities()
            .with_supported(application::BrokerCapability::Instruments)
    }
}

impl<
    Store: SecretStore,
    Time: Clock,
    Renewal: TokenRenewal,
    Authority: InstrumentMasterAuthority,
    Issuer: InstrumentIdIssuer,
> BrokerRead for TossInstrumentRead<Store, Time, Renewal, Authority, Issuer>
{
    async fn accounts(&self) -> Result<BrokerAccountRows, PortError> {
        self.reader.accounts().await
    }
    async fn holdings(&self, account: &AccountId) -> Result<HoldingsObservation, PortError> {
        self.reader
            .fetch_holdings(account, crate::account_read::MasterErrorMode::Dedicated)
            .await
    }
    async fn transactions(
        &self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> Result<TransactionRead, PortError> {
        self.reader.transactions(account, window, filter).await
    }
    async fn quotes(
        &self,
        instruments: &[QuoteInstrument],
    ) -> Result<Vec<QuoteObservation>, PortError> {
        self.reader.quotes(instruments).await
    }
    async fn candles(
        &self,
        instrument: &QuoteInstrument,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> Result<Vec<Candle>, PortError> {
        crate::market_read::candles(
            &self.reader,
            instrument,
            window,
            period,
            crate::market_read::candles::CandleMode {
                adjustment,
                master_errors: crate::account_read::MasterErrorMode::Dedicated,
            },
        )
        .await
    }
    async fn buying_power(
        &self,
        account: &AccountId,
        currency: Currency,
    ) -> Result<BuyingPower, PortError> {
        self.reader.buying_power(account, currency).await
    }
    async fn sellable_quantity(
        &self,
        account: &AccountId,
        instrument: &ResolvedOrderInstrument,
    ) -> Result<SellableQuantityFact, PortError> {
        self.reader.sellable_quantity(account, instrument).await
    }
    async fn open_orders(&self, account: &AccountId) -> Result<BrokerOrderSet, PortError> {
        self.reader.open_orders(account).await
    }
    async fn closed_orders(
        &self,
        account: &AccountId,
        evaluation_window: TimeInterval,
    ) -> Result<ProvenClosedOrderSet, PortError> {
        self.reader.closed_orders(account, evaluation_window).await
    }
    async fn order(
        &self,
        account: &AccountId,
        order_id: &BrokerOrderId,
    ) -> Result<BrokerOrder, PortError> {
        self.reader
            .fetch_order(
                account,
                order_id,
                crate::account_read::MasterErrorMode::Dedicated,
            )
            .await
    }
    async fn stock_warnings(
        &self,
        instrument: &ResolvedOrderInstrument,
    ) -> Result<StockWarnings, PortError> {
        self.reader.stock_warnings(instrument).await
    }
    async fn instruments(&self, market: KnownMarket) -> Result<InstrumentMaster, PortError> {
        crate::instrument_master::fetch(self, market).await
    }
    async fn orderbook(&self, instrument: &QuoteInstrument) -> Result<Orderbook, PortError> {
        self.reader.orderbook(instrument).await
    }
    async fn price_limits(&self, instrument: &QuoteInstrument) -> Result<PriceLimits, PortError> {
        self.reader.price_limits(instrument).await
    }
    async fn commission_rate(
        &self,
        account: &AccountId,
        market: KnownMarket,
    ) -> Result<CommissionRateFact, PortError> {
        self.reader.commission_rate(account, market).await
    }
}
