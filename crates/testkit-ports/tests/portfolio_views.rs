#[cfg(test)]
mod tests {
    use application::{GetAllocationView, GetPortfolioView, StoredView};
    use domain::{
        AccountId, BuyingPower, Currency, Money, Nanosecond, NumericInput, OffsetSeconds,
        Portfolio, SyncOutcome, SyncRecord, Timestamp, UnixSeconds,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::{
        MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
    };
    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(123),
            Nanosecond::new(17),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn portfolio() -> Portfolio {
        let power = |text, currency| {
            BuyingPower::new(
                Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액"),
                at(),
            )
        };
        Portfolio::new(
            vec![],
            power("137", Currency::Krw),
            power("29", Currency::Usd),
        )
        .expect("포트폴리오")
    }
    fn assert_native_and_incomplete(value: &domain::PortfolioValuation) {
        assert_eq!(
            value.buying_power(Currency::Krw).money(),
            portfolio().buying_power(Currency::Krw).money()
        );
        assert_eq!(
            value.buying_power(Currency::Usd).money(),
            portfolio().buying_power(Currency::Usd).money()
        );
        assert_eq!(
            value.assets().completeness(),
            domain::TotalCompleteness::Incomplete
        );
    }
    #[test]
    fn absent_failed_blocked_and_partial_are_distinct() {
        let account = AccountId::new(71);
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        store.script_latest().push(MemoryReply::Return(Ok(None)));
        assert_eq!(
            run(GetPortfolioView::new(&store, &market).execute(&account)),
            Ok(StoredView::Missing)
        );
        for outcome in [
            SyncOutcome::Failed,
            SyncOutcome::AccessBlocked,
            SyncOutcome::Partial(portfolio()),
            SyncOutcome::Complete(portfolio()),
        ] {
            store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    at(),
                    outcome.clone(),
                )))));
            match &outcome {
                SyncOutcome::Complete(_) | SyncOutcome::Partial(_) => {
                    market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
                }
                SyncOutcome::Failed | SyncOutcome::AccessBlocked => {}
            }
            let StoredView::Recorded(view) =
                run(GetPortfolioView::new(&store, &market).execute(&account)).expect("조회")
            else {
                panic!("기록")
            };
            assert_eq!(view.recorded_at(), at());
            match (outcome, view.outcome()) {
                (SyncOutcome::Failed, SyncOutcome::Failed)
                | (SyncOutcome::AccessBlocked, SyncOutcome::AccessBlocked) => {}
                (SyncOutcome::Partial(_), SyncOutcome::Partial(value))
                | (SyncOutcome::Complete(_), SyncOutcome::Complete(value)) => {
                    assert_native_and_incomplete(value);
                }
                _ => panic!("상태 보존"),
            }
        }
        assert_eq!(store.script_latest().calls(), vec![(account,); 5]);
        assert_eq!(
            market.script_fx_rate().calls(),
            vec![
                (
                    domain::FxPair::UsdKrw,
                    domain::FxRateKind::Mid,
                    application::FxRateQuery::At(at())
                );
                2
            ]
        );
    }
    #[test]
    fn allocation_retains_unavailable_conversion() {
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let account = AccountId::new(93);
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                at(),
                SyncOutcome::Complete(portfolio()),
            )))));
        market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
        let StoredView::Recorded(view) =
            run(GetAllocationView::new(&store, &market).execute(&account)).expect("조회")
        else {
            panic!("기록")
        };
        let SyncOutcome::Complete(value) = view.outcome() else {
            panic!("상태")
        };
        assert!(value.assets().instruments().is_err());
        assert_eq!(
            value.valuation().buying_power(Currency::Usd).money(),
            portfolio().buying_power(Currency::Usd).money()
        );
    }
}

