pub fn on_fill(callback: &dyn Fn(i64) -> bool) -> Subscription {
    Subscription::new(callback)
}
