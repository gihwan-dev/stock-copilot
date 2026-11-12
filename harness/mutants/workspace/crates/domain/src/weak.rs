pub fn is_positive(value: i32) -> bool {
    value > 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn one_is_positive() {
        assert!(super::is_positive(1), "one");
    }
}
