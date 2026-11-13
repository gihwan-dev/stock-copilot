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
    UsMarketAmountBuy {
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
            | Self::UsMarketAmountBuy { .. } => Market::Us,
        }
    }

    #[must_use]
    pub fn side(&self) -> OrderSide {
        match self {
            Self::KrLimit { side, .. }
            | Self::KrMarket { side, .. }
            | Self::UsLimit { side, .. } => *side,
            Self::UsMarketBuy { .. } | Self::UsMarketAmountBuy { .. } => OrderSide::Buy,
            Self::UsMarketSell { .. } => OrderSide::Sell,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderIntentError {
    UnknownMarket,
    MarketMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOrderInstrument {
    symbol: Symbol,
    market: Market,
}

impl ResolvedOrderInstrument {
    pub fn from_master_lookup_result(
        symbol: Symbol,
        market: Market,
    ) -> Result<Self, OrderIntentError> {
        match market {
            Market::Kr | Market::Us => Ok(Self { symbol, market }),
            Market::Unknown(_) => Err(OrderIntentError::UnknownMarket),
        }
    }

    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    #[must_use]
    pub fn market(&self) -> Market {
        self.market.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderIntent {
    account_id: AccountId,
    instrument: ResolvedOrderInstrument,
    client_order_id: ClientOrderId,
    terms: OrderTerms,
}

impl OrderIntent {
    pub fn new(
        account_id: AccountId,
        instrument: ResolvedOrderInstrument,
        client_order_id: ClientOrderId,
        terms: OrderTerms,
    ) -> Result<Self, OrderIntentError> {
        if instrument.market() != terms.market() {
            return Err(OrderIntentError::MarketMismatch);
        }
        Ok(Self {
            account_id,
            instrument,
            client_order_id,
            terms,
        })
    }

    #[must_use]
    pub fn account_id(&self) -> &AccountId {
        &self.account_id
    }

    #[must_use]
    pub fn symbol(&self) -> &Symbol {
        self.instrument.symbol()
    }

    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
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
        SellOrderQuantity, Symbol, UsOrderPrice, WholeOrderQuantity,
    };
    use crate::{Currency, NumericInput, Price, Quantity};

    fn allowed_terms(side: OrderSide) -> [(OrderTerms, Market); 4] {
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
                let instrument = crate::ResolvedOrderInstrument::from_master_lookup_result(
                    symbol.clone(),
                    market.clone(),
                )
                .expect("조회 결과");
                let intent =
                    OrderIntent::new(account.clone(), instrument.clone(), id.clone(), terms)
                        .expect("시장 일치");
                assert_eq!(intent.instrument(), &instrument);
                assert_eq!(instrument.symbol(), &symbol);
                assert_eq!(instrument.market(), market);
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

    mod review_regressions {
        use crate::{
            AccountId, ClientOrderId, Currency, KrOrderPrice, Market, MarketCode, NumericInput,
            OrderIntent, OrderIntentError, OrderSide, OrderTerms, Price, Quantity,
            ResolvedOrderInstrument, Symbol, UsOrderPrice, WholeOrderQuantity,
        };

        #[test]
        fn master_lookup_market_must_match_order_terms_and_unknown_is_rejected() {
            let quantity = WholeOrderQuantity::try_new(
                Quantity::parse(&NumericInput::try_new("10").expect("입력")).expect("수량"),
            )
            .expect("주문");
            let kr = OrderTerms::KrLimit {
                side: OrderSide::Buy,
                quantity,
                price: KrOrderPrice::try_new(
                    Price::parse(
                        &NumericInput::try_new("70000").expect("입력"),
                        Currency::Krw,
                    )
                    .expect("가격"),
                )
                .expect("주문"),
            };
            let us = OrderTerms::UsLimit {
                side: OrderSide::Buy,
                quantity,
                price: UsOrderPrice::try_new(
                    Price::parse(
                        &NumericInput::try_new("70000").expect("입력"),
                        Currency::Usd,
                    )
                    .expect("가격"),
                )
                .expect("주문"),
            };
            for (symbol, market, terms) in [("AAPL", Market::Us, kr), ("005930", Market::Kr, us)] {
                let instrument = ResolvedOrderInstrument::from_master_lookup_result(
                    Symbol::try_new(symbol).expect("종목"),
                    market,
                )
                .expect("조회 결과");
                assert_eq!(
                    OrderIntent::new(
                        AccountId::new(123_456),
                        instrument,
                        ClientOrderId::try_new("fake-key").expect("키"),
                        terms
                    ),
                    Err(OrderIntentError::MarketMismatch)
                );
            }
            assert_eq!(
                ResolvedOrderInstrument::from_master_lookup_result(
                    Symbol::try_new("FAKE").expect("종목"),
                    Market::new(MarketCode::try_new("UNKNOWN").expect("코드"))
                ),
                Err(OrderIntentError::UnknownMarket)
            );
        }
    }
    mod amount_buy_tests {
        use crate::{
            AccountId, Amount, AmountInput, ClientOrderId, Currency, Market, Money, OrderIntent,
            OrderSide, OrderTerms, ResolvedOrderInstrument, Symbol, UsdOrderAmount,
        };

        #[test]
        fn amount_order_is_us_market_buy_only() {
            let amount = UsdOrderAmount::try_new(Money::new(
                Amount::parse(&AmountInput::new("100.50")).expect("금액"),
                Currency::Usd,
            ))
            .expect("주문");
            let terms = OrderTerms::UsMarketAmountBuy { amount };
            let instrument = ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("AAPL").expect("종목"),
                Market::Us,
            )
            .expect("조회");
            let intent = OrderIntent::new(
                AccountId::new(123_456),
                instrument,
                ClientOrderId::try_new("fake-amount-key").expect("키"),
                terms,
            )
            .expect("시장");
            assert_eq!(intent.market(), Market::Us);
            assert_eq!(intent.side(), OrderSide::Buy);
            assert_eq!(intent.terms(), &OrderTerms::UsMarketAmountBuy { amount });
        }
    }
}
