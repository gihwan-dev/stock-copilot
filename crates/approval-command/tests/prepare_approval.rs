#[cfg(test)]
mod tests {
    use application::{
        AuditKind, ConditionalDraftOutcome, CurrentDraftOutcome, DraftCheck, DraftStoreCandidate,
        EventBus, EventBusBuilder, EventSubscriber, OrderConfirmation, OrderGatePolicyContext,
        OrderRateRepository, ProposalConsumptionRepository, ProposalRepository, QuoteObservation,
        SettingsRepository, SignalSnapshot, SubscriberFailure, SystemFailure, SystemPort,
        SystemPortError, WriteCompleted,
    };
    use approval_command::guard::{
        ApprovalGuard, CooldownError, CooldownSeconds, DialogLease, DialogStatus,
        GuardAcquisitionError, RequestCount,
    };
    use approval_command::ports::{
        ExpiredProposalRedrafter, ProposalContextPort, RedraftOutcome, SymbolFreezePort,
        SymbolFreezeStatus,
    };
    use approval_command::prepare::{
        ApprovalContextPorts, ApprovalPreparer, ApprovalReadPorts, ApprovalStoragePorts,
        PrepareError, PrepareOutcome, PrepareRejection, PreparedApproval, record_expired_proposal,
    };
    use domain::{
        AccountId, AdjustmentBasis, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
        BrokerAccountTypeCode, BrokerOrderSet, BuyingPower, CalendarDate, CalendarYear, Candle,
        CandleObservation, CandlePeriod, CandleStatus, CandleTime, ClientOrderId,
        CommissionRateFact, CommissionValidity, CommonShare, CostRate, Currency, DayNumber,
        DisplayInstrumentName, DomainEvent, DomainEventKind, Draft, DraftRestoreEvidence,
        EstimateContext, EstimateTask, Horizon, HourlyOrderLimit, HoursPermission,
        HoursPermissionState, InstrumentId, KoreanMasterDetail, KrOrderPrice, LiquidationTrading,
        ListingStatus, Market, MarketCalendar, MarketSession, MasterCode, MasterInstrument,
        MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, Money, MonthNumber,
        Nanosecond, NumericInput, NxtTrading, OffsetSeconds, Ohlcv, OpenOrderCoverage, OrderCount,
        OrderIntent, OrderPermission, OrderSide, OrderTerms, Portfolio, Price, PriceAdjustment,
        PriceLimits, PriceLimitsInput, Proposal, ProposalId, ProposalLifetime, ProposalPriceRange,
        Quantity, Quote, RiskAppetite, RiskSettings, SavedMarketOrderSetting, SectorAssignments,
        SectorCatalog, SessionData, SessionInformation, SpanSeconds, StockWarnings, Symbol,
        TimeInterval, Timestamp, TradingSuspension, UnixSeconds, Volatility, WeightCaps,
        WholeOrderQuantity, default_market_order_policy, sector_assignment_from_compositions,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};
    use testkit_ports::{
        PresenceCall, RecordedOrderSummary, ScriptedPresence, SteppedClock,
        contracts::user_presence_order,
        memory::{MemoryReply, MemorySettingsRepository},
        proposal_memory::{MemoryProposalConsumptionRepository, MemoryValidationReportRepository},
        scripted::{ScriptedBrokerRead, ScriptedMarketInfo},
        system_fakes::MemoryAuditLog,
    };

    const DAY_BASE: i64 = 1_775_055_600;

