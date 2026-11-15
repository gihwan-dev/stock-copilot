#[cfg(test)]
pub(crate) mod tests {
    use crate::orderbook::tests::{at, master, price};
    use crate::{
        AccountId, AcquisitionPrice, Currency, Holding, HoldingObservation, KnownMarket,
        LimitPriceError, Market, NumericInput, OrderSide, Orderbook, OrderbookInput,
        OrderbookLevelInput, PriceLimits, PriceLimitsInput, Quantity, ResolvedOrderInstrument,
        SellExecutionPrice, SellOrderCalculationError, SellQuantityMode, SellTarget,
        SellableQuantityFact, Symbol, TickAlignment, TickSizeError, TickTable, TimeSpan,
        bind_sell_limit, bind_sell_market, calculate_limit_price, calculate_sell_quantity,
        tick_alignment,
    };
    pub(crate) fn book(
        market: KnownMarket,
        ask: &str,
        bid: &str,
    ) -> (Orderbook, TickTable, PriceLimits) {
        let master_value = master(market, "STOCK", "stable");
        let instrument_value = master_value.quote_instrument().expect("종목");
        let currency = instrument_value.currency();
        let volume = Quantity::try_new(rust_decimal::Decimal::ONE).expect("수량");
        let orderbook = Orderbook::try_new(OrderbookInput {
            instrument: instrument_value.clone(),
            currency,
            asks: vec![OrderbookLevelInput {
                price: price(ask, currency),
                volume,
            }],
            bids: vec![OrderbookLevelInput {
                price: price(bid, currency),
                volume,
            }],
            timestamp: Some(at(10)),
            collected_at: at(11),
        })
        .expect("호가");
        let (lower, upper) = match market {
            KnownMarket::Kr => (Some(price("1", currency)), Some(price("900000", currency))),
            KnownMarket::Us => (None, None),
        };
        let limits = PriceLimits::try_new(PriceLimitsInput {
            instrument: instrument_value,
            currency,
            lower,
            upper,
            timestamp: at(9),
            collected_at: at(11),
        })
        .expect("제한");
        (
            orderbook,
            TickTable::from_master(&master_value).expect("표"),
            limits,
        )
    }
    #[test]
    fn limit_examples_side_rounding_and_boundary() {
        for (market, ask, bid, buy, sell) in [
            (KnownMarket::Kr, "50001", "49990", "50100", "49950"),
            (KnownMarket::Us, "0.99995", "0.99985", "1", "0.9998"),
        ] {
            let (orderbook, table, limits) = book(market, ask, bid);
            for (side, expected) in [(OrderSide::Buy, buy), (OrderSide::Sell, sell)] {
                let value = calculate_limit_price(&orderbook, Some(&table), &limits, side, at(12))
                    .expect("가격");
                assert_eq!(
                    value.price(),
                    price(expected, orderbook.instrument().currency())
                );
                assert_eq!(value.instrument(), orderbook.instrument());
                assert_eq!(value.side(), side);
                assert_eq!(format!("{value:?}"), "CalculatedLimitPrice(..)");
            }
        }
    }
    #[test]
    fn limit_rejects_missing_stale_and_mismatched_facts() {
        let (orderbook, table, limits) = book(KnownMarket::Kr, "101", "99");
        assert_eq!(
            calculate_limit_price(&orderbook, None, &limits, OrderSide::Buy, at(12)),
            Err(LimitPriceError::TickUnavailable)
        );
        for now in [at(9), at(15), at(16)] {
            assert_eq!(
                calculate_limit_price(&orderbook, Some(&table), &limits, OrderSide::Buy, now),
                Err(LimitPriceError::StaleFact)
            );
        }
        calculate_limit_price(&orderbook, Some(&table), &limits, OrderSide::Buy, at(14))
            .expect("경계");
        let master_value = master(KnownMarket::Kr, "STOCK", "other");
        let other = TickTable::from_master(&master_value).expect("표");
        assert_eq!(
            calculate_limit_price(&orderbook, Some(&other), &limits, OrderSide::Buy, at(12)),
            Err(LimitPriceError::DifferentInstrument)
        );
        let mut input = OrderbookInput {
            instrument: orderbook.instrument().clone(),
            currency: Currency::Krw,
            asks: vec![],
            bids: vec![],
            timestamp: None,
            collected_at: at(11),
        };
        let missing = Orderbook::try_new(input.clone()).expect("호가");
        assert_eq!(
            calculate_limit_price(&missing, Some(&table), &limits, OrderSide::Buy, at(12)),
            Err(LimitPriceError::UnknownTimestamp)
        );
        input.timestamp = Some(at(10));
        let empty = Orderbook::try_new(input.clone()).expect("호가");
        for side in [OrderSide::Buy, OrderSide::Sell] {
            assert_eq!(
                calculate_limit_price(&empty, Some(&table), &limits, side, at(12)),
                Err(LimitPriceError::NoQuote)
            );
        }
        input.asks = vec![OrderbookLevelInput {
            price: price("101", Currency::Krw),
            volume: Quantity::try_new(rust_decimal::Decimal::ZERO).expect("수량"),
        }];
        let zero = Orderbook::try_new(input).expect("호가");
        assert_eq!(
            calculate_limit_price(&zero, Some(&table), &limits, OrderSide::Buy, at(12)),
            Err(LimitPriceError::NoLiquidity)
        );
    }
    #[test]
    fn limit_matches_independent_tick_reference() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&(2_u32..899_000), |bid| {
                let ask = bid + 1;
                let (orderbook, table, limits) =
                    book(KnownMarket::Kr, &ask.to_string(), &bid.to_string());

                for (side, candidate, expected) in [
                    (
                        OrderSide::Buy,
                        ask,
                        ask.div_ceil(reference_unit(ask)) * reference_unit(ask),
                    ),
                    (
                        OrderSide::Sell,
                        bid,
                        bid.checked_div(reference_unit(bid)).expect("단위") * reference_unit(bid),
                    ),
                ] {
                    let rounded =
                        calculate_limit_price(&orderbook, Some(&table), &limits, side, at(12))
                            .expect("가격")
                            .price();
                    assert_eq!(rounded, price(&expected.to_string(), Currency::Krw));
                    assert_eq!(
                        tick_alignment(&table, orderbook.instrument(), rounded),
                        Ok(TickAlignment::OnTick)
                    );
                    assert!(candidate > 0);
                }
                Ok(())
            })
            .expect("속성");
    }
    #[test]
    fn sell_order_binds_quantity_price_and_policy() {
        let (orderbook, table, limits) = book(KnownMarket::Kr, "101", "99");
        let instrument = resolved(KnownMarket::Kr);
        let account = AccountId::new(31);
        let quantity = Quantity::try_new(rust_decimal::Decimal::from(7_u32)).expect("수량");
        let holding = Holding::new(
            instrument.clone(),
            quantity,
            AcquisitionPrice::try_new(price("73", Currency::Krw).as_money()).expect("취득가"),
            HoldingObservation::new(None, None, at(5)),
        )
        .expect("보유");
        let fact =
            SellableQuantityFact::from_lookup_result(account.clone(), instrument, quantity, at(6))
                .expect("사실");
        let plan = calculate_sell_quantity(
            &account,
            &holding,
            &fact,
            SellTarget::All,
            SellQuantityMode::Whole,
        )
        .expect("수량");
        let sell =
            calculate_limit_price(&orderbook, Some(&table), &limits, OrderSide::Sell, at(12))
                .expect("가격");
        let order = bind_sell_limit(plan.clone(), &sell).expect("결합");
        assert_eq!(order.plan(), &plan);
        assert_eq!(
            order.price(),
            SellExecutionPrice::Limit(price("99", Currency::Krw))
        );
        assert_eq!(format!("{order:?}"), "CalculatedSellOrder(..)");
        let buy = calculate_limit_price(&orderbook, Some(&table), &limits, OrderSide::Buy, at(12))
            .expect("가격");
        assert_eq!(
            bind_sell_limit(plan.clone(), &buy),
            Err(SellOrderCalculationError::DifferentDirection)
        );
        assert_eq!(
            bind_sell_market(
                plan.clone(),
                crate::default_market_order_policy(crate::Policy::new(
                    crate::InvestorProfile::default()
                ))
            ),
            Err(SellOrderCalculationError::MarketDisabled)
        );
        let market = bind_sell_market(
            plan,
            crate::restore_market_order_policy(crate::SavedMarketOrderPolicy {
                policy: crate::Policy::new(crate::InvestorProfile::default()),
                market_orders: crate::SavedMarketOrderSetting::Enabled,
            }),
        )
        .expect("시장가");
        assert_eq!(market.price(), SellExecutionPrice::Market);
    }
    #[test]
    fn limit_ranges_and_tick_errors_are_closed() {
        let (orderbook, table, limits) = book(KnownMarket::Kr, "50001", "49990");
        for (lower, upper, side) in [
            ("1", "50050", OrderSide::Buy),
            ("49975", "90000", OrderSide::Sell),
        ] {
            let bounds = PriceLimits::try_new(PriceLimitsInput {
                instrument: orderbook.instrument().clone(),
                currency: Currency::Krw,
                lower: Some(price(lower, Currency::Krw)),
                upper: Some(price(upper, Currency::Krw)),
                timestamp: at(10),
                collected_at: at(11),
            })
            .expect("제한");
            assert_eq!(
                calculate_limit_price(&orderbook, Some(&table), &bounds, side, at(12)),
                Err(LimitPriceError::OutsideLimits)
            );
        }
        let other = master(KnownMarket::Kr, "STOCK", "other")
            .quote_instrument()
            .expect("종목");
        let bounds = PriceLimits::try_new(PriceLimitsInput {
            instrument: other,
            currency: Currency::Krw,
            lower: Some(price("1", Currency::Krw)),
            upper: Some(price("90000", Currency::Krw)),
            timestamp: at(10),
            collected_at: at(11),
        })
        .expect("제한");
        assert_eq!(
            calculate_limit_price(&orderbook, Some(&table), &bounds, OrderSide::Sell, at(12)),
            Err(LimitPriceError::DifferentInstrument)
        );
        let (tiny, tiny_table, tiny_bounds) = book(KnownMarket::Us, "0.00002", "0.00001");
        assert_eq!(
            calculate_limit_price(
                &tiny,
                Some(&tiny_table),
                &tiny_bounds,
                OrderSide::Sell,
                at(12)
            ),
            Err(LimitPriceError::Tick(TickSizeError::BelowFirstTick))
        );
        let now = at(14).checked_add(TimeSpan::NANOSECOND).expect("시각");
        assert_eq!(
            calculate_limit_price(&orderbook, Some(&table), &limits, OrderSide::Buy, now),
            Err(LimitPriceError::StaleFact)
        );
    }
    #[test]
    fn fractional_limit_and_foreign_instrument_cannot_bind() {
        let (orderbook, table, limits) = book(KnownMarket::Us, "101", "99");
        let instrument = resolved(KnownMarket::Us);
        let account = AccountId::new(47);
        let qty = Quantity::parse(&NumericInput::try_new("1.23").expect("입력")).expect("수량");
        let holding = Holding::new(
            instrument.clone(),
            qty,
            AcquisitionPrice::try_new(price("73", Currency::Usd).as_money()).expect("취득가"),
            HoldingObservation::new(None, None, at(5)),
        )
        .expect("보유");
        let fact =
            SellableQuantityFact::from_lookup_result(account.clone(), instrument, qty, at(6))
                .expect("사실");
        let plan = calculate_sell_quantity(
            &account,
            &holding,
            &fact,
            SellTarget::All,
            SellQuantityMode::FractionalMarket,
        )
        .expect("수량");
        let sell =
            calculate_limit_price(&orderbook, Some(&table), &limits, OrderSide::Sell, at(12))
                .expect("가격");
        assert_eq!(
            bind_sell_limit(plan.clone(), &sell),
            Err(SellOrderCalculationError::FractionalLimit)
        );
        let (foreign_book, foreign_table, foreign_limits) = book(KnownMarket::Kr, "101", "99");
        let other = calculate_limit_price(
            &foreign_book,
            Some(&foreign_table),
            &foreign_limits,
            OrderSide::Sell,
            at(12),
        )
        .expect("가격");
        assert_eq!(
            bind_sell_limit(plan, &other),
            Err(SellOrderCalculationError::DifferentInstrument)
        );
        assert_eq!(
            crate::default_market_order_policy(crate::Policy::new(
                crate::InvestorProfile::default()
            ))
            .setting(),
            crate::SavedMarketOrderSetting::Disabled
        );
        assert_eq!(
            format!("{:?}", SellExecutionPrice::Market),
            "SellExecutionPrice(..)"
        );
    }

    fn resolved(market: KnownMarket) -> ResolvedOrderInstrument {
        let symbol = match market {
            KnownMarket::Kr => "005930",
            KnownMarket::Us => "AAPL",
        };
        ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new(symbol).expect("종목"),
            Market::from(market),
        )
        .expect("종목")
    }
    fn reference_unit(candidate: u32) -> u32 {
        if candidate < 2000 {
            1
        } else if candidate < 5000 {
            5
        } else if candidate < 20000 {
            10
        } else if candidate < 50000 {
            50
        } else if candidate < 200_000 {
            100
        } else if candidate < 500_000 {
            500
        } else {
            1000
        }
    }
}
