use crate::{BrokerRead, DeliveryReport, EventBus, PortError};
use domain::{
    AccountId, BrokerOrder, BrokerOrderId, DomainEvent, OrderProgress, OrderProgressChange,
    order_progress, order_progress_change,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedOrder {
    pub account: AccountId,
    pub order_id: BrokerOrderId,
    pub last_seen: Option<OrderProgress>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderHistoryRow {
    order: BrokerOrder,
    progress: OrderProgress,
}

impl OrderHistoryRow {
    #[must_use]
    pub fn from_order(order: BrokerOrder) -> Self {
        let progress = order_progress(&order);
        Self { order, progress }
    }

    #[must_use]
    pub const fn order(&self) -> &BrokerOrder {
        &self.order
    }

    #[must_use]
    pub const fn progress(&self) -> OrderProgress {
        self.progress
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackedOrderUpdate {
    Refreshed {
        row: Box<OrderHistoryRow>,
        change: OrderProgressChange,
        delivery: Option<DeliveryReport>,
    },
    Unavailable {
        order_id: BrokerOrderId,
        last_seen: Option<OrderProgress>,
        error: PortError,
    },
}

#[derive(Debug)]
pub struct TrackSubmittedOrders<'a, Broker> {
    broker: &'a Broker,
    events: &'a EventBus,
}

impl<'a, Broker: BrokerRead> TrackSubmittedOrders<'a, Broker> {
    #[must_use]
    pub const fn new(broker: &'a Broker, events: &'a EventBus) -> Self {
        Self { broker, events }
    }

    pub async fn refresh(&self, tracked: &[TrackedOrder]) -> Vec<TrackedOrderUpdate> {
        let mut updates = Vec::with_capacity(tracked.len());
        for item in tracked {
            match item.last_seen {
                Some(OrderProgress::Filled | OrderProgress::Ended) => continue,
                Some(OrderProgress::Unfilled | OrderProgress::PartiallyFilled) | None => {}
            }
            let update = match self.broker.order(&item.account, &item.order_id).await {
                Ok(order) => self.refreshed(item, order),
                Err(error) => TrackedOrderUpdate::Unavailable {
                    order_id: item.order_id.clone(),
                    last_seen: item.last_seen,
                    error,
                },
            };
            updates.push(update);
        }
        updates
    }

    fn refreshed(&self, item: &TrackedOrder, order: BrokerOrder) -> TrackedOrderUpdate {
        let row = OrderHistoryRow::from_order(order);
        let change = order_progress_change(item.last_seen, row.progress());
        let delivery = match change {
            OrderProgressChange::BecameFilled => Some(
                self.events
                    .publish(DomainEvent::OrderFilled(item.account.clone())),
            ),
            OrderProgressChange::BecameEnded => Some(
                self.events
                    .publish(DomainEvent::OrderCancelled(item.account.clone())),
            ),
            OrderProgressChange::Unchanged | OrderProgressChange::Advanced => None,
        };
        TrackedOrderUpdate::Refreshed {
            row: Box::new(row),
            change,
            delivery,
        }
    }
}
