use crate::{MasterCode, Money, Price, Quantity, Timestamp};
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BrokerOrderSize {
    Quantity(Quantity),
    Amount {
        reported_quantity: Quantity,
        order_amount: Money,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnfilledQuantity {
    Known(Quantity),
    UnknownAmountOrder,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BrokerOrderMoney {
    Absent,
    Known(Money),
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BrokerOrderPrice {
    Absent,
    Known(Price),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerOrderTime {
    Absent,
    Known(Timestamp),
}
impl fmt::Debug for BrokerOrderSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrderSize").finish_non_exhaustive()
    }
}
impl fmt::Debug for BrokerOrderMoney {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrderMoney").finish_non_exhaustive()
    }
}
impl fmt::Debug for BrokerOrderPrice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrderPrice").finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerOrderType {
    Limit,
    Market,
    Unknown(MasterCode),
}
impl BrokerOrderType {
    #[must_use]
    pub fn from_code(code: MasterCode) -> Self {
        match code.as_str() {
            "LIMIT" => Self::Limit,
            "MARKET" => Self::Market,
            _ => Self::Unknown(code),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrokerTimeInForce {
    Day,
    Cls,
    Opg,
    Unknown(MasterCode),
}
impl BrokerTimeInForce {
    #[must_use]
    pub fn from_code(code: MasterCode) -> Self {
        match code.as_str() {
            "DAY" => Self::Day,
            "CLS" => Self::Cls,
            "OPG" => Self::Opg,
            _ => Self::Unknown(code),
        }
    }
}
