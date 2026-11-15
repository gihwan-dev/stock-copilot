use crate::{CalculatedLimitPrice, Market, OrderSide, Price, SellQuantityPlan};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellOrderCalculationError {
    DifferentDirection,
    DifferentInstrument,
    FractionalLimit,
    MarketDisabled,
    OrderValue(crate::OrderValueError),
    Intent(crate::OrderIntentError),
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
    execution: SellOrderExecution,
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
        match &self.execution {
            SellOrderExecution::Limit(price) => SellExecutionPrice::Limit(*price),
            SellOrderExecution::Market(_policy) => SellExecutionPrice::Market,
        }
    }

    #[must_use]
    pub const fn market_policy(&self) -> Option<&crate::Policy> {
        match &self.execution {
            SellOrderExecution::Limit(_price) => None,
            SellOrderExecution::Market(policy) => Some(policy),
        }
    }
}

pub fn bind_sell_limit(
    plan: SellQuantityPlan,
    price: &CalculatedLimitPrice,
) -> Result<CalculatedSellOrder, SellOrderCalculationError> {
    match price.side() {
        OrderSide::Buy => return Err(SellOrderCalculationError::DifferentDirection),
        OrderSide::Sell => {}
    }
    if plan.instrument().symbol() != price.instrument().symbol() {
        return Err(SellOrderCalculationError::DifferentInstrument);
    }
    match (plan.instrument().market(), price.instrument().market()) {
        (Market::Kr, crate::KnownMarket::Kr) | (Market::Us, crate::KnownMarket::Us) => {}
        (Market::Kr, crate::KnownMarket::Us)
        | (Market::Us, crate::KnownMarket::Kr)
        | (Market::Unknown(_), crate::KnownMarket::Kr | crate::KnownMarket::Us) => {
            return Err(SellOrderCalculationError::DifferentInstrument);
        }
    }
    if plan.quantity().quantity().as_decimal().normalize().scale() != 0 {
        return Err(SellOrderCalculationError::FractionalLimit);
    }
    Ok(CalculatedSellOrder {
        plan,
        execution: SellOrderExecution::Limit(price.price()),
    })
}
pub fn bind_sell_market(
    plan: SellQuantityPlan,
    snapshot: crate::MarketOrderPolicy,
) -> Result<CalculatedSellOrder, SellOrderCalculationError> {
    let policy = crate::limit_price_policy::allow_market(snapshot)?;
    Ok(CalculatedSellOrder {
        plan,
        execution: SellOrderExecution::Market(policy),
    })
}
#[derive(PartialEq, Eq)]
enum SellOrderExecution {
    Limit(Price),
    Market(crate::Policy),
}

pub fn sell_order_into_intent(
    order: CalculatedSellOrder,
    client_order_id: crate::ClientOrderId,
) -> Result<crate::OrderIntent, SellOrderCalculationError> {
    let CalculatedSellOrder { plan, execution } = order;
    let calculated_terms = match execution {
        SellOrderExecution::Limit(price) => limit_terms(&plan, price),
        SellOrderExecution::Market(_enabled_policy) => match plan.currency() {
            crate::Currency::Krw => crate::WholeOrderQuantity::try_new(plan.quantity().quantity())
                .map(|quantity| crate::OrderTerms::KrMarket {
                    side: OrderSide::Sell,
                    quantity,
                })
                .map_err(SellOrderCalculationError::from),
            crate::Currency::Usd => Ok(crate::OrderTerms::UsMarketSell {
                quantity: plan.quantity(),
            }),
        },
    };
    let terms = calculated_terms?;
    crate::OrderIntent::new(
        plan.account_id().clone(),
        plan.instrument().clone(),
        client_order_id,
        terms,
    )
    .map_err(SellOrderCalculationError::from)
}

fn limit_terms(
    plan: &SellQuantityPlan,
    price: Price,
) -> Result<crate::OrderTerms, SellOrderCalculationError> {
    let quantity = crate::WholeOrderQuantity::try_new(plan.quantity().quantity())?;
    match plan.currency() {
        crate::Currency::Krw => crate::KrOrderPrice::try_new(price)
            .map(|value| crate::OrderTerms::KrLimit {
                side: OrderSide::Sell,
                quantity,
                price: value,
            })
            .map_err(SellOrderCalculationError::from),
        crate::Currency::Usd => crate::UsOrderPrice::try_new(price)
            .map(|value| crate::OrderTerms::UsLimit {
                side: OrderSide::Sell,
                quantity,
                price: value,
            })
            .map_err(SellOrderCalculationError::from),
    }
}
impl From<crate::OrderValueError> for SellOrderCalculationError {
    fn from(error: crate::OrderValueError) -> Self {
        Self::OrderValue(error)
    }
}
impl From<crate::OrderIntentError> for SellOrderCalculationError {
    fn from(error: crate::OrderIntentError) -> Self {
        Self::Intent(error)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn conversion_checks_internal_fractional_limit_state() {
        let (account, holding, fact) =
            crate::sell_quantity_review::tests::fixture(crate::KnownMarket::Us, "1.25", "2");
        let plan = crate::calculate_sell_quantity(
            &account,
            &holding,
            &fact,
            crate::SellTarget::All,
            crate::SellQuantityMode::FractionalMarket,
        )
        .expect("계산");
        let price = crate::orderbook::tests::price("99", crate::Currency::Usd);
        let order = super::CalculatedSellOrder {
            plan,
            execution: super::SellOrderExecution::Limit(price),
        };
        let id = crate::ClientOrderId::try_new("internal-limit-check").expect("키");
        assert_eq!(
            super::sell_order_into_intent(order, id),
            Err(super::SellOrderCalculationError::OrderValue(
                crate::OrderValueError::FractionalQuantity
            ))
        );
    }
}
