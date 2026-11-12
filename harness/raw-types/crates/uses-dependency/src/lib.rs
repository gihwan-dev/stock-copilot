use dependency_domain::Port;

mod orders {
    pub struct Order;
}

pub fn price(order: &orders::Order) -> dependency_domain::Price {
    order.price()
}

pub fn port(source: &dyn Port) -> crate::orders::Order {
    source.order()
}

pub fn missing(order: &orders::Order) -> dependency_domain::Missing {
    order.missing()
}

pub fn own(order: &self::orders::Order) -> dependency_domain::Price {
    order.price()
}

pub mod nested {
    pub fn parent(order: &super::orders::Order) -> dependency_domain::Price {
        order.price()
    }
}
