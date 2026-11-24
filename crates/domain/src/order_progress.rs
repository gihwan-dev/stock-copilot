use crate::{BrokerOrder, BrokerOrderStatus, OrderOpenState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderProgress {
    Unfilled,
    PartiallyFilled,
    Filled,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderProgressChange {
    Unchanged,
    BecameFilled,
    BecameEnded,
    Advanced,
}

#[must_use]
pub fn order_progress(order: &BrokerOrder) -> OrderProgress {
    let has_fill = !order.filled_quantity().as_decimal().is_zero();
    match (order.status().open_state(), order.status()) {
        (OrderOpenState::Closed, BrokerOrderStatus::Filled) => OrderProgress::Filled,
        (OrderOpenState::Closed, _) => OrderProgress::Ended,
        (OrderOpenState::Possible, BrokerOrderStatus::PartialFilled) => {
            OrderProgress::PartiallyFilled
        }
        (OrderOpenState::Possible, _) if has_fill => OrderProgress::PartiallyFilled,
        (OrderOpenState::Possible, _) => OrderProgress::Unfilled,
    }
}

#[must_use]
pub fn order_progress_change(
    previous: Option<OrderProgress>,
    current: OrderProgress,
) -> OrderProgressChange {
    if previous == Some(current) {
        return OrderProgressChange::Unchanged;
    }
    match current {
        OrderProgress::Filled => OrderProgressChange::BecameFilled,
        OrderProgress::Ended => OrderProgressChange::BecameEnded,
        OrderProgress::Unfilled | OrderProgress::PartiallyFilled => OrderProgressChange::Advanced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net_purchase::tests::row;
    use crate::{BrokerOrderMoney, OrderSide, Quantity};
    use rust_decimal::Decimal;

    fn with(status: BrokerOrderStatus, filled: i64) -> BrokerOrder {
        let mut input = row("b1", OrderSide::Buy, "39", 150);
        input.status = status;
        input.filled_quantity = Quantity::try_new(Decimal::from(filled)).expect("체결");
        if filled == 0 {
            input.filled_at = None;
            input.filled_amount = BrokerOrderMoney::Absent;
            input.average_filled_price = None;
        }
        BrokerOrder::from_lookup_result(input).expect("주문")
    }

    #[test]
    fn four_states_follow_status_and_fill() {
        assert_eq!(
            order_progress(&with(BrokerOrderStatus::Pending, 0)),
            OrderProgress::Unfilled
        );
        assert_eq!(
            order_progress(&with(BrokerOrderStatus::PartialFilled, 3)),
            OrderProgress::PartiallyFilled
        );
        assert_eq!(
            order_progress(&with(BrokerOrderStatus::PendingCancel, 3)),
            OrderProgress::PartiallyFilled
        );
        assert_eq!(
            order_progress(&with(BrokerOrderStatus::Filled, 17)),
            OrderProgress::Filled
        );
        for ended in [
            BrokerOrderStatus::Canceled,
            BrokerOrderStatus::Rejected,
            BrokerOrderStatus::Replaced,
        ] {
            assert_eq!(order_progress(&with(ended, 0)), OrderProgress::Ended);
        }
    }

    #[test]
    fn change_reports_only_new_terminal_states() {
        assert_eq!(
            order_progress_change(Some(OrderProgress::Filled), OrderProgress::Filled),
            OrderProgressChange::Unchanged
        );
        assert_eq!(
            order_progress_change(Some(OrderProgress::Unfilled), OrderProgress::Filled),
            OrderProgressChange::BecameFilled
        );
        assert_eq!(
            order_progress_change(None, OrderProgress::Ended),
            OrderProgressChange::BecameEnded
        );
        assert_eq!(
            order_progress_change(None, OrderProgress::Unfilled),
            OrderProgressChange::Advanced
        );
    }
}
