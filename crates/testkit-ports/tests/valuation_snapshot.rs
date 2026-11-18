#[cfg(test)]
mod tests {
    use application::{
        EventBusBuilder, EventSubscriber, FxRateQuery, RecordOutcome, SubscriberFailure,
        SystemFailure, SystemPort, SystemPortError, TakeValuationSnapshot, ValuationSnapshotError,
    };
    use domain::{
        AccountId, BuyingPower, Currency, DomainEvent, DomainEventKind, FxPair, FxRateKind, Market,
        MarketCalendar, MarketSession, Money, Nanosecond, NumericInput, OffsetSeconds, Portfolio,
        SessionData, SessionInformation, SnapshotCalendar, SnapshotError, SnapshotSource,
        SyncOutcome, SyncRecord, TimeInterval, Timestamp, UnixSeconds, ValuationSnapshot,
    };
    use std::{
        collections::BTreeSet,
        future::Future,
        sync::{
            Arc,
            mpsc::{Sender, channel},
        },
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
        memory::{
            MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
            MemoryValuationRepository,
        },
    };
    fn run<F: Future>(future: F) -> F::Output {
        match std::pin::pin!(future)
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn at(day: i64, hour: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                day.checked_mul(86400)
                    .expect("일")
                    .checked_add(hour.checked_mul(3600).expect("시"))
                    .expect("초"),
            ),
            Nanosecond::new(37),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn session(day: i64, market: &Market) -> MarketSession {
        session_with_close(day, market, 1)
    }
    fn session_with_close(day: i64, market: &Market, close: i64) -> MarketSession {
        let data = Some(
            SessionData::try_new(
                TimeInterval::try_new(at(day, 0), at(day, close)).expect("구간"),
                None,
            )
            .expect("장"),
        );
        let info = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
        };
        MarketSession::try_new(at(day, 0).offset_calendar_date(), info).expect("세션")
    }
    fn calendar(day: i64) -> SnapshotCalendar {
        let make = |market: &Market| {
            MarketCalendar::try_new(
                session(day.checked_sub(1).expect("전일"), market),
                session(day, market),
                session(day.checked_add(1).expect("익일"), market),
            )
            .expect("달력")
        };
        SnapshotCalendar::new(&make(&Market::Kr), &make(&Market::Us)).expect("평가 달력")
    }
    fn calendar_closes(kr: i64, us: i64) -> SnapshotCalendar {
        let make = |market: &Market, close| {
            MarketCalendar::try_new(
                session_with_close(2, market, close),
                session_with_close(3, market, close),
                session_with_close(4, market, close),
            )
            .expect("달력")
        };
        SnapshotCalendar::new(&make(&Market::Kr, kr), &make(&Market::Us, us)).expect("평가 달력")
    }
    fn portfolio() -> Portfolio {
        let power = |text, currency| {
            BuyingPower::new(
                Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액"),
                at(3, 1),
            )
        };
        Portfolio::new(
            vec![],
            power("137", Currency::Krw),
            power("29", Currency::Usd),
        )
        .expect("포트")
    }
    fn error() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied)
    }
    struct Observer {
        repository: Arc<MemoryValuationRepository>,
        events: Sender<DomainEvent>,
        order: Sender<usize>,
    }
    impl EventSubscriber for Observer {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.events
                .send(event.clone())
                .map_err(|_error| SubscriberFailure::Unavailable)?;
            self.order
                .send(self.repository.calls().len())
                .map_err(|_error| SubscriberFailure::Unavailable)?;
            Err(SubscriberFailure::Unavailable)
        }
    }
    fn assert_report(
        report: &application::ValuationSnapshotReport,
        dates: &SnapshotCalendar,
        index: usize,
    ) {
        assert_eq!(
            report.snapshot,
            ValuationSnapshot::observe(dates, &portfolio().evaluate(None), at(3, 1), None)
                .expect("도메인 위임")
        );
        assert_eq!(
            report.recorded,
            if index == 0 {
                RecordOutcome::Recorded
            } else {
                RecordOutcome::AlreadyRecorded
            }
        );
        assert_eq!(report.snapshot.source(), SnapshotSource::Observed);
        assert_eq!(report.snapshot.recorded_at(), at(3, 1));
        assert_eq!(report.snapshot.valuation_at(), at(3, 1));
        assert_eq!(report.delivery.failures.len(), 1);
        assert_eq!(
            report.delivery.failures[0].cause,
            SubscriberFailure::Unavailable
        );
    }
    fn assert_calls(repository: &MemoryValuationRepository, account: &AccountId) {
        let calls = repository.script_append().calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(&calls[0].0, account);
        assert_eq!(calls[0].1, calls[1].1);
    }
    fn assert_market(market: &MemoryMarketDataRepository) {
        assert_eq!(
            market.script_fx_rate().calls(),
            vec![(FxPair::UsdKrw, FxRateKind::Mid, FxRateQuery::At(at(3, 1))); 2]
        );
    }
    #[test]
    fn records_domain_snapshot_before_one_account_event_and_preserves_delivery_failure() {
        let portfolio_store = MemoryPortfolioRepository::default();
        let market = MemoryMarketDataRepository::default();
        let repository = Arc::new(MemoryValuationRepository::default());
        let (sender, events) = channel();
        let (order, stored) = channel();
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::ValuationSnapshotTaken]),
            Observer {
                repository: Arc::clone(&repository),
                events: sender,
                order,
            },
        );
        let bus = builder.build();
        let clock = SteppedClock::new(at(3, 2));
        let account = AccountId::new(71);
        let dates = calendar(3);
        for (index, outcome) in [
            SyncOutcome::Complete(portfolio()),
            SyncOutcome::Partial(portfolio()),
        ]
        .into_iter()
        .enumerate()
        {
            portfolio_store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    at(3, 1),
                    outcome,
                )))));
            market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
            repository.script_append().push(MemoryReply::Memory);
            let report = run(TakeValuationSnapshot::new(
                &portfolio_store,
                &market,
                repository.as_ref(),
                &clock,
                &bus,
            )
            .execute(&account, &dates))
            .expect("기록");
            assert_report(&report, &dates, index);
        }
        assert_eq!(
            events.try_iter().collect::<Vec<_>>().as_slice(),
            &[
                DomainEvent::ValuationSnapshotTaken(account.clone()),
                DomainEvent::ValuationSnapshotTaken(account.clone())
            ]
        );
        assert_eq!(stored.try_iter().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(clock.calls(), vec![at(3, 2); 2]);
        assert_eq!(
            portfolio_store.script_latest().calls(),
            vec![(account.clone(),); 2]
        );
        assert_market(&market);
        assert_calls(repository.as_ref(), &account);
    }
    #[test]
    fn missing_failed_blocked_and_repository_errors_do_not_write_or_publish() {
        for (input, expected) in [
            (Ok(None), ValuationSnapshotError::MissingPortfolio),
            (Err(error()), ValuationSnapshotError::Repository(error())),
            (
                Ok(Some(SyncRecord::new(at(3, 1), SyncOutcome::Failed))),
                ValuationSnapshotError::UnavailablePortfolio,
            ),
            (
                Ok(Some(SyncRecord::new(at(3, 1), SyncOutcome::AccessBlocked))),
                ValuationSnapshotError::UnavailablePortfolio,
            ),
        ] {
            let store = MemoryPortfolioRepository::default();
            store.script_latest().push(MemoryReply::Return(input));
            let market = MemoryMarketDataRepository::default();
            let repository = MemoryValuationRepository::default();
            let clock = SteppedClock::new(at(3, 2));
            assert_eq!(
                run(TakeValuationSnapshot::new(
                    &store,
                    &market,
                    &repository,
                    &clock,
                    &application::EventBus::default()
                )
                .execute(&AccountId::new(73), &calendar(3))),
                Err(expected)
            );
            assert_eq!(repository.calls().len(), 0);
            assert_eq!(market.script_fx_rate().calls().len(), 0);
            assert_eq!(clock.calls().len(), 0);
        }
    }
    #[test]
    fn market_snapshot_and_append_errors_are_preserved() {
        for stage in 0_usize..3 {
            let store = MemoryPortfolioRepository::default();
            store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    at(3, 1),
                    SyncOutcome::Complete(portfolio()),
                )))));
            let market = MemoryMarketDataRepository::default();
            market
                .script_fx_rate()
                .push(MemoryReply::Return(if stage == 0 {
                    Err(error())
                } else {
                    Ok(None)
                }));
            let repository = Arc::new(MemoryValuationRepository::default());
            repository
                .script_append()
                .push(MemoryReply::Return(Err(error())));
            let (sender, events) = channel();
            let (order, stored) = channel();
            let mut builder = EventBusBuilder::default();
            builder.subscribe(
                BTreeSet::from([DomainEventKind::ValuationSnapshotTaken]),
                Observer {
                    repository: Arc::clone(&repository),
                    events: sender,
                    order,
                },
            );
            let bus = builder.build();
            let clock = SteppedClock::new(at(3, 2));
            let dates = calendar(if stage == 1 { 4 } else { 3 });
            let result =
                run(
                    TakeValuationSnapshot::new(&store, &market, repository.as_ref(), &clock, &bus)
                        .execute(&AccountId::new(79), &dates),
                );
            assert_eq!(
                result,
                Err(if stage == 1 {
                    ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod)
                } else {
                    ValuationSnapshotError::Repository(error())
                })
            );
            assert_eq!(repository.calls().len(), usize::from(stage == 2));
            assert_eq!(events.try_iter().count(), 0);
            assert_eq!(stored.try_iter().count(), 0);
        }
    }
    fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
    }
    fn held(observed: Timestamp) -> Portfolio {
        held_currency(observed, Currency::Usd)
    }
    fn held_currency(observed: Timestamp, currency: Currency) -> Portfolio {
        let master = domain::MasterInstrument::try_new(domain::MasterInstrumentRow {
            listing_market: domain::MasterCode::try_new(match currency {
                Currency::Krw => "KOSPI",
                Currency::Usd => "NASDAQ",
            })
            .expect("시장"),
            instrument_id: domain::InstrumentId::try_new("snapshot-fixture").expect("ID"),
            symbol: domain::Symbol::try_new(match currency {
                Currency::Krw => "005930",
                Currency::Usd => "AAPL",
            })
            .expect("심볼"),
            name: domain::DisplayInstrumentName::try_new("고정 보유").expect("이름"),
            security_type: domain::MasterSecurityType::from_code(
                domain::MasterCode::try_new("STOCK").expect("종류"),
            ),
            currency,
            listing_status: domain::ListingStatus::Active,
            common_share: domain::CommonShare::Common,
            korean_detail: match currency {
                Currency::Krw => Some(domain::KoreanMasterDetail {
                    liquidation: domain::LiquidationTrading::Regular,
                    krx: domain::TradingSuspension::Clear,
                    nxt: domain::NxtTrading::Unsupported,
                }),
                Currency::Usd => None,
            },
            observed_at: observed,
        })
        .expect("마스터");
        let holding = domain::Holding::new(
            master.order_instrument().expect("종목"),
            domain::Quantity::parse(&NumericInput::try_new("3").expect("입력")).expect("수량"),
            domain::AcquisitionPrice::try_new(money("11", currency)).expect("원가"),
            domain::HoldingObservation::new(
                Some(domain::Price::try_new(money("17", currency)).expect("가격")),
                None,
                observed,
            ),
        )
        .expect("보유");
        Portfolio::new(
            vec![holding],
            BuyingPower::new(money("137", Currency::Krw), observed),
            BuyingPower::new(money("29", Currency::Usd), observed),
        )
        .expect("포트")
    }
    fn assert_saved(
        repo: &MemoryValuationRepository,
        result: &Result<application::ValuationSnapshotReport, ValuationSnapshotError>,
        events: usize,
        stored: usize,
    ) {
        let expected = usize::from(result.is_ok());
        assert_eq!(repo.calls().len(), expected);
        assert_eq!(events, expected);
        assert_eq!(stored, expected);
    }
    fn take_saved(
        saved: Timestamp,
        now: Timestamp,
        fx: Option<domain::FxRate>,
    ) -> Result<application::ValuationSnapshotReport, ValuationSnapshotError> {
        take_record(held(saved), saved, now, fx, &calendar(3))
    }
    fn take_record(
        portfolio: Portfolio,
        saved: Timestamp,
        now: Timestamp,
        fx: Option<domain::FxRate>,
        dates: &SnapshotCalendar,
    ) -> Result<application::ValuationSnapshotReport, ValuationSnapshotError> {
        let store = MemoryPortfolioRepository::default();
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                saved,
                SyncOutcome::Complete(portfolio),
            )))));
        let market = MemoryMarketDataRepository::default();
        market.script_fx_rate().push(MemoryReply::Return(Ok(fx)));
        let repo = Arc::new(MemoryValuationRepository::default());
        let (sender, events) = channel();
        let (order, stored) = channel();
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::ValuationSnapshotTaken]),
            Observer {
                repository: Arc::clone(&repo),
                events: sender,
                order,
            },
        );
        let bus = builder.build();
        repo.script_append().push(MemoryReply::Memory);
        let clock = SteppedClock::new(now);
        let result = run(
            TakeValuationSnapshot::new(&store, &market, repo.as_ref(), &clock, &bus)
                .execute(&AccountId::new(83), dates),
        );
        assert_saved(
            repo.as_ref(),
            &result,
            events.try_iter().count(),
            stored.try_iter().count(),
        );
        result
    }
    #[test]
    fn stale_and_future_records_cannot_be_relabelled_as_current_observations() {
        for saved in [at(1, 0), at(3, 3)] {
            assert_eq!(
                take_saved(saved, at(3, 2), None),
                Err(ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod))
            );
        }
    }
    #[test]
    fn preclose_record_is_definition_pending_without_append() {
        assert_eq!(
            take_saved(at(3, 0), at(3, 2), None),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
    }
    #[test]
    fn foreign_stock_uses_the_observation_mid_rate_and_keeps_conversion_evidence() {
        let fx = domain::FxRate::parse(
            &NumericInput::try_new("1300").expect("입력"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(at(3, 0), at(3, 3)).expect("구간"),
            domain::FxSource::TossOpenApi,
        )
        .expect("환율");
        let snapshot = take_saved(at(3, 1), at(3, 4), Some(fx))
            .expect("기록")
            .snapshot;
        assert_eq!(snapshot.fx(), Some(fx));
        assert_eq!(
            snapshot.stock_value(Currency::Usd),
            Ok(money("51", Currency::Usd))
        );
        assert_eq!(
            snapshot
                .stock_krw_value()
                .expect("환산")
                .clone()
                .into_parts(),
            (
                money("66300", Currency::Krw),
                vec![fx],
                domain::ValuationAccuracy::Approximate
            )
        );
    }
    #[test]
    fn observation_at_clock_boundary_is_allowed() {
        let report = take_saved(at(3, 1), at(3, 1), None).expect("경계 기록");
        assert_eq!(report.snapshot.valuation_at(), at(3, 1));
        assert_eq!(report.recorded, RecordOutcome::Recorded);
    }
    #[test]
    fn old_holding_price_cannot_be_relabelled_by_a_newer_sync_record() {
        assert_eq!(
            take_record(held(at(3, 0)), at(3, 1), at(3, 2), None, &calendar(3)),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
        assert_eq!(
            take_record(held(at(2, 2)), at(3, 1), at(3, 2), None, &calendar(3)),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
        assert_eq!(
            take_record(held(at(3, 2)), at(3, 1), at(3, 2), None, &calendar(3)),
            Err(ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod))
        );
    }
    #[test]
    fn either_market_awaiting_close_is_pending_without_storage_or_event() {
        for (portfolio, dates) in [
            (
                held_currency(at(3, 2), Currency::Krw),
                calendar_closes(3, 1),
            ),
            (held(at(3, 2)), calendar_closes(1, 3)),
        ] {
            assert_eq!(
                take_record(portfolio, at(3, 2), at(3, 4), None, &dates),
                Err(ValuationSnapshotError::PriceBasisDefinitionPending)
            );
        }
    }
    #[derive(Clone, Copy)]
    enum SessionState {
        Open,
        Closed,
    }
    fn real_at(day: i64, minute: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                1_794_787_200_i64
                    .checked_add(day.checked_mul(86400).expect("일"))
                    .expect("날짜")
                    .checked_add(minute.checked_mul(60).expect("분"))
                    .expect("시각"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn real_session(day: i64, market: &Market, state: SessionState) -> MarketSession {
        let (start, end) = match market {
            Market::Kr => (0, 390),
            Market::Us | Market::Unknown(_) => (870, 1260),
        };
        let data = match state {
            SessionState::Closed => None,
            SessionState::Open => Some(
                SessionData::try_new(
                    TimeInterval::try_new(real_at(day, start), real_at(day, end)).expect("구간"),
                    None,
                )
                .expect("장"),
            ),
        };
        let info = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
        };
        MarketSession::try_new(real_at(day, 0).offset_calendar_date(), info).expect("세션")
    }
    fn real_calendar(
        day: i64,
        kr: SessionState,
        us: SessionState,
    ) -> Result<SnapshotCalendar, SnapshotError> {
        let make = |market: &Market, state| {
            MarketCalendar::try_new(
                real_session(
                    day.checked_sub(1).expect("전일"),
                    market,
                    SessionState::Open,
                ),
                real_session(day, market, state),
                real_session(
                    day.checked_add(1).expect("익일"),
                    market,
                    SessionState::Open,
                ),
            )
            .expect("달력")
        };
        SnapshotCalendar::new(&make(&Market::Kr, kr), &make(&Market::Us, us))
    }
    #[test]
    fn real_16kst_empty_account_stores_without_us_price_evidence() {
        let at = real_at(2, 420);
        let dates = real_calendar(2, SessionState::Open, SessionState::Open).expect("달력");
        let snapshot = take_record(portfolio(), at, at, None, &dates)
            .expect("빈 16시 기록")
            .snapshot;
        assert_eq!(
            snapshot.price_basis(Currency::Krw),
            domain::SnapshotPriceBasis::CurrentSession
        );
        assert_eq!(
            snapshot.price_basis(Currency::Usd),
            domain::SnapshotPriceBasis::AwaitingSession
        );
        assert_eq!(
            snapshot.stock_value(Currency::Krw),
            Ok(money("0", Currency::Krw))
        );
        assert_eq!(
            snapshot.stock_value(Currency::Usd),
            Ok(money("0", Currency::Usd))
        );
    }
    #[test]
    fn real_16kst_domestic_account_stores_without_us_price_evidence() {
        let at = real_at(2, 420);
        let dates = real_calendar(2, SessionState::Open, SessionState::Open).expect("달력");
        let snapshot = take_record(held_currency(at, Currency::Krw), at, at, None, &dates)
            .expect("국내 16시 기록")
            .snapshot;
        assert_eq!(
            snapshot.price_basis(Currency::Usd),
            domain::SnapshotPriceBasis::AwaitingSession
        );
        assert_eq!(
            snapshot.stock_value(Currency::Krw),
            Ok(money("51", Currency::Krw))
        );
        assert_eq!(
            snapshot.stock_value(Currency::Usd),
            Ok(money("0", Currency::Usd))
        );
    }
    #[test]
    fn real_holidays_preserve_safe_empty_snapshots_and_closed_day_contract() {
        let at = real_at(10, 420);
        for (kr, us) in [
            (SessionState::Open, SessionState::Closed),
            (SessionState::Closed, SessionState::Open),
        ] {
            let dates = real_calendar(10, kr, us).expect("추수감사절 또는 한국 휴장");
            let snapshot = take_record(portfolio(), at, at, None, &dates)
                .expect("빈 기록")
                .snapshot;
            assert_eq!(
                snapshot.stock_value(Currency::Krw),
                Ok(money("0", Currency::Krw))
            );
            assert_eq!(
                snapshot.stock_value(Currency::Usd),
                Ok(money("0", Currency::Usd))
            );
        }
        assert_eq!(
            real_calendar(10, SessionState::Closed, SessionState::Closed),
            Err(SnapshotError::ClosedDay)
        );
    }
    #[test]
    fn real_intraday_and_nonzero_carry_prices_still_require_evidence() {
        let day = 2;
        let dates = real_calendar(day, SessionState::Open, SessionState::Open).expect("달력");
        for (at, currency) in [
            (real_at(day, 300), Currency::Krw),
            (real_at(day, 420), Currency::Usd),
        ] {
            assert_eq!(
                take_record(held_currency(at, currency), at, at, None, &dates),
                Err(ValuationSnapshotError::PriceBasisDefinitionPending)
            );
        }
        let at = real_at(10, 420);
        let holiday = real_calendar(10, SessionState::Open, SessionState::Closed).expect("휴장");
        assert_eq!(
            take_record(held(at), at, at, None, &holiday),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
    }
    fn reported(usd: Option<Money>, at: Timestamp) -> domain::ReportedPortfolio {
        let valuation =
            domain::CurrencyAmounts::new(money("51", Currency::Krw), usd).expect("평가");
        let purchase =
            domain::CurrencyAmounts::new(money("33", Currency::Krw), None).expect("원금");
        let profit = domain::CurrencyAmounts::new(money("18", Currency::Krw), None).expect("손익");
        domain::ReportedPortfolio::new(valuation, purchase, profit, at)
    }
    #[test]
    fn real_16kst_reported_domestic_valuation_with_absent_usd_is_supported() {
        let at = real_at(2, 420);
        let dates = real_calendar(2, SessionState::Open, SessionState::Open).expect("달력");
        let portfolio = held_currency(at, Currency::Krw).with_reported(reported(None, at));
        let snapshot = take_record(portfolio, at, at, None, &dates)
            .expect("보고 기록")
            .snapshot;
        assert_eq!(
            snapshot.stock_value(Currency::Krw),
            Ok(money("51", Currency::Krw))
        );
        assert_eq!(
            snapshot.stock_value(Currency::Usd),
            Ok(money("0", Currency::Usd))
        );
    }
    #[test]
    fn reported_usd_without_us_holdings_still_requires_close_evidence() {
        let at = real_at(2, 420);
        let dates = real_calendar(2, SessionState::Open, SessionState::Open).expect("달력");
        let portfolio = portfolio().with_reported(reported(Some(money("53", Currency::Usd)), at));
        assert_eq!(
            take_record(portfolio, at, at, None, &dates),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
    }
    #[test]
    fn reported_valuation_time_cannot_be_relabelled_by_a_newer_sync() {
        let at = real_at(2, 420);
        let dates = real_calendar(2, SessionState::Open, SessionState::Open).expect("달력");
        let old = held_currency(at, Currency::Krw).with_reported(reported(None, real_at(2, 300)));
        let future =
            held_currency(at, Currency::Krw).with_reported(reported(None, real_at(2, 480)));
        assert_eq!(
            take_record(old, at, at, None, &dates),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
        assert_eq!(
            take_record(future, at, at, None, &dates),
            Err(ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod))
        );
    }
    fn real_after_market_calendar() -> SnapshotCalendar {
        let day = 2;
        let regular = SessionData::try_new(
            TimeInterval::try_new(real_at(day, 0), real_at(day, 390)).expect("정규장"),
            None,
        )
        .expect("정규장");
        let after = SessionData::try_new(
            TimeInterval::try_new(real_at(day, 390), real_at(day, 660)).expect("연장 거래"),
            None,
        )
        .expect("연장 거래");
        let today = MarketSession::try_new(
            real_at(day, 0).offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(regular),
                after_market: Some(after),
            },
        )
        .expect("한국 장");
        let kr = MarketCalendar::try_new(
            real_session(1, &Market::Kr, SessionState::Open),
            today,
            real_session(3, &Market::Kr, SessionState::Open),
        )
        .expect("한국 달력");
        let us = MarketCalendar::try_new(
            real_session(1, &Market::Us, SessionState::Open),
            real_session(2, &Market::Us, SessionState::Open),
            real_session(3, &Market::Us, SessionState::Open),
        )
        .expect("미국 달력");
        SnapshotCalendar::new(&kr, &us).expect("평가 달력")
    }

    #[test]
    fn extended_market_observed_prices_do_not_store_or_publish_without_close_evidence() {
        let dates = real_after_market_calendar();
        for minute in [420, 600, 660, 720] {
            let at = real_at(2, minute);
            assert_eq!(
                take_record(held_currency(at, Currency::Krw), at, at, None, &dates),
                Err(ValuationSnapshotError::Snapshot(
                    SnapshotError::MissingPrice
                )),
            );
            let empty = take_record(portfolio(), at, at, None, &dates).expect("빈 계좌");
            assert_eq!(
                empty.snapshot.stock_value(Currency::Krw),
                Ok(money("0", Currency::Krw))
            );
        }
    }
}
