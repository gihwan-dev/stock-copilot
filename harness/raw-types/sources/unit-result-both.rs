pub fn submit(order: Order) -> Result<(), ()> {
    order.check()
}
