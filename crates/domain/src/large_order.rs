use crate::{
    ConvertedMoney, Currency, FxConversionError, FxPair, FxRate, FxRateKind, FxValidity, Money,
    Timestamp,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LargeOrderConfirmation {
    Required,
    NotRequired,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderFxRates {
    Missing,
    Observed { rate: FxRate, mid_rate: FxRate },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderFxUnavailable {
    Missing,
    InvalidRates,
    Conversion(FxConversionError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderKrwEstimate {
    Native(Money),
    Converted(ConvertedMoney),
    Unavailable(OrderFxUnavailable),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderAmountBasis {
    Fixed,
    Estimated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderExpectedAmount {
    money: Money,
    basis: OrderAmountBasis,
    krw_estimate: OrderKrwEstimate,
}
impl OrderExpectedAmount {
    pub(crate) fn from_order(
        money: Money,
        basis: OrderAmountBasis,
        rates: OrderFxRates,
        at: Timestamp,
    ) -> Self {
        let krw_estimate = match money.currency() {
            Currency::Krw => OrderKrwEstimate::Native(money),
            Currency::Usd => convert(money, rates, at),
        };
        Self {
            money,
            basis,
            krw_estimate,
        }
    }
    #[must_use]
    pub fn money(&self) -> Money {
        self.money
    }
    #[must_use]
    pub fn basis(&self) -> OrderAmountBasis {
        self.basis
    }
    #[must_use]
    pub fn krw_estimate(&self) -> OrderKrwEstimate {
        self.krw_estimate
    }
    #[must_use]
    pub fn confirmation(&self) -> LargeOrderConfirmation {
        let money = match self.krw_estimate {
            OrderKrwEstimate::Native(money) => money,
            OrderKrwEstimate::Converted(converted) => converted.into_parts().0,
            OrderKrwEstimate::Unavailable(_) => return LargeOrderConfirmation::Required,
        };
        if money.amount().as_decimal() >= rust_decimal::Decimal::from(80_000_000) {
            LargeOrderConfirmation::Required
        } else {
            LargeOrderConfirmation::NotRequired
        }
    }
}

fn convert(money: Money, rates: OrderFxRates, at: Timestamp) -> OrderKrwEstimate {
    let OrderFxRates::Observed { rate, mid_rate } = rates else {
        return OrderKrwEstimate::Unavailable(OrderFxUnavailable::Missing);
    };
    if rate.pair() != FxPair::UsdKrw
        || mid_rate.pair() != FxPair::UsdKrw
        || rate.kind() != FxRateKind::Buy
        || mid_rate.kind() != FxRateKind::Mid
        || rate.validity_at(at) != FxValidity::Valid
        || mid_rate.validity_at(at) != FxValidity::Valid
    {
        return OrderKrwEstimate::Unavailable(OrderFxUnavailable::InvalidRates);
    }
    match rate
        .conservative_with(mid_rate)
        .and_then(|selected| selected.convert_for_order(money, at))
    {
        Ok(converted) => OrderKrwEstimate::Converted(converted),
        Err(error) => OrderKrwEstimate::Unavailable(OrderFxUnavailable::Conversion(error)),
    }
}
