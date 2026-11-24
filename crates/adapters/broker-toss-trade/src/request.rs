use domain::{ClientOrderId, LargeOrderConfirmation, OrderSide, OrderTerms, Symbol};
use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OrderCreateBody {
    client_order_id: String,
    symbol: String,
    side: &'static str,
    order_type: &'static str,
    time_in_force: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    quantity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order_amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    price: Option<String>,
    confirm_high_value_order: bool,
}

const fn side_code(side: OrderSide) -> &'static str {
    match side {
        OrderSide::Buy => "BUY",
        OrderSide::Sell => "SELL",
    }
}

impl OrderCreateBody {
    pub(crate) fn from_parts(
        symbol: &Symbol,
        terms: &OrderTerms,
        client_order_id: &ClientOrderId,
        large_order: LargeOrderConfirmation,
    ) -> Self {
        let (order_type, quantity, order_amount, price) = match *terms {
            OrderTerms::KrLimit {
                quantity, price, ..
            } => (
                "LIMIT",
                Some(quantity.wire_value()),
                None,
                Some(price.wire_value()),
            ),
            OrderTerms::UsLimit {
                quantity, price, ..
            } => (
                "LIMIT",
                Some(quantity.wire_value()),
                None,
                Some(price.wire_value()),
            ),
            OrderTerms::KrMarket { quantity, .. } | OrderTerms::UsMarketBuy { quantity } => {
                ("MARKET", Some(quantity.wire_value()), None, None)
            }
            OrderTerms::UsMarketSell { quantity } => {
                ("MARKET", Some(quantity.wire_value()), None, None)
            }
            OrderTerms::UsMarketAmountBuy { amount } => {
                ("MARKET", None, Some(amount.wire_value()), None)
            }
        };
        Self {
            client_order_id: client_order_id.as_str().to_owned(),
            symbol: symbol.as_str().to_owned(),
            side: side_code(terms.side()),
            order_type,
            time_in_force: "DAY",
            quantity: quantity.map(|value| value.as_str().to_owned()),
            order_amount: order_amount.map(|value| value.as_str().to_owned()),
            price: price.map(|value| value.as_str().to_owned()),
            confirm_high_value_order: match large_order {
                LargeOrderConfirmation::Required => true,
                LargeOrderConfirmation::NotRequired => false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OrderCreateBody;
    use domain::{
        ClientOrderId, Currency, KrOrderPrice, LargeOrderConfirmation, Money, NumericInput,
        OrderSide, OrderTerms, Price, Quantity, SellOrderQuantity, Symbol, UsOrderPrice,
        UsdOrderAmount, WholeOrderQuantity,
    };
    use serde_json::{Value, json};

    fn quantity(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("입력")).expect("수량")
    }

    fn price(text: &str, currency: Currency) -> Price {
        Price::try_new(
            Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액"),
        )
        .expect("가격")
    }

    fn body(symbol: &str, terms: OrderTerms, large: LargeOrderConfirmation) -> Value {
        serde_json::to_value(OrderCreateBody::from_parts(
            &Symbol::try_new(symbol).expect("심볼"),
            &terms,
            &ClientOrderId::try_new("order-key-7").expect("키"),
            large,
        ))
        .expect("JSON")
    }

    #[test]
    fn limit_and_kr_market_orders_map_to_one_create_body() {
        let whole = WholeOrderQuantity::try_new(quantity("10")).expect("정수");
        assert_eq!(
            body(
                "005930",
                OrderTerms::KrLimit {
                    side: OrderSide::Buy,
                    quantity: whole,
                    price: KrOrderPrice::try_new(price("70000", Currency::Krw)).expect("원화"),
                },
                LargeOrderConfirmation::NotRequired,
            ),
            json!({"clientOrderId":"order-key-7","symbol":"005930","side":"BUY","orderType":"LIMIT","timeInForce":"DAY","quantity":"10","price":"70000","confirmHighValueOrder":false})
        );
        assert_eq!(
            body(
                "005930",
                OrderTerms::KrMarket {
                    side: OrderSide::Sell,
                    quantity: whole,
                },
                LargeOrderConfirmation::Required,
            ),
            json!({"clientOrderId":"order-key-7","symbol":"005930","side":"SELL","orderType":"MARKET","timeInForce":"DAY","quantity":"10","confirmHighValueOrder":true})
        );
        assert_eq!(
            body(
                "AAPL",
                OrderTerms::UsLimit {
                    side: OrderSide::Sell,
                    quantity: whole,
                    price: UsOrderPrice::try_new(price("185.5", Currency::Usd)).expect("달러"),
                },
                LargeOrderConfirmation::NotRequired,
            ),
            json!({"clientOrderId":"order-key-7","symbol":"AAPL","side":"SELL","orderType":"LIMIT","timeInForce":"DAY","quantity":"10","price":"185.5","confirmHighValueOrder":false})
        );
    }

    #[test]
    fn us_market_orders_map_quantity_or_amount() {
        let whole = WholeOrderQuantity::try_new(quantity("10")).expect("정수");
        assert_eq!(
            body(
                "AAPL",
                OrderTerms::UsMarketBuy { quantity: whole },
                LargeOrderConfirmation::NotRequired,
            ),
            json!({"clientOrderId":"order-key-7","symbol":"AAPL","side":"BUY","orderType":"MARKET","timeInForce":"DAY","quantity":"10","confirmHighValueOrder":false})
        );
        assert_eq!(
            body(
                "AAPL",
                OrderTerms::UsMarketSell {
                    quantity: SellOrderQuantity::try_new(quantity("0.5")).expect("소수"),
                },
                LargeOrderConfirmation::NotRequired,
            ),
            json!({"clientOrderId":"order-key-7","symbol":"AAPL","side":"SELL","orderType":"MARKET","timeInForce":"DAY","quantity":"0.5","confirmHighValueOrder":false})
        );
        assert_eq!(
            body(
                "AAPL",
                OrderTerms::UsMarketAmountBuy {
                    amount: UsdOrderAmount::try_new(
                        Money::parse(
                            &NumericInput::try_new("100.5").expect("입력"),
                            Currency::Usd
                        )
                        .expect("금액"),
                    )
                    .expect("금액 주문"),
                },
                LargeOrderConfirmation::Required,
            ),
            json!({"clientOrderId":"order-key-7","symbol":"AAPL","side":"BUY","orderType":"MARKET","timeInForce":"DAY","orderAmount":"100.5","confirmHighValueOrder":true})
        );
    }
}
