use crate::{Currency, Holding, Money, Timestamp};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortfolioError {
    TooManyHoldings,
    DuplicateInstrument,
    CurrencyMismatch,
    NegativeBuyingPower,
    CashLookupFailed(crate::CashObservationFailure),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuyingPowerAmount {
    Available(Money),
    NegativeBuyingPower,
    CashLookupFailed(crate::CashObservationFailure),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyingPower {
    amount: BuyingPowerAmount,
    currency: Currency,
    observed_at: Timestamp,
}
impl BuyingPower {
    #[must_use]
    pub fn new(money: Money, observed_at: Timestamp) -> Self {
        let amount = if money.amount().as_decimal() < rust_decimal::Decimal::ZERO {
            BuyingPowerAmount::NegativeBuyingPower
        } else {
            BuyingPowerAmount::Available(money)
        };
        Self {
            amount,
            currency: money.currency(),
            observed_at,
        }
    }
    pub const fn money(self) -> Result<Money, PortfolioError> {
        match self.amount {
            BuyingPowerAmount::Available(value) => Ok(value),
            BuyingPowerAmount::NegativeBuyingPower => Err(PortfolioError::NegativeBuyingPower),
            BuyingPowerAmount::CashLookupFailed(reason) => {
                Err(PortfolioError::CashLookupFailed(reason))
            }
        }
    }
    #[must_use]
    pub const fn currency(self) -> Currency {
        self.currency
    }
    #[must_use]
    pub const fn observed_at(self) -> Timestamp {
        self.observed_at
    }

    pub(crate) const fn into_historical_state(self) -> crate::HistoricalBuyingPowerState {
        match self.amount {
            BuyingPowerAmount::Available(value) => crate::HistoricalBuyingPowerState::Available(
                crate::HistoricalCashMoney::from_verified(value),
            ),
            BuyingPowerAmount::NegativeBuyingPower => {
                crate::HistoricalBuyingPowerState::NegativeBuyingPower
            }
            BuyingPowerAmount::CashLookupFailed(reason) => {
                crate::HistoricalBuyingPowerState::CashLookupFailed(reason)
            }
        }
    }

    pub(crate) fn from_historical(
        currency: Currency,
        state: crate::HistoricalBuyingPowerState,
        observed_at: Timestamp,
    ) -> Result<Self, PortfolioError> {
        match state {
            crate::HistoricalBuyingPowerState::Available(cash) => {
                let value = cash.money();
                if value.currency() != currency {
                    return Err(PortfolioError::CurrencyMismatch);
                }
                Ok(Self {
                    amount: BuyingPowerAmount::Available(value),
                    currency,
                    observed_at,
                })
            }
            crate::HistoricalBuyingPowerState::NegativeBuyingPower => Ok(Self {
                amount: BuyingPowerAmount::NegativeBuyingPower,
                currency,
                observed_at,
            }),
            crate::HistoricalBuyingPowerState::CashLookupFailed(reason) => Ok(Self {
                amount: BuyingPowerAmount::CashLookupFailed(reason),
                currency,
                observed_at,
            }),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Portfolio {
    holdings: Vec<Holding>,
    krw_buying_power: BuyingPower,
    usd_buying_power: BuyingPower,
    reported: Option<ReportedPortfolio>,
}
impl Portfolio {
    pub fn new(
        mut holdings: Vec<Holding>,
        krw_buying_power: BuyingPower,
        usd_buying_power: BuyingPower,
    ) -> Result<Self, PortfolioError> {
        if holdings.len() > 4096 {
            return Err(PortfolioError::TooManyHoldings);
        }
        if krw_buying_power.currency() != Currency::Krw
            || usd_buying_power.currency() != Currency::Usd
        {
            return Err(PortfolioError::CurrencyMismatch);
        }
        holdings.sort_by(|left, right| {
            left.instrument()
                .market()
                .cmp(&right.instrument().market())
                .then_with(|| left.instrument().symbol().cmp(right.instrument().symbol()))
        });
        let mut previous = None;
        for holding in &holdings {
            if previous == Some(holding.instrument()) {
                return Err(PortfolioError::DuplicateInstrument);
            }
            previous = Some(holding.instrument());
        }
        Ok(Self {
            holdings,
            krw_buying_power,
            usd_buying_power,
            reported: None,
        })
    }
    #[must_use]
    pub fn with_reported(mut self, reported: ReportedPortfolio) -> Self {
        self.reported = Some(reported);
        self
    }
    #[must_use]
    pub const fn reported(&self) -> Option<ReportedPortfolio> {
        self.reported
    }
    #[must_use]
    pub fn evaluate(&self, rate: Option<crate::FxRate>) -> crate::PortfolioValuation {
        crate::PortfolioValuation::evaluate(self, rate)
    }
    #[must_use]
    pub fn holdings(&self) -> &[Holding] {
        &self.holdings
    }
    #[must_use]
    pub const fn buying_power(&self, currency: Currency) -> BuyingPower {
        match currency {
            Currency::Krw => self.krw_buying_power,
            Currency::Usd => self.usd_buying_power,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyAmounts {
    krw: Money,
    usd: Option<Money>,
}
impl CurrencyAmounts {
    pub fn new(krw: Money, usd: Option<Money>) -> Result<Self, PortfolioError> {
        if krw.currency() != Currency::Krw
            || usd.is_some_and(|value| value.currency() != Currency::Usd)
        {
            return Err(PortfolioError::CurrencyMismatch);
        }
        Ok(Self { krw, usd })
    }
    #[must_use]
    pub const fn amount(self, currency: Currency) -> Option<Money> {
        match currency {
            Currency::Krw => Some(self.krw),
            Currency::Usd => self.usd,
        }
    }
    pub(crate) fn resolved(self, currency: Currency) -> Money {
        self.amount(currency)
            .unwrap_or_else(|| crate::holding::zero(currency))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortfolioDailyProfitLoss {
    amounts: CurrencyAmounts,
    rate: crate::Ratio,
    basis: crate::ReturnBasis,
}
impl PortfolioDailyProfitLoss {
    #[must_use]
    pub const fn new(amounts: CurrencyAmounts, rate: crate::Ratio) -> Self {
        Self {
            amounts,
            rate,
            basis: crate::ReturnBasis::KrwConverted,
        }
    }
    #[must_use]
    pub const fn amounts(self) -> CurrencyAmounts {
        self.amounts
    }
    #[must_use]
    pub const fn rate(self) -> crate::Ratio {
        self.rate
    }
    #[must_use]
    pub const fn basis(self) -> crate::ReturnBasis {
        self.basis
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportedPortfolio {
    valuation: CurrencyAmounts,
    purchase: CurrencyAmounts,
    profit: CurrencyAmounts,
    daily: Option<PortfolioDailyProfitLoss>,
    return_rate: Option<crate::ReturnObservation>,
    observed_at: Timestamp,
}
impl ReportedPortfolio {
    #[must_use]
    pub const fn new(
        valuation: CurrencyAmounts,
        purchase: CurrencyAmounts,
        profit: CurrencyAmounts,
        observed_at: Timestamp,
    ) -> Self {
        Self {
            valuation,
            purchase,
            profit,
            daily: None,
            return_rate: None,
            observed_at,
        }
    }
    #[must_use]
    pub fn with_daily_profit_loss(mut self, daily: PortfolioDailyProfitLoss) -> Self {
        self.daily = Some(daily);
        self
    }
    #[must_use]
    pub fn with_return_rate(mut self, rate: crate::Ratio) -> Self {
        self.return_rate = Some(crate::ReturnObservation::reported(
            rate,
            crate::ReturnBasis::KrwConverted,
        ));
        self
    }
    #[must_use]
    pub const fn valuation(self) -> CurrencyAmounts {
        self.valuation
    }
    #[must_use]
    pub const fn purchase(self) -> CurrencyAmounts {
        self.purchase
    }
    #[must_use]
    pub const fn profit(self) -> CurrencyAmounts {
        self.profit
    }
    #[must_use]
    pub const fn daily_profit_loss(self) -> Option<PortfolioDailyProfitLoss> {
        self.daily
    }
    #[must_use]
    pub const fn return_rate(self) -> Option<crate::ReturnObservation> {
        self.return_rate
    }
    #[must_use]
    pub const fn observed_at(self) -> Timestamp {
        self.observed_at
    }
}

pub(crate) fn buying_power_from_observation(observation: crate::CashObservation) -> BuyingPower {
    match observation {
        crate::CashObservation::Observed(value) => value,
        crate::CashObservation::Failed {
            currency,
            reason,
            observed_at,
        } => BuyingPower {
            amount: BuyingPowerAmount::CashLookupFailed(reason),
            currency,
            observed_at,
        },
    }
}
