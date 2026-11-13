use crate::{
    AssetClass, Currency, IsinCode, Market, Money, Quantity, Symbol, Transaction, TransactionKind,
};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum CurrencyOrder {
    Krw,
    Usd,
}
fn currency_order(currency: Currency) -> CurrencyOrder {
    match currency {
        Currency::Krw => CurrencyOrder::Krw,
        Currency::Usd => CurrencyOrder::Usd,
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrderedMoney(Money);
impl Ord for OrderedMoney {
    fn cmp(&self, other: &Self) -> Ordering {
        currency_order(self.0.currency())
            .cmp(&currency_order(other.0.currency()))
            .then_with(|| {
                self.0
                    .amount()
                    .as_decimal()
                    .cmp(&other.0.amount().as_decimal())
            })
    }
}
impl PartialOrd for OrderedMoney {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Transaction {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = self.data();
        let right = other.data();
        (
            left.settlement_date,
            &left.kind,
            &left.market,
            currency_order(left.currency),
            &left.asset_class,
            &left.symbol,
            &left.isin,
            left.quantity,
        )
            .cmp(&(
                right.settlement_date,
                &right.kind,
                &right.market,
                currency_order(right.currency),
                &right.asset_class,
                &right.symbol,
                &right.isin,
                right.quantity,
            ))
            .then_with(|| {
                (
                    left.filled_price
                        .map(|price| OrderedMoney(price.as_money())),
                    left.amount.map(OrderedMoney),
                    OrderedMoney(left.commission),
                    OrderedMoney(left.tax),
                    left.net_amount.map(OrderedMoney),
                    left.exchange_rate,
                    left.settlement,
                    left.transaction_time,
                    left.cancellation,
                )
                    .cmp(&(
                        right
                            .filled_price
                            .map(|price| OrderedMoney(price.as_money())),
                        right.amount.map(OrderedMoney),
                        OrderedMoney(right.commission),
                        OrderedMoney(right.tax),
                        right.net_amount.map(OrderedMoney),
                        right.exchange_rate,
                        right.settlement,
                        right.transaction_time,
                        right.cancellation,
                    ))
            })
    }
}
impl PartialOrd for Transaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CancellationIdentity {
    kind: TransactionKind,
    market: Market,
    currency: CurrencyOrder,
    symbol: Option<Symbol>,
    asset_class: Option<AssetClass>,
    isin: Option<IsinCode>,
    quantity: Option<Quantity>,
    amount: Option<OrderedMoney>,
}
impl CancellationIdentity {
    pub(crate) fn of(transaction: &Transaction) -> Self {
        let data = transaction.data();
        Self {
            kind: data.kind.clone(),
            market: data.market.clone(),
            currency: currency_order(data.currency),
            symbol: data.symbol.clone(),
            asset_class: data.asset_class.clone(),
            isin: data.isin.clone(),
            quantity: data.quantity,
            amount: data.amount.map(OrderedMoney),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OrderedMoney;
    use crate::{Amount, AmountInput, Currency, Money};
    use std::cmp::Ordering;
    #[test]
    fn money_order_respects_currency_and_exact_decimal_equivalence() {
        let money = |text, currency| {
            OrderedMoney(Money::new(
                Amount::parse(&AmountInput::new(text)).expect("금액"),
                currency,
            ))
        };
        let left = money("1", Currency::Krw);
        let right = money("1.0", Currency::Krw);
        assert_eq!(left.partial_cmp(&right), Some(Ordering::Equal));
        assert_eq!(
            left.partial_cmp(&money("1", Currency::Usd)),
            Some(Ordering::Less)
        );
        assert_eq!(
            money("2", Currency::Krw).partial_cmp(&left),
            Some(Ordering::Greater)
        );
    }
}
