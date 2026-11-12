pub trait BrokerRead {
    const LIMIT: u32;
    type Key: Into<String>;

    fn fetch(&self, id: u64) -> Holding;
}
