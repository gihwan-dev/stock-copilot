use crate::{
    BrokerOrder, BrokerOrderMoney, ClosedOrderCompleteness, ClosedOrderIncomplete, ClosedOrderSet,
    Currency, Money, MoneyError, OrderSide, ProvenClosedOrderSet, ResolvedOrderInstrument,
    SnapshotError, TimeInterval,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetPurchaseIncomplete {
    MissingTotal,
    UnallocatableExecution,
    MissingExecutionTime,
    Query(ClosedOrderIncomplete),
    OutsideQueryRange,
    ObservationTooEarly,
    Snapshot(SnapshotError),
    KrwSummaryNotComparable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetPurchaseInputError {
    AccountMismatch,
    NonClosedOrder,
    DuplicateOrder,
    ObservationMismatch,
    InvalidTime,
    ZeroFillMismatch,
    Arithmetic(MoneyError),
    Snapshot(SnapshotError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetPurchaseError {
    Incomplete(NetPurchaseIncomplete),
    InvalidInput(NetPurchaseInputError),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NetPurchases {
    kr: Result<Money, NetPurchaseError>,
    us: Result<Money, NetPurchaseError>,
}
impl NetPurchases {
    pub const fn currency(self, currency: Currency) -> Result<Money, NetPurchaseError> {
        match currency {
            Currency::Krw => self.kr,
            Currency::Usd => self.us,
        }
    }
}
impl fmt::Debug for NetPurchases {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NetPurchases").finish_non_exhaustive()
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
pub fn proven_net_purchases(orders: &ProvenClosedOrderSet, period: TimeInterval) -> NetPurchases {
    net_purchases(orders.orders(), period)
}
pub fn proven_net_purchase_amount(
    orders: &ProvenClosedOrderSet,
    period: TimeInterval,
    scope: NetPurchaseScope<'_>,
) -> Result<Money, NetPurchaseError> {
    net_purchase_amount(orders.orders(), period, scope)
}
#[must_use]
pub(crate) fn net_purchases(orders: &ClosedOrderSet, period: TimeInterval) -> NetPurchases {
    NetPurchases {
        kr: net_purchase_amount(orders, period, NetPurchaseScope::Currency(Currency::Krw)),
        us: net_purchase_amount(orders, period, NetPurchaseScope::Currency(Currency::Usd)),
    }
}
pub(crate) fn net_purchase_amount(
    orders: &ClosedOrderSet,
    period: TimeInterval,
    scope: NetPurchaseScope<'_>,
) -> Result<Money, NetPurchaseError> {
    validate_query(orders, period).map_err(NetPurchaseError::Incomplete)?;
    let mut total = crate::holding::zero(scope.currency());
    for order in orders.orders().values() {
        if scope.includes(order) == Inclusion::Excluded
            || order.ordered_at() > period.end()
            || order.filled_quantity().as_decimal().is_zero()
        {
            continue;
        }
        let at = order.filled_at().ok_or(NetPurchaseError::Incomplete(
            NetPurchaseIncomplete::MissingExecutionTime,
        ))?;
        if at <= period.start() {
            continue;
        }
        if order.ordered_at() <= period.start() || at > period.end() {
            return Err(NetPurchaseError::Incomplete(
                NetPurchaseIncomplete::UnallocatableExecution,
            ));
        }
        let BrokerOrderMoney::Known(amount) = order.filled_amount() else {
            return Err(NetPurchaseError::Incomplete(
                NetPurchaseIncomplete::MissingTotal,
            ));
        };
        total = match order.side() {
            OrderSide::Buy => total.checked_add(&amount),
            OrderSide::Sell => total.checked_sub(&amount),
        }
        .map_err(|error| {
            NetPurchaseError::InvalidInput(NetPurchaseInputError::Arithmetic(error))
        })?;
    }
    Ok(total)
}
fn validate_query(
    orders: &ClosedOrderSet,
    period: TimeInterval,
) -> Result<(), NetPurchaseIncomplete> {
    if orders.observed_at() < period.end() {
        return Err(NetPurchaseIncomplete::ObservationTooEarly);
    }
    if let Some(proof) = orders.completeness_proof() {
        return proof.verify_for_period(period);
    }
    if let ClosedOrderCompleteness::Incomplete(reason) = orders.completeness() {
        return Err(NetPurchaseIncomplete::Query(reason));
    }
    if orders.covered().start() > period.start() || orders.covered().end() < period.end() {
        return Err(NetPurchaseIncomplete::OutsideQueryRange);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        NetPurchaseError, NetPurchaseIncomplete, NetPurchaseInputError, NetPurchaseScope,
        net_purchase_amount, net_purchases,
    };
    use crate::{
        AccountId, BrokerOrder, BrokerOrderMoney, ClosedOrderCompleteness, ClosedOrderIncomplete,
        ClosedOrderSet, TimeInterval,
    };
    use crate::{
        CommonShare, Currency, DisplayInstrumentName, InstrumentId, KnownMarket,
        KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Nanosecond, NxtTrading, OffsetSeconds,
        ResolvedOrderInstrument, Symbol, Timestamp, TradingSuspension, UnixSeconds,
    };
    use crate::{Money, OrderSide};
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
        BrokerOrderTime, BrokerOrderType, BrokerTimeInForce, NumericInput, Price, Quantity,
    };
    use rust_decimal::Decimal;
    pub(crate) fn time(text: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(text),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    pub(crate) fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
    }
    pub(crate) fn row(id: &str, side: OrderSide, amount: &str, at: i64) -> BrokerOrderRow {
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

    pub(crate) fn interval(start: i64, end: i64) -> TimeInterval {
        TimeInterval::try_new(time(start), time(end)).expect("구간")
    }
    pub(crate) fn facts(rows: Vec<BrokerOrderRow>) -> ClosedOrderSet {
        ClosedOrderSet::new(
            AccountId::new(7),
            rows.into_iter()
                .map(|input| BrokerOrder::from_lookup_result(input).expect("사실"))
                .collect(),
            interval(0, 300),
            time(300),
            ClosedOrderCompleteness::Complete,
        )
        .expect("종료 집합")
    }
    fn calc(orders: &ClosedOrderSet) -> Result<Money, NetPurchaseError> {
        net_purchase_amount(
            orders,
            interval(100, 200),
            NetPurchaseScope::Currency(Currency::Krw),
        )
    }
    fn incomplete(reason: NetPurchaseIncomplete) -> Result<Money, NetPurchaseError> {
        Err(NetPurchaseError::Incomplete(reason))
    }
    #[test]
    fn review_zero_fill_and_future_orders_are_exact_zero() {
        let mut canceled = row("canceled", OrderSide::Buy, "0", 150);
        canceled.status = BrokerOrderStatus::Canceled;
        canceled.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("수량");
        canceled.filled_at = None;
        canceled.filled_amount = BrokerOrderMoney::Absent;
        assert_eq!(
            calc(&facts(vec![canceled.clone()])),
            Ok(money("0", Currency::Krw))
        );
        canceled.filled_amount = BrokerOrderMoney::Known(money("0", Currency::Krw));
        let mut future = row("future", OrderSide::Buy, "83", 250);
        future.ordered_at = time(240);
        assert_eq!(
            calc(&facts(vec![canceled, future])),
            Ok(money("0", Currency::Krw))
        );
    }
    #[test]
    fn realistic_day_is_complete_and_order_independent() {
        let mut canceled = row("cancel", OrderSide::Buy, "0", 150);
        canceled.status = BrokerOrderStatus::Canceled;
        canceled.filled_quantity = Quantity::try_new(Decimal::ZERO).expect("수량");
        canceled.filled_at = None;
        canceled.filled_amount = BrokerOrderMoney::Absent;
        let mut future = row("future", OrderSide::Buy, "9999", 250);
        future.ordered_at = time(240);
        let mut yesterday = row("old", OrderSide::Sell, "777", 90);
        yesterday.ordered_at = time(60);
        let rows = vec![
            row("buy1", OrderSide::Buy, "157", 160),
            row("buy2", OrderSide::Buy, "89", 200),
            row("sell", OrderSide::Sell, "31", 180),
            canceled,
            future,
            yesterday,
        ];
        assert_eq!(calc(&facts(rows.clone())), Ok(money("215", Currency::Krw)));
        assert_eq!(
            calc(&facts(rows.into_iter().rev().collect())),
            Ok(money("215", Currency::Krw))
        );
    }
    #[test]
    fn time_order_table() {
        for (ordered, filled, expected) in [
            (60, 90, Ok(money("0", Currency::Krw))),
            (90, 100, Ok(money("0", Currency::Krw))),
            (110, 110, Ok(money("41", Currency::Krw))),
            (110, 200, Ok(money("41", Currency::Krw))),
            (
                100,
                170,
                incomplete(NetPurchaseIncomplete::UnallocatableExecution),
            ),
            (
                90,
                170,
                incomplete(NetPurchaseIncomplete::UnallocatableExecution),
            ),
            (
                110,
                201,
                incomplete(NetPurchaseIncomplete::UnallocatableExecution),
            ),
            (
                90,
                250,
                incomplete(NetPurchaseIncomplete::UnallocatableExecution),
            ),
            (201, 250, Ok(money("0", Currency::Krw))),
            (300, 300, Ok(money("0", Currency::Krw))),
        ] {
            let mut input = row("table", OrderSide::Buy, "41", filled);
            input.ordered_at = time(ordered);
            assert_eq!(
                calc(&facts(vec![input])),
                expected,
                "o={ordered}, f={filled}"
            );
        }
    }
    #[test]
    fn missing_time_total_and_query_remain_incomplete() {
        let mut input = row("missing", OrderSide::Buy, "41", 150);
        input.filled_at = None;
        assert_eq!(
            calc(&facts(vec![input.clone()])),
            incomplete(NetPurchaseIncomplete::MissingExecutionTime)
        );
        input.filled_at = Some(time(150));
        input.filled_amount = BrokerOrderMoney::Absent;
        assert_eq!(
            calc(&facts(vec![input])),
            incomplete(NetPurchaseIncomplete::MissingTotal)
        );
        let mut orders = facts(vec![]);
        for reason in [
            ClosedOrderIncomplete::MissingPages,
            ClosedOrderIncomplete::RejectedRows,
            ClosedOrderIncomplete::RequestFailed,
        ] {
            orders = ClosedOrderSet::new(
                AccountId::new(7),
                vec![],
                interval(0, 300),
                time(300),
                ClosedOrderCompleteness::Incomplete(reason),
            )
            .expect("불완전");
            assert_eq!(
                calc(&orders),
                incomplete(NetPurchaseIncomplete::Query(reason))
            );
        }
        assert_eq!(
            orders.completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RequestFailed)
        );
    }
    #[test]
    fn coverage_and_observation_boundaries() {
        for (start, end, observed, expected) in [
            (100, 200, 200, Ok(money("0", Currency::Krw))),
            (
                101,
                200,
                200,
                incomplete(NetPurchaseIncomplete::OutsideQueryRange),
            ),
            (
                100,
                199,
                300,
                incomplete(NetPurchaseIncomplete::OutsideQueryRange),
            ),
            (
                0,
                190,
                199,
                incomplete(NetPurchaseIncomplete::ObservationTooEarly),
            ),
        ] {
            let orders = ClosedOrderSet::new(
                AccountId::new(7),
                vec![],
                interval(start, end),
                time(observed),
                ClosedOrderCompleteness::Complete,
            )
            .expect("범위");
            assert_eq!(calc(&orders), expected);
        }
    }
    #[test]
    fn scopes_native_amount_orders_and_arithmetic() {
        let mut usd = row("usd", OrderSide::Buy, "1", 170);
        usd.instrument = instrument(KnownMarket::Us, "AAPL");
        usd.average_filled_price = None;
        usd.filled_amount = BrokerOrderMoney::Known(money("43.21", Currency::Usd));
        usd.size = BrokerOrderSize::Amount {
            reported_quantity: Quantity::try_new(Decimal::ZERO).expect("수량"),
            order_amount: money("99", Currency::Usd),
        };
        let orders = facts(vec![usd, row("kr", OrderSide::Buy, "71", 180)]);
        let sums = net_purchases(&orders, interval(100, 200));
        assert_eq!(sums.currency(Currency::Krw), Ok(money("71", Currency::Krw)));
        assert_eq!(
            sums.currency(Currency::Usd),
            Ok(money("43.21", Currency::Usd))
        );
        let other = instrument(KnownMarket::Kr, "000660");
        let target = instrument(KnownMarket::Kr, "005930");
        assert_eq!(
            net_purchase_amount(
                &orders,
                interval(100, 200),
                NetPurchaseScope::Instrument(&other)
            ),
            Ok(money("0", Currency::Krw))
        );
        assert_eq!(
            net_purchase_amount(
                &orders,
                interval(100, 200),
                NetPurchaseScope::Instrument(&target)
            ),
            Ok(money("71", Currency::Krw))
        );
        assert_eq!(format!("{sums:?}"), "NetPurchases { .. }");
        assert_eq!(
            format!("{:?}", NetPurchaseScope::Currency(Currency::Usd)),
            "Currency(Usd)"
        );
        assert_eq!(
            calc(&facts(vec![
                row("a", OrderSide::Buy, "79228162514264337593543950335", 150),
                row("b", OrderSide::Buy, "1", 160)
            ])),
            Err(NetPurchaseError::InvalidInput(
                NetPurchaseInputError::Arithmetic(crate::MoneyError::Overflow)
            ))
        );
    }
    #[test]
    fn independent_integer_oracle_and_permutations_4096() {
        use proptest::{
            prop_assert_eq,
            test_runner::{Config, TestRunner},
        };
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(1_i64..100_000, 1_i64..100_000, 1_i64..100_000),
                |(first, second, sold)| {
                    let rows = vec![
                        row("buy1", OrderSide::Buy, &first.to_string(), 120),
                        row("buy2", OrderSide::Buy, &second.to_string(), 200),
                        row("sell", OrderSide::Sell, &sold.to_string(), 170),
                    ];
                    let expected = money(&(first + second - sold).to_string(), Currency::Krw);
                    prop_assert_eq!(calc(&facts(rows.clone())), Ok(expected));
                    prop_assert_eq!(calc(&facts(rows.into_iter().rev().collect())), Ok(expected));
                    Ok(())
                },
            )
            .expect("独立 정수");
    }

    fn assert_time_ordering(ordered: i64, filled: i64, observed: i64) {
        let mut input = row("matrix", OrderSide::Buy, "41", filled);
        input.ordered_at = time(ordered);
        input.observed_at = time(observed);
        let sealed = ClosedOrderSet::new(
            AccountId::new(7),
            vec![BrokerOrder::from_lookup_result(input).expect("행")],
            interval(0, observed),
            time(observed),
            ClosedOrderCompleteness::Complete,
        );
        if ordered > observed || filled < ordered || filled > observed {
            assert_eq!(sealed, Err(NetPurchaseInputError::InvalidTime));
            return;
        }
        let expected = if observed < 200 {
            incomplete(NetPurchaseIncomplete::ObservationTooEarly)
        } else if ordered > 200 || filled <= 100 {
            Ok(money("0", Currency::Krw))
        } else if ordered <= 100 || filled > 200 {
            incomplete(NetPurchaseIncomplete::UnallocatableExecution)
        } else {
            Ok(money("41", Currency::Krw))
        };
        assert_eq!(
            calc(&sealed.expect("유효 시각")),
            expected,
            "o={ordered}, f={filled}, R={observed}"
        );
    }
    #[test]
    fn every_time_ordering_including_equalities_matches_the_decision_table() {
        let instants = [25, 50, 75, 100, 125, 150, 175, 200, 225, 250, 275];
        for ordered in instants {
            for filled in instants {
                assert_observation_orderings(ordered, filled, &instants);
            }
        }
    }

    fn assert_observation_orderings(ordered: i64, filled: i64, instants: &[i64]) {
        for observed in instants {
            assert_time_ordering(ordered, filled, *observed);
        }
    }
    fn assert_missing_time(ordered: i64, observed: i64, quantity: u32) {
        let mut input = row("missing", OrderSide::Buy, "0", 150);
        input.ordered_at = time(ordered);
        input.observed_at = time(observed);
        input.filled_quantity = Quantity::try_new(Decimal::from(quantity)).expect("수량");
        input.filled_at = None;
        input.filled_amount = BrokerOrderMoney::Absent;
        let sealed = ClosedOrderSet::new(
            AccountId::new(7),
            vec![BrokerOrder::from_lookup_result(input).expect("행")],
            interval(0, observed),
            time(observed),
            ClosedOrderCompleteness::Complete,
        );
        if ordered > observed {
            assert_eq!(sealed, Err(NetPurchaseInputError::InvalidTime));
            return;
        }
        let expected = if observed < 200 {
            incomplete(NetPurchaseIncomplete::ObservationTooEarly)
        } else if quantity == 0 || ordered > 200 {
            Ok(money("0", Currency::Krw))
        } else {
            incomplete(NetPurchaseIncomplete::MissingExecutionTime)
        };
        assert_eq!(
            calc(&sealed.expect("행")),
            expected,
            "o={ordered}, R={observed}, q={quantity}"
        );
    }
    #[test]
    fn missing_fill_time_table_distinguishes_zero_and_nonzero_execution() {
        let instants = [25, 50, 75, 100, 125, 150, 175, 200, 225, 250, 275];
        for ordered in instants {
            for observed in instants {
                assert_missing_time(ordered, observed, 0);
                assert_missing_time(ordered, observed, 3);
            }
        }
    }

    #[test]
    fn past_execution_needs_no_total_and_canceled_partial_execution_is_counted() {
        let mut past = row("past", OrderSide::Buy, "9", 100);
        past.ordered_at = time(50);
        past.filled_amount = BrokerOrderMoney::Absent;
        assert_eq!(calc(&facts(vec![past])), Ok(money("0", Currency::Krw)));
        let mut partial = row("partial", OrderSide::Buy, "37", 150);
        partial.status = BrokerOrderStatus::Canceled;
        assert_eq!(
            calc(&facts(vec![partial.clone()])),
            Ok(money("37", Currency::Krw))
        );
        partial.ordered_at = time(90);
        assert_eq!(
            calc(&facts(vec![partial])),
            incomplete(NetPurchaseIncomplete::UnallocatableExecution)
        );
    }
}
