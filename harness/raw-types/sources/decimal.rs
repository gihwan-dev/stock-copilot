use rust_decimal::Decimal;

pub fn price(order: &Order) -> Decimal {
    order.price()
}
