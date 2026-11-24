use crate::{BrokerRead, Clock, PortError, SystemPortError};
use domain::{
    AccountId, BrokerOrder, BrokerOrderCoverage, ClosedOrderCompleteness, LocalSubmission,
    OpenOrderCoverage, ReconciliationReport, TimeInterval, reconcile_orders,
};
use std::collections::BTreeMap;
use std::future::Future;

pub trait SubmissionLedger: Send + Sync {
    fn submissions_between(
        &self,
        window: TimeInterval,
    ) -> impl Future<Output = Result<Vec<LocalSubmission>, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileOrdersError {
    Ledger(SystemPortError),
    Broker(PortError),
}

#[derive(Debug)]
pub struct ReconcileSubmittedOrders<'a, Ledger, Broker, Time> {
    ledger: &'a Ledger,
    broker: &'a Broker,
    clock: &'a Time,
}

impl<'a, Ledger, Broker, Time> ReconcileSubmittedOrders<'a, Ledger, Broker, Time>
where
    Ledger: SubmissionLedger,
    Broker: BrokerRead,
    Time: Clock,
{
    #[must_use]
    pub const fn new(ledger: &'a Ledger, broker: &'a Broker, clock: &'a Time) -> Self {
        Self {
            ledger,
            broker,
            clock,
        }
    }

    pub async fn execute(
        &self,
        account: &AccountId,
        window: TimeInterval,
    ) -> Result<ReconciliationReport, ReconcileOrdersError> {
        let submissions = self
            .ledger
            .submissions_between(window)
            .await
            .map_err(ReconcileOrdersError::Ledger)?;
        let open = self
            .broker
            .open_orders(account)
            .await
            .map_err(ReconcileOrdersError::Broker)?;
        let closed = self
            .broker
            .closed_orders(account, window)
            .await
            .map_err(ReconcileOrdersError::Broker)?;
        let coverage = match (open.coverage(), closed.orders().completeness()) {
            (OpenOrderCoverage::Complete, ClosedOrderCompleteness::Complete) => {
                BrokerOrderCoverage::Complete
            }
            (OpenOrderCoverage::Complete | OpenOrderCoverage::Incomplete, _) => {
                BrokerOrderCoverage::Incomplete
            }
        };
        let mut merged: BTreeMap<_, BrokerOrder> = BTreeMap::new();
        for order in closed
            .orders()
            .orders()
            .values()
            .chain(open.orders().values())
        {
            let _previous = merged.insert(order.order_id().clone(), order.clone());
        }
        let orders: Vec<BrokerOrder> = merged.into_values().collect();
        Ok(reconcile_orders(
            &submissions,
            &orders,
            coverage,
            self.clock.now(),
        ))
    }
}
