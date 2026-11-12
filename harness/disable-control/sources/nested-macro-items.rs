#[expect(clippy::must_use_candidate, reason = "항목 하나")]
pub fn build() {
    declare_quotes! {
        impl Book {}
    }
}
