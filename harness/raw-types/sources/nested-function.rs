pub fn outer(order: Order) -> Order {
    fn inner(code: u8) -> Code {
        Code::from(code)
    }
    order
}
