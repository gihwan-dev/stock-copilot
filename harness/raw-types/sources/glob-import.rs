use std::collections::*;

pub fn index(orders: &Order) -> HashMap<Code, Order> {
    orders.index()
}
