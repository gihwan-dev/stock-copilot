use crate::{Currency, Money, Price, Quantity, Ratio, ResolvedOrderInstrument, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldingError {
    NegativeAcquisitionPrice,
    NegativeValuation,
    CurrencyMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcquisitionPrice(Money);

impl AcquisitionPrice {
    pub fn try_new(value: Money) -> Result<Self, HoldingError> {
        if value.amount().as_decimal().is_sign_negative() && !value.amount().as_decimal().is_zero()
        {
            return Err(HoldingError::NegativeAcquisitionPrice);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn money(self) -> Money {
        self.0
    }
    pub fn checked_mul(self, quantity: Quantity) -> Result<Money, crate::MoneyError> {
        crate::exact_arithmetic::multiply(
            self.0.amount(),
            crate::Amount::new(quantity.as_decimal()),
        )
        .map(|amount| Money::new(amount, self.0.currency()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnBasis {
    TradingCurrency(Currency),
    KrwConverted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyProfitLoss {
    amount: Money,
    rate: Ratio,
}
impl DailyProfitLoss {
    #[must_use]
    pub const fn new(amount: Money, rate: Ratio) -> Self {
        Self { amount, rate }
    }
    #[must_use]
    pub const fn amount(self) -> Money {
        self.amount
    }
    #[must_use]
    pub const fn rate(self) -> Ratio {
        self.rate
    }
    #[must_use]
    pub fn basis(self) -> ReturnBasis {
        ReturnBasis::TradingCurrency(self.amount.currency())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportedHolding {
    valuation: Money,
    profit: Money,
    return_rate: Ratio,
    daily_profit_loss: Option<DailyProfitLoss>,
}

impl ReportedHolding {
    pub fn new(valuation: Money, profit: Money, return_rate: Ratio) -> Result<Self, HoldingError> {
        if valuation.currency() != profit.currency() {
            return Err(HoldingError::CurrencyMismatch);
        }
        if valuation.amount().as_decimal() < rust_decimal::Decimal::ZERO {
            return Err(HoldingError::NegativeValuation);
        }
        Ok(Self {
            valuation,
            profit,
            return_rate,
            daily_profit_loss: None,
        })
    }
    pub fn with_daily_profit_loss(mut self, daily: DailyProfitLoss) -> Result<Self, HoldingError> {
        if daily.amount().currency() != self.valuation.currency() {
            return Err(HoldingError::CurrencyMismatch);
        }
        self.daily_profit_loss = Some(daily);
        Ok(self)
    }
    #[must_use]
    pub const fn daily_profit_loss(self) -> Option<DailyProfitLoss> {
        self.daily_profit_loss
    }
    #[must_use]
    pub const fn valuation(self) -> Money {
        self.valuation
    }
    #[must_use]
    pub const fn profit(self) -> Money {
        self.profit
    }
    #[must_use]
    pub const fn return_rate(self) -> Ratio {
        self.return_rate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoldingObservation {
    current_price: Option<Price>,
    reported: Option<ReportedHolding>,
    observed_at: Timestamp,
}
impl HoldingObservation {
    #[must_use]
    pub const fn new(
        current_price: Option<Price>,
        reported: Option<ReportedHolding>,
        observed_at: Timestamp,
    ) -> Self {
        Self {
            current_price,
            reported,
            observed_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holding {
    instrument: ResolvedOrderInstrument,
    quantity: Quantity,
    acquisition_price: AcquisitionPrice,
    current_price: Option<Price>,
    reported: Option<ReportedHolding>,
    observed_at: Timestamp,
}

impl Holding {
    pub fn new(
        instrument: ResolvedOrderInstrument,
        quantity: Quantity,
        acquisition_price: AcquisitionPrice,
        observation: HoldingObservation,
    ) -> Result<Self, HoldingError> {
        let HoldingObservation {
            current_price,
            reported,
            observed_at,
        } = observation;
        let currency = acquisition_price.money().currency();
        let expected_market = match currency {
            Currency::Krw => crate::Market::Kr,
            Currency::Usd => crate::Market::Us,
        };
        if instrument.market() != expected_market
            || current_price.is_some_and(|price| price.as_money().currency() != currency)
            || reported.is_some_and(|report| report.valuation().currency() != currency)
        {
            return Err(HoldingError::CurrencyMismatch);
        }
        Ok(Self {
            instrument,
            quantity,
            acquisition_price,
            current_price,
            reported,
            observed_at,
        })
    }
    #[must_use]
    pub fn evaluate(&self) -> crate::HoldingValuation {
        crate::HoldingValuation::evaluate(self)
    }
    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn quantity(&self) -> Quantity {
        self.quantity
    }
    #[must_use]
    pub const fn acquisition_price(&self) -> AcquisitionPrice {
        self.acquisition_price
    }
    #[must_use]
    pub const fn current_price(&self) -> Option<Price> {
        self.current_price
    }
    #[must_use]
    pub const fn reported(&self) -> Option<ReportedHolding> {
        self.reported
    }
    #[must_use]
    pub const fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
    #[must_use]
    pub fn currency(&self) -> Currency {
        self.acquisition_price.money().currency()
    }
}

pub(crate) fn zero(currency: Currency) -> Money {
    Money::new(crate::Amount::new(rust_decimal::Decimal::ZERO), currency)
}
