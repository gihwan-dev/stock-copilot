pub fn read(source: impl std::io::Read) -> Order {
    Order::read(source)
}

pub fn any(value: &dyn std::any::Any) -> Order {
    Order::any(value)
}
