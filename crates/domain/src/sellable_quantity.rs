use crate::{AccountId, Market, Quantity, ResolvedOrderInstrument, SellOrderQuantity, Timestamp};
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct SellableQuantityFact {
    account_id: AccountId,
    instrument: ResolvedOrderInstrument,
    quantity: Quantity,
    observed_at: Timestamp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellableQuantityError {
    FractionalDomestic,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SellableQuantityDecision {
    Allowed,
    Exceeded,
    DifferentFact,
}
impl SellableQuantityFact {
    pub fn from_lookup_result(
        account_id: AccountId,
        instrument: ResolvedOrderInstrument,
        quantity: Quantity,
        observed_at: Timestamp,
    ) -> Result<Self, SellableQuantityError> {
        if instrument.market() == Market::Kr && quantity.as_decimal().normalize().scale() != 0 {
            return Err(SellableQuantityError::FractionalDomestic);
        }
        Ok(Self {
            account_id,
            instrument,
            quantity,
            observed_at,
        })
    }
    #[must_use]
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }
    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    #[must_use]
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }
    #[must_use]
    pub fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
}
impl fmt::Debug for SellableQuantityFact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SellableQuantityFact")
            .finish_non_exhaustive()
    }
}
#[must_use]
pub fn check_sellable_quantity(
    fact: &SellableQuantityFact,
    account: &AccountId,
    instrument: &ResolvedOrderInstrument,
    requested: SellOrderQuantity,
) -> SellableQuantityDecision {
    if fact.account_id() != account || fact.instrument() != instrument {
        SellableQuantityDecision::DifferentFact
    } else if requested.quantity() > fact.quantity() {
        SellableQuantityDecision::Exceeded
    } else {
        SellableQuantityDecision::Allowed
    }
}
