#[expect(clippy::must_use_candidate, reason = "extern 블록 전체")]
unsafe extern "C" {
    fn native(value: Native) -> Native;
}
