mod order_gate_support;
#[cfg(test)]
mod tests {
    use crate::order_gate_support::tests::{at, instrument};
    use domain::{
        AccountId, BrokerOrder, BrokerOrderError, BrokerOrderId, BrokerOrderMoney,
        BrokerOrderPrice, BrokerOrderRow, BrokerOrderSet, BrokerOrderSize, BrokerOrderStatus,
        BrokerOrderTime, BrokerOrderType, BrokerTimeInForce, Currency, KnownMarket, MasterCode,
        Money, MoneyError, Nanosecond, NumericInput, OffsetSeconds, OpenOrderCoverage,
        OppositeOpenOrder, OrderOpenState, OrderSide, Price, Quantity, Timestamp, UnfilledQuantity,
        UnixSeconds, opposite_open_order,
    };
    use proptest::{
        arbitrary::any,
        prop_assert_eq,
        test_runner::{Config, TestRunner},
    };
    use rust_decimal::Decimal;
    fn time(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn quantity(value: u32) -> Quantity {
        Quantity::try_new(Decimal::from(value)).expect("수량")
    }
    fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
    }
    fn price(currency: Currency) -> Price {
        Price::parse(&NumericInput::try_new("100").expect("입력"), currency).expect("가격")
    }
    fn row(id: &str, side: OrderSide, status: BrokerOrderStatus) -> BrokerOrderRow {
        BrokerOrderRow {
            account_id: AccountId::new(1),
            order_id: BrokerOrderId::try_new(id).expect("키"),
            instrument: instrument(KnownMarket::Kr, "005930"),
            side,
            status,
            size: BrokerOrderSize::Quantity(quantity(10)),
            filled_quantity: quantity(0),
            average_filled_price: None,
            filled_amount: BrokerOrderMoney::Absent,
            filled_at: None,
            ordered_at: at(),
            canceled_at: BrokerOrderTime::Absent,
            price: BrokerOrderPrice::Absent,
            order_type: BrokerOrderType::Market,
            time_in_force: BrokerTimeInForce::Day,
            observed_at: at(),
        }
    }
    fn set(
        rows: Vec<BrokerOrderRow>,
        coverage: OpenOrderCoverage,
    ) -> Result<BrokerOrderSet, BrokerOrderError> {
        BrokerOrderSet::from_open_lookup_result(AccountId::new(1), rows, coverage, at())
    }
    #[test]
    fn filled_amount_is_independent_and_every_field_is_readable_without_extracting_input() {
        let mut input = row("a", OrderSide::Sell, BrokerOrderStatus::PartialFilled);
        input.order_type = BrokerOrderType::Limit;
        input.time_in_force = BrokerTimeInForce::Opg;
        input.filled_quantity = quantity(3);
        input.average_filled_price = Some(price(Currency::Krw));
        input.filled_amount = BrokerOrderMoney::Known(money("301", Currency::Krw));
        input.filled_at = Some(time(95));
        input.ordered_at = time(80);
        input.price = BrokerOrderPrice::Known(
            Price::parse(&NumericInput::try_new("101").expect("입력"), Currency::Krw)
                .expect("가격"),
        );
        input.canceled_at = BrokerOrderTime::Known(time(90));
        let order = BrokerOrder::from_lookup_result(input.clone()).expect("주문");
        assert_eq!(order.account_id(), &input.account_id);
        assert_eq!(order.order_id(), &input.order_id);
        assert_eq!(order.instrument(), &input.instrument);
        assert_eq!(order.side(), input.side);
        assert_eq!(order.status(), &input.status);
        assert_eq!(order.size(), input.size);
        assert_eq!(order.quantity(), quantity(10));
        assert_eq!(
            order.unfilled_quantity(),
            UnfilledQuantity::Known(quantity(7))
        );
        assert_eq!(order.filled_quantity(), quantity(3));
        assert_eq!(order.average_filled_price(), input.average_filled_price);
        assert_eq!(order.filled_amount(), input.filled_amount);
        assert_eq!(order.filled_at(), Some(time(95)));
        assert_eq!(order.observed_at(), at());
        assert_eq!(order.ordered_at(), time(80));
        assert_eq!(order.canceled_at(), input.canceled_at);
        assert_eq!(order.price(), input.price);
        assert_eq!(order.order_type(), &input.order_type);
        assert_eq!(order.time_in_force(), &input.time_in_force);
        assert_eq!(order.order_amount(), BrokerOrderMoney::Absent);
        assert_eq!(format!("{order:?}"), "BrokerOrder { .. }");
        assert_eq!(format!("{input:?}"), "BrokerOrderRow { .. }");
        assert_eq!(format!("{:?}", input.order_id), "BrokerOrderId { .. }");
        assert_eq!(
            format!("{:?}", input.filled_amount),
            "BrokerOrderMoney { .. }"
        );
        assert_eq!(format!("{:?}", input.price), "BrokerOrderPrice { .. }");
        assert_eq!(format!("{:?}", input.size), "BrokerOrderSize { .. }");
    }
    #[test]
    fn amount_buy_accepts_zero_reported_quantity_and_fractional_fills_with_unknown_remainder() {
        let mut input = row("amount", OrderSide::Buy, BrokerOrderStatus::PartialFilled);
        input.instrument = instrument(KnownMarket::Us, "AAPL");
        input.size = BrokerOrderSize::Amount {
            reported_quantity: quantity(0),
            order_amount: money("100", Currency::Usd),
        };
        input.filled_quantity = Quantity::try_new(Decimal::new(5, 1)).expect("소수");
        input.average_filled_price = Some(price(Currency::Usd));
        input.filled_amount = BrokerOrderMoney::Known(money("50.01", Currency::Usd));
        let order = BrokerOrder::from_lookup_result(input.clone()).expect("금액 주문");
        assert_eq!(order.quantity(), quantity(0));
        assert_eq!(order.filled_quantity(), input.filled_quantity);
        assert_eq!(
            order.unfilled_quantity(),
            UnfilledQuantity::UnknownAmountOrder
        );
        assert_eq!(
            order.order_amount(),
            BrokerOrderMoney::Known(money("100", Currency::Usd))
        );
        assert_eq!(order.filled_amount(), input.filled_amount);
        let facts = set(vec![input], OpenOrderCoverage::Complete).expect("목록");
        assert_eq!(
            opposite_open_order(
                &facts,
                &AccountId::new(1),
                &instrument(KnownMarket::Us, "AAPL"),
                OrderSide::Sell
            ),
            OppositeOpenOrder::NotApplicable
        );
    }
    #[test]
    fn order_validation_rejects_currency_mismatch_excess_and_precision_loss() {
        let mut input = row("a", OrderSide::Sell, BrokerOrderStatus::Pending);
        input.filled_quantity = quantity(11);
        assert_eq!(
            set(vec![input.clone()], OpenOrderCoverage::Complete),
            Err(BrokerOrderError::FilledExceedsQuantity)
        );
        assert_eq!(
            BrokerOrder::from_lookup_result(input.clone()),
            Err(BrokerOrderError::FilledExceedsQuantity)
        );
        input.filled_quantity = quantity(0);
        input.average_filled_price = Some(price(Currency::Usd));
        assert_eq!(
            BrokerOrder::from_lookup_result(input.clone()),
            Err(BrokerOrderError::CurrencyMismatch)
        );
        input.average_filled_price = None;
        input.price = BrokerOrderPrice::Known(price(Currency::Usd));
        assert_eq!(
            BrokerOrder::from_lookup_result(input.clone()),
            Err(BrokerOrderError::CurrencyMismatch)
        );
        input.price = BrokerOrderPrice::Absent;
        input.filled_amount = BrokerOrderMoney::Known(money("1", Currency::Usd));
        assert_eq!(
            BrokerOrder::from_lookup_result(input.clone()),
            Err(BrokerOrderError::CurrencyMismatch)
        );
        input.filled_amount = BrokerOrderMoney::Known(money("-1", Currency::Krw));
        assert_eq!(
            BrokerOrder::from_lookup_result(input.clone()),
            Err(BrokerOrderError::NegativeFilledAmount)
        );
        input.filled_amount = BrokerOrderMoney::Absent;
        input.size = BrokerOrderSize::Quantity(Quantity::try_new(Decimal::MAX).expect("최대"));
        input.filled_quantity = Quantity::try_new(Decimal::new(1, 1)).expect("소수");
        assert_eq!(
            BrokerOrder::from_lookup_result(input),
            Err(BrokerOrderError::Arithmetic(MoneyError::PrecisionLoss))
        );
    }
    #[test]
    fn amount_variant_rejects_non_us_market_buy_and_invalid_amounts() {
        let mut input = row("amount", OrderSide::Buy, BrokerOrderStatus::Pending);
        input.instrument = instrument(KnownMarket::Us, "AAPL");
        for amount in [
            money("1", Currency::Krw),
            money("0", Currency::Usd),
            money("-1", Currency::Usd),
        ] {
            input.size = BrokerOrderSize::Amount {
                reported_quantity: quantity(0),
                order_amount: amount,
            };
            assert_eq!(
                BrokerOrder::from_lookup_result(input.clone()),
                Err(BrokerOrderError::InvalidAmountOrder)
            );
        }
        input.size = BrokerOrderSize::Amount {
            reported_quantity: quantity(0),
            order_amount: money("1", Currency::Usd),
        };
        input.side = OrderSide::Sell;
        assert_eq!(
            BrokerOrder::from_lookup_result(input.clone()),
            Err(BrokerOrderError::InvalidAmountOrder)
        );
        input.side = OrderSide::Buy;
        input.order_type = BrokerOrderType::Limit;
        assert_eq!(
            BrokerOrder::from_lookup_result(input.clone()),
            Err(BrokerOrderError::InvalidAmountOrder)
        );
        input.order_type = BrokerOrderType::Market;
        input.instrument = instrument(KnownMarket::Kr, "005930");
        assert_eq!(
            BrokerOrder::from_lookup_result(input),
            Err(BrokerOrderError::InvalidAmountOrder)
        );
    }
    #[test]
    fn account_duplicates_time_and_complete_open_membership_are_enforced() {
        let input = row("a", OrderSide::Sell, BrokerOrderStatus::Pending);
        assert_eq!(
            set(
                vec![input.clone(), input.clone()],
                OpenOrderCoverage::Complete
            ),
            Err(BrokerOrderError::DuplicateOrderId)
        );
        let mut wrong = input.clone();
        wrong.account_id = AccountId::new(2);
        assert_eq!(
            set(vec![wrong], OpenOrderCoverage::Complete),
            Err(BrokerOrderError::AccountMismatch)
        );
        let mut stale = input.clone();
        stale.observed_at = Timestamp::from_parts(
            UnixSeconds::new(101),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        assert_eq!(
            set(vec![stale], OpenOrderCoverage::Complete),
            Err(BrokerOrderError::ObservationMismatch)
        );
        let facts = set(vec![input], OpenOrderCoverage::Complete).expect("목록");
        assert_eq!(facts.account_id(), &AccountId::new(1));
        assert_eq!(facts.coverage(), OpenOrderCoverage::Complete);
        assert_eq!(facts.observed_at(), at());
        assert_eq!(facts.orders().len(), 1);
        assert_eq!(format!("{facts:?}"), "BrokerOrderSet { .. }");
        for status in [
            BrokerOrderStatus::Filled,
            BrokerOrderStatus::Canceled,
            BrokerOrderStatus::Rejected,
            BrokerOrderStatus::CancelRejected,
            BrokerOrderStatus::ReplaceRejected,
            BrokerOrderStatus::Replaced,
        ] {
            let closed = row("closed", OrderSide::Sell, status);
            assert_eq!(
                set(vec![closed.clone()], OpenOrderCoverage::Complete),
                Err(BrokerOrderError::ClosedInOpenResult)
            );
            let partial = set(vec![closed], OpenOrderCoverage::Incomplete).expect("부분 목록");
            assert_eq!(
                opposite_open_order(
                    &partial,
                    &AccountId::new(1),
                    &instrument(KnownMarket::Kr, "005930"),
                    OrderSide::Buy
                ),
                OppositeOpenOrder::Incomplete
            );
        }
    }
    #[test]
    fn account_precedes_us_exemption_unknown_zero_blocks_and_same_side_does_not() {
        let facts = set(vec![], OpenOrderCoverage::Complete).expect("목록");
        let us = instrument(KnownMarket::Us, "AAPL");
        assert_eq!(
            opposite_open_order(&facts, &AccountId::new(2), &us, OrderSide::Buy),
            OppositeOpenOrder::DifferentAccount
        );
        assert_eq!(
            opposite_open_order(&facts, &AccountId::new(1), &us, OrderSide::Buy),
            OppositeOpenOrder::NotApplicable
        );
        let stock = instrument(KnownMarket::Kr, "005930");
        assert_eq!(
            opposite_open_order(&facts, &AccountId::new(1), &stock, OrderSide::Buy),
            OppositeOpenOrder::Absent
        );
        let mut input = row(
            "unknown",
            OrderSide::Sell,
            BrokerOrderStatus::Unknown(MasterCode::try_new("FUTURE").expect("코드")),
        );
        input.filled_quantity = quantity(10);
        let unknown = set(vec![input], OpenOrderCoverage::Complete).expect("미지 주문");
        assert_eq!(
            opposite_open_order(&unknown, &AccountId::new(1), &stock, OrderSide::Buy),
            OppositeOpenOrder::Present
        );
        assert_eq!(
            opposite_open_order(&unknown, &AccountId::new(1), &stock, OrderSide::Sell),
            OppositeOpenOrder::Absent
        );
        assert_eq!(
            opposite_open_order(
                &unknown,
                &AccountId::new(1),
                &instrument(KnownMarket::Kr, "000660"),
                OrderSide::Buy
            ),
            OppositeOpenOrder::Absent
        );
        assert_eq!(
            opposite_open_order(
                &set(vec![], OpenOrderCoverage::Incomplete).expect("부분"),
                &AccountId::new(1),
                &stock,
                OrderSide::Buy
            ),
            OppositeOpenOrder::Incomplete
        );
    }
    #[test]
    fn status_schema_mapping_is_exact_and_unknown_is_open() {
        for (code, status, expected) in [
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
            (
                "FUTURE",
                BrokerOrderStatus::Unknown(MasterCode::try_new("FUTURE").expect("코드")),
                OrderOpenState::Possible,
            ),
        ] {
            assert_eq!(
                BrokerOrderStatus::from_code(MasterCode::try_new(code).expect("코드")),
                status
            );
            assert_eq!(status.open_state(), expected);
        }
    }
    #[test]
    fn order_identifiers_reject_whitespace_and_accept_exact_bounds() {
        for text in ["", "a b", " a", "a ", "a\nb", &"a".repeat(257)] {
            assert_eq!(
                BrokerOrderId::try_new(text),
                Err(BrokerOrderError::InvalidOrderId)
            );
        }
        assert_eq!(BrokerOrderId::try_new("a").expect("최소").as_str(), "a");
        assert_eq!(
            BrokerOrderId::try_new(&"a".repeat(256))
                .expect("최대")
                .as_str()
                .len(),
            256
        );
    }
    #[test]
    fn direction_symmetry_markets_and_set_permutations_hold() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(any::<bool>(), any::<bool>(), any::<bool>()),
                |(buy, us, reverse)| {
                    let side = if buy { OrderSide::Buy } else { OrderSide::Sell };
                    let opposite = if buy { OrderSide::Sell } else { OrderSide::Buy };
                    let stock = if us {
                        instrument(KnownMarket::Us, "AAPL")
                    } else {
                        instrument(KnownMarket::Kr, "005930")
                    };
                    let mut first = row("a", opposite, BrokerOrderStatus::Pending);
                    first.instrument = stock.clone();
                    let mut second = row("b", side, BrokerOrderStatus::Pending);
                    second.instrument = stock.clone();
                    let mut rows = vec![first, second];
                    let original = set(rows.clone(), OpenOrderCoverage::Complete).expect("목록");
                    if reverse {
                        rows.reverse();
                    }
                    let reordered = set(rows, OpenOrderCoverage::Complete).expect("목록");
                    prop_assert_eq!(&original, &reordered);
                    prop_assert_eq!(
                        opposite_open_order(&original, &AccountId::new(1), &stock, side),
                        if us {
                            OppositeOpenOrder::NotApplicable
                        } else {
                            OppositeOpenOrder::Present
                        }
                    );
                    Ok(())
                },
            )
            .expect("대칭 시장 순서");
    }
    #[test]
    fn order_type_tif_unknown_and_absence_are_preserved() {
        for (code, expected) in [
            ("LIMIT", BrokerOrderType::Limit),
            ("MARKET", BrokerOrderType::Market),
            (
                "FUTURE",
                BrokerOrderType::Unknown(MasterCode::try_new("FUTURE").expect("코드")),
            ),
        ] {
            assert_eq!(
                BrokerOrderType::from_code(MasterCode::try_new(code).expect("코드")),
                expected
            );
        }
        for (code, expected) in [
            ("DAY", BrokerTimeInForce::Day),
            ("CLS", BrokerTimeInForce::Cls),
            ("OPG", BrokerTimeInForce::Opg),
            (
                "FUTURE",
                BrokerTimeInForce::Unknown(MasterCode::try_new("FUTURE").expect("코드")),
            ),
        ] {
            assert_eq!(
                BrokerTimeInForce::from_code(MasterCode::try_new(code).expect("코드")),
                expected
            );
        }
        let mut unknown = row("unknown", OrderSide::Buy, BrokerOrderStatus::Pending);
        unknown.order_type =
            BrokerOrderType::Unknown(MasterCode::try_new("NEW-TYPE").expect("코드"));
        unknown.time_in_force =
            BrokerTimeInForce::Unknown(MasterCode::try_new("NEW-TIF").expect("코드"));
        let preserved = BrokerOrder::from_lookup_result(unknown.clone()).expect("미지 값");
        assert_eq!(preserved.order_type(), &unknown.order_type);
        assert_eq!(preserved.time_in_force(), &unknown.time_in_force);
        let input = row("empty", OrderSide::Buy, BrokerOrderStatus::Pending);
        let order = BrokerOrder::from_lookup_result(input).expect("주문");
        assert_eq!(order.price(), BrokerOrderPrice::Absent);
        assert_eq!(order.canceled_at(), BrokerOrderTime::Absent);
        assert_eq!(order.filled_amount(), BrokerOrderMoney::Absent);
        assert_eq!(order.filled_at(), None);
        assert_eq!(order.average_filled_price(), None);
    }
}
