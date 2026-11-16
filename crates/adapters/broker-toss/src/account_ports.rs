use crate::{TokenRenewal, TossAccountRead};
use application::{
    BrokerRead, Clock, HoldingsObservation, HttpStatus, PortError, PortErrorKind, QuoteObservation,
    ReadEndpoint, SecretStore, TransactionRead,
};
use domain::{AccountId, BrokerAccountRows, MasterInstrumentFacts};
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
        self.fetch_holdings(account).await
    }
    fn transactions(
        &self,
        _account: &AccountId,
        _window: SettlementWindow,
        _filter: &TransactionFilter,
    ) -> impl Future<Output = Result<TransactionRead, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Transactions,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidRequest(application::RequestRejection::UnsupportedQuery),
        )))
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
        crate::market_read::candles(self, instrument, window, period, adjustment)
    }
    fn buying_power(
        &self,
        _account: &AccountId,
        _currency: Currency,
    ) -> impl Future<Output = Result<BuyingPower, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::BuyingPower,
            HttpStatus::NotReceived,
            PortErrorKind::InvalidRequest(application::RequestRejection::UnsupportedQuery),
        )))
    }
    fn sellable_quantity(
        &self,
        _account: &AccountId,
        _instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<SellableQuantityFact, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::SellableQuantity,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(application::BrokerCapability::SellableQuantity),
        )))
    }
    fn open_orders(
        &self,
        _account: &AccountId,
    ) -> impl Future<Output = Result<BrokerOrderSet, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::OpenOrders,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(application::BrokerCapability::OpenOrders),
        )))
    }
    fn closed_orders(
        &self,
        _account: &AccountId,
        _evaluation_window: TimeInterval,
    ) -> impl Future<Output = Result<ClosedOrderSet, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::ClosedOrders,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(application::BrokerCapability::ClosedOrders),
        )))
    }
    fn order(
        &self,
        _account: &AccountId,
        _order_id: &BrokerOrderId,
    ) -> impl Future<Output = Result<BrokerOrder, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Order,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(application::BrokerCapability::Order),
        )))
    }
    fn stock_warnings(
        &self,
        _instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<StockWarnings, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::StockWarnings,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(application::BrokerCapability::StockWarnings),
        )))
    }
    fn instruments(
        &self,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<MasterInstrumentFacts, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::Instruments,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(application::BrokerCapability::Instruments),
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
        _account: &AccountId,
        _market: KnownMarket,
    ) -> impl Future<Output = Result<CommissionRateFact, PortError>> + Send {
        std::future::ready(Err(PortError::new(
            ReadEndpoint::CommissionRate,
            HttpStatus::NotReceived,
            PortErrorKind::Unsupported(application::BrokerCapability::CommissionRate),
        )))
    }
}
