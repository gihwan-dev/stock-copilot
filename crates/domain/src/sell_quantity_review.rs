#[cfg(test)]
mod tests {
    use crate::orderbook::tests::at;
    use crate::{
        AccountId, AcquisitionPrice, Currency, Holding, HoldingObservation, KnownMarket, Market,
        Money, NoSellQuantity, NumericInput, ProfileWeight, Quantity, ResolvedOrderInstrument,
        SellQuantityMode, SellTarget, SellableQuantityFact, Symbol, calculate_sell_quantity,
    };

    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }
    fn fixture(
        market: KnownMarket,
        held: &str,
        available: &str,
    ) -> (AccountId, Holding, SellableQuantityFact) {
        let instrument = resolved(market);
        let currency = match market {
            KnownMarket::Kr => Currency::Krw,
            KnownMarket::Us => Currency::Usd,
        };
        let holding = Holding::new(
            instrument.clone(),
            quantity(held),
            AcquisitionPrice::try_new(
                Money::parse(&NumericInput::try_new("73").expect("입력"), currency).expect("금액"),
            )
            .expect("취득가"),
            HoldingObservation::new(None, None, at(3)),
        )
        .expect("보유");
        let account = AccountId::new(17);
        let fact = SellableQuantityFact::from_lookup_result(
            account.clone(),
            instrument,
            quantity(available),
            at(5),
        )
        .expect("사실");
        (account, holding, fact)
    }
    #[test]
    fn sell_examples_and_identity() {
        let (account, holding, fact) = fixture(KnownMarket::Kr, "17", "11");
        let plan = calculate_sell_quantity(
            &account,
            &holding,
            &fact,
            SellTarget::All,
            SellQuantityMode::Whole,
        )
        .expect("계산");
        assert_eq!(plan.quantity().quantity(), quantity("11"));
        assert_eq!(plan.account_id(), &account);
        assert_eq!(plan.instrument(), holding.instrument());
        assert_eq!(format!("{plan:?}"), "SellQuantityPlan(..)");
        assert_eq!(
            calculate_sell_quantity(
                &AccountId::new(19),
                &holding,
                &fact,
                SellTarget::All,
                SellQuantityMode::Whole
            ),
            Err(NoSellQuantity::DifferentFact)
        );
        let other = SellableQuantityFact::from_lookup_result(
            account.clone(),
            ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new("OTHER").expect("심볼"),
                Market::Kr,
            )
            .expect("종목"),
            quantity("9"),
            at(5),
        )
        .expect("사실");
        assert_eq!(
            calculate_sell_quantity(
                &account,
                &holding,
                &other,
                SellTarget::All,
                SellQuantityMode::Whole
            ),
            Err(NoSellQuantity::DifferentFact)
        );
    }
    #[test]
    fn sell_target_examples() {
        for (held, available, target, expected) in [
            ("7", "19", SellTarget::All, "7"),
            (
                "17",
                "11",
                SellTarget::Fraction(
                    ProfileWeight::parse(&NumericInput::try_new("0.5").expect("입력"))
                        .expect("비율"),
                ),
                "8",
            ),
        ] {
            let (account, holding, fact) = fixture(KnownMarket::Kr, held, available);
            assert_eq!(
                calculate_sell_quantity(&account, &holding, &fact, target, SellQuantityMode::Whole)
                    .expect("계산")
                    .quantity()
                    .quantity(),
                quantity(expected)
            );
        }
    }
    #[test]
    fn sell_zero_fraction_and_precision() {
        for (held, available) in [("0", "7"), ("9", "0"), ("0.9", "3")] {
            let (account, holding, fact) = fixture(KnownMarket::Us, held, available);
            assert_eq!(
                calculate_sell_quantity(
                    &account,
                    &holding,
                    &fact,
                    SellTarget::All,
                    SellQuantityMode::Whole
                ),
                Err(NoSellQuantity::BelowMinimumUnit)
            );
        }
        let (account, holding, fact) = fixture(KnownMarket::Us, "1.23456789", "2.4");
        assert_eq!(
            calculate_sell_quantity(
                &account,
                &holding,
                &fact,
                SellTarget::All,
                SellQuantityMode::FractionalMarket
            )
            .expect("소수")
            .quantity()
            .quantity(),
            quantity("1.234567")
        );
    }
    #[test]
    fn domestic_fraction_is_rejected() {
        let (account, holding, fact) = fixture(KnownMarket::Kr, "1.5", "2");
        assert_eq!(
            calculate_sell_quantity(
                &account,
                &holding,
                &fact,
                SellTarget::All,
                SellQuantityMode::Whole
            ),
            Err(NoSellQuantity::FractionalDomestic)
        );
    }
    #[test]
    fn sell_matches_independent_integer_reference() {
        use proptest::test_runner::{Config, TestRunner};
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(100_u32..100_000, 1_u32..100_000, 1_u32..=100),
                |(held, available, percent)| {
                    let (account, holding, fact) =
                        fixture(KnownMarket::Kr, &held.to_string(), &available.to_string());
                    let target = SellTarget::Fraction(
                        ProfileWeight::parse_percent(
                            &NumericInput::try_new(&percent.to_string()).expect("입력"),
                        )
                        .expect("비율"),
                    );
                    let expected = ((u64::from(held) * u64::from(percent))
                        .checked_div(100)
                        .expect("비율"))
                    .min(u64::from(available));
                    let result = calculate_sell_quantity(
                        &account,
                        &holding,
                        &fact,
                        target,
                        SellQuantityMode::Whole,
                    );
                    assert_eq!(
                        result.expect("계산").quantity().quantity(),
                        quantity(&expected.to_string())
                    );
                    Ok(())
                },
            )
            .expect("속성");
    }
    #[test]
    fn sell_arithmetic_failure_is_closed() {
        let (account, holding, fact) = fixture(
            KnownMarket::Us,
            "79228162514264337593543950335",
            "79228162514264337593543950335",
        );
        let weight = ProfileWeight::parse(
            &NumericInput::try_new("0.1234567890123456789012345678").expect("입력"),
        )
        .expect("비율");
        assert_eq!(
            calculate_sell_quantity(
                &account,
                &holding,
                &fact,
                SellTarget::Fraction(weight),
                SellQuantityMode::Whole
            ),
            Err(NoSellQuantity::Arithmetic)
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
    #[test]
    fn fractional_target_below_one_has_no_order() {
        let (account, holding, fact) = fixture(KnownMarket::Kr, "1", "11");
        let target = SellTarget::Fraction(
            ProfileWeight::parse(&NumericInput::try_new("0.5").expect("입력")).expect("비율"),
        );
        assert_eq!(
            calculate_sell_quantity(&account, &holding, &fact, target, SellQuantityMode::Whole),
            Err(NoSellQuantity::BelowMinimumUnit)
        );
    }
}
