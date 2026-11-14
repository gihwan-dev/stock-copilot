use crate::{DisplayMoney, Money};

#[derive(Clone, PartialEq, Eq)]
pub struct MinimumBuyUnit {
    required: DisplayMoney,
    target: DisplayMoney,
}
impl std::fmt::Debug for MinimumBuyUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MinimumBuyUnit(..)")
    }
}
impl MinimumBuyUnit {
    #[must_use]
    pub const fn required(&self) -> &DisplayMoney {
        &self.required
    }
    #[must_use]
    pub const fn target(&self) -> &DisplayMoney {
        &self.target
    }
}
pub(crate) fn minimum_unit(required: Money, target: Money) -> MinimumBuyUnit {
    MinimumBuyUnit {
        required: crate::money_display::buy_minimum_money_display(required),
        target: crate::money_display::buy_minimum_money_display(target),
    }
}
