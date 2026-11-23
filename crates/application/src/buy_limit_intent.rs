#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuyLimitIntentError {
    Direction,
    Instrument,
    RecalculateRequired,
    OrderValue(domain::OrderValueError),
    Intent(domain::OrderIntentError),
}

pub fn buy_limit_intent(
    plan: &domain::BuyQuantityPlan,
    limit: &domain::CalculatedLimitPrice,
    account: domain::AccountId,
    id: domain::ClientOrderId,
) -> Result<domain::OrderIntent, BuyLimitIntentError> {
    if limit.side() != domain::OrderSide::Buy {
        return Err(BuyLimitIntentError::Direction);
    }
    let identity = limit.instrument();
    let task = plan.signal().facts().task();
    if identity.instrument_id() != task.volatility().instrument()
        || identity.symbol() != plan.instrument().symbol()
        || domain::Market::from(*identity.market()) != plan.instrument().market()
    {
        return Err(BuyLimitIntentError::Instrument);
    }
    let bound = domain::check_buy_plan_order_price(plan, limit.price())
        .into_valid()
        .ok_or(BuyLimitIntentError::RecalculateRequired)?;
    let terms = match identity.market() {
        domain::KnownMarket::Kr => domain::OrderTerms::KrLimit {
            side: domain::OrderSide::Buy,
            quantity: bound.plan().quantity(),
            price: domain::KrOrderPrice::try_new(bound.price())
                .map_err(BuyLimitIntentError::OrderValue)?,
        },
        domain::KnownMarket::Us => domain::OrderTerms::UsLimit {
            side: domain::OrderSide::Buy,
            quantity: bound.plan().quantity(),
            price: domain::UsOrderPrice::try_new(bound.price())
                .map_err(BuyLimitIntentError::OrderValue)?,
        },
    };
    domain::OrderIntent::new(account, plan.instrument().clone(), id, terms)
        .map_err(BuyLimitIntentError::Intent)
}
