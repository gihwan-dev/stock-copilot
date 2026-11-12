pub mod order_gate;
pub mod other;

#[cfg(test)]
mod tests {
    #[test]
    fn gates() {
        assert_eq!(super::order_gate::open(1), 2);
    }
}
