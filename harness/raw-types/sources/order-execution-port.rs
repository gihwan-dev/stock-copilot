pub trait BrokerTrade {
    fn submit(&self, order: ApprovedOrder) -> Result<OrderNumber, String>;
}
