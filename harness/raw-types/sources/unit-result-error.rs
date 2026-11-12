pub fn submit(order: Order) -> Result<Submission, ()> {
    order.submit()
}
