mod order_gate_support;
#[cfg(test)]
mod tests {
    use crate::order_gate_support::tests::{at, instrument};
    use domain::{
        AccountId, KnownMarket, NumericInput, Quantity, SellOrderQuantity,
        SellableQuantityDecision, SellableQuantityError, SellableQuantityFact,
        check_sellable_quantity,
    };
    use proptest::{
        prop_assert_eq,
        test_runner::{Config, TestRunner},
    };
    use rust_decimal::Decimal;
    fn quantity(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("입력")).expect("수량")
    }
    #[test]
    fn identity_fractional_domestic_and_normalized_integral_requests_are_closed() {
        let stock = instrument(KnownMarket::Kr, "005930");
        let fact = SellableQuantityFact::from_lookup_result(
            AccountId::new(1),
            stock.clone(),
            quantity("10.0"),
            at(),
        )
        .expect("사실");
        assert_eq!(fact.account_id(), &AccountId::new(1));
        assert_eq!(fact.instrument(), &stock);
        assert_eq!(fact.quantity(), quantity("10"));
        assert_eq!(fact.observed_at(), at());
        for (text, expected) in [
            ("10.0", SellableQuantityDecision::Allowed),
            ("11", SellableQuantityDecision::Exceeded),
            ("0.5", SellableQuantityDecision::FractionalDomesticRequest),
            ("10.5", SellableQuantityDecision::FractionalDomesticRequest),
        ] {
            assert_eq!(
                check_sellable_quantity(
                    &fact,
                    &AccountId::new(1),
                    &stock,
                    SellOrderQuantity::try_new(quantity(text)).expect("매도")
                ),
                expected
            );
        }
        let requested = SellOrderQuantity::try_new(quantity("1")).expect("매도");
        assert_eq!(
            check_sellable_quantity(&fact, &AccountId::new(2), &stock, requested),
            SellableQuantityDecision::DifferentFact
        );
        assert_eq!(
            check_sellable_quantity(
                &fact,
                &AccountId::new(1),
                &instrument(KnownMarket::Kr, "000660"),
                requested
            ),
            SellableQuantityDecision::DifferentFact
        );
        assert_eq!(
            SellableQuantityFact::from_lookup_result(
                AccountId::new(1),
                stock,
                quantity("0.5"),
                at()
            ),
            Err(SellableQuantityError::FractionalDomestic)
        );
        assert_eq!(format!("{fact:?}"), "SellableQuantityFact { .. }");
    }
    #[test]
    fn fractional_us_sell_never_exceeds_available_quantity() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(0_u32..100_000, 1_u32..100_000),
                |(available, requested)| {
                    let stock = instrument(KnownMarket::Us, "AAPL");
                    let fact = SellableQuantityFact::from_lookup_result(
                        AccountId::new(1),
                        stock.clone(),
                        Quantity::try_new(Decimal::new(i64::from(available), 3))
                            .expect("가능 수량"),
                        at(),
                    )
                    .expect("사실");
                    let request = SellOrderQuantity::try_new(
                        Quantity::try_new(Decimal::new(i64::from(requested), 3)).expect("수량"),
                    )
                    .expect("매도");
                    prop_assert_eq!(
                        check_sellable_quantity(&fact, &AccountId::new(1), &stock, request),
                        if requested <= available {
                            SellableQuantityDecision::Allowed
                        } else {
                            SellableQuantityDecision::Exceeded
                        }
                    );
                    Ok(())
                },
            )
            .expect("상한 속성");
    }
}
