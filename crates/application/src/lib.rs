mod observations;
mod port_error;
use domain::{
    AccountId, BuyingPower, Candle, CandlePeriod, Currency, FxPair, FxRate, FxRateKind,
    OrderFxRates, PriceAdjustment, ResolvedOrderInstrument, SettlementWindow, TimeInterval,
    Timestamp, TransactionFilter,
};
pub use observations::{
    AccountObservation, HoldingsObservation, IncompleteReason, IncompleteTransactions,
    QuarantinedTransaction, QuoteObservation, TransactionRead,
};
pub use port_error::{
    AccessDenial, HttpStatus, MissingData, PortError, PortErrorKind, ReadEndpoint,
    RequestRejection, ResponseMismatch, RetryDelay, UnavailableCause,
};
use std::{collections::BTreeSet, future::Future};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BrokerCapability {
    DailyCandles,
    MinuteCandles,
    AdjustedHistory,
    RawHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilitySupport {
    Supported,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerCapabilities(BTreeSet<BrokerCapability>);
impl BrokerCapabilities {
    #[must_use]
    pub const fn new(supported: BTreeSet<BrokerCapability>) -> Self {
        Self(supported)
    }

    #[must_use]
    pub fn support_for(&self, capability: BrokerCapability) -> CapabilitySupport {
        if self.0.contains(&capability) {
            CapabilitySupport::Supported
        } else {
            CapabilitySupport::Unsupported
        }
    }
}

pub trait BrokerRead: Send + Sync {
    fn accounts(&self) -> impl Future<Output = Result<Vec<AccountObservation>, PortError>> + Send;
    fn holdings(
        &self,
        account: &AccountId,
    ) -> impl Future<Output = Result<HoldingsObservation, PortError>> + Send;
    fn transactions(
        &self,
        account: &AccountId,
        window: SettlementWindow,
        filter: &TransactionFilter,
    ) -> impl Future<Output = Result<TransactionRead, PortError>> + Send;
    fn quotes(
        &self,
        instruments: &[ResolvedOrderInstrument],
    ) -> impl Future<Output = Result<Vec<QuoteObservation>, PortError>> + Send;
    fn candles(
        &self,
        instrument: &ResolvedOrderInstrument,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> impl Future<Output = Result<Vec<Candle>, PortError>> + Send;
    fn buying_power(
        &self,
        account: &AccountId,
        currency: Currency,
    ) -> impl Future<Output = Result<BuyingPower, PortError>> + Send;
}

pub trait MarketInfo: Send + Sync {
    fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: Option<Timestamp>,
    ) -> impl Future<Output = Result<FxRate, PortError>> + Send;
    fn order_fx_rates(&self) -> impl Future<Output = Result<OrderFxRates, PortError>> + Send;
}

mod error_class;
pub use error_class::{
    BugDiagnostic, ClassifiedError, ErrorClass, UserAction, classify_port_error,
};
