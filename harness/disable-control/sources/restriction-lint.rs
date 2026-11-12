#[expect(clippy::absolute_paths, reason = "restriction 그룹")]
pub fn now(clock: &Clock) -> Instant {
    clock.instant()
}
