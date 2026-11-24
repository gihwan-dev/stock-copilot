use crate::{
    BrokerOrder, BrokerOrderId, BrokerOrderPrice, BrokerOrderSize, BrokerOrderType, Money,
    OrderTerms, Price, Quantity, ResolvedOrderInstrument, Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownOrderFacts {
    pub instrument: ResolvedOrderInstrument,
    pub terms: OrderTerms,
    pub sent_at: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownOrderResolution {
    Matched(BrokerOrderId),
    Unconfirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedSize {
    Quantity(Quantity),
    Amount(Money),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedPrice {
    Limit(Price),
    Market,
}

fn expected(terms: &OrderTerms) -> (ExpectedPrice, ExpectedSize) {
    match terms {
        OrderTerms::KrLimit {
            quantity, price, ..
        } => (
            ExpectedPrice::Limit(price.price()),
            ExpectedSize::Quantity(quantity.quantity()),
        ),
        OrderTerms::UsLimit {
            quantity, price, ..
        } => (
            ExpectedPrice::Limit(price.price()),
            ExpectedSize::Quantity(quantity.quantity()),
        ),
        OrderTerms::KrMarket { quantity, .. } | OrderTerms::UsMarketBuy { quantity } => (
            ExpectedPrice::Market,
            ExpectedSize::Quantity(quantity.quantity()),
        ),
        OrderTerms::UsMarketSell { quantity } => (
            ExpectedPrice::Market,
            ExpectedSize::Quantity(quantity.quantity()),
        ),
        OrderTerms::UsMarketAmountBuy { amount } => {
            (ExpectedPrice::Market, ExpectedSize::Amount(amount.money()))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Candidate {
    Same,
    Different,
}

fn same_decimal(left: Quantity, right: Quantity) -> Candidate {
    if left.as_decimal().normalize() == right.as_decimal().normalize() {
        Candidate::Same
    } else {
        Candidate::Different
    }
}

fn price_matches(expected: ExpectedPrice, order: &BrokerOrder) -> Candidate {
    match (expected, order.order_type(), order.price()) {
        (ExpectedPrice::Limit(price), BrokerOrderType::Limit, BrokerOrderPrice::Known(known))
            if known.as_money() == price.as_money() =>
        {
            Candidate::Same
        }
        (ExpectedPrice::Market, BrokerOrderType::Market, _) => Candidate::Same,
        (ExpectedPrice::Limit(_) | ExpectedPrice::Market, _, _) => Candidate::Different,
    }
}

fn size_matches(expected: ExpectedSize, order: &BrokerOrder) -> Candidate {
    match (expected, order.size()) {
        (ExpectedSize::Quantity(quantity), BrokerOrderSize::Quantity(reported)) => {
            same_decimal(quantity, reported)
        }
        (ExpectedSize::Amount(amount), BrokerOrderSize::Amount { order_amount, .. })
            if order_amount == amount =>
        {
            Candidate::Same
        }
        (ExpectedSize::Quantity(_) | ExpectedSize::Amount(_), _) => Candidate::Different,
    }
}

fn candidate(facts: &UnknownOrderFacts, order: &BrokerOrder) -> Candidate {
    let (price, size) = expected(&facts.terms);
    let fields = [
        if order.instrument() == &facts.instrument {
            Candidate::Same
        } else {
            Candidate::Different
        },
        if order.side() == facts.terms.side() {
            Candidate::Same
        } else {
            Candidate::Different
        },
        if order.ordered_at() >= facts.sent_at {
            Candidate::Same
        } else {
            Candidate::Different
        },
        price_matches(price, order),
        size_matches(size, order),
    ];
    if fields.contains(&Candidate::Different) {
        Candidate::Different
    } else {
        Candidate::Same
    }
}

#[must_use]
pub fn resolve_unknown_order(
    facts: &UnknownOrderFacts,
    broker_orders: &[BrokerOrder],
) -> UnknownOrderResolution {
    let mut matches = broker_orders
        .iter()
        .filter(|order| candidate(facts, order) == Candidate::Same);
    match (matches.next(), matches.next()) {
        (Some(only), None) => UnknownOrderResolution::Matched(only.order_id().clone()),
        (Some(_), Some(_)) | (None, _) => UnknownOrderResolution::Unconfirmed,
    }
}

#[cfg(test)]
mod tests {
    use super::{UnknownOrderFacts, UnknownOrderResolution, resolve_unknown_order};
    use crate::net_purchase::tests::{row, time};
    use crate::{
        BrokerOrder, BrokerOrderId, BrokerOrderPrice, BrokerOrderType, Currency, KrOrderPrice,
        NumericInput, OrderSide, OrderTerms, Price, Quantity, WholeOrderQuantity,
    };
    use rust_decimal::Decimal;

    fn price(text: &str) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("가격")
    }

    fn order(id: &str, at: i64, limit: &str) -> BrokerOrder {
        let mut input = row(id, OrderSide::Buy, "39", 150);
        input.ordered_at = time(at);
        input.order_type = BrokerOrderType::Limit;
        input.price = BrokerOrderPrice::Known(price(limit));
        BrokerOrder::from_lookup_result(input).expect("주문")
    }

    fn facts(limit: &str) -> UnknownOrderFacts {
        UnknownOrderFacts {
            instrument: order("probe", 110, "13").instrument().clone(),
            terms: OrderTerms::KrLimit {
                side: OrderSide::Buy,
                quantity: WholeOrderQuantity::try_new(
                    Quantity::try_new(Decimal::from(17)).expect("수량"),
                )
                .expect("주문 수량"),
                price: KrOrderPrice::try_new(price(limit)).expect("지정가"),
            },
            sent_at: time(100),
        }
    }

    #[test]
    fn exactly_one_match_after_sending_resolves_the_order() {
        assert_eq!(
            resolve_unknown_order(
                &facts("13"),
                &[order("b1", 105, "13"), order("b2", 90, "13")]
            ),
            UnknownOrderResolution::Matched(BrokerOrderId::try_new("b1").expect("주문번호"))
        );
    }

    #[test]
    fn none_or_several_matches_stay_unconfirmed() {
        assert_eq!(
            resolve_unknown_order(&facts("13"), &[order("b2", 90, "13")]),
            UnknownOrderResolution::Unconfirmed,
            "전송 전 주문은 대상 아님"
        );
        assert_eq!(
            resolve_unknown_order(&facts("14"), &[order("b1", 105, "13")]),
            UnknownOrderResolution::Unconfirmed,
            "가격이 다르면 대상 아님"
        );
        assert_eq!(
            resolve_unknown_order(
                &facts("13"),
                &[order("b1", 105, "13"), order("b3", 106, "13")]
            ),
            UnknownOrderResolution::Unconfirmed,
            "둘 이상이면 확인 불가"
        );
    }
}
