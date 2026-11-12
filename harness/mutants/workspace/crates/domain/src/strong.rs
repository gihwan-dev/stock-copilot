pub fn double(value: u32) -> u32 {
    value.saturating_mul(2)
}

#[cfg(test)]
mod tests {
    #[test]
    fn doubles_three() {
        assert_eq!(super::double(3), 6);
    }
}
