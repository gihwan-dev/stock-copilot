pub fn settle(ledger: &mut Ledger) {
    #[expect(clippy::redundant_clone, reason = "블록 전체")]
    if ledger.open() {
        ledger.close();
    }
}
