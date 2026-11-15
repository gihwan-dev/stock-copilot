use crate::{CalculatedLimitPrice, Market, OrderSide, Price, SellQuantityPlan};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarketOrderPolicy {
    #[default]
    Disabled,
    Enabled,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellOrderCalculationError {
    DifferentDirection,
    DifferentInstrument,
    FractionalLimit,
    MarketDisabled,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SellExecutionPrice {
    Limit(Price),
    Market,
}
impl std::fmt::Debug for SellExecutionPrice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SellExecutionPrice(..)")
    }
}
#[derive(PartialEq, Eq)]
pub struct CalculatedSellOrder {
    plan: SellQuantityPlan,
    price: SellExecutionPrice,
}
impl std::fmt::Debug for CalculatedSellOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CalculatedSellOrder(..)")
    }
}
impl CalculatedSellOrder {
    #[must_use]
    pub const fn plan(&self) -> &SellQuantityPlan {
        &self.plan
    }
    #[must_use]
    pub const fn price(&self) -> SellExecutionPrice {
        self.price
    }
}
pub fn bind_sell_limit(
    plan: SellQuantityPlan,
    price: &CalculatedLimitPrice,
) -> Result<CalculatedSellOrder, SellOrderCalculationError> {
    if price.side() != OrderSide::Sell {
        return Err(SellOrderCalculationError::DifferentDirection);
    }
    if plan.instrument().symbol() != price.instrument().symbol()
        || plan.instrument().market() != Market::from(*price.instrument().market())
    {
        return Err(SellOrderCalculationError::DifferentInstrument);
    }
    if plan.quantity().quantity().as_decimal().normalize().scale() != 0 {
        return Err(SellOrderCalculationError::FractionalLimit);
    }
    Ok(CalculatedSellOrder {
        plan,
        price: SellExecutionPrice::Limit(price.price()),
    })
}
pub fn bind_sell_market(
    plan: SellQuantityPlan,
    policy: MarketOrderPolicy,
) -> Result<CalculatedSellOrder, SellOrderCalculationError> {
    match policy {
        MarketOrderPolicy::Disabled => Err(SellOrderCalculationError::MarketDisabled),
        MarketOrderPolicy::Enabled => Ok(CalculatedSellOrder {
            plan,
            price: SellExecutionPrice::Market,
        }),
    }
}
