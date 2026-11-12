impl Ledger {
    #[expect(clippy::unused_self, reason = "메서드 하나")]
    pub fn label(&self) -> Label {
        Label::ledger()
    }
}

pub trait Port {
    #[expect(clippy::must_use_candidate, reason = "트레이트 메서드 하나")]
    fn read(&self) -> Reading;
}
