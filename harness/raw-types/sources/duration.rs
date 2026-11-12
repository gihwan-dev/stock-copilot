use std::time::Duration;

pub fn wait(timeout: Duration) -> Order {
    Order::after(timeout)
}
