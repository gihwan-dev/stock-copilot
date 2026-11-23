use crate::{
    AccountId, Amount, ClientOrderId, KrOrderPrice, Market, Money, OrderIntent, OrderSide,
    OrderTerms, OrderValueError, OrderWireValue, Quantity, ResolvedOrderInstrument,
    SellOrderQuantity, Symbol, UsOrderPrice, UsdOrderAmount, WholeOrderQuantity,
};

#[derive(Clone, PartialEq, Eq)]
pub struct HistoricalDecimalText(String);
impl std::fmt::Debug for HistoricalDecimalText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoricalDecimalText([비공개])")
    }
}
impl HistoricalDecimalText {
    pub(crate) fn from_amount(amount: Amount) -> Self {
        Self(OrderWireValue::from_amount(amount).as_str().to_owned())
    }
    pub(crate) fn from_wire(wire: &OrderWireValue) -> Self {
        Self(wire.as_str().to_owned())
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HistoricalWholeQuantity(WholeOrderQuantity);
impl std::fmt::Debug for HistoricalWholeQuantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoricalWholeQuantity([비공개])")
    }
}
impl HistoricalWholeQuantity {
    pub fn try_new(quantity: Quantity) -> Result<Self, OrderValueError> {
        WholeOrderQuantity::try_new(quantity).map(Self)
    }
    #[must_use]
    pub fn display_text(self) -> HistoricalDecimalText {
        HistoricalDecimalText::from_wire(&self.0.wire_value())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HistoricalSellQuantity(SellOrderQuantity);
impl std::fmt::Debug for HistoricalSellQuantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoricalSellQuantity([비공개])")
    }
}
impl HistoricalSellQuantity {
    pub fn try_new(quantity: Quantity) -> Result<Self, OrderValueError> {
        SellOrderQuantity::try_new(quantity).map(Self)
    }
    #[must_use]
    pub fn display_text(self) -> HistoricalDecimalText {
        HistoricalDecimalText::from_wire(&self.0.wire_value())
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HistoricalOrderAmount(UsdOrderAmount);
impl std::fmt::Debug for HistoricalOrderAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoricalOrderAmount([비공개])")
    }
}
impl HistoricalOrderAmount {
    pub fn try_new(amount: Money) -> Result<Self, OrderValueError> {
        UsdOrderAmount::try_new(amount).map(Self)
    }
    #[must_use]
    pub fn display_text(self) -> HistoricalDecimalText {
        HistoricalDecimalText::from_wire(&self.0.wire_value())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoricalOrderTerms {
    KrLimit {
        side: OrderSide,
        quantity: HistoricalWholeQuantity,
        price: KrOrderPrice,
    },
    KrMarket {
        side: OrderSide,
        quantity: HistoricalWholeQuantity,
    },
    UsLimit {
        side: OrderSide,
        quantity: HistoricalWholeQuantity,
        price: UsOrderPrice,
    },
    UsMarketBuy {
        quantity: HistoricalWholeQuantity,
    },
    UsMarketSell {
        quantity: HistoricalSellQuantity,
    },
    UsMarketAmountBuy {
        amount: HistoricalOrderAmount,
    },
}
impl HistoricalOrderTerms {
    pub(crate) fn from_order_terms(terms: OrderTerms) -> Self {
        match terms {
            OrderTerms::KrLimit {
                side,
                quantity,
                price,
            } => Self::KrLimit {
                side,
                quantity: HistoricalWholeQuantity(quantity),
                price,
            },
            OrderTerms::KrMarket { side, quantity } => Self::KrMarket {
                side,
                quantity: HistoricalWholeQuantity(quantity),
            },
            OrderTerms::UsLimit {
                side,
                quantity,
                price,
            } => Self::UsLimit {
                side,
                quantity: HistoricalWholeQuantity(quantity),
                price,
            },
            OrderTerms::UsMarketBuy { quantity } => Self::UsMarketBuy {
                quantity: HistoricalWholeQuantity(quantity),
            },
            OrderTerms::UsMarketSell { quantity } => Self::UsMarketSell {
                quantity: HistoricalSellQuantity(quantity),
            },
            OrderTerms::UsMarketAmountBuy { amount } => Self::UsMarketAmountBuy {
                amount: HistoricalOrderAmount(amount),
            },
        }
    }
    pub(crate) fn into_order_terms(self) -> OrderTerms {
        match self {
            Self::KrLimit {
                side,
                quantity,
                price,
            } => OrderTerms::KrLimit {
                side,
                quantity: quantity.0,
                price,
            },
            Self::KrMarket { side, quantity } => OrderTerms::KrMarket {
                side,
                quantity: quantity.0,
            },
            Self::UsLimit {
                side,
                quantity,
                price,
            } => OrderTerms::UsLimit {
                side,
                quantity: quantity.0,
                price,
            },
            Self::UsMarketBuy { quantity } => OrderTerms::UsMarketBuy {
                quantity: quantity.0,
            },
            Self::UsMarketSell { quantity } => OrderTerms::UsMarketSell {
                quantity: quantity.0,
            },
            Self::UsMarketAmountBuy { amount } => {
                OrderTerms::UsMarketAmountBuy { amount: amount.0 }
            }
        }
    }
    #[must_use]
    pub fn side(self) -> OrderSide {
        self.into_order_terms().side()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct DraftOrderIntent(OrderIntent);
impl std::fmt::Debug for DraftOrderIntent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DraftOrderIntent").finish_non_exhaustive()
    }
}
impl DraftOrderIntent {
    pub(crate) fn from_intent(value: &OrderIntent) -> Self {
        Self(value.clone())
    }
    #[must_use]
    pub fn account_id(&self) -> &AccountId {
        self.0.account_id()
    }
    #[must_use]
    pub fn client_order_id(&self) -> &ClientOrderId {
        self.0.client_order_id()
    }
    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        self.0.instrument()
    }
    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        self.0.symbol()
    }
    #[must_use]
    pub fn market(&self) -> Market {
        self.0.market()
    }
    #[must_use]
    pub fn terms(&self) -> HistoricalOrderTerms {
        HistoricalOrderTerms::from_order_terms(*self.0.terms())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HistoricalOrderAmount, HistoricalOrderTerms, HistoricalSellQuantity,
        HistoricalWholeQuantity,
    };
    use crate::{
        Currency, KrOrderPrice, Money, NumericInput, OrderSide, OrderTerms, Price, Quantity,
        SellOrderQuantity, UsOrderPrice, UsdOrderAmount, WholeOrderQuantity,
    };

    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }
    fn price(value: &str, currency: Currency) -> Price {
        Price::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("가격")
    }
    fn cases() -> Vec<OrderTerms> {
        let whole = WholeOrderQuantity::try_new(quantity("3")).expect("정수");
        let fractional = SellOrderQuantity::try_new(quantity("2.345")).expect("소수");
        let kr = KrOrderPrice::try_new(price("1900", Currency::Krw)).expect("원화 가격");
        let us = UsOrderPrice::try_new(price("42.13", Currency::Usd)).expect("달러 가격");
        let amount = UsdOrderAmount::try_new(
            Money::parse(
                &NumericInput::try_new("77.25").expect("입력"),
                Currency::Usd,
            )
            .expect("달러"),
        )
        .expect("금액");
        vec![
            OrderTerms::KrLimit {
                side: OrderSide::Buy,
                quantity: whole,
                price: kr,
            },
            OrderTerms::KrMarket {
                side: OrderSide::Sell,
                quantity: whole,
            },
            OrderTerms::UsLimit {
                side: OrderSide::Sell,
                quantity: whole,
                price: us,
            },
            OrderTerms::UsMarketBuy { quantity: whole },
            OrderTerms::UsMarketSell {
                quantity: fractional,
            },
            OrderTerms::UsMarketAmountBuy { amount },
        ]
    }
    #[test]
    fn every_historical_order_condition_roundtrips_with_its_own_values() {
        for original in cases() {
            let historical = HistoricalOrderTerms::from_order_terms(original);
            assert_eq!(historical.side(), original.side());
            assert_eq!(historical.into_order_terms(), original);
        }
    }
    #[test]
    fn historical_quantity_and_amount_have_display_but_no_live_value_accessor() {
        let whole = HistoricalWholeQuantity::try_new(quantity("3"))
            .expect("정수")
            .display_text();
        assert_eq!(whole.as_str(), "3");
        assert_eq!(format!("{whole:?}"), "HistoricalDecimalText([비공개])");
        assert_eq!(
            HistoricalSellQuantity::try_new(quantity("2.345"))
                .expect("소수")
                .display_text()
                .as_str(),
            "2.345"
        );
        assert_eq!(
            HistoricalOrderAmount::try_new(
                Money::parse(
                    &NumericInput::try_new("77.25").expect("입력"),
                    Currency::Usd
                )
                .expect("금액")
            )
            .expect("달러")
            .display_text()
            .as_str(),
            "77.25"
        );
        assert_eq!(
            HistoricalWholeQuantity::try_new(quantity("0")),
            Err(crate::OrderValueError::NonPositive)
        );
        assert_eq!(
            HistoricalSellQuantity::try_new(quantity("0")),
            Err(crate::OrderValueError::NonPositive)
        );
    }
}
