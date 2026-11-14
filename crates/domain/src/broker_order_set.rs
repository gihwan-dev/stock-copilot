use crate::{
    AccountId, BrokerOrder, BrokerOrderError, BrokerOrderId, BrokerOrderRow, Market,
    OrderOpenState, OrderSide, ResolvedOrderInstrument, Timestamp,
};
use std::{collections::BTreeMap, fmt};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenOrderCoverage {
    Complete,
    Incomplete,
}
#[derive(Clone, PartialEq, Eq)]
pub struct BrokerOrderSet {
    account_id: AccountId,
    orders: BTreeMap<BrokerOrderId, BrokerOrder>,
    coverage: OpenOrderCoverage,
    observed_at: Timestamp,
}
impl BrokerOrderSet {
    pub fn from_open_lookup_result(
        account_id: AccountId,
        rows: Vec<BrokerOrderRow>,
        coverage: OpenOrderCoverage,
        observed_at: Timestamp,
    ) -> Result<Self, BrokerOrderError> {
        let mut unique = BTreeMap::new();
        for row in rows {
            if row.account_id != account_id {
                return Err(BrokerOrderError::AccountMismatch);
            }
            if coverage == OpenOrderCoverage::Complete
                && row.status.open_state() == OrderOpenState::Closed
            {
                return Err(BrokerOrderError::ClosedInOpenResult);
            }
            let order = BrokerOrder::from_lookup_result(row)?;
            if order.observed_at() != observed_at {
                return Err(BrokerOrderError::ObservationMismatch);
            }
            if unique.insert(order.order_id().clone(), order).is_some() {
                return Err(BrokerOrderError::DuplicateOrderId);
            }
        }
        Ok(Self {
            account_id,
            orders: unique,
            coverage,
            observed_at,
        })
    }
    #[must_use]
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }
    #[must_use]
    pub fn orders(&self) -> &BTreeMap<BrokerOrderId, BrokerOrder> {
        &self.orders
    }
    #[must_use]
    pub fn coverage(&self) -> OpenOrderCoverage {
        self.coverage
    }
    #[must_use]
    pub fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
}
impl fmt::Debug for BrokerOrderSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrderSet").finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OppositeOpenOrder {
    Present,
    Absent,
    NotApplicable,
    DifferentAccount,
    Incomplete,
}
#[must_use]
pub fn opposite_open_order(
    fact: &BrokerOrderSet,
    account: &AccountId,
    instrument: &ResolvedOrderInstrument,
    side: OrderSide,
) -> OppositeOpenOrder {
    if fact.account_id() != account {
        return OppositeOpenOrder::DifferentAccount;
    }
    if instrument.market() != Market::Kr {
        return OppositeOpenOrder::NotApplicable;
    }
    if fact.orders.values().any(|order| {
        order.instrument() == instrument
            && order.side() != side
            && order.status().open_state() == OrderOpenState::Possible
    }) {
        return OppositeOpenOrder::Present;
    }
    match fact.coverage {
        OpenOrderCoverage::Complete => OppositeOpenOrder::Absent,
        OpenOrderCoverage::Incomplete => OppositeOpenOrder::Incomplete,
    }
}
