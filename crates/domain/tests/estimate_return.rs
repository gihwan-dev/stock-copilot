#[cfg(test)]
mod tests {
    use domain::{
        BenchmarkCashFlows, Currency, EstimateCashFlows, EstimateReturnError, Money, Nanosecond,
        NumericInput, OffsetSeconds, Price, Ratio, RatioQuotient, TimeInterval, Timestamp,
        UnixSeconds,
    };
    use proptest::test_runner::Config as ProptestConfig;
    use proptest::{prop_assert_eq, prop_assert_ne, proptest};

    fn amount(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("금액 입력"), currency).expect("금액")
    }

    fn price(value: &str, currency: Currency) -> Price {
        Price::try_new(amount(value, currency)).expect("가격")
    }

    fn ratio(value: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(value).expect("비율 입력")).expect("비율")
    }

    fn interval(start: i64, end: i64) -> TimeInterval {
        let at = |value| {
            Timestamp::from_parts(
                UnixSeconds::new(value),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각")
        };
        TimeInterval::try_new(at(start), at(end)).expect("구간")
    }

    fn cash(entry: &str, exit: &str, buy: &str, sell: &str) -> EstimateCashFlows {
        EstimateCashFlows::try_new(
            price(entry, Currency::Krw),
            price(exit, Currency::Krw),
            amount(buy, Currency::Krw),
            amount(sell, Currency::Krw),
            interval(1, 5),
        )
        .expect("현금 흐름")
    }

    fn benchmark(entry: &str, exit: &str) -> BenchmarkCashFlows {
        BenchmarkCashFlows::new(
            price(entry, Currency::Krw),
            price(exit, Currency::Krw),
            interval(1, 5),
        )
        .expect("벤치마크")
    }

    fn cmp(quotient: RatioQuotient, against: Ratio) -> std::cmp::Ordering {
        match quotient {
            RatioQuotient::Exact(value) => value.cmp(&against),
            RatioQuotient::Approximate(value) => value.cmp_ratio(&against),
        }
    }

    fn expect_approximate(quotient: RatioQuotient) {
        match quotient {
            RatioQuotient::Approximate(_) => (),
            RatioQuotient::Exact(_) => panic!("근삿값이어야 함"),
        }
    }

    #[test]
    fn d54_return_examples_preserve_sign_cost_and_excess() {
        for (exit, buy, sell, expected) in [
            ("120", "0", "0", "0.2"),
            ("80", "0", "0", "-0.2"),
            ("115", "2", "3", "0.0980392156862745098039215686"),
            ("100", "0", "0", "0"),
        ] {
            let actual = cash("100", exit, buy, sell).net().expect("수익률");
            assert_eq!(cmp(actual, ratio(expected)), std::cmp::Ordering::Equal);
        }
        let equal = cash("100", "120", "0", "0")
            .excess(&benchmark("250", "300"))
            .expect("같은 수익률");
        assert_eq!(cmp(equal, ratio("0")), std::cmp::Ordering::Equal);
        let excess = cash("100", "120", "0", "0")
            .excess(&benchmark("250", "275"))
            .expect("초과 수익률");
        assert_eq!(excess, RatioQuotient::Exact(ratio("0.1")));
    }

    #[test]
    fn d54_cash_flows_reject_mixed_currency_negative_cost_and_incomparable_periods() {
        let base = price("100", Currency::Krw);
        let end = price("120", Currency::Krw);
        let zero = amount("0", Currency::Krw);
        let span = interval(1, 5);
        assert_eq!(
            EstimateCashFlows::try_new(base, price("120", Currency::Usd), zero, zero, span),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        assert_eq!(
            EstimateCashFlows::try_new(base, end, amount("0", Currency::Usd), zero, span),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        assert_eq!(
            EstimateCashFlows::try_new(base, end, zero, amount("1", Currency::Usd), span),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        assert_eq!(
            EstimateCashFlows::try_new(base, end, amount("-1", Currency::Krw), zero, span),
            Err(EstimateReturnError::NegativeCost)
        );
        assert_eq!(
            EstimateCashFlows::try_new(base, end, zero, amount("-1", Currency::Krw), span),
            Err(EstimateReturnError::NegativeCost)
        );
        assert_eq!(
            cash("100", "120", "0", "121")
                .net()
                .expect("수취액보다 큰 비용"),
            RatioQuotient::Exact(ratio("-1.01"))
        );
        assert_eq!(
            BenchmarkCashFlows::new(base, price("100", Currency::Usd), span),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        let wrong_currency = BenchmarkCashFlows::new(
            price("100", Currency::Usd),
            price("110", Currency::Usd),
            span,
        )
        .expect("달러 비교");
        assert_eq!(
            cash("100", "120", "0", "0").excess(&wrong_currency),
            Err(EstimateReturnError::CurrencyMismatch)
        );
        let wrong_period = BenchmarkCashFlows::new(base, end, interval(1, 6)).expect("다른 구간");
        assert_eq!(
            cash("100", "120", "0", "0").excess(&wrong_period),
            Err(EstimateReturnError::PeriodMismatch)
        );
    }

    #[test]
    fn d54_nonterminating_division_is_marked_approximate_and_direction_is_retained() {
        let positive = cash("3", "4", "0", "0").net().expect("양수");
        expect_approximate(positive);
        assert_eq!(cmp(positive, ratio("0")), std::cmp::Ordering::Greater);
        let negative = cash("3", "2", "0", "0").net().expect("음수");
        expect_approximate(negative);
        assert_eq!(cmp(negative, ratio("0")), std::cmp::Ordering::Less);
        let positive_two_thirds = cash("3", "5", "0", "0").net().expect("양수 반올림");
        assert_eq!(
            cmp(positive_two_thirds, ratio("0.6666666666666666666666666667")),
            std::cmp::Ordering::Equal
        );
        let negative_two_thirds = cash("3", "1", "0", "0").net().expect("음수 반올림");
        assert_eq!(
            cmp(
                negative_two_thirds,
                ratio("-0.6666666666666666666666666667")
            ),
            std::cmp::Ordering::Equal
        );
        let equal_approximate = cash("3", "4", "0", "0")
            .excess(&benchmark("6", "8"))
            .expect("동일 근사 수익률");
        expect_approximate(equal_approximate);
        assert_eq!(
            cmp(equal_approximate, ratio("0")),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn d54_overflow_is_not_turned_into_a_return() {
        let maximum = "79228162514264337593543950335";
        assert_eq!(
            cash(maximum, "1", "1", "0").net(),
            Err(EstimateReturnError::Arithmetic(
                domain::MoneyError::Overflow
            ))
        );
        assert_eq!(
            cash("1", maximum, "0", "0").excess(&benchmark("1", "2")),
            Ok(RatioQuotient::Exact(ratio("79228162514264337593543950333")))
        );
    }

    #[test]
    fn d54_cash_flow_errors_propagate_at_each_arithmetic_boundary() {
        let maximum = "79228162514264337593543950335";
        let tiny = "0.0000000000000000000000000001";
        assert_eq!(
            cash("1", tiny, "0", maximum).net(),
            Err(EstimateReturnError::Arithmetic(
                domain::MoneyError::PrecisionLoss
            ))
        );
        assert_eq!(
            cash(maximum, "1", "0", maximum).net(),
            Err(EstimateReturnError::Arithmetic(
                domain::MoneyError::Overflow
            ))
        );
        assert_eq!(
            cash(maximum, "1", "1", "0").excess(&benchmark("1", "2")),
            Err(EstimateReturnError::Arithmetic(
                domain::MoneyError::Overflow
            ))
        );
        assert_eq!(
            cash("1", "2", "0", "0").excess(&benchmark(tiny, maximum)),
            Err(EstimateReturnError::Arithmetic(
                domain::MoneyError::PrecisionLoss
            ))
        );
        assert_eq!(
            cash("1", maximum, "0", "0").excess(&benchmark("2", "1")),
            Err(EstimateReturnError::Arithmetic(
                domain::MoneyError::PrecisionLoss
            ))
        );
    }

    #[test]
    fn d54_cash_flow_debug_hides_prices_and_costs() {
        assert_eq!(
            format!("{:?}", cash("123", "234", "5", "6")),
            "EstimateCashFlows([REDACTED])"
        );
        assert_eq!(
            format!("{:?}", benchmark("345", "456")),
            "BenchmarkCashFlows([REDACTED])"
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn d54_exit_is_monotone_and_cost_never_increases_return(
            first in 100_u32..10_000,
            delta in 0_u32..10_000,
            cost in 0_u32..100,
        ) {
            let last = first.checked_add(delta).expect("종가 범위");
            let expensive = cash("10000", &last.to_string(), &cost.to_string(), "0").net().expect("비용 수익률");
            let sell_cost = cash("10000", &last.to_string(), "0", &cost.to_string()).net().expect("매도 비용 수익률");
            let free = cash("10000", &last.to_string(), "0", "0").net().expect("무비용 수익률");
            let earlier = cash("10000", &first.to_string(), "0", "0").net().expect("낮은 종가 수익률");
            let equal = cash("10000", &last.to_string(), "0", "0").excess(&benchmark("10000", &last.to_string())).expect("동일 수익률");
            let value = |result| match result {
                RatioQuotient::Exact(value) => value,
                RatioQuotient::Approximate(_) => panic!("무비용 수익률은 유한 십진수"),
            };
            prop_assert_ne!(cmp(expensive, value(free)), std::cmp::Ordering::Greater);
            prop_assert_ne!(cmp(sell_cost, value(free)), std::cmp::Ordering::Greater);
            prop_assert_ne!(cmp(earlier, value(free)), std::cmp::Ordering::Greater);
            prop_assert_eq!(cmp(equal, ratio("0")), std::cmp::Ordering::Equal);
        }
    }
}
