#[cfg(test)]
mod tests {
    use crate::limit_price_review::tests::book;
    use crate::orderbook::tests::{at, price};
    use crate::sell_quantity_review::tests::fixture;
    use crate::{
        ClientOrderId, Currency, InvestorProfile, KnownMarket, LimitPriceError, NoSellQuantity,
        NumericInput, OrderSide, OrderTerms, Policy, PriceLimits, PriceLimitsInput, ProfileWeight,
        SavedMarketOrderPolicy, SavedMarketOrderSetting, SellOrderCalculationError,
        SellQuantityMode, SellTarget, TickAlignment, bind_sell_limit, bind_sell_market,
        calculate_limit_price, calculate_sell_quantity, default_market_order_policy,
        restore_market_order_policy, sell_order_into_intent, tick_alignment,
    };
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;
    fn enabled(policy: Policy) -> crate::MarketOrderPolicy {
        restore_market_order_policy(SavedMarketOrderPolicy {
            policy,
            market_orders: SavedMarketOrderSetting::Enabled,
        })
    }
    fn policy() -> Policy {
        Policy::new(InvestorProfile::default()).with_minimum_cash(
            ProfileWeight::parse(&NumericInput::try_new("0.17").expect("입력")).expect("비중"),
        )
    }
    #[test]
    fn policy_snapshot_restores_real_policy_and_default_rejects_market() {
        let original = policy();
        let snapshot = default_market_order_policy(original.clone());
        assert_eq!(snapshot.policy(), &original);
        assert_eq!(snapshot.setting(), SavedMarketOrderSetting::Disabled);
        assert_eq!(
            SavedMarketOrderSetting::default(),
            SavedMarketOrderSetting::Disabled
        );
        assert_eq!(format!("{snapshot:?}"), "MarketOrderPolicy(..)");
        let (account, holding, fact) = fixture(KnownMarket::Us, "17", "11");
        let plan = calculate_sell_quantity(
            &account,
            &holding,
            &fact,
            SellTarget::All,
            SellQuantityMode::Whole,
        )
        .expect("계산");
        assert_eq!(
            bind_sell_market(plan.clone(), snapshot),
            Err(SellOrderCalculationError::MarketDisabled)
        );
        let restored = enabled(original.clone());
        assert_eq!(restored.setting(), SavedMarketOrderSetting::Enabled);
        let order = bind_sell_market(plan, restored).expect("시장가");
        assert_eq!(order.market_policy(), Some(&original));
    }
    #[test]
    fn restored_disabled_policy_rejects_market() {
        let original = policy();
        let saved = SavedMarketOrderPolicy {
            policy: original,
            market_orders: SavedMarketOrderSetting::Disabled,
        };
        assert_eq!(format!("{saved:?}"), "SavedMarketOrderPolicy(..)");
        let (account, holding, fact) = fixture(KnownMarket::Kr, "7", "5");
        let plan = calculate_sell_quantity(
            &account,
            &holding,
            &fact,
            SellTarget::All,
            SellQuantityMode::Whole,
        )
        .expect("계산");
        assert_eq!(
            bind_sell_market(plan, restore_market_order_policy(saved)),
            Err(SellOrderCalculationError::MarketDisabled)
        );
    }
    #[test]
    fn consuming_conversion_preserves_all_fields_and_us_precision() {
        for (market, ask, bid, expected) in [
            (KnownMarket::Kr, "101", "99", "99"),
            (KnownMarket::Us, "0.99995", "0.99985", "0.9998"),
            (KnownMarket::Us, "1.00001", "0.99999", "0.9999"),
            (KnownMarket::Us, "100.019", "100.011", "100.01"),
        ] {
            let (account, holding, fact) = fixture(market, "17", "11");
            let plan = calculate_sell_quantity(
                &account,
                &holding,
                &fact,
                SellTarget::All,
                SellQuantityMode::Whole,
            )
            .expect("수량");
            let (quotes, table, limits) = book(market, ask, bid);
            let calculated =
                calculate_limit_price(&quotes, Some(&table), &limits, OrderSide::Sell, at(12))
                    .expect("가격");
            let order = bind_sell_limit(plan, &calculated).expect("결합");
            assert_eq!(order.market_policy(), None);
            let key = ClientOrderId::try_new("revision-limit-31").expect("키");
            let intent = sell_order_into_intent(order, key.clone()).expect("주문");
            assert_eq!(intent.account_id(), &account);
            assert_eq!(intent.instrument(), holding.instrument());
            assert_eq!(intent.client_order_id(), &key);
            assert_eq!(intent.side(), OrderSide::Sell);
            let quantity = crate::WholeOrderQuantity::try_new(
                crate::Quantity::try_new(Decimal::from(11_u32)).expect("수량"),
            )
            .expect("정수");
            let terms = match market {
                KnownMarket::Kr => OrderTerms::KrLimit {
                    side: OrderSide::Sell,
                    quantity,
                    price: crate::KrOrderPrice::try_new(price(expected, Currency::Krw))
                        .expect("가격"),
                },
                KnownMarket::Us => OrderTerms::UsLimit {
                    side: OrderSide::Sell,
                    quantity,
                    price: crate::UsOrderPrice::try_new(price(expected, Currency::Usd))
                        .expect("가격"),
                },
            };
            assert_eq!(intent.terms(), &terms);
        }
    }
    #[test]
    fn market_conversion_preserves_policy_checked_quantity() {
        for (market, held, available, expected) in [
            (KnownMarket::Kr, "17", "11", "11"),
            (KnownMarket::Us, "1.23456789", "2.4", "1.234567"),
        ] {
            let (account, holding, fact) = fixture(market, held, available);
            let plan = calculate_sell_quantity(
                &account,
                &holding,
                &fact,
                SellTarget::All,
                SellQuantityMode::FractionalMarket,
            )
            .expect("계산");
            let order = bind_sell_market(plan, enabled(policy())).expect("허용");
            let key = ClientOrderId::try_new("revision-market-47").expect("키");
            let intent = sell_order_into_intent(order, key.clone()).expect("주문");
            assert_eq!(intent.account_id(), &account);
            assert_eq!(intent.instrument(), holding.instrument());
            assert_eq!(intent.client_order_id(), &key);
            assert_eq!(intent.side(), OrderSide::Sell);
            let quantity = crate::Quantity::parse(&NumericInput::try_new(expected).expect("입력"))
                .expect("수량");
            let terms = match market {
                KnownMarket::Kr => OrderTerms::KrMarket {
                    side: OrderSide::Sell,
                    quantity: crate::WholeOrderQuantity::try_new(quantity).expect("정수"),
                },
                KnownMarket::Us => OrderTerms::UsMarketSell {
                    quantity: crate::SellOrderQuantity::try_new(quantity).expect("소수"),
                },
            };
            assert_eq!(intent.terms(), &terms);
        }
    }
    fn unit(value: u32) -> u32 {
        match value {
            0..2000 => 1,
            2000..5000 => 5,
            5000..20000 => 10,
            20000..50000 => 50,
            50000..200_000 => 100,
            200_000..500_000 => 500,
            500_000.. => 1000,
        }
    }
    #[test]
    fn variable_limits_match_independent_integer_reference() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(2_u32..899_000, 1_u32..900_000, 1_u32..5000),
                |(bid, lower, width)| {
                    let ask = bid.checked_add(1).expect("가격");
                    let upper = lower + width;
                    let (quotes, table, _limits) =
                        book(KnownMarket::Kr, &ask.to_string(), &bid.to_string());
                    let limits = PriceLimits::try_new(PriceLimitsInput {
                        instrument: quotes.instrument().clone(),
                        currency: Currency::Krw,
                        lower: Some(price(&lower.to_string(), Currency::Krw)),
                        upper: Some(price(&upper.to_string(), Currency::Krw)),
                        timestamp: at(10),
                        collected_at: at(11),
                    })
                    .expect("범위");
                    for (side, expected) in [
                        (
                            OrderSide::Buy,
                            ask.div_ceil(unit(ask))
                                .checked_mul(unit(ask))
                                .expect("단위"),
                        ),
                        (
                            OrderSide::Sell,
                            bid.checked_div(unit(bid))
                                .expect("단위")
                                .checked_mul(unit(bid))
                                .expect("단위"),
                        ),
                    ] {
                        let result =
                            calculate_limit_price(&quotes, Some(&table), &limits, side, at(12));
                        assert_price_range(result, expected, (lower, upper), &table);
                    }
                    Ok(())
                },
            )
            .expect("속성");
    }
    #[test]
    fn every_domestic_band_and_us_dollar_boundary_is_fixed() {
        for edge in [2000_u32, 5000, 20000, 50000, 200_000, 500_000] {
            for bid in [edge - 1, edge, edge + 1] {
                check_domestic_boundary(bid);
            }
        }
        let (quotes, table, limits) = book(KnownMarket::Us, "0.99995", "0.99985");
        assert_eq!(
            calculate_limit_price(&quotes, Some(&table), &limits, OrderSide::Buy, at(12))
                .expect("경계")
                .price(),
            price("1", Currency::Usd)
        );
    }
    #[test]
    fn fractional_us_quantity_matches_independent_microshare_integers() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(100_u64..1_000_000_000, 10_u64..1_000_000_000),
                |(held, available)| {
                    let held_value = Decimal::from_i128_with_scale(i128::from(held), 8);
                    let available_value = Decimal::from_i128_with_scale(i128::from(available), 7);
                    let (account, holding, fact) = fixture(
                        KnownMarket::Us,
                        &held_value.to_string(),
                        &available_value.to_string(),
                    );
                    let plan = calculate_sell_quantity(
                        &account,
                        &holding,
                        &fact,
                        SellTarget::All,
                        SellQuantityMode::FractionalMarket,
                    )
                    .expect("계산");
                    let expected = held
                        .checked_div(100)
                        .expect("단위")
                        .min(available.checked_div(10).expect("단위"));
                    assert_eq!(
                        plan.quantity().quantity().as_decimal(),
                        Decimal::from_i128_with_scale(i128::from(expected), 6)
                    );
                    Ok(())
                },
            )
            .expect("속성");
    }
    #[test]
    fn mantissa_floor_covers_348_scale_and_precision_cases() {
        for coefficient in [
            0_i128,
            1,
            9,
            123_456_789,
            i128::from(u64::MAX),
            Decimal::MAX.mantissa(),
        ] {
            for scale in 0_u32..=28 {
                assert_floor(coefficient, scale);
            }
        }
    }
    fn assert_floor(coefficient: i128, scale: u32) {
        for (mode, precision) in [
            (SellQuantityMode::Whole, 0_u32),
            (SellQuantityMode::FractionalMarket, 6),
        ] {
            let value = Decimal::from_i128_with_scale(coefficient, scale);
            let target_scale = scale.min(precision);
            let divisor = 10_i128.pow(scale.checked_sub(target_scale).expect("자릿수"));
            let expected = Decimal::from_i128_with_scale(
                coefficient.checked_div(divisor).expect("나눗셈"),
                target_scale,
            );
            let (account, holding, fact) = fixture(
                KnownMarket::Us,
                &value.to_string(),
                &Decimal::MAX.to_string(),
            );
            let result = calculate_sell_quantity(&account, &holding, &fact, SellTarget::All, mode);
            if expected.is_zero() {
                assert_eq!(result, Err(NoSellQuantity::BelowMinimumUnit));
            } else {
                assert_eq!(
                    result.expect("계산").quantity().quantity().as_decimal(),
                    expected
                );
            }
        }
    }

    fn check_domestic_boundary(bid: u32) {
        let ask = bid.checked_add(1).expect("가격");
        let (quotes, table, limits) = book(KnownMarket::Kr, &ask.to_string(), &bid.to_string());
        for (side, expected) in [
            (
                OrderSide::Buy,
                ask.div_ceil(unit(ask))
                    .checked_mul(unit(ask))
                    .expect("단위"),
            ),
            (
                OrderSide::Sell,
                bid.checked_div(unit(bid))
                    .expect("단위")
                    .checked_mul(unit(bid))
                    .expect("단위"),
            ),
        ] {
            assert_eq!(
                calculate_limit_price(&quotes, Some(&table), &limits, side, at(12))
                    .expect("경계")
                    .price(),
                price(&expected.to_string(), Currency::Krw)
            );
        }
    }

    fn assert_price_range(
        result: Result<crate::CalculatedLimitPrice, LimitPriceError>,
        expected: u32,
        bounds: (u32, u32),
        table: &crate::TickTable,
    ) {
        let (lower, upper) = bounds;
        if expected < lower || expected > upper {
            assert_eq!(result, Err(LimitPriceError::OutsideLimits));
        } else {
            let calculated = result.expect("범위 안");
            assert_eq!(
                calculated.price(),
                price(&expected.to_string(), Currency::Krw)
            );
            assert_eq!(
                tick_alignment(table, table.instrument(), calculated.price()),
                Ok(TickAlignment::OnTick)
            );
        }
    }
    #[test]
    fn inclusive_price_limits_are_preserved() {
        let (quotes, table, _limits) = book(KnownMarket::Kr, "101", "99");
        let limits = PriceLimits::try_new(PriceLimitsInput {
            instrument: quotes.instrument().clone(),
            currency: Currency::Krw,
            lower: Some(price("99", Currency::Krw)),
            upper: Some(price("101", Currency::Krw)),
            timestamp: at(10),
            collected_at: at(11),
        })
        .expect("범위");
        for (side, expected) in [(OrderSide::Buy, "101"), (OrderSide::Sell, "99")] {
            assert_eq!(
                calculate_limit_price(&quotes, Some(&table), &limits, side, at(12))
                    .expect("양 끝")
                    .price(),
                price(expected, Currency::Krw)
            );
        }
    }
    #[test]
    fn identical_symbol_in_another_market_cannot_bind() {
        let (account, original_holding, original_fact) = fixture(KnownMarket::Kr, "17", "11");
        let instrument = crate::ResolvedOrderInstrument::from_master_lookup_result(
            crate::Symbol::try_new("A00593").expect("심볼"),
            crate::Market::Kr,
        )
        .expect("종목");
        let holding = crate::Holding::new(
            instrument.clone(),
            original_holding.quantity(),
            original_holding.acquisition_price(),
            crate::HoldingObservation::new(None, None, at(5)),
        )
        .expect("보유");
        let fact = crate::SellableQuantityFact::from_lookup_result(
            account.clone(),
            instrument,
            original_fact.quantity(),
            at(6),
        )
        .expect("사실");
        let plan = calculate_sell_quantity(
            &account,
            &holding,
            &fact,
            SellTarget::All,
            SellQuantityMode::Whole,
        )
        .expect("수량");
        let (quotes, table, limits) =
            cross_market_quotes(holding.instrument().symbol().clone(), fact.quantity());
        let calculated =
            calculate_limit_price(&quotes, Some(&table), &limits, OrderSide::Sell, at(12))
                .expect("계산");
        assert_eq!(
            bind_sell_limit(plan, &calculated),
            Err(SellOrderCalculationError::DifferentInstrument)
        );
    }
    fn cross_market_quotes(
        symbol: crate::Symbol,
        volume: crate::Quantity,
    ) -> (crate::Orderbook, crate::TickTable, PriceLimits) {
        let original = crate::orderbook::tests::master(KnownMarket::Us, "STOCK", "cross-market");
        let mut row = original.row().clone();
        row.symbol = symbol;
        let master = crate::MasterInstrument::try_new(row).expect("마스터");
        let quote_instrument = master.quote_instrument().expect("시세");
        let quotes = crate::Orderbook::try_new(crate::OrderbookInput {
            instrument: quote_instrument.clone(),
            currency: Currency::Usd,
            asks: vec![],
            bids: vec![crate::OrderbookLevelInput {
                price: price("99", Currency::Usd),
                volume,
            }],
            timestamp: Some(at(10)),
            collected_at: at(11),
        })
        .expect("호가");
        let limits = PriceLimits::try_new(PriceLimitsInput {
            instrument: quote_instrument,
            currency: Currency::Usd,
            lower: None,
            upper: None,
            timestamp: at(10),
            collected_at: at(11),
        })
        .expect("범위");
        let table = crate::TickTable::from_master(&master).expect("표");

        (quotes, table, limits)
    }

    #[test]
    fn conversion_error_payloads_are_preserved() {
        assert_eq!(
            SellOrderCalculationError::from(crate::OrderValueError::PricePrecision),
            SellOrderCalculationError::OrderValue(crate::OrderValueError::PricePrecision)
        );
        assert_eq!(
            SellOrderCalculationError::from(crate::OrderIntentError::MarketMismatch),
            SellOrderCalculationError::Intent(crate::OrderIntentError::MarketMismatch)
        );
        assert_eq!(
            LimitPriceError::from(crate::PriceLimitsError::DifferentInstrument),
            LimitPriceError::Limits(crate::PriceLimitsError::DifferentInstrument)
        );
    }
    #[test]
    fn us_prices_match_independent_integer_ticks_for_4096_cases() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(100_u32..2_000_000), |bid| {
                let ask = bid + 7;
                let ask_value = Decimal::from_i128_with_scale(i128::from(ask), 5);
                let bid_value = Decimal::from_i128_with_scale(i128::from(bid), 5);
                let (quotes, table, limits) = book(
                    KnownMarket::Us,
                    &ask_value.to_string(),
                    &bid_value.to_string(),
                );
                let ask_unit = if ask < 100_000 { 10 } else { 1000 };
                let bid_unit = if bid < 100_000 { 10 } else { 1000 };
                for (side, expected) in [
                    (OrderSide::Buy, ask.div_ceil(ask_unit) * ask_unit),
                    (
                        OrderSide::Sell,
                        bid.checked_div(bid_unit).expect("단위") * bid_unit,
                    ),
                ] {
                    let calculated =
                        calculate_limit_price(&quotes, Some(&table), &limits, side, at(12))
                            .expect("가격");
                    let expected_value = Decimal::from_i128_with_scale(i128::from(expected), 5);
                    assert_eq!(
                        calculated.price(),
                        price(&expected_value.to_string(), Currency::Usd)
                    );
                    assert_eq!(
                        crate::UsOrderPrice::try_new(calculated.price())
                            .expect("미국 자릿수")
                            .price(),
                        calculated.price()
                    );
                }
                Ok(())
            })
            .expect("속성");
    }
}
