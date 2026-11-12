pub fn allowed(order: &Order) -> std::net::Ipv4Addr {
    order.address()
}
