use crate::{
    BrokerOrderStatus, Currency, Market, OrderSide, Price, Quantity, ResolvedOrderInstrument,
    Timestamp,
};
use std::fmt;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BrokerOrderId(String);
impl BrokerOrderId {
    pub fn try_new(value: &str) -> Result<Self, BrokerOrderError> {
        if value.is_empty()
            || value.len() > 256
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(BrokerOrderError::InvalidOrderId);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for BrokerOrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrderId").finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrokerOrderError {
    InvalidOrderId,
    FilledExceedsQuantity,
    CurrencyMismatch,
    DuplicateOrderId,
    ObservationMismatch,
    Arithmetic(crate::MoneyError),
}
#[derive(Clone, PartialEq, Eq)]
pub struct BrokerOrderRow {
    pub order_id: BrokerOrderId,
    pub instrument: ResolvedOrderInstrument,
    pub side: OrderSide,
    pub status: BrokerOrderStatus,
    pub quantity: Quantity,
    pub filled_quantity: Quantity,
    pub average_filled_price: Option<Price>,
    pub filled_at: Option<Timestamp>,
    pub observed_at: Timestamp,
}
impl fmt::Debug for BrokerOrderRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrderRow").finish_non_exhaustive()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct BrokerOrder {
    row: BrokerOrderRow,
    unfilled_quantity: Quantity,
}
impl BrokerOrder {
    pub fn from_lookup_result(row: BrokerOrderRow) -> Result<Self, BrokerOrderError> {
        let remaining = crate::Amount::new(row.quantity.as_decimal())
            .checked_sub(&crate::Amount::new(row.filled_quantity.as_decimal()))
            .map_err(BrokerOrderError::Arithmetic)?;
        let unfilled_quantity = Quantity::try_new(remaining.as_decimal())
            .map_err(|_error| BrokerOrderError::FilledExceedsQuantity)?;
        let currency = if row.instrument.market() == Market::Kr {
            Currency::Krw
        } else {
            Currency::Usd
        };
        if row
            .average_filled_price
            .is_some_and(|price| price.as_money().currency() != currency)
        {
            return Err(BrokerOrderError::CurrencyMismatch);
        }
        Ok(Self {
            row,
            unfilled_quantity,
        })
    }
    #[must_use]
    pub fn row(&self) -> &BrokerOrderRow {
        &self.row
    }
    #[must_use]
    pub fn unfilled_quantity(&self) -> Quantity {
        self.unfilled_quantity
    }
    #[must_use]
    pub fn filled_quantity(&self) -> Quantity {
        self.row.filled_quantity
    }
    #[must_use]
    pub fn average_filled_price(&self) -> Option<Price> {
        self.row.average_filled_price
    }
    #[must_use]
    pub fn filled_at(&self) -> Option<Timestamp> {
        self.row.filled_at
    }
    #[must_use]
    pub fn observed_at(&self) -> Timestamp {
        self.row.observed_at
    }
}
impl fmt::Debug for BrokerOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrokerOrder").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AccountId, BrokerOrder, BrokerOrderError, BrokerOrderId, BrokerOrderRow, BrokerOrderSet,
        BrokerOrderStatus, Currency, Market, MasterCode, MoneyError, Nanosecond, NumericInput,
        OffsetSeconds, OpenOrderCoverage, OppositeOpenOrder, OrderOpenState, OrderSide, Price,
        Quantity, ResolvedOrderInstrument, SellOrderQuantity, SellableQuantityDecision,
        SellableQuantityError, SellableQuantityFact, StockWarning, StockWarningDecision,
        StockWarnings, Symbol, Timestamp, UnixSeconds, check_sellable_quantity,
        check_stock_warnings, opposite_open_order,
    };
    use proptest::{
        arbitrary::any,
        prop_assert_eq,
        test_runner::{Config, TestRunner},
    };
    use rust_decimal::Decimal;

    fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(100),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn qty(value: u32) -> Quantity {
        Quantity::try_new(Decimal::from(value)).expect("수량")
    }
    fn instrument(market: Market, symbol: &str) -> ResolvedOrderInstrument {
        ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new(symbol).expect("종목"),
            market,
        )
        .expect("시장")
    }
    fn row(id: &str, side: OrderSide, status: BrokerOrderStatus) -> BrokerOrderRow {
        BrokerOrderRow {
            order_id: BrokerOrderId::try_new(id).expect("주문 키"),
            instrument: instrument(Market::Kr, "005930"),
            side,
            status,
            quantity: qty(10),
            filled_quantity: qty(0),
            average_filled_price: None,
            filled_at: None,
            observed_at: at(),
        }
    }
    #[test]
    fn sell_boundary_identity_and_accessors() {
        let stock = instrument(Market::Kr, "005930");
        let fact = SellableQuantityFact::from_lookup_result(
            AccountId::new(1),
            stock.clone(),
            qty(10),
            at(),
        )
        .expect("사실");
        assert_eq!(fact.account_id(), &AccountId::new(1));
        assert_eq!(fact.instrument(), &stock);
        assert_eq!(fact.quantity(), qty(10));
        assert_eq!(fact.observed_at(), at());
        for (account, symbol, count, expected) in [
            (1, "005930", 10, SellableQuantityDecision::Allowed),
            (1, "005930", 11, SellableQuantityDecision::Exceeded),
            (2, "005930", 1, SellableQuantityDecision::DifferentFact),
            (1, "000660", 1, SellableQuantityDecision::DifferentFact),
        ] {
            assert_eq!(
                check_sellable_quantity(
                    &fact,
                    &AccountId::new(account),
                    &instrument(Market::Kr, symbol),
                    SellOrderQuantity::try_new(qty(count)).expect("매도")
                ),
                expected
            );
        }
        let fractional = Quantity::try_new(Decimal::new(1, 1)).expect("소수");
        assert_eq!(
            SellableQuantityFact::from_lookup_result(AccountId::new(1), stock, fractional, at()),
            Err(SellableQuantityError::FractionalDomestic)
        );
        let fractional_fact = SellableQuantityFact::from_lookup_result(
            AccountId::new(1),
            instrument(Market::Us, "AAPL"),
            fractional,
            at(),
        )
        .expect("미국 소수");
        assert_eq!(fractional_fact.quantity(), fractional);
        assert_eq!(format!("{fact:?}"), "SellableQuantityFact { .. }");
    }
    #[test]
    fn sell_property_never_exceeds_fact() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_u32..10000, 1_u32..10000), |(available, requested)| {
                let stock = instrument(Market::Us, "AAPL");
                let fact = SellableQuantityFact::from_lookup_result(
                    AccountId::new(1),
                    stock.clone(),
                    qty(available),
                    at(),
                )
                .expect("사실");
                let result = check_sellable_quantity(
                    &fact,
                    &AccountId::new(1),
                    &stock,
                    SellOrderQuantity::try_new(qty(requested)).expect("매도"),
                );
                prop_assert_eq!(
                    result,
                    if requested <= available {
                        SellableQuantityDecision::Allowed
                    } else {
                        SellableQuantityDecision::Exceeded
                    }
                );
                Ok(())
            })
            .expect("상한 속성");
    }
    #[test]
    fn order_schema_mapping_and_pending_policy() {
        for (code, expected, pending) in [
            (
                "PENDING",
                BrokerOrderStatus::Pending,
                OrderOpenState::Possible,
            ),
            (
                "PENDING_CANCEL",
                BrokerOrderStatus::PendingCancel,
                OrderOpenState::Possible,
            ),
            (
                "PENDING_REPLACE",
                BrokerOrderStatus::PendingReplace,
                OrderOpenState::Possible,
            ),
            (
                "PARTIAL_FILLED",
                BrokerOrderStatus::PartialFilled,
                OrderOpenState::Possible,
            ),
            ("FILLED", BrokerOrderStatus::Filled, OrderOpenState::Closed),
            (
                "CANCELED",
                BrokerOrderStatus::Canceled,
                OrderOpenState::Closed,
            ),
            (
                "REJECTED",
                BrokerOrderStatus::Rejected,
                OrderOpenState::Closed,
            ),
            (
                "CANCEL_REJECTED",
                BrokerOrderStatus::CancelRejected,
                OrderOpenState::Closed,
            ),
            (
                "REPLACE_REJECTED",
                BrokerOrderStatus::ReplaceRejected,
                OrderOpenState::Closed,
            ),
            (
                "REPLACED",
                BrokerOrderStatus::Replaced,
                OrderOpenState::Closed,
            ),
        ] {
            assert_eq!(
                BrokerOrderStatus::from_code(MasterCode::try_new(code).expect("코드")),
                expected
            );
            assert_eq!(expected.open_state(), pending);
        }
        check_unknown_status();
    }
    fn check_unknown_status() {
        let unknown = BrokerOrderStatus::from_code(MasterCode::try_new("FUTURE").expect("코드"));
        assert_eq!(
            unknown,
            BrokerOrderStatus::Unknown(MasterCode::try_new("FUTURE").expect("코드"))
        );
        assert_eq!(unknown.open_state(), OrderOpenState::Possible);
        check_order_ids();
    }
    fn check_order_ids() {
        assert_eq!(
            BrokerOrderId::try_new(&"a".repeat(256))
                .expect("상한 키")
                .as_str()
                .len(),
            256
        );
        assert_eq!(
            BrokerOrderId::try_new(""),
            Err(BrokerOrderError::InvalidOrderId)
        );
        assert_eq!(
            BrokerOrderId::try_new(&"a".repeat(257)),
            Err(BrokerOrderError::InvalidOrderId)
        );
        assert_eq!(
            BrokerOrderId::try_new("a\nb"),
            Err(BrokerOrderError::InvalidOrderId)
        );
        assert_eq!(BrokerOrderId::try_new("a").expect("키").as_str(), "a");
    }
    #[test]
    fn broker_order_seals_execution_and_rejects_invalid_inputs() {
        let mut input = row("a", OrderSide::Sell, BrokerOrderStatus::PartialFilled);
        input.filled_quantity = qty(3);
        input.average_filled_price = Some(
            Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Krw)
                .expect("가격"),
        );
        input.filled_at = Some(at());
        let fact = BrokerOrder::from_lookup_result(input.clone()).expect("주문");
        assert_eq!(fact.row(), &input);
        assert_eq!(fact.unfilled_quantity(), qty(7));
        assert_eq!(fact.filled_quantity(), qty(3));
        assert_eq!(fact.average_filled_price(), input.average_filled_price);
        assert_eq!(fact.filled_at(), Some(at()));
        assert_eq!(fact.observed_at(), at());
        assert_eq!(format!("{fact:?}"), "BrokerOrder { .. }");
        assert_eq!(format!("{input:?}"), "BrokerOrderRow { .. }");
        assert_eq!(format!("{:?}", input.order_id), "BrokerOrderId { .. }");
        input.filled_quantity = qty(11);
        assert_eq!(
            BrokerOrder::from_lookup_result(input.clone()),
            Err(BrokerOrderError::FilledExceedsQuantity)
        );
        input.filled_quantity = qty(3);
        input.average_filled_price = Some(
            Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Usd)
                .expect("가격"),
        );
        assert_eq!(
            BrokerOrder::from_lookup_result(input),
            Err(BrokerOrderError::CurrencyMismatch)
        );
    }
    #[test]
    fn order_sets_preserve_identity_completeness_and_time() {
        let order =
            BrokerOrder::from_lookup_result(row("a", OrderSide::Sell, BrokerOrderStatus::Pending))
                .expect("注文");
        assert_eq!(
            BrokerOrderSet::from_open_lookup_result(
                AccountId::new(1),
                vec![order.clone(), order.clone()],
                OpenOrderCoverage::Complete,
                at()
            ),
            Err(BrokerOrderError::DuplicateOrderId)
        );
        let set = BrokerOrderSet::from_open_lookup_result(
            AccountId::new(1),
            vec![order],
            OpenOrderCoverage::Complete,
            at(),
        )
        .expect("목록");
        assert_eq!(set.account_id(), &AccountId::new(1));
        assert_eq!(set.observed_at(), at());
        assert_eq!(set.orders().len(), 1);
        assert_eq!(format!("{set:?}"), "BrokerOrderSet { .. }");
        let stock = instrument(Market::Kr, "005930");
        assert_eq!(
            opposite_open_order(&set, &AccountId::new(2), &stock, OrderSide::Buy),
            OppositeOpenOrder::DifferentAccount
        );
        assert_eq!(
            opposite_open_order(
                &set,
                &AccountId::new(1),
                &instrument(Market::Kr, "000660"),
                OrderSide::Buy
            ),
            OppositeOpenOrder::Absent
        );
        let incomplete = BrokerOrderSet::from_open_lookup_result(
            AccountId::new(1),
            vec![],
            OpenOrderCoverage::Incomplete,
            at(),
        )
        .expect("목록");
        assert_eq!(
            opposite_open_order(&incomplete, &AccountId::new(1), &stock, OrderSide::Buy),
            OppositeOpenOrder::Incomplete
        );
    }
    #[test]
    fn opposite_direction_market_symmetry_and_order_invariance() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(any::<bool>(), any::<bool>(), any::<bool>()),
                |(buy, us, reverse)| {
                    let side = if buy { OrderSide::Buy } else { OrderSide::Sell };
                    let opposite = if buy { OrderSide::Sell } else { OrderSide::Buy };
                    let stock = if us {
                        instrument(Market::Us, "AAPL")
                    } else {
                        instrument(Market::Kr, "005930")
                    };
                    let mut first = row("a", opposite, BrokerOrderStatus::Pending);
                    first.instrument = stock.clone();
                    let mut second = row("b", side, BrokerOrderStatus::Filled);
                    second.instrument = stock.clone();
                    let mut rows = vec![
                        BrokerOrder::from_lookup_result(first).expect("주문"),
                        BrokerOrder::from_lookup_result(second).expect("주문"),
                    ];
                    let original = BrokerOrderSet::from_open_lookup_result(
                        AccountId::new(1),
                        rows.clone(),
                        OpenOrderCoverage::Complete,
                        at(),
                    )
                    .expect("집합");
                    if reverse {
                        rows.reverse();
                    }
                    let permuted = BrokerOrderSet::from_open_lookup_result(
                        AccountId::new(1),
                        rows,
                        OpenOrderCoverage::Complete,
                        at(),
                    )
                    .expect("집합");
                    prop_assert_eq!(&original, &permuted);
                    let expected = if us {
                        OppositeOpenOrder::NotApplicable
                    } else {
                        OppositeOpenOrder::Present
                    };
                    prop_assert_eq!(
                        opposite_open_order(&original, &AccountId::new(1), &stock, side),
                        expected
                    );
                    Ok(())
                },
            )
            .expect("대칭 시장 순서");
    }
    #[test]
    fn warnings_schema_set_and_blocking() {
        let codes = [
            ("LIQUIDATION_TRADING", StockWarning::LiquidationTrading),
            ("OVERHEATED", StockWarning::Overheated),
            ("INVESTMENT_WARNING", StockWarning::InvestmentWarning),
            ("INVESTMENT_RISK", StockWarning::InvestmentRisk),
            ("VI_STATIC_AND_DYNAMIC", StockWarning::ViStaticAndDynamic),
            ("VI_STATIC", StockWarning::ViStatic),
            ("VI_DYNAMIC", StockWarning::ViDynamic),
            ("STOCK_WARRANTS", StockWarning::StockWarrants),
        ];
        let stock = instrument(Market::Kr, "005930");
        for (code, expected) in codes {
            assert_eq!(
                StockWarning::from_code(MasterCode::try_new(code).expect("코드")),
                expected
            );
            let facts = StockWarnings::from_lookup_result(
                stock.clone(),
                vec![expected.clone(), expected],
                at(),
            );
            assert_eq!(facts.warnings().len(), 1);
            assert_eq!(facts.instrument(), &stock);
            assert_eq!(facts.observed_at(), at());
            assert_eq!(
                check_stock_warnings(&facts, &stock),
                StockWarningDecision::Blocked
            );
            assert_eq!(
                check_stock_warnings(&facts, &instrument(Market::Kr, "000660")),
                StockWarningDecision::DifferentInstrument
            );
            assert_eq!(format!("{facts:?}"), "StockWarnings { .. }");
        }
        let unknown = StockWarning::from_code(MasterCode::try_new("FUTURE").expect("코드"));
        assert_eq!(
            unknown,
            StockWarning::Unknown(MasterCode::try_new("FUTURE").expect("코드"))
        );
        assert_eq!(
            check_stock_warnings(
                &StockWarnings::from_lookup_result(stock.clone(), vec![unknown], at()),
                &stock
            ),
            StockWarningDecision::Blocked
        );
        assert_eq!(
            check_stock_warnings(
                &StockWarnings::from_lookup_result(stock.clone(), vec![], at()),
                &stock
            ),
            StockWarningDecision::Clear
        );
    }

    #[test]
    fn unknown_orders_block_even_with_zero_remainder_and_times_must_match() {
        let stock = instrument(Market::Kr, "005930");
        let mut input = row(
            "unknown",
            OrderSide::Sell,
            BrokerOrderStatus::Unknown(MasterCode::try_new("FUTURE").expect("코드")),
        );
        input.filled_quantity = qty(10);
        let order = BrokerOrder::from_lookup_result(input).expect("주문");
        let set = BrokerOrderSet::from_open_lookup_result(
            AccountId::new(1),
            vec![order.clone()],
            OpenOrderCoverage::Incomplete,
            at(),
        )
        .expect("목록");
        assert_eq!(
            opposite_open_order(&set, &AccountId::new(1), &stock, OrderSide::Buy),
            OppositeOpenOrder::Present
        );
        assert_eq!(
            opposite_open_order(&set, &AccountId::new(1), &stock, OrderSide::Sell),
            OppositeOpenOrder::Incomplete
        );
        let later = Timestamp::from_parts(
            UnixSeconds::new(101),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        assert_eq!(
            BrokerOrderSet::from_open_lookup_result(
                AccountId::new(1),
                vec![order],
                OpenOrderCoverage::Complete,
                later
            ),
            Err(BrokerOrderError::ObservationMismatch)
        );
    }

    #[test]
    fn warning_set_arbitrary_permutations_preserve_decision() {
        TestRunner::new(Config::with_cases(4096))
            .run(&proptest::collection::vec(0_u8..4, 0..30), |values| {
                let stock = instrument(Market::Kr, "005930");
                let warnings: Vec<_> = values
                    .iter()
                    .map(|value| match value {
                        0 => StockWarning::ViStatic,
                        1 => StockWarning::InvestmentRisk,
                        2 => StockWarning::Unknown(MasterCode::try_new("FUTURE-A").expect("코드")),
                        _ => StockWarning::Unknown(MasterCode::try_new("FUTURE-B").expect("코드")),
                    })
                    .collect();
                let original =
                    StockWarnings::from_lookup_result(stock.clone(), warnings.clone(), at());
                let permuted = StockWarnings::from_lookup_result(
                    stock.clone(),
                    warnings.into_iter().rev().collect(),
                    at(),
                );
                prop_assert_eq!(&original, &permuted);
                prop_assert_eq!(
                    check_stock_warnings(&original, &stock),
                    if values.is_empty() {
                        StockWarningDecision::Clear
                    } else {
                        StockWarningDecision::Blocked
                    }
                );
                Ok(())
            })
            .expect("집합 순서");
    }

    #[test]
    fn terminal_opposite_orders_do_not_block_and_same_side_never_conflicts() {
        for status in [
            BrokerOrderStatus::Filled,
            BrokerOrderStatus::Canceled,
            BrokerOrderStatus::Rejected,
            BrokerOrderStatus::CancelRejected,
            BrokerOrderStatus::ReplaceRejected,
            BrokerOrderStatus::Replaced,
        ] {
            let order =
                BrokerOrder::from_lookup_result(row("a", OrderSide::Sell, status)).expect("주문");
            let set = BrokerOrderSet::from_open_lookup_result(
                AccountId::new(1),
                vec![order],
                OpenOrderCoverage::Complete,
                at(),
            )
            .expect("목록");
            assert_eq!(
                opposite_open_order(
                    &set,
                    &AccountId::new(1),
                    &instrument(Market::Kr, "005930"),
                    OrderSide::Buy
                ),
                OppositeOpenOrder::Absent
            );
        }
        let order =
            BrokerOrder::from_lookup_result(row("a", OrderSide::Buy, BrokerOrderStatus::Pending))
                .expect("주문");
        let set = BrokerOrderSet::from_open_lookup_result(
            AccountId::new(1),
            vec![order],
            OpenOrderCoverage::Complete,
            at(),
        )
        .expect("목록");
        assert_eq!(
            opposite_open_order(
                &set,
                &AccountId::new(1),
                &instrument(Market::Kr, "005930"),
                OrderSide::Buy
            ),
            OppositeOpenOrder::Absent
        );
    }

    #[test]
    fn warning_set_equality_includes_instrument_time_and_membership() {
        let stock = instrument(Market::Kr, "005930");
        let base =
            StockWarnings::from_lookup_result(stock.clone(), vec![StockWarning::ViStatic], at());
        assert_ne!(
            base,
            StockWarnings::from_lookup_result(
                instrument(Market::Kr, "000660"),
                vec![StockWarning::ViStatic],
                at()
            )
        );
        assert_ne!(
            base,
            StockWarnings::from_lookup_result(stock.clone(), vec![StockWarning::ViDynamic], at())
        );
        assert_ne!(
            base,
            StockWarnings::from_lookup_result(stock.clone(), vec![], at())
        );
        let later = Timestamp::from_parts(
            UnixSeconds::new(101),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        assert_ne!(
            base,
            StockWarnings::from_lookup_result(stock, vec![StockWarning::ViStatic], later)
        );
    }

    #[test]
    fn remaining_quantity_rejects_precision_loss() {
        let mut input = row(
            "precision",
            OrderSide::Buy,
            BrokerOrderStatus::PartialFilled,
        );
        input.quantity = Quantity::try_new(Decimal::MAX).expect("최대 수량");
        input.filled_quantity = Quantity::try_new(Decimal::new(1, 1)).expect("소수");
        assert_eq!(
            BrokerOrder::from_lookup_result(input),
            Err(BrokerOrderError::Arithmetic(MoneyError::PrecisionLoss))
        );
    }
}
