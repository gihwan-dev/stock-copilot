#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellLimitIntentError {
    Direction,
    Account,
    Instrument,
    OrderValue(domain::OrderValueError),
    Intent(domain::OrderIntentError),
}

pub fn sell_limit_intent(
    plan: &domain::SellDraftQuantityPlan,
    limit: &domain::CalculatedLimitPrice,
    account: &domain::AccountId,
    id: domain::ClientOrderId,
) -> Result<domain::OrderIntent, SellLimitIntentError> {
    match limit.side() {
        domain::OrderSide::Sell => {}
        domain::OrderSide::Buy => return Err(SellLimitIntentError::Direction),
    }
    if plan.account_id() != account {
        return Err(SellLimitIntentError::Account);
    }
    let identity = limit.instrument();
    if identity.instrument_id() != plan.instrument_id()
        || identity.symbol() != plan.instrument().symbol()
        || domain::Market::from(*identity.market()) != plan.instrument().market()
    {
        return Err(SellLimitIntentError::Instrument);
    }
    let terms = match identity.market() {
        domain::KnownMarket::Kr => domain::OrderTerms::KrLimit {
            side: domain::OrderSide::Sell,
            quantity: plan.quantity(),
            price: domain::KrOrderPrice::try_new(limit.price())
                .map_err(SellLimitIntentError::OrderValue)?,
        },
        domain::KnownMarket::Us => domain::OrderTerms::UsLimit {
            side: domain::OrderSide::Sell,
            quantity: plan.quantity(),
            price: domain::UsOrderPrice::try_new(limit.price())
                .map_err(SellLimitIntentError::OrderValue)?,
        },
    };
    domain::OrderIntent::new(account.clone(), plan.instrument().clone(), id, terms)
        .map_err(SellLimitIntentError::Intent)
}
