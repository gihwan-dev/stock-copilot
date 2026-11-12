pub fn run(runner: &Runner) {
    #[expect(clippy::must_use_candidate, reason = "항목 하나")]
    runner.run(|| {
        mod hidden {}
    });
}
