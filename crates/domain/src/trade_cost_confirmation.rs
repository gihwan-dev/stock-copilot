use crate::{
    BrokerOrder, Cancellation, Money, OrderProgress, OrderSide, Settlement, Transaction,
    TransactionKind, order_progress,
};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EstimatedTradeCost {
    pub commission: Option<Money>,
    pub tax: Option<Money>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfirmedTradeCost {
    commission: Money,
    tax: Money,
    net_amount: Option<Money>,
}

impl ConfirmedTradeCost {
    #[must_use]
    pub const fn commission(&self) -> Money {
        self.commission
    }

    #[must_use]
    pub const fn tax(&self) -> Money {
        self.tax
    }

    #[must_use]
    pub const fn net_amount(&self) -> Option<Money> {
        self.net_amount
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeCostDisplay {
    Estimated(EstimatedTradeCost),
    Confirmed(ConfirmedTradeCost),
}

fn trade_kind(side: OrderSide) -> TransactionKind {
    match side {
        OrderSide::Buy => TransactionKind::Buy,
        OrderSide::Sell => TransactionKind::Sell,
    }
}

fn belongs_to(order: &BrokerOrder, transaction: &Transaction) -> bool {
    let data = transaction.data();
    data.kind == trade_kind(order.side())
        && data.market == order.instrument().market()
        && data.symbol.as_ref() == Some(order.instrument().symbol())
        && data.settlement == Settlement::Settled
        && data.cancellation == Cancellation::Regular
        && data
            .transaction_time
            .is_some_and(|time| time >= order.ordered_at())
}

struct Totals {
    quantity: Decimal,
    commission: Money,
    tax: Money,
    net_amount: Option<Money>,
}

fn accumulate(totals: Option<Totals>, transaction: &Transaction) -> Option<Totals> {
    let data = transaction.data();
    let quantity = data.quantity?.as_decimal();
    let Some(previous) = totals else {
        return Some(Totals {
            quantity,
            commission: data.commission,
            tax: data.tax,
            net_amount: data.net_amount,
        });
    };
    let net_amount = match (previous.net_amount, data.net_amount) {
        (Some(left), Some(right)) => Some(left.checked_add(&right).ok()?),
        (None, _) | (_, None) => None,
    };
    Some(Totals {
        quantity: previous.quantity.checked_add(quantity)?,
        commission: previous.commission.checked_add(&data.commission).ok()?,
        tax: previous.tax.checked_add(&data.tax).ok()?,
        net_amount,
    })
}

#[must_use]
pub fn trade_cost_display(
    order: &BrokerOrder,
    estimate: EstimatedTradeCost,
    transactions: &[Transaction],
) -> TradeCostDisplay {
    if order_progress(order) != OrderProgress::Filled {
        return TradeCostDisplay::Estimated(estimate);
    }
    let mut totals = None;
    for transaction in transactions.iter().filter(|item| belongs_to(order, item)) {
        totals = accumulate(totals, transaction);
        if totals.is_none() {
            return TradeCostDisplay::Estimated(estimate);
        }
    }
    match totals {
        Some(sum)
            if sum.quantity.normalize() == order.filled_quantity().as_decimal().normalize() =>
        {
            TradeCostDisplay::Confirmed(ConfirmedTradeCost {
                commission: sum.commission,
                tax: sum.tax,
                net_amount: sum.net_amount,
            })
        }
        Some(_) | None => TradeCostDisplay::Estimated(estimate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net_purchase::tests::{money, row, time};
    use crate::{
        BrokerOrderStatus, CalendarDate, CalendarYear, Currency, DayNumber, Market, MonthNumber,
        Quantity, Symbol, TransactionData,
    };

    fn order(status: BrokerOrderStatus, filled: i64) -> BrokerOrder {
        let mut input = row("b1", OrderSide::Buy, "39", 150);
        input.status = status;
        input.filled_quantity = Quantity::try_new(Decimal::from(filled)).expect("체결");
        BrokerOrder::from_lookup_result(input).expect("주문")
    }

    fn fill(quantity: i64, commission: &str, tax: &str, at: i64) -> TransactionData {
        TransactionData {
            kind: TransactionKind::Buy,
            market: Market::Kr,
            currency: Currency::Krw,
            symbol: Some(Symbol::try_new("005930").expect("종목")),
            asset_class: None,
            isin: None,
            quantity: Some(Quantity::try_new(Decimal::from(quantity)).expect("수량")),
            filled_price: None,
            amount: None,
            commission: money(commission, Currency::Krw),
            tax: money(tax, Currency::Krw),
            net_amount: Some(money("1000", Currency::Krw)),
            exchange_rate: None,
            settlement_date: CalendarDate::from_parts(
                CalendarYear::new(2026),
                MonthNumber::new(11),
                DayNumber::new(3),
            )
            .expect("날짜"),
            settlement: Settlement::Settled,
            transaction_time: Some(time(at)),
            cancellation: Cancellation::Regular,
        }
    }

    fn tx(data: TransactionData) -> Transaction {
        Transaction::try_new(data).expect("거래")
    }

    fn estimate() -> EstimatedTradeCost {
        EstimatedTradeCost {
            commission: Some(money("20", Currency::Krw)),
            tax: None,
        }
    }

    #[test]
    fn settled_fills_replace_the_estimate() {
        let display = trade_cost_display(
            &order(BrokerOrderStatus::Filled, 17),
            estimate(),
            &[tx(fill(10, "11", "3", 160)), tx(fill(7, "8", "2", 170))],
        );
        let TradeCostDisplay::Confirmed(confirmed) = display else {
            panic!("확정이어야 함");
        };
        assert_eq!(confirmed.commission(), money("19", Currency::Krw));
        assert_eq!(confirmed.tax(), money("5", Currency::Krw));
        assert_eq!(confirmed.net_amount(), Some(money("2000", Currency::Krw)));
    }

    #[test]
    fn estimate_stays_until_fills_are_settled_and_complete() {
        let filled = order(BrokerOrderStatus::Filled, 17);
        let mut pending = fill(17, "19", "5", 160);
        pending.settlement = Settlement::Pending;
        pending.transaction_time = None;
        let mut cancelled = fill(17, "19", "5", 160);
        cancelled.cancellation = Cancellation::Cancelled;
        let mut other_symbol = fill(17, "19", "5", 160);
        other_symbol.symbol = Some(Symbol::try_new("000660").expect("종목"));
        for transactions in [
            vec![],
            vec![tx(pending)],
            vec![tx(cancelled)],
            vec![tx(other_symbol)],
            vec![tx(fill(10, "11", "3", 160))],
            vec![tx(fill(17, "19", "5", 100))],
        ] {
            assert_eq!(
                trade_cost_display(&filled, estimate(), &transactions),
                TradeCostDisplay::Estimated(estimate())
            );
        }
        assert_eq!(
            trade_cost_display(
                &order(BrokerOrderStatus::PartialFilled, 10),
                estimate(),
                &[tx(fill(10, "11", "3", 160))],
            ),
            TradeCostDisplay::Estimated(estimate())
        );
    }
}
