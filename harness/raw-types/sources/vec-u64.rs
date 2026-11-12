pub trait Ledger {
    fn ids(&self) -> Vec<u64>;
}
