use crate::{
    ClosedOrderSet, Comparison, Currency, HoldingsDailyCurrency, HoldingsDailyProfit,
    HoldingsDailySelection, Money, NetPurchaseError, NetPurchaseIncomplete, NetPurchaseInputError,
    NetPurchaseScope, PortfolioDailyProfitLoss, SettledRowSet, SnapshotError, TimeInterval,
    ValuationSnapshot, net_purchase_amount,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDailyComparison {
    Compared(Comparison),
    NotCompared(NetPurchaseError),
    NotProvided,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct OrderDailyCurrency {
    computed: Result<Money, NetPurchaseError>,
    selected: Result<HoldingsDailySelection, SnapshotError>,
    comparison: OrderDailyComparison,
}
impl OrderDailyCurrency {
    pub const fn computed(self) -> Result<Money, NetPurchaseError> {
        self.computed
    }
    pub const fn selected(self) -> Result<HoldingsDailySelection, SnapshotError> {
        self.selected
    }
    #[must_use]
    pub const fn comparison(self) -> OrderDailyComparison {
        self.comparison
    }
}
impl fmt::Debug for OrderDailyCurrency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrderDailyCurrency").finish_non_exhaustive()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct HoldingsDailyWithOrders {
    original: HoldingsDailyProfit,
    kr: OrderDailyCurrency,
    us: OrderDailyCurrency,
}
impl HoldingsDailyWithOrders {
    #[must_use]
    pub const fn currency(&self, currency: Currency) -> OrderDailyCurrency {
        match currency {
            Currency::Krw => self.kr,
            Currency::Usd => self.us,
        }
    }
    #[must_use]
    pub const fn original(&self) -> &HoldingsDailyProfit {
        &self.original
    }
}
impl fmt::Debug for HoldingsDailyWithOrders {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HoldingsDailyWithOrders")
            .finish_non_exhaustive()
    }
}
pub fn holdings_daily_profit_with_orders(
    today: &ValuationSnapshot,
    previous: Option<&ValuationSnapshot>,
    orders: &ClosedOrderSet,
    currency: Currency,
) -> Result<Money, NetPurchaseError> {
    let old = previous.ok_or_else(|| snapshot_error(SnapshotError::MissingPrevious))?;
    let period = TimeInterval::try_new(old.valuation_at(), today.valuation_at())
        .map_err(|_error| snapshot_error(SnapshotError::WrongPeriod))?;
    let net = net_purchase_amount(orders, period, NetPurchaseScope::Currency(currency))?;
    today
        .stock_value(currency)
        .map_err(snapshot_error)?
        .checked_sub(&old.stock_value(currency).map_err(snapshot_error)?)
        .and_then(|change| change.checked_sub(&net))
        .map_err(|error| NetPurchaseError::InvalidInput(NetPurchaseInputError::Arithmetic(error)))
}
fn snapshot_error(error: SnapshotError) -> NetPurchaseError {
    if error == SnapshotError::WrongPeriod {
        NetPurchaseError::InvalidInput(NetPurchaseInputError::Snapshot(error))
    } else {
        NetPurchaseError::Incomplete(NetPurchaseIncomplete::Snapshot(error))
    }
}
#[must_use]
pub fn compare_holdings_daily_profit_with_orders(
    today: &ValuationSnapshot,
    previous: Option<&ValuationSnapshot>,
    orders: &ClosedOrderSet,
    rows: &SettledRowSet,
    reported: Option<PortfolioDailyProfitLoss>,
) -> HoldingsDailyWithOrders {
    let original = HoldingsDailyProfit::compare(today, previous, rows, reported);
    let one = |currency| {
        let computed = holdings_daily_profit_with_orders(today, previous, orders, currency);
        let restriction = (reported.is_some()
            && currency == Currency::Krw
            && (!today
                .stock_value(Currency::Usd)
                .is_ok_and(|amount| amount.amount().as_decimal().is_zero())
                || previous.is_some_and(|old| {
                    !old.stock_value(Currency::Usd)
                        .is_ok_and(|amount| amount.amount().as_decimal().is_zero())
                })))
        .then_some(NetPurchaseError::Incomplete(
            NetPurchaseIncomplete::KrwSummaryNotComparable,
        ));
        compare_currency(computed, original.currency(currency), restriction)
    };
    HoldingsDailyWithOrders {
        kr: one(Currency::Krw),
        us: one(Currency::Usd),
        original,
    }
}
fn compare_currency(
    computed: Result<Money, NetPurchaseError>,
    original: HoldingsDailyCurrency,
    restriction: Option<NetPurchaseError>,
) -> OrderDailyCurrency {
    let selected = original.selected();
    let comparison = match computed {
        Err(reason) => OrderDailyComparison::NotCompared(reason),
        Ok(amount) => match selected {
            Err(_) => OrderDailyComparison::NotProvided,
            Ok(value) => match restriction {
                Some(reason) => OrderDailyComparison::NotCompared(reason),
                None => OrderDailyComparison::Compared(crate::holding_valuation::compare_money(
                    amount,
                    value.amount(),
                )),
            },
        },
    };
    OrderDailyCurrency {
        computed,
        selected,
        comparison,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        OrderDailyComparison, compare_holdings_daily_profit_with_orders,
        holdings_daily_profit_with_orders,
    };
    use crate::net_purchase::tests::{instrument, money, row};
    use crate::valuation_snapshot::tests::{at as day_at, reported};
    use crate::{
        AccountId, BrokerOrder, BrokerOrderId, BrokerOrderMoney, ClosedOrderCompleteness,
        ClosedOrderIncomplete, ClosedOrderSet, Comparison, Currency, KnownMarket, NetPurchaseError,
        NetPurchaseIncomplete, NetPurchaseInputError, OrderSide, SettledRowSet, SnapshotError,
        ValuationSnapshot,
    };
    use crate::{Price, Quantity};
    use rust_decimal::Decimal;
    pub(crate) fn valuation(day: i64, kr: &str, usd: Option<&str>) -> ValuationSnapshot {
        use crate::valuation_snapshot::tests::{at as day_at, calendar};
        use crate::{AcquisitionPrice, BuyingPower, Holding, HoldingObservation, Portfolio};
        let holding = |market, symbol, currency, price: Option<&str>| {
            Holding::new(
                instrument(market, symbol),
                Quantity::try_new(Decimal::ONE).expect("수량"),
                AcquisitionPrice::try_new(money("11", currency)).expect("취득"),
                HoldingObservation::new(
                    price.map(|value| Price::try_new(money(value, currency)).expect("가격")),
                    None,
                    day_at(day, 7),
                ),
            )
            .expect("보유")
        };
        let mut holdings = vec![holding(KnownMarket::Kr, "005930", Currency::Krw, Some(kr))];
        if usd != Some("0") {
            holdings.push(holding(KnownMarket::Us, "AAPL", Currency::Usd, usd));
        }
        let portfolio = Portfolio::new(
            holdings,
            BuyingPower::new(money("0", Currency::Krw), day_at(day, 7)),
            BuyingPower::new(money("0", Currency::Usd), day_at(day, 7)),
        )
        .expect("포트");
        ValuationSnapshot::observe(
            &calendar(day),
            &portfolio.evaluate(None),
            day_at(day, 7),
            None,
        )
        .expect("평가")
    }

    fn inputs() -> (ValuationSnapshot, ValuationSnapshot, ClosedOrderSet) {
        let old = valuation(2, "1400", Some("7000"));
        let today = valuation(3, "1700", Some("6500"));
        let mut buy = row("buy", OrderSide::Buy, "157", 150);
        buy.ordered_at = day_at(2, 8);
        buy.filled_at = Some(day_at(3, 7));
        buy.observed_at = day_at(3, 8);
        let mut sell = buy.clone();
        sell.order_id = BrokerOrderId::try_new("sell").expect("키");
        sell.side = OrderSide::Sell;
        sell.filled_at = Some(day_at(3, 6));
        sell.filled_amount = BrokerOrderMoney::Known(money("31", Currency::Krw));
        let orders = ClosedOrderSet::new(
            AccountId::new(7),
            vec![
                BrokerOrder::from_lookup_result(buy).expect("매수"),
                BrokerOrder::from_lookup_result(sell).expect("매도"),
            ],
            crate::TimeInterval::try_new(day_at(1, 0), day_at(3, 8)).expect("구간"),
            day_at(3, 8),
            ClosedOrderCompleteness::Complete,
        )
        .expect("종료 조회");
        (old, today, orders)
    }
    #[test]
    fn daily_formula_and_reported_selection_are_connected() {
        let (old, today, orders) = inputs();
        let rows = SettledRowSet::new(AccountId::new(7));
        assert_eq!(
            holdings_daily_profit_with_orders(&today, Some(&old), &orders, Currency::Krw),
            Ok(money("174", Currency::Krw))
        );
        assert_eq!(
            holdings_daily_profit_with_orders(&today, Some(&old), &orders, Currency::Usd),
            Ok(money("-500", Currency::Usd))
        );
        let result = compare_holdings_daily_profit_with_orders(
            &today,
            Some(&old),
            &orders,
            &rows,
            Some(reported("174", Some("-500"))),
        );
        let usd = result.currency(Currency::Usd);
        assert_eq!(usd.computed(), Ok(money("-500", Currency::Usd)));
        assert_eq!(
            usd.selected().expect("토스").amount(),
            money("-500", Currency::Usd)
        );
        assert_eq!(
            usd.comparison(),
            OrderDailyComparison::Compared(Comparison::Matches)
        );
        assert_eq!(
            result.currency(Currency::Krw).comparison(),
            OrderDailyComparison::NotCompared(NetPurchaseError::Incomplete(
                NetPurchaseIncomplete::KrwSummaryNotComparable
            ))
        );
        assert_eq!(
            result.original().krw_selected(),
            Ok(money("174", Currency::Krw))
        );
        assert_eq!(format!("{result:?}"), "HoldingsDailyWithOrders { .. }");
        assert_eq!(format!("{usd:?}"), "OrderDailyCurrency { .. }");
    }
    #[test]
    fn incomplete_keeps_toss_and_reason_and_complete_can_differ() {
        let (old, today, orders) = inputs();
        let rows = SettledRowSet::new(AccountId::new(7));
        let incomplete = ClosedOrderSet::new(
            orders.account_id().clone(),
            orders.orders().values().cloned().collect(),
            orders.covered(),
            orders.observed_at(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages),
        )
        .expect("불완전");
        let result = compare_holdings_daily_profit_with_orders(
            &today,
            Some(&old),
            &incomplete,
            &rows,
            Some(reported("888", Some("-499"))),
        );
        let usd = result.currency(Currency::Usd);
        let reason = NetPurchaseError::Incomplete(NetPurchaseIncomplete::Query(
            ClosedOrderIncomplete::MissingPages,
        ));
        assert_eq!(usd.computed(), Err(reason));
        assert_eq!(
            usd.selected().expect("토스 보존").amount(),
            money("-499", Currency::Usd)
        );
        assert_eq!(usd.comparison(), OrderDailyComparison::NotCompared(reason));
        let different = compare_holdings_daily_profit_with_orders(
            &today,
            Some(&old),
            &orders,
            &rows,
            Some(reported("888", Some("-490"))),
        );
        assert_eq!(
            different.currency(Currency::Usd).comparison(),
            OrderDailyComparison::Compared(Comparison::Different)
        );
        let missing = compare_holdings_daily_profit_with_orders(
            &today,
            Some(&old),
            &orders,
            &rows,
            Some(reported("888", None)),
        );
        assert_eq!(
            missing.currency(Currency::Usd).comparison(),
            OrderDailyComparison::NotProvided
        );
    }
    #[test]
    fn snapshot_errors_do_not_erase_reported_values() {
        let (old, today, orders) = inputs();
        let rows = SettledRowSet::new(AccountId::new(7));
        assert_eq!(
            holdings_daily_profit_with_orders(&today, None, &orders, Currency::Krw),
            Err(NetPurchaseError::Incomplete(
                NetPurchaseIncomplete::Snapshot(SnapshotError::MissingPrevious)
            ))
        );
        assert_eq!(
            holdings_daily_profit_with_orders(&old, Some(&today), &orders, Currency::Krw),
            Err(NetPurchaseError::InvalidInput(
                NetPurchaseInputError::Snapshot(SnapshotError::WrongPeriod)
            ))
        );
        for (before, current) in [
            (old.clone(), valuation(3, "1700", None)),
            (valuation(2, "1400", None), today.clone()),
        ] {
            assert_eq!(
                holdings_daily_profit_with_orders(&current, Some(&before), &orders, Currency::Usd),
                Err(NetPurchaseError::Incomplete(
                    NetPurchaseIncomplete::Snapshot(SnapshotError::IncompleteValuation)
                ))
            );
        }
        let missing = compare_holdings_daily_profit_with_orders(
            &today,
            None,
            &orders,
            &rows,
            Some(reported("7", Some("13"))),
        );
        assert_eq!(
            missing
                .currency(Currency::Usd)
                .selected()
                .expect("토스")
                .amount(),
            money("13", Currency::Usd)
        );
        let empty =
            compare_holdings_daily_profit_with_orders(&today, Some(&old), &orders, &rows, None);
        assert_eq!(
            empty.currency(Currency::Usd).comparison(),
            OrderDailyComparison::NotProvided
        );
        let domestic_old = valuation(2, "1400", Some("0"));
        let domestic_today = valuation(3, "1700", Some("0"));
        let domestic = compare_holdings_daily_profit_with_orders(
            &domestic_today,
            Some(&domestic_old),
            &orders,
            &rows,
            Some(reported("174", None)),
        );
        assert_eq!(
            domestic.currency(Currency::Krw).comparison(),
            OrderDailyComparison::Compared(Comparison::Matches)
        );
    }

    #[test]
    fn daily_arithmetic_failure_is_input_error_with_reported_value_retained() {
        let (before, after, orders) = inputs();
        let sells = orders
            .orders()
            .values()
            .filter(|order| order.side() == OrderSide::Sell)
            .cloned()
            .collect();
        let sold = ClosedOrderSet::new(
            orders.account_id().clone(),
            sells,
            orders.covered(),
            orders.observed_at(),
            ClosedOrderCompleteness::Complete,
        )
        .expect("매도");
        let old = valuation(2, "1", Some("0"));
        let today = valuation(3, "79228162514264337593543950335", Some("0"));
        let expected = NetPurchaseError::InvalidInput(NetPurchaseInputError::Arithmetic(
            crate::MoneyError::Overflow,
        ));
        assert_eq!(
            holdings_daily_profit_with_orders(&today, Some(&old), &sold, Currency::Krw),
            Err(expected)
        );
        let rows = SettledRowSet::new(AccountId::new(7));
        let result = compare_holdings_daily_profit_with_orders(
            &today,
            Some(&old),
            &sold,
            &rows,
            Some(reported("17", None)),
        );
        assert_eq!(
            result.currency(Currency::Krw).comparison(),
            OrderDailyComparison::NotCompared(expected)
        );
        assert_eq!(
            result
                .currency(Currency::Krw)
                .selected()
                .expect("토스")
                .amount(),
            money("17", Currency::Krw)
        );
        let reversed = compare_holdings_daily_profit_with_orders(
            &before,
            Some(&after),
            &orders,
            &rows,
            Some(reported("19", Some("23"))),
        );
        assert_eq!(
            reversed.currency(Currency::Usd).comparison(),
            OrderDailyComparison::NotCompared(NetPurchaseError::InvalidInput(
                NetPurchaseInputError::Snapshot(SnapshotError::WrongPeriod)
            ))
        );
    }
}
