use crate::{BuyingPower, Currency, Holding, Portfolio, PortfolioError, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashLookupFailure {
    Unavailable,
    AccessDenied,
    InvalidResponse,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CashObservation {
    Observed(BuyingPower),
    Failed {
        currency: Currency,
        reason: CashLookupFailure,
        observed_at: Timestamp,
    },
}
pub fn portfolio_from_cash_observations(
    holdings: Vec<Holding>,
    krw: CashObservation,
    usd: CashObservation,
) -> Result<Portfolio, PortfolioError> {
    Portfolio::new(
        holdings,
        crate::portfolio::buying_power_from_observation(krw),
        crate::portfolio::buying_power_from_observation(usd),
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        AcquisitionPrice, Amount, BuyingPower, CashLookupFailure, CashObservation, Currency,
        FxPair, FxRate, FxRateKind, FxSource, Holding, HoldingObservation, Market, Money,
        Nanosecond, NumericInput, OffsetSeconds, Portfolio, PortfolioError, Quantity,
        ResolvedOrderInstrument, Symbol, TimeInterval, Timestamp, Total, UnixSeconds,
        portfolio_from_cash_observations,
    };
    use proptest::{
        prop_assert_eq,
        test_runner::{Config, TestRunner},
    };
    fn time() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(100),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn money(value: i64, currency: Currency) -> Money {
        Money::new(Amount::new(rust_decimal::Decimal::from(value)), currency)
    }
    fn power(value: i64, currency: Currency) -> BuyingPower {
        BuyingPower::new(money(value, currency), time())
    }
    fn failure(currency: Currency) -> CashObservation {
        CashObservation::Failed {
            currency,
            reason: CashLookupFailure::Unavailable,
            observed_at: time(),
        }
    }
    fn rate() -> FxRate {
        FxRate::parse(
            &NumericInput::try_new("2").expect("입력"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(
                time(),
                Timestamp::from_parts(
                    UnixSeconds::new(200),
                    Nanosecond::new(0),
                    OffsetSeconds::new(0),
                )
                .expect("끝"),
            )
            .expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    fn holdings() -> Vec<Holding> {
        [
            ("A", Currency::Krw, Market::Kr),
            ("B", Currency::Usd, Market::Us),
        ]
        .into_iter()
        .map(|(symbol, currency, market)| {
            Holding::new(
                ResolvedOrderInstrument::from_master_lookup_result(
                    Symbol::try_new(symbol).expect("심볼"),
                    market,
                )
                .expect("종목"),
                Quantity::parse(&NumericInput::try_new("1").expect("입력")).expect("수량"),
                AcquisitionPrice::try_new(money(1, currency)).expect("원가"),
                HoldingObservation::new(
                    Some(
                        crate::Price::parse(&NumericInput::try_new("10").expect("입력"), currency)
                            .expect("가격"),
                    ),
                    None,
                    time(),
                ),
            )
            .expect("보유")
        })
        .collect()
    }
    fn asset_money(total: &Total<crate::AssetEstimate>) -> Money {
        match total {
            Total::Complete(value) => value.value().clone().into_parts().0,
            Total::Incomplete(value) => value.subtotal().clone().into_parts().0,
        }
    }
    fn full_amount(krw: i64, usd: i64) -> Money {
        let dollars = usd.checked_mul(2).expect("달러 환산");
        let cash = krw.checked_add(dollars).expect("현금 합");
        money(cash.checked_add(30).expect("보유 합산"), Currency::Krw)
    }
    fn complete_portfolio(krw: i64, usd: i64) -> crate::PortfolioValuation {
        Portfolio::new(
            holdings(),
            power(krw, Currency::Krw),
            power(usd, Currency::Usd),
        )
        .expect("정상")
        .evaluate(Some(rate()))
    }
    fn assert_holdings_unchanged(
        partial: &crate::PortfolioValuation,
        baseline: &crate::PortfolioValuation,
    ) {
        assert_eq!(partial.holdings(), baseline.holdings());
        assert_eq!(partial.valuation(), baseline.valuation());
        assert_eq!(partial.total_purchase(), baseline.total_purchase());
        assert_eq!(partial.total_profit(), baseline.total_profit());
        for currency in [Currency::Krw, Currency::Usd] {
            assert_eq!(
                partial.currency_summary(currency),
                baseline.currency_summary(currency)
            );
        }
    }
    #[test]
    fn failed_currency_has_no_money_and_preserves_others() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(
                    0_i64..10000,
                    0_i64..10000,
                    proptest::sample::select(vec![Currency::Krw, Currency::Usd]),
                ),
                |(krw, usd, failed)| {
                    let baseline = complete_portfolio(krw, usd);
                    let krw_observation = if failed == Currency::Krw {
                        failure(failed)
                    } else {
                        CashObservation::Observed(power(krw, Currency::Krw))
                    };
                    let usd_observation = if failed == Currency::Usd {
                        failure(failed)
                    } else {
                        CashObservation::Observed(power(usd, Currency::Usd))
                    };
                    let partial = portfolio_from_cash_observations(
                        holdings(),
                        krw_observation,
                        usd_observation,
                    )
                    .expect("부분 성공")
                    .evaluate(Some(rate()));
                    assert_holdings_unchanged(&partial, &baseline);
                    prop_assert_eq!(
                        partial.buying_power(failed).money(),
                        Err(PortfolioError::CashLookupFailed(
                            CashLookupFailure::Unavailable
                        ))
                    );
                    prop_assert_eq!(partial.buying_power(failed).currency(), failed);
                    prop_assert_eq!(partial.buying_power(failed).observed_at(), time());
                    let (other, expected) = match failed {
                        Currency::Krw => (
                            Currency::Usd,
                            30_i64
                                .checked_add(usd.checked_mul(2).expect("두 배"))
                                .expect("합"),
                        ),
                        Currency::Usd => (Currency::Krw, 30_i64.checked_add(krw).expect("합")),
                    };
                    prop_assert_eq!(partial.buying_power(other), baseline.buying_power(other));
                    prop_assert_eq!(
                        partial.assets().completeness(),
                        crate::TotalCompleteness::Incomplete
                    );
                    prop_assert_eq!(
                        asset_money(partial.assets()),
                        money(expected, Currency::Krw)
                    );
                    prop_assert_eq!(asset_money(baseline.assets()), full_amount(krw, usd));
                    Ok(())
                },
            )
            .expect("한 통화 실패 격리");
    }
    #[test]
    fn observations_preserve_success_errors_and_all_failure_reasons() {
        let original = Portfolio::new(
            holdings(),
            power(10, Currency::Krw),
            power(20, Currency::Usd),
        )
        .expect("기존");
        assert_eq!(
            portfolio_from_cash_observations(
                holdings(),
                CashObservation::Observed(power(10, Currency::Krw)),
                CashObservation::Observed(power(20, Currency::Usd))
            ),
            Ok(original)
        );
        for reason in [
            CashLookupFailure::Unavailable,
            CashLookupFailure::AccessDenied,
            CashLookupFailure::InvalidResponse,
        ] {
            let observation = CashObservation::Failed {
                currency: Currency::Usd,
                reason,
                observed_at: time(),
            };
            let portfolio = portfolio_from_cash_observations(
                vec![],
                CashObservation::Observed(power(-1, Currency::Krw)),
                observation,
            )
            .expect("현금 오류 보존");
            assert_eq!(
                portfolio.buying_power(Currency::Krw).money(),
                Err(PortfolioError::NegativeBuyingPower)
            );
            assert_eq!(
                portfolio.buying_power(Currency::Usd).money(),
                Err(PortfolioError::CashLookupFailed(reason))
            );
            assert_eq!(portfolio.evaluate(None).buying_power_errors().len(), 2);
        }
        assert_eq!(
            portfolio_from_cash_observations(
                vec![],
                failure(Currency::Usd),
                failure(Currency::Krw)
            ),
            Err(PortfolioError::CurrencyMismatch)
        );
    }
    #[test]
    fn allocation_and_policy_block_incomplete_cash() {
        let value = portfolio_from_cash_observations(
            holdings(),
            CashObservation::Observed(power(100, Currency::Krw)),
            failure(Currency::Usd),
        )
        .expect("부분 성공")
        .evaluate(Some(rate()));
        let assignments = crate::SectorAssignments::try_new(vec![]).expect("빈 분류");
        let allocation = crate::Allocation::new(&value, &assignments);
        assert_eq!(
            allocation.assets().instruments(),
            &Err(crate::AllocationError::NegativeCash(Currency::Usd))
        );
        assert!(allocation.holdings().instruments().is_ok());
        let findings = crate::Policy::new(crate::InvestorProfile::default()).check_current(
            &value,
            &assignments,
            &crate::PolicySecurityFacts::default(),
        );
        assert_eq!(
            findings,
            Err(crate::PolicyViolation::Allocation(
                crate::AllocationError::NegativeCash(Currency::Usd)
            ))
        );
    }
    #[test]
    fn buy_policy_blocks_with_known_candidate_and_failed_cash() {
        let instrument = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("A").expect("심볼"),
            Market::Kr,
        )
        .expect("종목");
        let assignments = crate::SectorAssignments::try_new(vec![crate::SectorAssignment::new(
            &instrument,
            crate::InstrumentClassification::Sector(
                crate::SectorId::try_new("Tech").expect("섹터"),
            ),
        )])
        .expect("분류");
        let facts = crate::PolicySecurityFacts::from_master_lookup_results(
            std::iter::once((
                crate::AllocationInstrument::new(&instrument),
                crate::ExcludedSecurityType::Stock,
            ))
            .collect(),
        );
        let value = portfolio_from_cash_observations(
            holdings(),
            CashObservation::Observed(power(100, Currency::Krw)),
            failure(Currency::Usd),
        )
        .expect("부분 성공")
        .evaluate(Some(rate()));
        let candidate =
            crate::PolicyCandidate::new(&instrument, money(1, Currency::Krw)).expect("후보");
        assert_eq!(
            crate::Policy::new(crate::InvestorProfile::default()).check_buy(
                &value,
                &assignments,
                &facts,
                &candidate
            ),
            crate::PolicyBuyDecision::Block(crate::PolicyViolation::Allocation(
                crate::AllocationError::NegativeCash(Currency::Usd)
            ))
        );
    }
}
