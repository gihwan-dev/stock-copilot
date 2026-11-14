use crate::{BuyQuantityPlan, Price};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuyPlanOrderPriceKind {
    Valid,
    RecalculateRequired,
}
#[derive(Debug, PartialEq, Eq)]
pub enum BuyPlanOrderPrice<'plan> {
    Valid(PriceBoundBuyPlan<'plan>),
    RecalculateRequired,
}
#[derive(PartialEq, Eq)]
pub struct PriceBoundBuyPlan<'plan> {
    plan: &'plan BuyQuantityPlan,
    price: Price,
}
impl std::fmt::Debug for PriceBoundBuyPlan<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PriceBoundBuyPlan(..)")
    }
}
impl<'plan> PriceBoundBuyPlan<'plan> {
    #[must_use]
    pub const fn plan(&self) -> &'plan BuyQuantityPlan {
        self.plan
    }
    #[must_use]
    pub const fn price(&self) -> Price {
        self.price
    }
}
impl<'plan> BuyPlanOrderPrice<'plan> {
    #[must_use]
    pub fn into_valid(self) -> Option<PriceBoundBuyPlan<'plan>> {
        match self {
            Self::Valid(value) => Some(value),
            Self::RecalculateRequired => None,
        }
    }
    #[must_use]
    pub const fn kind(&self) -> BuyPlanOrderPriceKind {
        match self {
            Self::Valid(_) => BuyPlanOrderPriceKind::Valid,
            Self::RecalculateRequired => BuyPlanOrderPriceKind::RecalculateRequired,
        }
    }
}
#[must_use]
pub fn check_buy_plan_order_price(plan: &BuyQuantityPlan, price: Price) -> BuyPlanOrderPrice<'_> {
    let limit = plan.price().price().as_money();
    let proposed = price.as_money();
    if proposed.currency() != limit.currency()
        || proposed.amount().as_decimal() > limit.amount().as_decimal()
    {
        BuyPlanOrderPrice::RecalculateRequired
    } else {
        BuyPlanOrderPrice::Valid(PriceBoundBuyPlan { plan, price })
    }
}
