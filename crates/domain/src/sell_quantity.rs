use crate::{
    AccountId, Holding, Market, ProfileWeight, Quantity, ResolvedOrderInstrument,
    SellOrderQuantity, SellableQuantityFact,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellTarget {
    All,
    Fraction(ProfileWeight),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellQuantityMode {
    Whole,
    FractionalMarket,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoSellQuantity {
    DifferentFact,
    FractionalDomestic,
    BelowMinimumUnit,
    Arithmetic,
}
#[derive(Clone, PartialEq, Eq)]
pub struct SellQuantityPlan {
    account: AccountId,
    instrument: ResolvedOrderInstrument,
    quantity: SellOrderQuantity,
}
impl std::fmt::Debug for SellQuantityPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SellQuantityPlan(..)")
    }
}
impl SellQuantityPlan {
    #[must_use]
    pub const fn account_id(&self) -> &AccountId {
        &self.account
    }
    #[must_use]
    pub const fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    #[must_use]
    pub const fn quantity(&self) -> SellOrderQuantity {
        self.quantity
    }
}
pub fn calculate_sell_quantity(
    account: &AccountId,
    holding: &Holding,
    fact: &SellableQuantityFact,
    target: SellTarget,
    mode: SellQuantityMode,
) -> Result<SellQuantityPlan, NoSellQuantity> {
    if fact.account_id() != account || fact.instrument() != holding.instrument() {
        return Err(NoSellQuantity::DifferentFact);
    }
    if holding.instrument().market() == Market::Kr
        && holding.quantity().as_decimal().normalize().scale() != 0
    {
        return Err(NoSellQuantity::FractionalDomestic);
    }
    let held = holding.quantity().as_decimal();
    let desired = match target {
        SellTarget::All => held,
        SellTarget::Fraction(weight) => crate::exact_arithmetic::multiply(
            crate::Amount::new(held),
            crate::Amount::new(weight.as_ratio().as_decimal()),
        )
        .map_err(|_error| NoSellQuantity::Arithmetic)?
        .as_decimal(),
    };
    let precision = match (holding.instrument().market(), mode) {
        (Market::Us, SellQuantityMode::FractionalMarket) => 6,
        (Market::Kr | Market::Unknown(_), SellQuantityMode::FractionalMarket)
        | (_, SellQuantityMode::Whole) => 0,
    };
    let value = desired
        .min(held)
        .min(fact.quantity().as_decimal())
        .round_dp_with_strategy(precision, rust_decimal::RoundingStrategy::ToZero);
    let quantity = Quantity::try_new(value)
        .map_err(|_error| NoSellQuantity::Arithmetic)
        .and_then(|bounded| {
            SellOrderQuantity::try_new(bounded).map_err(|_error| NoSellQuantity::BelowMinimumUnit)
        })?;
    Ok(SellQuantityPlan {
        account: account.clone(),
        instrument: holding.instrument().clone(),
        quantity,
    })
}
