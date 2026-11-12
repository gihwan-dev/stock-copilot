pub fn submit(value: u32) -> u32 {
    value.saturating_add(1)
}

pub fn defensive(value: u32) -> u32 {
    value.saturating_sub(1)
}

pub fn forgotten(value: u32) -> u32 {
    value.saturating_mul(5)
}

#[cfg(test)]
mod tests {
    #[test]
    fn submits() {
        assert_eq!(super::submit(1), 2);
    }
}
