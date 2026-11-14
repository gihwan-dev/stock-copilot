mod order_gate_support;
#[cfg(test)]
mod tests {
    use crate::order_gate_support::tests::{at, instrument};
    use domain::{
        KnownMarket, MasterCode, Nanosecond, OffsetSeconds, OrderSide, StockWarning,
        StockWarningDecision, StockWarnings, Timestamp, UnixSeconds, check_stock_warnings,
    };
    use proptest::{
        prop_assert_eq,
        test_runner::{Config, TestRunner},
    };
    #[test]
    fn schema_values_unknown_and_sell_liquidation_are_preserved() {
        let stock = instrument(KnownMarket::Kr, "005930");
        for (code, expected) in [
            ("LIQUIDATION_TRADING", StockWarning::LiquidationTrading),
            ("OVERHEATED", StockWarning::Overheated),
            ("INVESTMENT_WARNING", StockWarning::InvestmentWarning),
            ("INVESTMENT_RISK", StockWarning::InvestmentRisk),
            ("VI_STATIC_AND_DYNAMIC", StockWarning::ViStaticAndDynamic),
            ("VI_STATIC", StockWarning::ViStatic),
            ("VI_DYNAMIC", StockWarning::ViDynamic),
            ("STOCK_WARRANTS", StockWarning::StockWarrants),
            (
                "FUTURE",
                StockWarning::Unknown(MasterCode::try_new("FUTURE").expect("코드")),
            ),
        ] {
            assert_eq!(
                StockWarning::from_code(MasterCode::try_new(code).expect("코드")),
                expected
            );
            let fact = StockWarnings::from_lookup_result(
                stock.clone(),
                vec![expected.clone(), expected],
                at(),
            );
            assert_eq!(fact.warnings().len(), 1);
            assert_eq!(fact.instrument(), &stock);
            assert_eq!(fact.observed_at(), at());
            assert_eq!(
                check_stock_warnings(&fact, &stock, OrderSide::Buy),
                StockWarningDecision::Blocked
            );
            assert_eq!(
                check_stock_warnings(&fact, &stock, OrderSide::Sell),
                StockWarningDecision::NotApplicable
            );
            assert_eq!(
                check_stock_warnings(
                    &fact,
                    &instrument(KnownMarket::Kr, "000660"),
                    OrderSide::Buy
                ),
                StockWarningDecision::DifferentInstrument
            );
            assert_eq!(
                check_stock_warnings(
                    &fact,
                    &instrument(KnownMarket::Kr, "000660"),
                    OrderSide::Sell
                ),
                StockWarningDecision::NotApplicable
            );
            assert_eq!(format!("{fact:?}"), "StockWarnings { .. }");
        }
    }
    #[test]
    fn empty_warning_set_allows_buy() {
        let stock = instrument(KnownMarket::Kr, "005930");
        assert_eq!(
            check_stock_warnings(
                &StockWarnings::from_lookup_result(stock.clone(), vec![], at()),
                &stock,
                OrderSide::Buy
            ),
            StockWarningDecision::Clear
        );
    }
    #[test]
    fn equality_rejects_strict_supersets_in_both_directions_and_other_identity_or_time() {
        let stock = instrument(KnownMarket::Kr, "005930");
        let short =
            StockWarnings::from_lookup_result(stock.clone(), vec![StockWarning::ViStatic], at());
        let wide = StockWarnings::from_lookup_result(
            stock.clone(),
            vec![StockWarning::ViStatic, StockWarning::ViDynamic],
            at(),
        );
        assert_ne!(short, wide);
        assert_ne!(wide, short);
        assert_ne!(
            short,
            StockWarnings::from_lookup_result(stock.clone(), vec![StockWarning::ViDynamic], at())
        );
        assert_ne!(
            short,
            StockWarnings::from_lookup_result(
                instrument(KnownMarket::Kr, "000660"),
                vec![StockWarning::ViStatic],
                at()
            )
        );
        let later = Timestamp::from_parts(
            UnixSeconds::new(101),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        assert_ne!(
            short,
            StockWarnings::from_lookup_result(stock, vec![StockWarning::ViStatic], later)
        );
    }
    #[test]
    fn arbitrary_warning_permutations_and_duplicates_preserve_the_set() {
        TestRunner::new(Config::with_cases(4096))
            .run(&proptest::collection::vec(0_u8..4, 0..30), |values| {
                let stock = instrument(KnownMarket::Kr, "005930");
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
                    check_stock_warnings(&original, &stock, OrderSide::Buy),
                    if values.is_empty() {
                        StockWarningDecision::Clear
                    } else {
                        StockWarningDecision::Blocked
                    }
                );
                Ok(())
            })
            .expect("순서 속성");
    }
}
