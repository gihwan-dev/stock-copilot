pub fn notify(notifier: &Notifier) {
    #[expect(clippy::semicolon_if_nothing_returned, reason = "식 문장 하나")]
    notifier.send()
}
