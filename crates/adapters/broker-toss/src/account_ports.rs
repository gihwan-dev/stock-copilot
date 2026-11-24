use crate::{TokenRenewal, TossAccountRead};
use application::{
    BrokerRead, Clock, HoldingsObservation, HttpStatus, PortError, PortErrorKind, QuoteObservation,
    ReadEndpoint, SecretStore, TransactionRead,
};
use domain::{AccountId, BrokerAccountRows, InstrumentMaster};
use domain::{
    BrokerOrder, BrokerOrderId, BrokerOrderSet, BuyingPower, Candle, CandlePeriod, ClosedOrderSet,
    CommissionRateFact, Currency, KnownMarket, Orderbook, PriceAdjustment, PriceLimits,
    QuoteInstrument, ResolvedOrderInstrument, SellableQuantityFact, SettlementWindow,
    StockWarnings, TimeInterval, TransactionFilter,
};
impl<Store: SecretStore, Time: Clock, Renewal: TokenRenewal> BrokerRead
    for TossAccountRead<Store, Time, Renewal>
{
    async fn accounts(&self) -> Result<BrokerAccountRows, PortError> {
        self.fetch_accounts().await
    }
    async fn holdings(&self, account: &AccountId) -> Result<HoldingsObservation, PortError> {
        self.fetch_holdings(account, crate::account_read::MasterErrorMode::Legacy)
            .await
    }
    fn transactions(
        &self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> impl Future<Output = Result<TransactionRead, PortError>> + Send {
        self.fetch_transactions(account, window, filter)
    }
    fn quotes(
        &self,
        instruments: &[QuoteInstrument],
    ) -> impl Future<Output = Result<Vec<QuoteObservation>, PortError>> + Send {
        crate::market_read::quotes(self, instruments)
    }
    fn candles(
        &self,
        instrument: &QuoteInstrument,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> impl Future<Output = Result<Vec<Candle>, PortError>> + Send {
        crate::market_read::candles(
            self,
            instrument,
            window,
            period,
            crate::market_read::candles::CandleMode {
                adjustment,
                master_errors: crate::account_read::MasterErrorMode::Legacy,
            },
        )
    }
    fn buying_power(
        &self,
        account: &AccountId,
        currency: Currency,
    ) -> impl Future<Output = Result<BuyingPower, PortError>> + Send {
        crate::order_info_read::buying_power(self, account, currency)
    }
    fn sellable_quantity(
        &self,
        account: &AccountId,
        instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<SellableQuantityFact, PortError>> + Send {
        crate::order_info_read::sellable_quantity(self, account, instrument)
    }
    fn open_orders(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<BrokerOrderSet, PortError>> + Send {
        crate::order_read::open_orders(self, account)
    }
    fn closed_orders(
        &self,
        account: &AccountId,
        evaluation_window: TimeInterval,
    ) -> impl Future<Output = Result<ClosedOrderSet, PortError>> + Send {
        crate::order_read::closed_orders(self, account, evaluation_window)
    }
    fn order(
        &self,
        account: &AccountId,
        order_id: &BrokerOrderId,
    ) -> impl Future<Output = Result<BrokerOrder, PortError>> + Send {
        self.fetch_order(
            account,
            order_id,
            crate::account_read::MasterErrorMode::Legacy,
        )
    }
    fn stock_warnings(
        &self,
        instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<StockWarnings, PortError>> + Send {
        crate::order_info_read::stock_warnings(self, instrument)
    }
    fn instruments(
        &self,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<InstrumentMaster, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Instruments,
            HttpStatus::NotReceived,
            PortErrorKind::MasterNotReady(application::MasterNotReady::StatusCoverage),
        )))
    }
    fn orderbook(
        &self,
        _instrument: &QuoteInstrument,
    ) -> impl Future<Output = Result<Orderbook, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Orderbook,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(application::BrokerCapability::Orderbook),
        )))
    }
    fn price_limits(
        &self,
        _instrument: &QuoteInstrument,
    ) -> impl Future<Output = Result<PriceLimits, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::PriceLimits,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(application::BrokerCapability::PriceLimits),
        )))
    }
    fn commission_rate(
        &self,
        account: &AccountId,
        market: KnownMarket,
    ) -> impl Future<Output = Result<CommissionRateFact, PortError>> + Send {
        crate::order_info_read::commission_rate(self, account, market)
    }
}
