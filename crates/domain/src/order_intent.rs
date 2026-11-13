use crate::{
    AccountId, ClientOrderId, KrOrderPrice, Market, SellOrderQuantity, Symbol, UsOrderPrice,
    UsdOrderAmount, WholeOrderQuantity,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderTerms {
    KrLimit {
        side: OrderSide,
        quantity: WholeOrderQuantity,
        price: KrOrderPrice,
    },
    KrMarket {
        side: OrderSide,
        quantity: WholeOrderQuantity,
    },
    UsLimit {
        side: OrderSide,
        quantity: WholeOrderQuantity,
        price: UsOrderPrice,
    },
    UsMarketBuy {
        quantity: WholeOrderQuantity,
    },
    UsMarketSell {
        quantity: SellOrderQuantity,
    },
    UsMarketAmount {
        side: OrderSide,
        amount: UsdOrderAmount,
    },
}

impl OrderTerms {
    #[must_use]
    pub fn market(&self) -> Market {
        match self {
            Self::KrLimit { .. } | Self::KrMarket { .. } => Market::Kr,
            Self::UsLimit { .. }
            | Self::UsMarketBuy { .. }
            | Self::UsMarketSell { .. }
            | Self::UsMarketAmount { .. } => Market::Us,
        }
    }

    #[must_use]
    pub fn side(&self) -> OrderSide {
        match self {
            Self::KrLimit { side, .. }
            | Self::KrMarket { side, .. }
            | Self::UsLimit { side, .. }
            | Self::UsMarketAmount { side, .. } => *side,
            Self::UsMarketBuy { .. } => OrderSide::Buy,
            Self::UsMarketSell { .. } => OrderSide::Sell,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderIntent {
    account_id: AccountId,
    symbol: Symbol,
    client_order_id: ClientOrderId,
    terms: OrderTerms,
}

impl OrderIntent {
    #[must_use]
    pub fn new(
        account_id: AccountId,
        symbol: Symbol,
        client_order_id: ClientOrderId,
        terms: OrderTerms,
    ) -> Self {
        Self {
            account_id,
            symbol,
            client_order_id,
            terms,
        }
    }

    #[must_use]
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    #[must_use]
    pub fn client_order_id(&self) -> &ClientOrderId {
        &self.client_order_id
    }

    #[must_use]
    pub fn terms(&self) -> &OrderTerms {
        &self.terms
    }

    #[must_use]
    pub fn market(&self) -> Market {
        self.terms.market()
    }

    #[must_use]
    pub fn side(&self) -> OrderSide {
        self.terms.side()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AccountId, ClientOrderId, KrOrderPrice, Market, OrderIntent, OrderSide, OrderTerms,
        SellOrderQuantity, Symbol, UsOrderPrice, UsdOrderAmount, WholeOrderQuantity,
    };
    use crate::{Amount, AmountInput, Currency, Money, NumericInput, Price, Quantity};

    fn allowed_terms(side: OrderSide) -> [(OrderTerms, Market); 5] {
        let quantity = Quantity::parse(&NumericInput::try_new("2").expect("길이")).expect("수량");
        let whole = WholeOrderQuantity::try_new(quantity).expect("정수");
        let sell = SellOrderQuantity::try_new(quantity).expect("매도");
        let kr = KrOrderPrice::try_new(
            Price::parse(
                &NumericInput::try_new("70000").expect("길이"),
                Currency::Krw,
            )
            .expect("가격"),
        )
        .expect("원화");
        let us = UsOrderPrice::try_new(
            Price::parse(
                &NumericInput::try_new("100.50").expect("길이"),
                Currency::Usd,
            )
            .expect("가격"),
        )
        .expect("달러");
        let amount = UsdOrderAmount::try_new(Money::new(
            Amount::parse(&AmountInput::new("100.5")).expect("금액"),
            Currency::Usd,
        ))
        .expect("달러");
        [
            (
                OrderTerms::KrLimit {
                    side,
                    quantity: whole,
                    price: kr,
                },
                Market::Kr,
            ),
            (
                OrderTerms::KrMarket {
                    side,
                    quantity: whole,
                },
                Market::Kr,
            ),
            (
                OrderTerms::UsLimit {
                    side,
                    quantity: whole,
                    price: us,
                },
                Market::Us,
            ),
            (OrderTerms::UsMarketAmount { side, amount }, Market::Us),
            (
                match side {
                    OrderSide::Buy => OrderTerms::UsMarketBuy { quantity: whole },
                    OrderSide::Sell => OrderTerms::UsMarketSell { quantity: sell },
                },
                Market::Us,
            ),
        ]
    }

    #[test]
    fn every_allowed_day_cell_and_all_intent_fields_are_readable() {
        for side in [OrderSide::Buy, OrderSide::Sell] {
            for (terms, market) in allowed_terms(side) {
                let account = AccountId::new(123_456);
                let symbol = Symbol::try_new("FAKE").expect("종목");
                let id = ClientOrderId::try_new("fake-order-001").expect("주문 키");
                let intent = OrderIntent::new(account.clone(), symbol.clone(), id.clone(), terms);
                assert_eq!(intent.account_id(), &account);
                assert_eq!(intent.symbol(), &symbol);
                assert_eq!(intent.client_order_id(), &id);
                assert_eq!(intent.terms(), &terms);
                assert_eq!(intent.market(), market);
                assert_eq!(intent.side(), side);
                assert!(format!("{intent:?}").contains("AccountId(123456)"));
            }
        }
    }

    #[test]
    fn fractional_us_market_sell_is_allowed() {
        let fraction =
            Quantity::parse(&NumericInput::try_new("0.123456").expect("길이")).expect("수량");
        let terms = OrderTerms::UsMarketSell {
            quantity: SellOrderQuantity::try_new(fraction).expect("소수"),
        };
        assert_eq!(terms.market(), Market::Us);
        assert_eq!(terms.side(), OrderSide::Sell);
    }
}
