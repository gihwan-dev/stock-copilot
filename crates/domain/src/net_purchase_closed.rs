use crate::{
    AccountId, BrokerOrder, BrokerOrderId, BrokerOrderMoney, ClosedOrderCompletenessProof,
    NetPurchaseInputError, OrderOpenState, TimeInterval, Timestamp,
};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedOrderIncomplete {
    MissingPages,
    RejectedRows,
    RequestFailed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosedOrderCompleteness {
    Complete,
    Incomplete(ClosedOrderIncomplete),
}
#[derive(Clone, PartialEq, Eq)]
pub struct ClosedOrderSet {
    account: AccountId,
    orders: BTreeMap<BrokerOrderId, BrokerOrder>,
    covered: TimeInterval,
    observed: Timestamp,
    completeness: ClosedOrderCompleteness,
    proof: Option<ClosedOrderCompletenessProof>,
}
impl ClosedOrderSet {
    pub fn new(
        account: AccountId,
        orders: Vec<BrokerOrder>,
        covered: TimeInterval,
        observed: Timestamp,
        completeness: ClosedOrderCompleteness,
    ) -> Result<Self, NetPurchaseInputError> {
        Self::build(account, orders, covered, observed, completeness)
    }
    pub(crate) fn from_verified_proof(
        account: AccountId,
        orders: BTreeMap<BrokerOrderId, BrokerOrder>,
        covered: TimeInterval,
        observed: Timestamp,
        proof: ClosedOrderCompletenessProof,
    ) -> Self {
        let completeness = proof.completeness();
        Self {
            account,
            orders,
            covered,
            observed,
            completeness,
            proof: Some(proof),
        }
    }
    fn build(
        account: AccountId,
        orders: Vec<BrokerOrder>,
        covered: TimeInterval,
        observed: Timestamp,
        completeness: ClosedOrderCompleteness,
    ) -> Result<Self, NetPurchaseInputError> {
        if covered.end() > observed {
            return Err(NetPurchaseInputError::InvalidTime);
        }
        let mut unique = BTreeMap::new();
        for order in orders {
            validate_order(&order, &account, observed)?;
            if unique.insert(order.order_id().clone(), order).is_some() {
                return Err(NetPurchaseInputError::DuplicateOrder);
            }
        }
        Ok(Self {
            account,
            orders: unique,
            covered,
            observed,
            completeness,
            proof: None,
        })
    }
    #[must_use]
    pub const fn account_id(&self) -> &AccountId {
        &self.account
    }
    #[must_use]
    pub fn orders(&self) -> &BTreeMap<BrokerOrderId, BrokerOrder> {
        &self.orders
    }
    #[must_use]
    pub const fn covered(&self) -> TimeInterval {
        self.covered
    }
    #[must_use]
    pub const fn observed_at(&self) -> Timestamp {
        self.observed
    }
    #[must_use]
    pub const fn completeness(&self) -> ClosedOrderCompleteness {
        self.completeness
    }
    #[must_use]
    pub const fn completeness_proof(&self) -> Option<&ClosedOrderCompletenessProof> {
        self.proof.as_ref()
    }
}
impl fmt::Debug for ClosedOrderSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClosedOrderSet").finish_non_exhaustive()
    }
}
fn validate_order(
    order: &BrokerOrder,
    account: &AccountId,
    observed: Timestamp,
) -> Result<(), NetPurchaseInputError> {
    if order.account_id() != account {
        return Err(NetPurchaseInputError::AccountMismatch);
    }
    if order.status().open_state() != OrderOpenState::Closed {
        return Err(NetPurchaseInputError::NonClosedOrder);
    }
    if order.observed_at() != observed {
        return Err(NetPurchaseInputError::ObservationMismatch);
    }
    if order.ordered_at() > observed
        || order
            .filled_at()
            .is_some_and(|at| at < order.ordered_at() || at > observed)
    {
        return Err(NetPurchaseInputError::InvalidTime);
    }
    if order.filled_quantity().as_decimal().is_zero()
        && (order.filled_at().is_some()
            || match order.filled_amount() {
                BrokerOrderMoney::Known(amount) => !amount.amount().as_decimal().is_zero(),
                BrokerOrderMoney::Absent => false,
            })
    {
        return Err(NetPurchaseInputError::ZeroFillMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::net_purchase::tests::{facts, interval, money, row, time};
    use crate::{
        AccountId, BrokerOrder, BrokerOrderMoney, BrokerOrderStatus, ClosedOrderCompleteness,
        ClosedOrderSet, Currency, MasterCode, NetPurchaseInputError, OrderSide, Quantity,
    };
    use rust_decimal::Decimal;
    fn sealed(input: crate::BrokerOrderRow) -> BrokerOrder {
        BrokerOrder::from_lookup_result(input).expect("행")
    }
    fn seal(input: crate::BrokerOrderRow) -> Result<ClosedOrderSet, NetPurchaseInputError> {
        ClosedOrderSet::new(
            AccountId::new(7),
            vec![sealed(input)],
            interval(0, 300),
            time(300),
            ClosedOrderCompleteness::Complete,
        )
    }
    #[test]
    fn identity_status_duplicates_and_metadata() {
        let base = row("buy", OrderSide::Buy, "41", 150);
        let mut different = base.clone();
        different.account_id = AccountId::new(8);
        assert_eq!(seal(different), Err(NetPurchaseInputError::AccountMismatch));
        for status in [
            BrokerOrderStatus::Pending,
            BrokerOrderStatus::PendingCancel,
            BrokerOrderStatus::PendingReplace,
            BrokerOrderStatus::PartialFilled,
            BrokerOrderStatus::Unknown(MasterCode::try_new("NEW").expect("미지")),
        ] {
            let mut input = base.clone();
            input.status = status;
            assert_eq!(seal(input), Err(NetPurchaseInputError::NonClosedOrder));
        }
        let mut observed = base.clone();
        observed.observed_at = time(299);
        assert_eq!(
            seal(observed),
            Err(NetPurchaseInputError::ObservationMismatch)
        );
        let order = BrokerOrder::from_lookup_result(base).expect("행");
        assert_eq!(
            ClosedOrderSet::new(
                AccountId::new(7),
                vec![order.clone(), order],
                interval(0, 300),
                time(300),
                ClosedOrderCompleteness::Complete
            ),
            Err(NetPurchaseInputError::DuplicateOrder)
        );
        let orders = facts(vec![]);
        assert_eq!(orders.account_id(), &AccountId::new(7));
        assert!(orders.orders().is_empty());
        assert_eq!(orders.covered(), interval(0, 300));
        assert_eq!(orders.observed_at(), time(300));
        assert_eq!(format!("{orders:?}"), "ClosedOrderSet { .. }");
    }
    #[test]
    fn invalid_time_orders_are_rejected_before_any_exclusion() {
        for (ordered, filled) in [(151, 150), (110, 301), (301, 301)] {
            let mut input = row("time", OrderSide::Buy, "41", filled);
            input.ordered_at = time(ordered);
            assert_eq!(seal(input), Err(NetPurchaseInputError::InvalidTime));
        }
        assert_eq!(
            ClosedOrderSet::new(
                AccountId::new(7),
                vec![],
                interval(0, 301),
                time(300),
                ClosedOrderCompleteness::Complete
            ),
            Err(NetPurchaseInputError::InvalidTime)
        );
        let mut equal = row("equal", OrderSide::Buy, "23", 300);
        equal.ordered_at = time(300);
        seal(equal).expect("동시 시각 허용");
    }
    #[test]
    fn zero_quantity_with_execution_time_or_nonzero_total_is_inconsistent() {
        let mut input = row("zero", OrderSide::Buy, "0", 150);
        input.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("수량");
        assert_eq!(
            seal(input.clone()),
            Err(NetPurchaseInputError::ZeroFillMismatch)
        );
        input.filled_at = None;
        input.filled_amount = BrokerOrderMoney::Known(money("1", Currency::Krw));
        assert_eq!(seal(input), Err(NetPurchaseInputError::ZeroFillMismatch));
    }
}
