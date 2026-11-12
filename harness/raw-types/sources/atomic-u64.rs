use std::sync::atomic::AtomicU64;

pub fn counter(order: &Order) -> &AtomicU64 {
    order.counter()
}
