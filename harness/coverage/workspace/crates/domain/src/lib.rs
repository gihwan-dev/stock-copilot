pub mod below;
pub mod signal;

pub fn covered(value: u32) -> u32 {
    value.saturating_add(1)
}

#[cfg(test)]
mod tests {
    #[test]
    fn covers_domain() {
        assert_eq!(super::covered(1), 2);
        assert_eq!(super::below::half(1), 2);
        assert_eq!(super::signal::decide(1), 2);
    }
}
