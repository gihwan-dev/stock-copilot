pub fn order(price: Price) -> Order {
    Order::new(price)
}

#[cfg(test)]
mod tests {
    fn helper(value: u64) -> bool {
        value == 1
    }
}
