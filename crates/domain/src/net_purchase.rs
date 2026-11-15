use crate::{
    AccountId, BrokerOrder, BrokerOrderMoney, BrokerOrderSet, Currency, Money, MoneyError,
    OrderSide, ResolvedOrderInstrument, SnapshotError, Timestamp, ValuationSnapshot,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetPurchaseError {
    MissingTotal,
    UnallocatableExecution,
    IncompleteQuery,
    MissingExecutionTime,
    AccountMismatch,
    WrongPeriod,
    InvalidExecutionTime,
    Arithmetic(MoneyError),
    Snapshot(SnapshotError),
}

#[derive(Clone, PartialEq, Eq)]
pub struct ClosedOrderEvidence {
    facts: BrokerOrderSet,
    previous: Timestamp,
    current: Timestamp,
}
impl ClosedOrderEvidence {
    pub(crate) fn new(facts: BrokerOrderSet, previous: Timestamp, current: Timestamp) -> Self {
        Self {
            facts,
            previous,
            current,
        }
    }
}
impl fmt::Debug for ClosedOrderEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClosedOrderEvidence")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NetPurchaseAmount(Result<Money, NetPurchaseError>);
impl NetPurchaseAmount {
    pub const fn amount(self) -> Result<Money, NetPurchaseError> {
        self.0
    }
}
impl fmt::Debug for NetPurchaseAmount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetPurchaseAmount").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum NetPurchaseScope<'a> {
    Instrument(&'a ResolvedOrderInstrument),
    Currency(Currency),
}
impl NetPurchaseScope<'_> {
    fn currency(self) -> Currency {
        match self {
            Self::Instrument(instrument) => instrument_currency(instrument),
            Self::Currency(currency) => currency,
        }
    }
    fn includes(self, order: &BrokerOrder) -> Inclusion {
        match self {
            Self::Instrument(instrument) if order.instrument() != instrument => Inclusion::Excluded,
            _ if instrument_currency(order.instrument()) != self.currency() => Inclusion::Excluded,
            Self::Instrument(_) | Self::Currency(_) => Inclusion::Included,
        }
    }
}
fn instrument_currency(instrument: &ResolvedOrderInstrument) -> Currency {
    if instrument.market() == crate::Market::Kr {
        Currency::Krw
    } else {
        Currency::Usd
    }
}
#[derive(PartialEq, Eq)]
enum Inclusion {
    Included,
    Excluded,
}

#[must_use]
pub fn net_purchase_amount(
    read: (&BrokerOrderSet, &AccountId),
    period: (Timestamp, Timestamp),
    evidence: Option<&ClosedOrderEvidence>,
    scope: NetPurchaseScope<'_>,
) -> NetPurchaseAmount {
    NetPurchaseAmount(calculate(read, period, evidence, scope))
}
fn calculate(
    read: (&BrokerOrderSet, &AccountId),
    period: (Timestamp, Timestamp),
    evidence: Option<&ClosedOrderEvidence>,
    scope: NetPurchaseScope<'_>,
) -> Result<Money, NetPurchaseError> {
    let (facts, account) = read;
    let (previous, current) = period;
    if facts.account_id() != account {
        return Err(NetPurchaseError::AccountMismatch);
    }
    if previous >= current {
        return Err(NetPurchaseError::WrongPeriod);
    }
    if facts.observed_at() < current {
        return Err(NetPurchaseError::IncompleteQuery);
    }
    let expected = ClosedOrderEvidence::new(facts.clone(), previous, current);
    if evidence != Some(&expected) {
        return Err(NetPurchaseError::IncompleteQuery);
    }
    let mut total = crate::holding::zero(scope.currency());
    for order in facts.orders().values() {
        if scope.includes(order) == Inclusion::Excluded {
            continue;
        }
        let Some(at) = order.filled_at() else {
            return Err(NetPurchaseError::MissingExecutionTime);
        };
        if at < order.ordered_at() || at > order.observed_at() {
            return Err(NetPurchaseError::InvalidExecutionTime);
        }
        if at <= previous {
            continue;
        }
        if order.ordered_at() <= previous || at > current {
            return Err(NetPurchaseError::UnallocatableExecution);
        }
        let BrokerOrderMoney::Known(amount) = order.filled_amount() else {
            return Err(NetPurchaseError::MissingTotal);
        };
        total = match order.side() {
            OrderSide::Buy => total.checked_add(&amount),
            OrderSide::Sell => total.checked_sub(&amount),
        }
        .map_err(NetPurchaseError::Arithmetic)?;
    }
    Ok(total)
}

