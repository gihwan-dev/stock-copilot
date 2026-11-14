use crate::{
    AccountId, BrokerOrderMoney, BrokerOrderPrice, BrokerOrderSize, BrokerOrderStatus,
    BrokerOrderTime, BrokerOrderType, BrokerTimeInForce, Currency, Market, OrderSide, Price,
    Quantity, ResolvedOrderInstrument, Timestamp, UnfilledQuantity,
};
use std::fmt;
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BrokerOrderId(String);
impl BrokerOrderId {
    pub fn try_new(value: &str) -> Result<Self, BrokerOrderError> {
        if value.is_empty()
            || value.len() > 256
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(BrokerOrderError::InvalidOrderId);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for BrokerOrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrderId").finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerOrderError {
    InvalidOrderId,
    FilledExceedsQuantity,
    CurrencyMismatch,
    DuplicateOrderId,
    ObservationMismatch,
    AccountMismatch,
    ClosedInOpenResult,
    InvalidAmountOrder,
    NegativeFilledAmount,
    Arithmetic(crate::MoneyError),
}
#[derive(Clone, PartialEq, Eq)]
pub struct BrokerOrderRow {
    pub account_id: AccountId,
    pub order_id: BrokerOrderId,
    pub instrument: ResolvedOrderInstrument,
    pub side: OrderSide,
    pub status: BrokerOrderStatus,
    pub size: BrokerOrderSize,
    pub filled_quantity: Quantity,
    pub average_filled_price: Option<Price>,
    pub filled_amount: BrokerOrderMoney,
    pub filled_at: Option<Timestamp>,
    pub ordered_at: Timestamp,
    pub canceled_at: BrokerOrderTime,
    pub price: BrokerOrderPrice,
    pub order_type: BrokerOrderType,
    pub time_in_force: BrokerTimeInForce,
    pub observed_at: Timestamp,
}
impl fmt::Debug for BrokerOrderRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrderRow").finish_non_exhaustive()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct BrokerOrder {
    row: BrokerOrderRow,
    unfilled_quantity: UnfilledQuantity,
}
impl BrokerOrder {
    pub fn from_lookup_result(row: BrokerOrderRow) -> Result<Self, BrokerOrderError> {
        let unfilled_quantity = remaining_quantity(&row)?;
        let currency = if row.instrument.market() == Market::Kr {
            Currency::Krw
        } else {
            Currency::Usd
        };
        validate_prices_and_amount(&row, currency)?;
        Ok(Self {
            row,
            unfilled_quantity,
        })
    }
    #[must_use]
    pub fn account_id(&self) -> &AccountId {
        &self.row.account_id
    }
    #[must_use]
    pub fn order_id(&self) -> &BrokerOrderId {
        &self.row.order_id
    }
    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.row.instrument
    }
    #[must_use]
    pub fn side(&self) -> OrderSide {
        self.row.side
    }
    #[must_use]
    pub fn status(&self) -> &BrokerOrderStatus {
        &self.row.status
    }
    #[must_use]
    pub fn size(&self) -> BrokerOrderSize {
        self.row.size
    }
    #[must_use]
    pub fn filled_quantity(&self) -> Quantity {
        self.row.filled_quantity
    }
    #[must_use]
    pub fn average_filled_price(&self) -> Option<Price> {
        self.row.average_filled_price
    }
    #[must_use]
    pub fn filled_amount(&self) -> BrokerOrderMoney {
        self.row.filled_amount
    }
    #[must_use]
    pub fn filled_at(&self) -> Option<Timestamp> {
        self.row.filled_at
    }
    #[must_use]
    pub fn ordered_at(&self) -> Timestamp {
        self.row.ordered_at
    }
    #[must_use]
    pub fn canceled_at(&self) -> BrokerOrderTime {
        self.row.canceled_at
    }
    #[must_use]
    pub fn price(&self) -> BrokerOrderPrice {
        self.row.price
    }
    #[must_use]
    pub fn order_type(&self) -> &BrokerOrderType {
        &self.row.order_type
    }
    #[must_use]
    pub fn time_in_force(&self) -> &BrokerTimeInForce {
        &self.row.time_in_force
    }
    #[must_use]
    pub fn observed_at(&self) -> Timestamp {
        self.row.observed_at
    }
    #[must_use]
    pub fn quantity(&self) -> Quantity {
        match self.row.size {
            BrokerOrderSize::Quantity(quantity) => quantity,
            BrokerOrderSize::Amount {
                reported_quantity, ..
            } => reported_quantity,
        }
    }
    #[must_use]
    pub fn order_amount(&self) -> BrokerOrderMoney {
        match self.row.size {
            BrokerOrderSize::Quantity(_) => BrokerOrderMoney::Absent,
            BrokerOrderSize::Amount { order_amount, .. } => BrokerOrderMoney::Known(order_amount),
        }
    }
    #[must_use]
    pub fn unfilled_quantity(&self) -> UnfilledQuantity {
        self.unfilled_quantity
    }
}
impl fmt::Debug for BrokerOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrder").finish_non_exhaustive()
    }
}
fn remaining_quantity(row: &BrokerOrderRow) -> Result<UnfilledQuantity, BrokerOrderError> {
    match row.size {
        BrokerOrderSize::Quantity(quantity) => {
            let remaining = crate::Amount::new(quantity.as_decimal())
                .checked_sub(&crate::Amount::new(row.filled_quantity.as_decimal()))
                .map_err(BrokerOrderError::Arithmetic)?;
            Quantity::try_new(remaining.as_decimal())
                .map(UnfilledQuantity::Known)
                .map_err(|_error| BrokerOrderError::FilledExceedsQuantity)
        }
        BrokerOrderSize::Amount { order_amount, .. } => {
            if row.instrument.market() != Market::Us
                || row.side != OrderSide::Buy
                || row.order_type != BrokerOrderType::Market
                || order_amount.currency() != Currency::Usd
                || order_amount.amount().as_decimal() <= rust_decimal::Decimal::ZERO
            {
                return Err(BrokerOrderError::InvalidAmountOrder);
            }
            Ok(UnfilledQuantity::UnknownAmountOrder)
        }
    }
}
fn validate_prices_and_amount(
    row: &BrokerOrderRow,
    currency: Currency,
) -> Result<(), BrokerOrderError> {
    if row
        .average_filled_price
        .is_some_and(|price| price.as_money().currency() != currency)
    {
        return Err(BrokerOrderError::CurrencyMismatch);
    }
    if let BrokerOrderPrice::Known(price) = row.price
        && price.as_money().currency() != currency
    {
        return Err(BrokerOrderError::CurrencyMismatch);
    }
    if let BrokerOrderMoney::Known(amount) = row.filled_amount {
        if amount.currency() != currency {
            return Err(BrokerOrderError::CurrencyMismatch);
        }
        if amount.amount().as_decimal() < rust_decimal::Decimal::ZERO {
            return Err(BrokerOrderError::NegativeFilledAmount);
        }
    }
    Ok(())
}
