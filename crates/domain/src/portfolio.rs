use crate::{Currency, Holding, Money, Timestamp};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortfolioError {
    TooManyHoldings,
    DuplicateInstrument,
    CurrencyMismatch,
    NegativeBuyingPower,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyingPower {
    money: Money,
    observed_at: Timestamp,
}
impl BuyingPower {
    pub fn new(money: Money, observed_at: Timestamp) -> Result<Self, PortfolioError> {
        if money.amount().as_decimal() < rust_decimal::Decimal::ZERO {
            return Err(PortfolioError::NegativeBuyingPower);
        }
        Ok(Self { money, observed_at })
    }
    #[must_use]
    pub const fn money(self) -> Money {
        self.money
    }
    #[must_use]
    pub const fn observed_at(self) -> Timestamp {
        self.observed_at
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Portfolio {
    holdings: Vec<Holding>,
    krw_buying_power: BuyingPower,
    usd_buying_power: BuyingPower,
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
        if krw_buying_power.money().currency() != Currency::Krw
            || usd_buying_power.money().currency() != Currency::Usd
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
        })
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