    fn run<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut pinned = Box::pin(future);
        loop {
            match Pin::as_mut(&mut pinned).poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn at_kst(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각")
    }

    fn kst(hour: i64, minute: i64) -> Timestamp {
        at_kst(
            DAY_BASE
                .saturating_add(hour.saturating_mul(3600))
                .saturating_add(minute.saturating_mul(60)),
        )
    }

    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(4),
            DayNumber::new(day),
        )
        .expect("날짜")
    }

    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("숫자"), currency).expect("금액")
    }

    fn price(value: &str, currency: Currency) -> Price {
        Price::try_new(money(value, currency)).expect("가격")
    }

    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("숫자")).expect("수량")
    }

    fn kr_master(observed_at: Timestamp) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            symbol: Symbol::try_new("069500").expect("심볼"),
            instrument_id: InstrumentId::try_new("kr-069500").expect("ID"),
            name: DisplayInstrumentName::try_new("테스트 ETF").expect("이름"),
            security_type: MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("유형")),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at,
        })
        .expect("마스터")
    }

    fn build_volatility(master: &MasterInstrument, now: Timestamp) -> Volatility {
        let adjustment = PriceAdjustment::Adjusted(AdjustmentBasis::new(now));
        let volume = quantity("1000");
        let candles: Vec<Candle> = (0_i64..=60_i64)
            .map(|index| {
                let offset_days = 60_i64.saturating_sub(index);
                let stamp = at_kst(DAY_BASE.saturating_sub(offset_days.saturating_mul(86_400)));
                let close = if index.rem_euclid(2) == 0 {
                    price("34500", Currency::Krw)
                } else {
                    price("35500", Currency::Krw)
                };
                let values = Ohlcv::try_new(close, close, close, close, volume).expect("봉");
                Candle::try_new(
                    master.row().instrument_id.clone(),
                    CandleTime::try_new(stamp, CandlePeriod::Daily).expect("일봉 시각"),
                    adjustment,
                    values,
                    CandleObservation::new(now, CandleStatus::Finalized),
                )
                .expect("일봉")
            })
            .collect();
        Volatility::calculate(&candles).expect("변동성")
    }

    fn make_kr_draft(
        id_str: &str,
        master: &MasterInstrument,
        created_at: Timestamp,
        limit_price: &str,
    ) -> Proposal<Draft> {
        let client_id = ClientOrderId::try_new(id_str).expect("주문 키");
        let proposal_id = ProposalId::new(client_id.clone());
        let resolved = master.order_instrument().expect("종목");
        let obs = master
            .price_observation(price(limit_price, Currency::Krw), created_at)
            .expect("시세 관측");
        let task = EstimateTask::create(
            EstimateContext::new(obs.clone(), created_at),
            build_volatility(master, created_at),
            Horizon::default(),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("추정 과제");
        let range = ProposalPriceRange::from_estimate_task(task.range_facts(), obs).expect("범위");
        let intent = OrderIntent::new(
            AccountId::new(42),
            resolved,
            client_id,
            OrderTerms::KrLimit {
                side: OrderSide::Buy,
                quantity: WholeOrderQuantity::try_new(quantity("2")).expect("정수 수량"),
                price: KrOrderPrice::try_new(price(limit_price, Currency::Krw)).expect("지정가"),
            },
        )
        .expect("주문 의도");
        Proposal::new(
            proposal_id,
            intent,
            created_at,
            range,
            ProposalLifetime::default(),
        )
        .expect("초안")
    }

    fn make_context(master: &MasterInstrument, now: Timestamp) -> OrderGatePolicyContext {
        let portfolio = Portfolio::new(
            Vec::new(),
            BuyingPower::new(money("5000000", Currency::Krw), now),
            BuyingPower::new(money("0", Currency::Usd), now),
        )
        .expect("포트폴리오")
        .evaluate(None);
        let catalog = SectorCatalog::from_response(Vec::new(), now).expect("섹터");
        let assignment = sector_assignment_from_compositions(master, &catalog, &[]).expect("배정");
        let assignments = SectorAssignments::try_new(vec![assignment]).expect("배정 목록");
        let security_facts = MasterInstrumentFacts::try_new(vec![master.clone()])
            .expect("마스터 집합")
            .policy_security_facts();
        OrderGatePolicyContext {
            master: master.clone(),
            market_order_policy: default_market_order_policy(domain::Policy::new(
                domain::InvestorProfile::default(),
            )),
            valuation: portfolio,
            assignments,
            security_facts,
        }
    }

    fn kr_calendar() -> MarketCalendar {
        let make_day = |day: u8, start: i64, end: i64| {
            MarketSession::try_new(
                date(day),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: Some(
                        SessionData::try_new(
                            TimeInterval::try_new(at_kst(start), at_kst(end)).expect("구간"),
                            None,
                        )
                        .expect("세션"),
                    ),
                    after_market: None,
                },
            )
            .expect("일별")
        };
        MarketCalendar::try_new(
            make_day(1, 1_775_001_600, 1_775_025_000),
            make_day(2, 1_775_088_000, 1_775_111_400),
            make_day(3, 1_775_174_400, 1_775_197_800),
        )
        .expect("달력")
    }

    fn kr_accounts(now: Timestamp) -> BrokerAccountRows {
        BrokerAccountRows::try_new(vec![
            BrokerAccountRow::from_broker_account_result(
                AccountId::new(42),
                BrokerAccountTypeCode::new("BROKERAGE"),
                BrokerAccountNumberInput::new("111122223333".to_owned()),
                now,
            )
            .expect("계좌 행"),
        ])
        .expect("계좌 목록")
    }

    fn kr_quote(master: &MasterInstrument, price_str: &str, now: Timestamp) -> QuoteObservation {
        let quote_instrument = master.quote_instrument().expect("시세 종목");
        let quote = Quote::try_new(
            master.row().symbol.clone(),
            Currency::Krw,
            Some(price(price_str, Currency::Krw)),
            Some(now),
            now,
        )
        .expect("시세");
        QuoteObservation {
            instrument: quote_instrument,
            result: Ok(quote),
        }
    }

    fn kr_price_limits(master: &MasterInstrument, now: Timestamp) -> PriceLimits {
        PriceLimits::try_new(PriceLimitsInput {
            instrument: master.quote_instrument().expect("시세 종목"),
            currency: Currency::Krw,
            lower: Some(price("25000", Currency::Krw)),
            upper: Some(price("45000", Currency::Krw)),
            timestamp: now,
            collected_at: now,
        })
        .expect("가격 제한")
    }

    fn script_valid_market_and_broker(
        broker: &ScriptedBrokerRead,
        market: &ScriptedMarketInfo,
        master: &MasterInstrument,
        now: Timestamp,
        buying_power_krw: &str,
    ) {
        let quote = kr_quote(master, "35000", now);
        broker.script_quotes().push(Ok(vec![quote.clone()]));
        broker.script_quotes().push(Ok(vec![quote]));
        broker.script_accounts().push(Ok(kr_accounts(now)));
        broker
            .script_price_limits()
            .push(Ok(kr_price_limits(master, now)));
        let open = BrokerOrderSet::from_open_lookup_result(
            AccountId::new(42),
            Vec::new(),
            OpenOrderCoverage::Complete,
            now,
        )
        .expect("미체결");
        broker.script_open_orders().push(Ok(open));
        broker
            .script_commission_rate()
            .push(Ok(CommissionRateFact::from_commissions_result(
                Market::Kr,
                CostRate::try_new(
                    domain::Ratio::parse(&NumericInput::try_new("0.00015").expect("수치"))
                        .expect("비율"),
                )
                .expect("요율"),
                CommissionValidity::try_new(None, None).expect("기간"),
            )));
        broker.script_buying_power().push(Ok(BuyingPower::new(
            money(buying_power_krw, Currency::Krw),
            now,
        )));
        let resolved = master.order_instrument().expect("종목");
        broker
            .script_stock_warnings()
            .push(Ok(StockWarnings::from_lookup_result(
                resolved,
                Vec::new(),
                now,
            )));
        market.script_calendar().push(Ok(kr_calendar()));
    }

    struct StubRateRepo {
        limit: Option<HourlyOrderLimit>,
        times: Vec<Timestamp>,
    }

    impl OrderRateRepository for StubRateRepo {
        fn hourly_limit(
            &self,
        ) -> impl Future<Output = Result<Option<HourlyOrderLimit>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.limit))
        }

        fn recent_submission_times(
            &self,
            _since: Timestamp,
        ) -> impl Future<Output = Result<Vec<Timestamp>, SystemPortError>> + Send {
            std::future::ready(Ok(self.times.clone()))
        }
    }

    struct StubProposalRepo {
        drafts: RwLock<BTreeMap<ProposalId, Proposal<Draft>>>,
    }

    impl StubProposalRepo {
        fn with_draft(proposal: Proposal<Draft>) -> Self {
            let mut map = BTreeMap::new();
            map.insert(proposal.id().clone(), proposal);
            Self {
                drafts: RwLock::new(map),
            }
        }
    }

    impl ProposalRepository for StubProposalRepo {
        fn store_draft(
            &self,
            proposal: &Proposal<Draft>,
            _evidence: &DraftRestoreEvidence,
            _master: &MasterInstrument,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            let mut lock = match self.drafts.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            lock.insert(proposal.id().clone(), proposal.clone());
            std::future::ready(Ok(WriteCompleted))
        }

        fn draft(
            &self,
            proposal: &ProposalId,
            _master: &MasterInstrument,
        ) -> impl Future<Output = Result<Option<Proposal<Draft>>, SystemPortError>> + Send {
            let lock = match self.drafts.read() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            std::future::ready(Ok(lock.get(proposal).cloned()))
        }

        fn current_draft(
            &self,
            _instrument: &InstrumentId,
            _expected: &SignalSnapshot,
            _check: DraftCheck<'_>,
        ) -> impl Future<Output = Result<CurrentDraftOutcome, SystemPortError>> + Send {
            std::future::ready(Ok(CurrentDraftOutcome::Absent))
        }

        fn store_current_draft(
            &self,
            _instrument: &InstrumentId,
            _expected: &SignalSnapshot,
            _candidate: DraftStoreCandidate<'_>,
            _check: DraftCheck<'_>,
        ) -> impl Future<Output = Result<ConditionalDraftOutcome, SystemPortError>> + Send {
            std::future::ready(Ok(ConditionalDraftOutcome::Stored))
        }
    }

    struct StubContextPort {
        contexts: BTreeMap<ProposalId, OrderGatePolicyContext>,
    }

    impl ProposalContextPort for StubContextPort {
        fn policy_context(
            &self,
            proposal_id: &ProposalId,
        ) -> impl Future<Output = Result<Option<OrderGatePolicyContext>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.contexts.get(proposal_id).cloned()))
        }
    }

    struct StubRedrafter {
        reply: RwLock<Option<RedraftOutcome>>,
    }

    impl StubRedrafter {
        fn returning(outcome: RedraftOutcome) -> Self {
            Self {
                reply: RwLock::new(Some(outcome)),
            }
        }
    }

    impl ExpiredProposalRedrafter for StubRedrafter {
        fn redraft(
            &self,
            _expired: &Proposal<Draft>,
            _now: Timestamp,
        ) -> impl Future<Output = Result<RedraftOutcome, SystemPortError>> + Send {
            let mut lock = match self.reply.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let outcome = lock.take().unwrap_or(RedraftOutcome::NoSignal);
            std::future::ready(Ok(outcome))
        }
    }

    struct StubFreeze(SymbolFreezeStatus);

    impl SymbolFreezePort for StubFreeze {
        fn freeze_status(
            &self,
            _account: &AccountId,
            _instrument: &domain::ResolvedOrderInstrument,
        ) -> impl Future<Output = Result<SymbolFreezeStatus, SystemPortError>> + Send {
            std::future::ready(Ok(self.0))
        }
    }

    struct EventCollector {
        events: Arc<RwLock<Vec<DomainEvent>>>,
    }

    impl EventSubscriber for EventCollector {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            let mut lock = match self.events.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            lock.push(event.clone());
            Ok(())
        }
    }

    fn settings_with_orders(permission: OrderPermission) -> MemorySettingsRepository {
        let settings = MemorySettingsRepository::default();
        settings
            .script_store_risk_settings()
            .push(MemoryReply::Memory);
        for _ in 0_u8..4 {
            settings.script_risk_settings().push(MemoryReply::Memory);
        }
        let risk = RiskSettings::new(
            permission,
            SavedMarketOrderSetting::Disabled,
            HoursPermissionState::Disabled,
            WeightCaps::default(),
            RiskAppetite::Neutral,
        );
        run(settings.store_risk_settings(&risk)).expect("설정 저장");
        settings
    }

    fn make_audit_memory(count: u8) -> MemoryAuditLog {
        let audit = MemoryAuditLog::default();
        for _ in 0_u8..count {
            audit.script_append().push(MemoryReply::Memory);
        }
        audit
    }

    fn make_reports_memory(count: u8) -> MemoryValidationReportRepository {
        let reports = MemoryValidationReportRepository::default();
        for _ in 0_u8..count {
            reports.script_append().push(MemoryReply::Memory);
        }
        reports
    }

    fn make_consumption_memory(count: u8) -> MemoryProposalConsumptionRepository {
        let consumption = MemoryProposalConsumptionRepository::default();
        for _ in 0_u8..count {
            consumption.script_status().push(MemoryReply::Memory);
            consumption.script_consume().push(MemoryReply::Memory);
        }
        consumption
    }

    fn make_bus_with_collector() -> (EventBus, Arc<RwLock<Vec<DomainEvent>>>) {
        let collected = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::ProposalExpired]),
            EventCollector {
                events: Arc::clone(&collected),
            },
        );
        (builder.build(), collected)
    }

    fn read_events(events: &Arc<RwLock<Vec<DomainEvent>>>) -> Vec<DomainEvent> {
        match events.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn expect_rejected(outcome: PrepareOutcome<'_>) -> PrepareRejection {
        match outcome {
            PrepareOutcome::Ready(_) => panic!("거부되어야 한다"),
            PrepareOutcome::Rejected(rejection) => rejection,
        }
    }

    fn release_lease(_lease: DialogLease<'_>) {}

    fn consume_ready(_ready: PreparedApproval<'_>) {}

    #[test]
    fn guard_enforces_single_dialog_and_cooldown_bounds() {
        assert_eq!(
            CooldownSeconds::try_new(SpanSeconds::new(0)),
            Err(CooldownError::OutOfRange),
            "0초 쿨다운 불가"
        );
        assert_eq!(
            CooldownSeconds::try_new(SpanSeconds::new(3601)),
            Err(CooldownError::OutOfRange),
            "3601초 쿨다운 불가"
        );
        let guard = ApprovalGuard::default();
        let pid = ProposalId::new(ClientOrderId::try_new("prop-guard-1").expect("키"));
        let t0 = kst(10, 0);
        let lease = guard.try_acquire(&pid, t0).expect("첫 창 획득");
        assert_eq!(guard.dialog_status(), DialogStatus::Active, "창 활성");
        assert_eq!(
            guard.try_acquire(&pid, t0).err(),
            Some(GuardAcquisitionError::DialogInProgress),
            "동시 두 번째 요청 거부"
        );
        release_lease(lease);
        assert_eq!(guard.dialog_status(), DialogStatus::Idle, "해제 후 유휴");
        guard.record_rejection(pid.clone(), t0);
        assert_eq!(
            guard
                .try_acquire(&pid, at_kst(t0.utc_seconds().as_i64().saturating_add(59)))
                .err(),
            Some(GuardAcquisitionError::RejectionCooldown {
                retry_at: at_kst(t0.utc_seconds().as_i64().saturating_add(60)),
            }),
            "거절 후 60초 미만 재요청 거부"
        );
        let lease2 = guard
            .try_acquire(&pid, at_kst(t0.utc_seconds().as_i64().saturating_add(60)))
            .expect("60초 도달 후 허용");
        release_lease(lease2);
        assert_eq!(
            guard.request_count(&pid),
            RequestCount::new(4),
            "요청 횟수 누적"
        );
        assert_eq!(
            guard.total_requests(),
            RequestCount::new(4),
            "전체 요청 횟수"
        );
    }

    #[test]
    fn expiry_notification_emits_domain_event_exactly_once() {
        let guard = ApprovalGuard::default();
        let pid = ProposalId::new(ClientOrderId::try_new("prop-exp-once").expect("키"));
        let (bus, events) = make_bus_with_collector();
        assert!(
            record_expired_proposal(&guard, &bus, &pid).is_some(),
            "첫 만료 이벤트 발행"
        );
        assert!(
            record_expired_proposal(&guard, &bus, &pid).is_none(),
            "중복 만료 이벤트 억제"
        );
        assert_eq!(
            read_events(&events),
            vec![DomainEvent::ProposalExpired(pid)],
            "ProposalExpired는 정확히 1회"
        );
    }

    struct PrepareHarness {
        guard: ApprovalGuard,
        broker: ScriptedBrokerRead,
        market: ScriptedMarketInfo,
        settings: MemorySettingsRepository,
        rates: StubRateRepo,
        proposals: StubProposalRepo,
        consumption: MemoryProposalConsumptionRepository,
        reports: MemoryValidationReportRepository,
        audit: MemoryAuditLog,
        contexts: StubContextPort,
        redrafter: StubRedrafter,
        freeze: StubFreeze,
        clock: SteppedClock,
        bus: EventBus,
        events: Arc<RwLock<Vec<DomainEvent>>>,
    }

    impl PrepareHarness {
        fn new(
            draft: Proposal<Draft>,
            context: OrderGatePolicyContext,
            now: Timestamp,
            permission: OrderPermission,
        ) -> Self {
            let pid = draft.id().clone();
            let (bus, events) = make_bus_with_collector();
            Self {
                guard: ApprovalGuard::default(),
                broker: ScriptedBrokerRead::default(),
                market: ScriptedMarketInfo::default(),
                settings: settings_with_orders(permission),
                rates: StubRateRepo {
                    limit: None,
                    times: Vec::new(),
                },
                proposals: StubProposalRepo::with_draft(draft),
                consumption: make_consumption_memory(4),
                reports: make_reports_memory(2),
                audit: make_audit_memory(4),
                contexts: StubContextPort {
                    contexts: BTreeMap::from([(pid, context)]),
                },
                redrafter: StubRedrafter::returning(RedraftOutcome::NoSignal),
                freeze: StubFreeze(SymbolFreezeStatus::Unfrozen),
                clock: SteppedClock::new(now),
                bus,
                events,
            }
        }

        fn run_prepare(
            &self,
            proposal_id: &ProposalId,
        ) -> Result<PrepareOutcome<'_>, PrepareError> {
            let preparer = ApprovalPreparer::new(
                &self.guard,
                ApprovalReadPorts {
                    broker: &self.broker,
                    market: &self.market,
                    settings: &self.settings,
                    rates: &self.rates,
                },
                ApprovalStoragePorts {
                    proposals: &self.proposals,
                    consumption: &self.consumption,
                    reports: &self.reports,
                    audit: &self.audit,
                },
                ApprovalContextPorts {
                    contexts: &self.contexts,
                    redrafter: &self.redrafter,
                    freeze: &self.freeze,
                    clock: &self.clock,
                    events: &self.bus,
                },
            );
            run(preparer.prepare(proposal_id))
        }
    }

    #[test]
    fn valid_draft_prepares_summary_and_passes_a04_user_presence_contract() {
        let now = kst(10, 0);
        let master = kr_master(now);
        let draft = make_kr_draft("ord-o04-happy-01", &master, now, "35000");
        let pid = draft.id().clone();
        let harness = PrepareHarness::new(
            draft,
            make_context(&master, now),
            now,
            OrderPermission::Enabled,
        );
        script_valid_market_and_broker(&harness.broker, &harness.market, &master, now, "5000000");
        let ready = match harness.run_prepare(&pid).expect("준비 성공") {
            PrepareOutcome::Ready(ready) => *ready,
            PrepareOutcome::Rejected(other) => panic!("예상 밖 거부: {other:?}"),
        };
        assert_eq!(
            harness.guard.dialog_status(),
            DialogStatus::Active,
            "창 잠금 유지"
        );
        verify_a04_handoff(&ready);
        assert_eq!(
            harness
                .audit
                .entries()
                .iter()
                .filter(|entry| entry.kind() == &AuditKind::ApprovalRequested(pid.clone()))
                .count(),
            1,
            "승인 요청 감사 기록 1건"
        );
        let (_confirmation, lease) = ready.into_confirmation();
        release_lease(lease);
        assert_eq!(
            harness.guard.dialog_status(),
            DialogStatus::Idle,
            "종료 시 잠금 해제"
        );
    }

    fn verify_a04_handoff(ready: &PreparedApproval<'_>) {
        let summary = ready.summary();
        let expected_recorded = RecordedOrderSummary::from_summary(summary);
        assert_eq!(
            expected_recorded.instrument_name.as_str(),
            "테스트 ETF",
            "종목명"
        );
        assert_eq!(expected_recorded.symbol.as_str(), "069500", "종목 코드");
        assert_eq!(expected_recorded.market, Market::Kr, "시장");
        assert_eq!(expected_recorded.side, OrderSide::Buy, "방향");
        assert_eq!(
            expected_recorded.account_suffix.to_string(),
            "3333",
            "계좌 뒤 4자리"
        );
        assert_eq!(
            expected_recorded.client_order_prefix.as_str(),
            "ord-o04-",
            "주문키 앞 8자"
        );
        let presence = ScriptedPresence::new(
            [
                OrderConfirmation::Confirmed,
                OrderConfirmation::Denied,
                OrderConfirmation::Unavailable,
            ],
            [],
        );
        run(user_presence_order(
            &presence,
            summary,
            OrderConfirmation::Confirmed,
        ));
        run(user_presence_order(
            &presence,
            summary,
            OrderConfirmation::Denied,
        ));
        run(user_presence_order(
            &presence,
            summary,
            OrderConfirmation::Unavailable,
        ));
        assert_eq!(presence.order_calls().len(), 3, "주문 확인 3회");
        assert_eq!(
            presence.calls(),
            vec![
                PresenceCall::Order(Box::new(expected_recorded.clone())),
                PresenceCall::Order(Box::new(expected_recorded.clone())),
                PresenceCall::Order(Box::new(expected_recorded)),
            ],
            "FIFO 순서와 열 필드 일치"
        );
    }

    #[test]
    fn rejects_switch_off_and_rate_cap() {
        let now = kst(10, 0);
        let master = kr_master(now);
        let draft = make_kr_draft("ord-o04-reject-1", &master, now, "35000");
        let pid = draft.id().clone();
        let context = make_context(&master, now);
        let disabled = PrepareHarness::new(
            draft.clone(),
            context.clone(),
            now,
            OrderPermission::Disabled,
        );
        assert_eq!(
            expect_rejected(disabled.run_prepare(&pid).expect("스위치 거부")),
            PrepareRejection::OrdersDisabled,
            "주문 기능 스위치 꺼짐 시 거부"
        );
        let mut capped = PrepareHarness::new(draft, context, now, OrderPermission::Enabled);
        capped.rates.times = (0_i64..10_i64)
            .map(|offset| {
                at_kst(
                    now.utc_seconds()
                        .as_i64()
                        .saturating_sub(100_i64.saturating_add(offset)),
                )
            })
            .collect();
        match expect_rejected(capped.run_prepare(&pid).expect("상한 거부")) {
            PrepareRejection::HourlyLimitExceeded { recent_count, .. } => {
                assert_eq!(recent_count, OrderCount::new(10), "시간당 상한 초과");
            }
            other @ (PrepareRejection::DialogInProgress
            | PrepareRejection::RejectionCooldown { .. }
            | PrepareRejection::ClockRollback
            | PrepareRejection::OrdersDisabled
            | PrepareRejection::AlreadyConsumed
            | PrepareRejection::ProposalNotFound
            | PrepareRejection::SymbolFrozen
            | PrepareRejection::ObservationInvalid
            | PrepareRejection::EstimatePriceOutOfRange
            | PrepareRejection::MarketClosed
            | PrepareRejection::ProposalExpired
            | PrepareRejection::GateRejected(_)
            | PrepareRejection::ExecutionCheckFailed(_)
            | PrepareRejection::SummaryFailed(_)) => panic!("상한 초과 거부 기대: {other:?}"),
        }
    }

    #[test]
    fn rejects_consumed_and_frozen_symbol() {
        let now = kst(10, 0);
        let master = kr_master(now);
        let draft = make_kr_draft("ord-o04-reject-2", &master, now, "35000");
        let pid = draft.id().clone();
        let context = make_context(&master, now);
        let mut frozen = PrepareHarness::new(
            draft.clone(),
            context.clone(),
            now,
            OrderPermission::Enabled,
        );
        frozen.freeze = StubFreeze(SymbolFreezeStatus::Frozen);
        assert_eq!(
            expect_rejected(frozen.run_prepare(&pid).expect("동결 거부")),
            PrepareRejection::SymbolFrozen,
            "확인 불가 종목 동결 시 거부"
        );
        let consumed = PrepareHarness::new(draft, context, now, OrderPermission::Enabled);
        let _outcome = run(consumed.consumption.consume(&pid)).expect("소비 기록");
        assert_eq!(
            expect_rejected(consumed.run_prepare(&pid).expect("재사용 거부")),
            PrepareRejection::AlreadyConsumed,
            "이미 소비된 제안 거부"
        );
    }

    #[test]
    fn expired_draft_emits_expired_event_and_redrafts() {
        let created = kst(10, 0);
        let expired_now = kst(10, 15);
        let master = kr_master(created);
        let old_draft = make_kr_draft("ord-o04-exp-old", &master, created, "35000");
        let fresh_draft = make_kr_draft("ord-o04-exp-new", &master, expired_now, "35000");
        let old_id = old_draft.id().clone();
        let mut harness = PrepareHarness::new(
            old_draft,
            make_context(&master, expired_now),
            expired_now,
            OrderPermission::Enabled,
        );
        script_valid_market_and_broker(
            &harness.broker,
            &harness.market,
            &master,
            expired_now,
            "5000000",
        );
        harness.redrafter =
            StubRedrafter::returning(RedraftOutcome::Redrafted(Box::new(fresh_draft)));
        match harness.run_prepare(&old_id).expect("재생성 성공") {
            PrepareOutcome::Ready(ready) => {
                assert_eq!(
                    ready.summary().client_order_prefix().as_str(),
                    "ord-o04-",
                    "새 초안 요약 생성"
                );
                consume_ready(*ready);
            }
            PrepareOutcome::Rejected(other) => panic!("재생성 후 통과 기대: {other:?}"),
        }
        assert_eq!(
            read_events(&harness.events),
            vec![DomainEvent::ProposalExpired(old_id)],
            "만료 이벤트 1회 발행"
        );
    }

    #[test]
    fn expired_draft_rejects_when_out_of_estimate_range() {
        let created = kst(10, 0);
        let expired_now = kst(10, 15);
        let master = kr_master(created);
        let old_draft = make_kr_draft("ord-o04-exp-oor", &master, created, "35000");
        let old_id = old_draft.id().clone();
        let mut harness = PrepareHarness::new(
            old_draft,
            make_context(&master, expired_now),
            expired_now,
            OrderPermission::Enabled,
        );
        harness
            .broker
            .script_quotes()
            .push(Ok(vec![kr_quote(&master, "35000", expired_now)]));
        harness.redrafter = StubRedrafter::returning(RedraftOutcome::OutOfEstimateRange);
        assert_eq!(
            expect_rejected(harness.run_prepare(&old_id).expect("범위 이탈 거부")),
            PrepareRejection::EstimatePriceOutOfRange,
            "추정 기준가 이탈 거부"
        );
    }

    #[test]
    fn audit_failure_and_gate_rejection_block_preparation() {
        let now = kst(10, 0);
        let master = kr_master(now);
        let draft = make_kr_draft("ord-o04-audit-1", &master, now, "35000");
        let pid = draft.id().clone();
        let context = make_context(&master, now);
        let mut failing = PrepareHarness::new(
            draft.clone(),
            context.clone(),
            now,
            OrderPermission::Enabled,
        );
        let storage_err = SystemPortError::new(
            SystemPort::AuditLog,
            SystemFailure::Unavailable(application::RetryDelay::Unknown),
        );
        failing.audit = MemoryAuditLog::default();
        failing
            .audit
            .script_append()
            .push(MemoryReply::Return(Err(storage_err)));
        assert_eq!(
            failing.run_prepare(&pid).err(),
            Some(PrepareError::Storage(storage_err)),
            "감사 로그 쓰기 실패 시 즉시 거부"
        );
        assert_eq!(
            failing.guard.dialog_status(),
            DialogStatus::Idle,
            "오류 시 잠금 해제"
        );
        let gate_fail = PrepareHarness::new(draft, context, now, OrderPermission::Enabled);
        script_valid_market_and_broker(&gate_fail.broker, &gate_fail.market, &master, now, "1000");
        match expect_rejected(gate_fail.run_prepare(&pid).expect("게이트 거부")) {
            PrepareRejection::GateRejected(_) => (),
            other @ (PrepareRejection::DialogInProgress
            | PrepareRejection::RejectionCooldown { .. }
            | PrepareRejection::ClockRollback
            | PrepareRejection::OrdersDisabled
            | PrepareRejection::HourlyLimitExceeded { .. }
            | PrepareRejection::AlreadyConsumed
            | PrepareRejection::ProposalNotFound
            | PrepareRejection::SymbolFrozen
            | PrepareRejection::ObservationInvalid
            | PrepareRejection::EstimatePriceOutOfRange
            | PrepareRejection::MarketClosed
            | PrepareRejection::ProposalExpired
            | PrepareRejection::ExecutionCheckFailed(_)
            | PrepareRejection::SummaryFailed(_)) => {
                panic!("잔고 부족 게이트 거부 기대: {other:?}")
            }
        }
    }
}