#[cfg(test)]
mod conversion_tests {
    use application::{
        FxRateQuery, GetAllocationView, GetPortfolioView, StoredView, SystemFailure, SystemPort,
        SystemPortError, ViewFreshness,
    };
    use domain::{
        AccountId, BuyingPower, Currency, FxPair, FxRate, FxRateKind, FxSource, Money, Nanosecond,
        NumericInput, OffsetSeconds, Portfolio, SyncOutcome, SyncRecord, TimeInterval, Timestamp,
        Total, UnixSeconds, ValuationAccuracy,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::{
        MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
    };
    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
    }
    fn portfolio() -> Portfolio {
        Portfolio::new(
            vec![],
            BuyingPower::new(money("137", Currency::Krw), at(71)),
            BuyingPower::new(money("29", Currency::Usd), at(83)),
        )
        .expect("포트폴리오")
    }
    fn rate(pair: FxPair, kind: FxRateKind, start: i64, end: i64) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new("11").expect("입력"),
            pair,
            kind,
            TimeInterval::try_new(at(start), at(end)).expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    #[test]
    fn mid_conversion_and_allocation_preserve_independent_values_and_metadata() {
        let account = AccountId::new(97);
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let fx = rate(FxPair::UsdKrw, FxRateKind::Mid, 100, 200);
        for () in [(), ()] {
            store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    at(123),
                    SyncOutcome::Partial(portfolio()),
                )))));
            market
                .script_fx_rate()
                .push(MemoryReply::Return(Ok(Some(fx))));
        }
        let StoredView::Recorded(view) =
            run(GetPortfolioView::new(&store, &market).execute(&account)).expect("조회")
        else {
            panic!("기록")
        };
        assert_eq!(view.exchange_rate_lookup(), &Ok(Some(fx)));
        assert_eq!(view.freshness(), ViewFreshness::DefinitionPending);
        let SyncOutcome::Partial(value) = view.outcome() else {
            panic!("부분")
        };
        assert_eq!(value.buying_power(Currency::Krw).observed_at(), at(71));
        assert_eq!(value.buying_power(Currency::Usd).observed_at(), at(83));
        let Total::Complete(assets) = value.assets() else {
            panic!("완전 합계")
        };
        assert_eq!(
            assets.value().clone().into_parts(),
            (
                money("456", Currency::Krw),
                vec![fx],
                ValuationAccuracy::Approximate
            )
        );
        let StoredView::Recorded(allocation_view) =
            run(GetAllocationView::new(&store, &market).execute(&account)).expect("조회")
        else {
            panic!("기록")
        };
        assert_eq!(allocation_view.recorded_at(), at(123));
        assert_eq!(allocation_view.exchange_rate_lookup(), &Ok(Some(fx)));
        let SyncOutcome::Partial(allocation) = allocation_view.outcome() else {
            panic!("부분")
        };
        let entries = allocation
            .assets()
            .currencies()
            .as_ref()
            .expect("통화")
            .entries();
        assert_eq!(entries[0].display().units().as_u32(), 3004);
        assert_eq!(entries[1].display().units().as_u32(), 6996);
        assert_eq!(
            market.script_fx_rate().calls(),
            vec![(FxPair::UsdKrw, FxRateKind::Mid, FxRateQuery::At(at(123))); 2]
        );
    }
    #[test]
    fn wrong_kind_pair_and_interval_never_convert_but_preserve_lookup_evidence() {
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        for fx in [
            rate(FxPair::KrwUsd, FxRateKind::Mid, 100, 200),
            rate(FxPair::UsdKrw, FxRateKind::Buy, 100, 200),
            rate(FxPair::UsdKrw, FxRateKind::Mid, 124, 200),
            rate(FxPair::UsdKrw, FxRateKind::Mid, 100, 123),
        ] {
            store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    at(123),
                    SyncOutcome::Complete(portfolio()),
                )))));
            market
                .script_fx_rate()
                .push(MemoryReply::Return(Ok(Some(fx))));
            let StoredView::Recorded(view) =
                run(GetPortfolioView::new(&store, &market).execute(&AccountId::new(3)))
                    .expect("조회")
            else {
                panic!("기록")
            };
            assert_eq!(view.exchange_rate_lookup(), &Ok(Some(fx)));
            let SyncOutcome::Complete(value) = view.outcome() else {
                panic!("상태")
            };
            assert_eq!(
                value.assets().completeness(),
                domain::TotalCompleteness::Incomplete
            );
        }
    }
    #[test]
    fn storage_errors_propagate_and_fx_errors_preserve_native_values() {
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        for () in [(), ()] {
            store.script_latest().push(MemoryReply::Return(Err(error)));
        }
        assert_eq!(
            run(GetPortfolioView::new(&store, &market).execute(&AccountId::new(1))),
            Err(error)
        );
        assert_eq!(
            run(GetAllocationView::new(&store, &market).execute(&AccountId::new(2))),
            Err(error)
        );
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                at(123),
                SyncOutcome::Partial(portfolio()),
            )))));
        market
            .script_fx_rate()
            .push(MemoryReply::Return(Err(error)));
        let StoredView::Recorded(view) =
            run(GetPortfolioView::new(&store, &market).execute(&AccountId::new(3))).expect("조회")
        else {
            panic!("기록")
        };
        assert_eq!(view.exchange_rate_lookup(), &Err(error));
        let SyncOutcome::Partial(value) = view.outcome() else {
            panic!("부분")
        };
        assert_eq!(
            value.buying_power(Currency::Krw).money(),
            Ok(money("137", Currency::Krw))
        );
    }
    #[test]
    fn allocation_missing_failed_and_blocked_do_not_read_fx() {
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        store.script_latest().push(MemoryReply::Return(Ok(None)));
        assert_eq!(
            run(GetAllocationView::new(&store, &market).execute(&AccountId::new(3))),
            Ok(StoredView::Missing)
        );
        for outcome in [SyncOutcome::Failed, SyncOutcome::AccessBlocked] {
            store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    at(91),
                    outcome.clone(),
                )))));
            let StoredView::Recorded(view) =
                run(GetAllocationView::new(&store, &market).execute(&AccountId::new(3)))
                    .expect("조회")
            else {
                panic!("기록")
            };
            assert_eq!(view.recorded_at(), at(91));
            assert_eq!(view.exchange_rate_lookup(), &Ok(None));
            match (outcome, view.outcome()) {
                (SyncOutcome::Failed, SyncOutcome::Failed)
                | (SyncOutcome::AccessBlocked, SyncOutcome::AccessBlocked) => {}
                _ => panic!("상태"),
            }
        }
        assert_eq!(market.calls(), vec![]);
    }
    #[test]
    fn complete_conversion_is_not_dropped() {
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let fx = rate(FxPair::UsdKrw, FxRateKind::Mid, 123, 200);
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                at(123),
                SyncOutcome::Complete(portfolio()),
            )))));
        market
            .script_fx_rate()
            .push(MemoryReply::Return(Ok(Some(fx))));
        let StoredView::Recorded(view) =
            run(GetPortfolioView::new(&store, &market).execute(&AccountId::new(41))).expect("조회")
        else {
            panic!("기록")
        };
        let SyncOutcome::Complete(value) = view.outcome() else {
            panic!("완료")
        };
        let Total::Complete(assets) = value.assets() else {
            panic!("완전")
        };
        assert_eq!(
            assets.value().clone().into_parts(),
            (
                money("456", Currency::Krw),
                vec![fx],
                ValuationAccuracy::Approximate
            )
        );
    }
    fn holding() -> domain::Holding {
        let master = domain::MasterInstrument::try_new(domain::MasterInstrumentRow {
            listing_market: domain::MasterCode::try_new("NASDAQ").expect("시장"),
            instrument_id: domain::InstrumentId::try_new("fixture-view").expect("ID"),
            symbol: domain::Symbol::try_new("AAPL").expect("심볼"),
            name: domain::DisplayInstrumentName::try_new("고정 보유").expect("이름"),
            security_type: domain::MasterSecurityType::from_code(
                domain::MasterCode::try_new("STOCK").expect("종류"),
            ),
            currency: Currency::Usd,
            listing_status: domain::ListingStatus::Active,
            common_share: domain::CommonShare::Common,
            korean_detail: None,
            observed_at: at(43),
        })
        .expect("마스터");
        domain::Holding::new(
            master.order_instrument().expect("종목"),
            domain::Quantity::parse(&NumericInput::try_new("3").expect("입력")).expect("수량"),
            domain::AcquisitionPrice::try_new(money("11", Currency::Usd)).expect("원가"),
            domain::HoldingObservation::new(
                Some(
                    domain::Price::parse(
                        &NumericInput::try_new("17").expect("입력"),
                        Currency::Usd,
                    )
                    .expect("가격"),
                ),
                None,
                at(47),
            ),
        )
        .expect("보유")
    }
    #[test]
    fn holdings_and_incomplete_cash_remain_visible_without_zero_substitution() {
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let portfolio = Portfolio::new(
            vec![holding()],
            BuyingPower::new(money("-5", Currency::Krw), at(73)),
            BuyingPower::new(money("29", Currency::Usd), at(89)),
        )
        .expect("자산");
        let fx = rate(FxPair::UsdKrw, FxRateKind::Mid, 100, 200);
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                at(123),
                SyncOutcome::Partial(portfolio),
            )))));
        market
            .script_fx_rate()
            .push(MemoryReply::Return(Ok(Some(fx))));
        let StoredView::Recorded(view) =
            run(GetPortfolioView::new(&store, &market).execute(&AccountId::new(41))).expect("조회")
        else {
            panic!("기록")
        };
        let SyncOutcome::Partial(value) = view.outcome() else {
            panic!("부분")
        };
        assert_eq!(value.holdings()[0].holding(), &holding());
        let Total::Complete(native) = value.currency_total(Currency::Usd) else {
            panic!("통화 평가")
        };
        assert_eq!(native.value(), &money("51", Currency::Usd));
        assert_eq!(
            value.buying_power(Currency::Krw).money(),
            Err(domain::PortfolioError::NegativeBuyingPower)
        );
        let Total::Incomplete(assets) = value.assets() else {
            panic!("부분 합계")
        };
        assert_eq!(
            assets.subtotal().clone().into_parts(),
            (
                money("880", Currency::Krw),
                vec![fx],
                ValuationAccuracy::Approximate
            )
        );
    }
    #[test]
    fn latest_failure_preserves_failure_without_reusing_a_success() {
        use application::PortfolioRepository;
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let account = AccountId::new(67);
        for record in [
            SyncRecord::new(at(100), SyncOutcome::Complete(portfolio())),
            SyncRecord::new(at(200), SyncOutcome::Failed),
        ] {
            store.script_record().push(MemoryReply::Memory);
            assert_eq!(
                run(store.record(&account, &record)),
                Ok(application::WriteCompleted)
            );
        }
        store.script_latest().push(MemoryReply::Memory);
        let StoredView::Recorded(view) =
            run(GetPortfolioView::new(&store, &market).execute(&account)).expect("조회")
        else {
            panic!("기록")
        };
        assert_eq!(view.recorded_at(), at(200));
        assert_eq!(view.outcome(), &SyncOutcome::Failed);
        assert_eq!(market.calls(), vec![]);
    }
    #[test]
    fn allocation_without_stored_assignments_is_unclassified_with_cash_separate() {
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let portfolio = Portfolio::new(
            vec![holding()],
            portfolio().buying_power(Currency::Krw),
            portfolio().buying_power(Currency::Usd),
        )
        .expect("보유");
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                at(123),
                SyncOutcome::Complete(portfolio),
            )))));
        market
            .script_fx_rate()
            .push(MemoryReply::Return(Ok(Some(rate(
                FxPair::UsdKrw,
                FxRateKind::Mid,
                100,
                200,
            )))));
        let StoredView::Recorded(view) =
            run(GetAllocationView::new(&store, &market).execute(&AccountId::new(71)))
                .expect("조회")
        else {
            panic!("기록")
        };
        assert_eq!(view.freshness(), ViewFreshness::DefinitionPending);
        let SyncOutcome::Complete(allocation) = view.outcome() else {
            panic!("완료")
        };
        let holdings = allocation
            .holdings()
            .sectors()
            .as_ref()
            .expect("보유 구성비")
            .entries();
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].key(), &domain::AllocationSector::Unclassified);
        assert_eq!(holdings[0].display().units().as_u32(), 10000);
        let assets = allocation
            .assets()
            .sectors()
            .as_ref()
            .expect("총자산 비중")
            .entries();
        assert_eq!(assets.len(), 2);
        assert_eq!(assets[0].key(), &domain::AllocationSector::Cash);
        assert_eq!(assets[1].key(), &domain::AllocationSector::Unclassified);
        assert_eq!(assets[0].display().units().as_u32(), 4484);
        assert_eq!(assets[1].display().units().as_u32(), 5516);
    }
    fn reported_summary() -> domain::ReportedPortfolio {
        domain::ReportedPortfolio::new(
            domain::CurrencyAmounts::new(
                money("31", Currency::Krw),
                Some(money("7", Currency::Usd)),
            )
            .expect("평가"),
            domain::CurrencyAmounts::new(
                money("23", Currency::Krw),
                Some(money("5", Currency::Usd)),
            )
            .expect("원가"),
            domain::CurrencyAmounts::new(
                money("8", Currency::Krw),
                Some(money("2", Currency::Usd)),
            )
            .expect("손익"),
            at(97),
        )
    }
    #[test]
    fn complete_reported_summary_preserves_authoritative_values_and_used_rate() {
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let reported = reported_summary();
        let portfolio = Portfolio::new(
            vec![holding()],
            portfolio().buying_power(Currency::Krw),
            portfolio().buying_power(Currency::Usd),
        )
        .expect("보유")
        .with_reported(reported);
        let fx = rate(FxPair::UsdKrw, FxRateKind::Mid, 100, 200);
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                at(123),
                SyncOutcome::Complete(portfolio),
            )))));
        market
            .script_fx_rate()
            .push(MemoryReply::Return(Ok(Some(fx))));
        let StoredView::Recorded(view) =
            run(GetPortfolioView::new(&store, &market).execute(&AccountId::new(73))).expect("조회")
        else {
            panic!("기록")
        };
        let SyncOutcome::Complete(value) = view.outcome() else {
            panic!("완료")
        };
        assert_eq!(value.reported(), Some(reported));
        let Total::Complete(native) = value.currency_total(Currency::Usd) else {
            panic!("정본 평가")
        };
        assert_eq!(native.value(), &money("7", Currency::Usd));
        let Total::Complete(assets) = value.assets() else {
            panic!("완전")
        };
        assert_eq!(
            assets.value().clone().into_parts(),
            (
                money("564", Currency::Krw),
                vec![fx],
                ValuationAccuracy::Approximate
            )
        );
    }
    #[test]
    fn complete_fx_failure_preserves_native_data_in_both_views() {
        let store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        for () in [(), ()] {
            store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    at(123),
                    SyncOutcome::Complete(portfolio()),
                )))));
            market
                .script_fx_rate()
                .push(MemoryReply::Return(Err(error)));
        }
        let StoredView::Recorded(view) =
            run(GetPortfolioView::new(&store, &market).execute(&AccountId::new(79))).expect("조회")
        else {
            panic!("기록")
        };
        assert_eq!(view.exchange_rate_lookup(), &Err(error));
        let SyncOutcome::Complete(value) = view.outcome() else {
            panic!("완료 기록")
        };
        assert_eq!(
            value.buying_power(Currency::Krw).money(),
            Ok(money("137", Currency::Krw))
        );
        assert_eq!(
            value.buying_power(Currency::Usd).money(),
            Ok(money("29", Currency::Usd))
        );
        assert_eq!(
            value.assets().completeness(),
            domain::TotalCompleteness::Incomplete
        );
        let StoredView::Recorded(allocation_view) =
            run(GetAllocationView::new(&store, &market).execute(&AccountId::new(79)))
                .expect("조회")
        else {
            panic!("기록")
        };
        assert_eq!(allocation_view.exchange_rate_lookup(), &Err(error));
        let SyncOutcome::Complete(allocation) = allocation_view.outcome() else {
            panic!("완료 기록")
        };
        assert_eq!(
            allocation.valuation().buying_power(Currency::Krw).money(),
            Ok(money("137", Currency::Krw))
        );
        assert_eq!(
            allocation.valuation().buying_power(Currency::Usd).money(),
            Ok(money("29", Currency::Usd))
        );
        assert_eq!(
            allocation.valuation().assets().completeness(),
            domain::TotalCompleteness::Incomplete
        );
    }
}
