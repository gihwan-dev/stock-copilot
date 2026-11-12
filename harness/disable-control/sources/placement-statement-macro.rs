pub fn log(logger: &Logger) {
    #[expect(clippy::must_use_candidate, reason = "문장 매크로")]
    record!(logger, "event");
}
