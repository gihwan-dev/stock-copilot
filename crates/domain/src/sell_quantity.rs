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
    currency: crate::Currency,
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
    pub(crate) const fn currency(&self) -> crate::Currency {
        self.currency
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
    match holding.instrument().market() {
        Market::Kr if holding.quantity().as_decimal().normalize().scale() != 0 => {
            return Err(NoSellQuantity::FractionalDomestic);
        }
        Market::Kr | Market::Us | Market::Unknown(_) => {}
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
        | (Market::Kr | Market::Us | Market::Unknown(_), SellQuantityMode::Whole) => 0,
    };
    let bounded = desired.min(held).min(fact.quantity().as_decimal());
    let scale = bounded.scale().min(precision);
    let coefficient =
        (scale..bounded.scale()).fold(bounded.mantissa(), |digits, _place| digits.wrapping_div(10));
    let value = rust_decimal::Decimal::from_i128_with_scale(coefficient, scale);
    let quantity = Quantity::try_new(value)
        .map_err(|_error| NoSellQuantity::Arithmetic)
        .and_then(|quantity_value| {
            SellOrderQuantity::try_new(quantity_value)
                .map_err(|_error| NoSellQuantity::BelowMinimumUnit)
        })?;
    Ok(SellQuantityPlan {
        account: account.clone(),
        currency: holding.currency(),
        instrument: holding.instrument().clone(),
        quantity,
    })
}