pub fn holdings_daily_profit_with_orders(
    today: &ValuationSnapshot,
    previous: Option<&ValuationSnapshot>,
    read: (&BrokerOrderSet, &AccountId),
    evidence: Option<&ClosedOrderEvidence>,
    currency: Currency,
) -> Result<Money, NetPurchaseError> {
    let (_, old) = crate::daily_profit::validated_previous(today, previous)
        .map_err(NetPurchaseError::Snapshot)?;
    let net = net_purchase_amount(
        read,
        (old.valuation_at(), today.valuation_at()),
        evidence,
        NetPurchaseScope::Currency(currency),
    )
    .amount()?;
    today
        .stock_value(currency)
        .map_err(NetPurchaseError::Snapshot)?
        .checked_sub(
            &old.stock_value(currency)
                .map_err(NetPurchaseError::Snapshot)?,
        )
        .and_then(|change| change.checked_sub(&net))
        .map_err(NetPurchaseError::Arithmetic)
}

#[cfg(test)]
mod tests {
    use super::{
        ClosedOrderEvidence, NetPurchaseError, NetPurchaseScope, holdings_daily_profit_with_orders,
        net_purchase_amount,
    };
    use crate::{
        AccountId, BrokerOrderMoney, BrokerOrderSet, Money, MoneyError, OrderSide, SnapshotError,
        ValuationSnapshot,
    };
    use crate::{
        CommonShare, Currency, DisplayInstrumentName, InstrumentId, KnownMarket,
        KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Nanosecond, NxtTrading, OffsetSeconds,
        ResolvedOrderInstrument, Symbol, Timestamp, TradingSuspension, UnixSeconds,
    };
    pub(crate) fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(100),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    pub(crate) fn instrument(market: KnownMarket, symbol: &str) -> ResolvedOrderInstrument {
        let (listing, currency, detail) = match market {
            KnownMarket::Kr => (
                "KOSPI",
                Currency::Krw,
                Some(KoreanMasterDetail {
                    liquidation: LiquidationTrading::Regular,
                    krx: TradingSuspension::Clear,
                    nxt: NxtTrading::Unsupported,
                }),
            ),
            KnownMarket::Us => ("NASDAQ", Currency::Usd, None),
        };
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new(listing).expect("시장"),
            symbol: Symbol::try_new(symbol).expect("종목"),
            instrument_id: InstrumentId::try_new(symbol).expect("키"),
            name: DisplayInstrumentName::try_new("테스트 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: detail,
            observed_at: at(),
        })
        .expect("마스터")
        .order_instrument()
        .expect("주문 종목")
    }

    use crate::{
        BrokerOrderId, BrokerOrderPrice, BrokerOrderRow, BrokerOrderSize, BrokerOrderStatus,
        BrokerOrderTime, BrokerOrderType, BrokerTimeInForce, NumericInput, OpenOrderCoverage,
        Price, Quantity,
    };
    use rust_decimal::Decimal;
    fn time(text: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(text),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
    }
    fn row(id: &str, side: OrderSide, amount: &str, at: i64) -> BrokerOrderRow {
        BrokerOrderRow {
            account_id: AccountId::new(7),
            order_id: BrokerOrderId::try_new(id).expect("주문"),
            instrument: instrument(KnownMarket::Kr, "005930"),
            side,
            status: BrokerOrderStatus::Filled,
            size: BrokerOrderSize::Quantity(Quantity::try_new(Decimal::from(17)).expect("수량")),
            filled_quantity: Quantity::try_new(Decimal::from(3)).expect("체결"),
            average_filled_price: Some(
                Price::parse(&NumericInput::try_new("13").expect("가격"), Currency::Krw)
                    .expect("가격"),
            ),
            filled_amount: BrokerOrderMoney::Known(money(amount, Currency::Krw)),
            filled_at: Some(time(at)),
            ordered_at: time(110),
            canceled_at: BrokerOrderTime::Absent,
            price: BrokerOrderPrice::Absent,
            order_type: BrokerOrderType::Market,
            time_in_force: BrokerTimeInForce::Day,
            observed_at: time(300),
        }
    }
    fn facts(rows: Vec<BrokerOrderRow>) -> BrokerOrderSet {
        BrokerOrderSet::from_open_lookup_result(
            AccountId::new(7),
            rows,
            OpenOrderCoverage::Incomplete,
            time(300),
        )
        .expect("봉인")
    }
    fn calc(orders: &BrokerOrderSet) -> Result<Money, NetPurchaseError> {
        let evidence = ClosedOrderEvidence::new(orders.clone(), time(100), time(200));
        net_purchase_amount(
            (orders, &AccountId::new(7)),
            (time(100), time(200)),
            Some(&evidence),
            NetPurchaseScope::Currency(Currency::Krw),
        )
        .amount()
    }
    #[test]
    fn boundaries_totals_sides_and_scope() {
        let mut left = row("left", OrderSide::Buy, "999", 100);
        left.ordered_at = time(90);
        let orders = facts(vec![
            left,
            row("buy", OrderSide::Buy, "157", 200),
            row("sell", OrderSide::Sell, "31", 150),
        ]);
        assert_eq!(calc(&orders), Ok(money("126", Currency::Krw)));
        let evidence = ClosedOrderEvidence::new(orders.clone(), time(100), time(200));
        let target = instrument(KnownMarket::Kr, "005930");
        assert_eq!(
            net_purchase_amount(
                (&orders, &AccountId::new(7)),
                (time(100), time(200)),
                Some(&evidence),
                NetPurchaseScope::Instrument(&target)
            )
            .amount(),
            calc(&orders)
        );
        let other = instrument(KnownMarket::Kr, "000660");
        assert_eq!(
            net_purchase_amount(
                (&orders, &AccountId::new(7)),
                (time(100), time(200)),
                Some(&evidence),
                NetPurchaseScope::Instrument(&other)
            )
            .amount(),
            Ok(money("0", Currency::Krw))
        );
        assert_eq!(
            net_purchase_amount(
                (&orders, &AccountId::new(7)),
                (time(100), time(200)),
                Some(&evidence),
                NetPurchaseScope::Currency(Currency::Usd)
            )
            .amount(),
            Ok(money("0", Currency::Usd))
        );
        assert_eq!(calc(&facts(vec![])), Ok(money("0", Currency::Krw)));
        assert_eq!(
            calc(&facts(vec![row("s", OrderSide::Sell, "31", 150)])),
            Ok(money("-31", Currency::Krw))
        );
        assert_eq!(
            calc(&facts(vec![
                row("buy", OrderSide::Buy, "31", 150),
                row("sell", OrderSide::Sell, "31", 160)
            ])),
            Ok(money("0", Currency::Krw))
        );
    }
    #[test]
    fn incomplete_and_period_evidence() {
        let base = row("sell", OrderSide::Buy, "157", 150);
        for (input, err) in [
            (
                {
                    let mut input = base.clone();
                    input.filled_amount = BrokerOrderMoney::Absent;
                    input
                },
                NetPurchaseError::MissingTotal,
            ),
            (
                {
                    let mut input = base.clone();
                    input.filled_at = None;
                    input
                },
                NetPurchaseError::MissingExecutionTime,
            ),
            (
                {
                    let mut input = base.clone();
                    input.ordered_at = time(100);
                    input
                },
                NetPurchaseError::UnallocatableExecution,
            ),
            (
                {
                    let mut input = base.clone();
                    input.filled_at = Some(time(201));
                    input
                },
                NetPurchaseError::UnallocatableExecution,
            ),
            (
                {
                    let mut input = base.clone();
                    input.filled_at = Some(time(109));
                    input
                },
                NetPurchaseError::InvalidExecutionTime,
            ),
            (
                {
                    let mut input = base;
                    input.filled_at = Some(time(301));
                    input
                },
                NetPurchaseError::InvalidExecutionTime,
            ),
        ] {
            assert_eq!(calc(&facts(vec![input])), Err(err));
        }
    }
    #[test]
    fn evidence_account_and_period_are_bound() {
        let base = row("buy", OrderSide::Buy, "157", 150);
        let orders = facts(vec![base]);
        let evidence = ClosedOrderEvidence::new(orders.clone(), time(100), time(200));
        let run = |account, prev, curr, proof| {
            net_purchase_amount(
                (&orders, &account),
                (time(prev), time(curr)),
                proof,
                NetPurchaseScope::Currency(Currency::Krw),
            )
            .amount()
        };
        assert_eq!(
            run(AccountId::new(7), 100, 200, None),
            Err(NetPurchaseError::IncompleteQuery)
        );
        assert_eq!(
            run(AccountId::new(8), 100, 200, Some(&evidence)),
            Err(NetPurchaseError::AccountMismatch)
        );
        assert_eq!(
            run(AccountId::new(7), 200, 200, Some(&evidence)),
            Err(NetPurchaseError::WrongPeriod)
        );
        assert_eq!(
            run(AccountId::new(7), 201, 200, Some(&evidence)),
            Err(NetPurchaseError::WrongPeriod)
        );
        assert_eq!(
            run(AccountId::new(7), 99, 200, Some(&evidence)),
            Err(NetPurchaseError::IncompleteQuery)
        );
        assert_eq!(
            run(AccountId::new(7), 100, 201, Some(&evidence)),
            Err(NetPurchaseError::IncompleteQuery)
        );
        assert_eq!(
            run(AccountId::new(7), 100, 301, Some(&evidence)),
            Err(NetPurchaseError::IncompleteQuery)
        );
    }
    #[test]
    fn evidence_dataset_debug_and_mixed_account() {
        let orders = facts(vec![row("buy", OrderSide::Buy, "157", 150)]);
        let evidence = ClosedOrderEvidence::new(orders.clone(), time(100), time(200));
        let run = |account, prev, curr, proof| {
            net_purchase_amount(
                (&orders, &account),
                (time(prev), time(curr)),
                proof,
                NetPurchaseScope::Currency(Currency::Krw),
            )
            .amount()
        };
        let other = ClosedOrderEvidence::new(facts(vec![]), time(100), time(200));
        assert_eq!(
            run(AccountId::new(7), 100, 200, Some(&other)),
            Err(NetPurchaseError::IncompleteQuery)
        );
        assert_eq!(format!("{evidence:?}"), "ClosedOrderEvidence { .. }");
        assert_eq!(
            format!(
                "{:?}",
                net_purchase_amount(
                    (&orders, &AccountId::new(7)),
                    (time(100), time(200)),
                    Some(&evidence),
                    NetPurchaseScope::Currency(Currency::Krw)
                )
            ),
            "NetPurchaseAmount { .. }"
        );
        let mut wrong = row("bad", OrderSide::Buy, "1", 150);
        wrong.account_id = AccountId::new(8);
        assert_eq!(
            BrokerOrderSet::from_open_lookup_result(
                AccountId::new(7),
                vec![wrong],
                OpenOrderCoverage::Incomplete,
                time(300)
            ),
            Err(crate::BrokerOrderError::AccountMismatch)
        );
    }
    #[test]
    fn amount_order_native_currency_and_precision_failures() {
        let mut usd = row("usd", OrderSide::Buy, "1", 170);
        usd.instrument = instrument(KnownMarket::Us, "AAPL");
        usd.average_filled_price = None;
        usd.filled_amount = BrokerOrderMoney::Known(money("43.21", Currency::Usd));
        usd.size = BrokerOrderSize::Amount {
            reported_quantity: Quantity::try_new(Decimal::ZERO).expect("수량"),
            order_amount: money("99", Currency::Usd),
        };
        usd.status = BrokerOrderStatus::PartialFilled;
        let orders = facts(vec![usd, row("kr", OrderSide::Buy, "71", 180)]);
        let evidence = ClosedOrderEvidence::new(orders.clone(), time(100), time(200));
        assert_eq!(calc(&orders), Ok(money("71", Currency::Krw)));
        assert_eq!(
            net_purchase_amount(
                (&orders, &AccountId::new(7)),
                (time(100), time(200)),
                Some(&evidence),
                NetPurchaseScope::Currency(Currency::Usd)
            )
            .amount(),
            Ok(money("43.21", Currency::Usd))
        );
        assert_eq!(
            calc(&facts(vec![
                row("buy", OrderSide::Buy, "79228162514264337593543950335", 150),
                row("sell", OrderSide::Buy, "1", 160)
            ])),
            Err(NetPurchaseError::Arithmetic(MoneyError::Overflow))
        );
    }
    #[test]
    fn independent_integer_oracle_and_permutations_4096() {
        use proptest::{
            prop_assert_eq,
            test_runner::{Config, TestRunner},
        };
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(1_i64..100_000, 1_i64..100_000, 1_i64..100_000),
                |(buy, sell, third)| {
                    let rows = vec![
                        row("buy", OrderSide::Buy, &buy.to_string(), 120),
                        row("sell", OrderSide::Sell, &sell.to_string(), 170),
                        row("currency", OrderSide::Buy, &third.to_string(), 200),
                    ];
                    let expected = money(&(buy - sell + third).to_string(), Currency::Krw);
                    prop_assert_eq!(calc(&facts(rows.clone())), Ok(expected));
                    prop_assert_eq!(calc(&facts(rows.into_iter().rev().collect())), Ok(expected));
                    Ok(())
                },
            )
            .expect("독립 정수 대조");
    }
    #[test]
    fn daily_connection_propagates_incomplete() {
        use crate::valuation_snapshot::tests::{at as day_at, observed};
        let old = observed(2, "999", "0");
        let today = observed(3, "123", "0");
        let orders = BrokerOrderSet::from_open_lookup_result(
            AccountId::new(7),
            vec![],
            OpenOrderCoverage::Incomplete,
            day_at(3, 7),
        )
        .expect("사실");
        let evidence =
            ClosedOrderEvidence::new(orders.clone(), old.valuation_at(), today.valuation_at());
        assert_eq!(
            holdings_daily_profit_with_orders(
                &today,
                Some(&old),
                (&orders, &AccountId::new(7)),
                Some(&evidence),
                Currency::Krw
            ),
            Ok(money("0", Currency::Krw))
        );
        assert_eq!(
            holdings_daily_profit_with_orders(
                &today,
                Some(&old),
                (&orders, &AccountId::new(7)),
                None,
                Currency::Krw
            ),
            Err(NetPurchaseError::IncompleteQuery)
        );
        assert_eq!(
            holdings_daily_profit_with_orders(
                &today,
                None,
                (&orders, &AccountId::new(7)),
                None,
                Currency::Krw
            ),
            Err(NetPurchaseError::Snapshot(SnapshotError::MissingPrevious))
        );
    }
    fn valuation(day: i64, kr: &str, usd: Option<&str>) -> ValuationSnapshot {
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
        let portfolio = Portfolio::new(
            vec![
                holding(KnownMarket::Kr, "005930", Currency::Krw, Some(kr)),
                holding(KnownMarket::Us, "AAPL", Currency::Usd, usd),
            ],
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
    fn daily_inputs() -> (
        ValuationSnapshot,
        ValuationSnapshot,
        BrokerOrderSet,
        ClosedOrderEvidence,
    ) {
        use crate::valuation_snapshot::tests::at as day_at;
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
        let orders = BrokerOrderSet::from_open_lookup_result(
            AccountId::new(7),
            vec![buy, sell],
            OpenOrderCoverage::Incomplete,
            day_at(3, 8),
        )
        .expect("주문");
        let evidence =
            ClosedOrderEvidence::new(orders.clone(), old.valuation_at(), today.valuation_at());
        (old, today, orders, evidence)
    }
    #[test]
    fn daily_formula_and_adr52_native_currency_example() {
        let (old, today, orders, evidence) = daily_inputs();
        assert_eq!(
            holdings_daily_profit_with_orders(
                &today,
                Some(&old),
                (&orders, &AccountId::new(7)),
                Some(&evidence),
                Currency::Krw
            ),
            Ok(money("174", Currency::Krw))
        );
        assert_eq!(
            holdings_daily_profit_with_orders(
                &today,
                Some(&old),
                (&orders, &AccountId::new(7)),
                Some(&evidence),
                Currency::Usd
            ),
            Ok(money("-500", Currency::Usd))
        );
    }
    #[test]
    fn daily_snapshot_errors() {
        let (old, today, orders, evidence) = daily_inputs();
        let missing = valuation(3, "1700", None);
        assert_eq!(
            holdings_daily_profit_with_orders(
                &missing,
                Some(&old),
                (&orders, &AccountId::new(7)),
                Some(&evidence),
                Currency::Usd
            ),
            Err(NetPurchaseError::Snapshot(
                SnapshotError::IncompleteValuation
            ))
        );
        let old_missing = valuation(2, "1400", None);
        assert_eq!(
            holdings_daily_profit_with_orders(
                &today,
                Some(&old_missing),
                (&orders, &AccountId::new(7)),
                Some(&evidence),
                Currency::Usd
            ),
            Err(NetPurchaseError::Snapshot(
                SnapshotError::IncompleteValuation
            ))
        );
        assert_eq!(
            holdings_daily_profit_with_orders(
                &old,
                Some(&today),
                (&orders, &AccountId::new(7)),
                Some(&evidence),
                Currency::Krw
            ),
            Err(NetPurchaseError::Snapshot(SnapshotError::WrongPeriod))
        );
    }
    #[test]
    fn equal_order_fill_and_observation_times_are_valid() {
        let mut input = row("edge", OrderSide::Buy, "23", 200);
        input.ordered_at = time(200);
        input.observed_at = time(200);
        let orders = BrokerOrderSet::from_open_lookup_result(
            AccountId::new(7),
            vec![input],
            OpenOrderCoverage::Incomplete,
            time(200),
        )
        .expect("경계");
        assert_eq!(calc(&orders), Ok(money("23", Currency::Krw)));
        assert_eq!(
            format!("{:?}", NetPurchaseScope::Currency(Currency::Krw)),
            "Currency(Krw)"
        );
    }
    #[test]
    fn cumulative_partial_fill_crossing_start_remains_incomplete() {
        let mut partial = row("partial", OrderSide::Buy, "81", 170);
        partial.status = BrokerOrderStatus::PartialFilled;
        partial.ordered_at = time(90);
        assert_eq!(
            calc(&facts(vec![partial])),
            Err(NetPurchaseError::UnallocatableExecution)
        );
        let mut past = row("past", OrderSide::Buy, "5", 99);
        past.ordered_at = time(80);
        past.filled_amount = BrokerOrderMoney::Absent;
        assert_eq!(calc(&facts(vec![past])), Ok(money("0", Currency::Krw)));
    }
}
