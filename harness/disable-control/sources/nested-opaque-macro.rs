#[expect(clippy::must_use_candidate, reason = "항목 하나")]
pub fn build() {
    proptest! {
        fn quantity(value in 0..100) {}
    }
}
