#[cfg(test)]
mod tests {
    use application::{
        AuditEntry, AuditKind, AuditLog, ConditionalDraftOutcome, CurrentDraftOutcome, DraftCheck,
        DraftStoreCandidate, EventBus, EventBusBuilder, EventSubscriber, OrderConfirmation,
        OrderGatePolicyContext, OrderRateRepository, ProposalConsumptionRepository,
        ProposalRepository, QuoteObservation, SettingsRepository, SignalSnapshot,
        SubscriberFailure, SystemFailure, SystemPort, SystemPortError, WriteCompleted,
    };
    use approval_command::execute::{
        ApprovalCommand, ApprovalCommandError, ApprovalCommandOutcome, ApprovalCommandRejection,
        ApprovalExecutionPorts, SubmissionAuditStatus, UnknownOrderVault,
    };
    use approval_command::guard::{ApprovalGuard, DialogStatus};
    use approval_command::ports::{
        BrokerOrderRejectionReason, ExpiredProposalRedrafter, PreSubmitCommitOutcome,
        PreSubmitTransactionPort, PreparedSubmissionRecord, ProposalContextPort, RedraftOutcome,
        SubmissionResponseRecord, SymbolFreezePort, SymbolFreezeStatus, UnknownOrderCause,
        UnknownOrderRetryStatus,
    };
    use approval_command::prepare::{
        ApprovalContextPorts, ApprovalReadPorts, ApprovalStoragePorts, PrepareRejection,
    };
    use domain::{
        AccountId, AdjustmentBasis, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
        BrokerAccountTypeCode, BrokerOrderId, BrokerOrderSet, BuyingPower, CalendarDate,
        CalendarYear, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        ClientOrderId, CommissionRateFact, CommissionValidity, CommonShare, CostRate, Currency,
        DayNumber, DisplayInstrumentName, DomainEvent, DomainEventKind, Draft,
        DraftRestoreEvidence, EstimateContext, EstimateTask, Horizon, HourlyOrderLimit,
        HoursPermission, HoursPermissionState, InstrumentId, KoreanMasterDetail, KrOrderPrice,
        LiquidationTrading, ListingStatus, Market, MarketCalendar, MarketSession, MasterCode,
        MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, Money,
        MonthNumber, Nanosecond, NumericInput, NxtTrading, OffsetSeconds, Ohlcv, OpenOrderCoverage,
        OrderIntent, OrderPermission, OrderSide, OrderTerms, Portfolio, Price, PriceAdjustment,
        PriceLimits, PriceLimitsInput, Proposal, ProposalExecutionError, ProposalId,
        ProposalLifetime, ProposalPriceRange, Quantity, Quote, RiskAppetite, RiskSettings,
        SavedMarketOrderSetting, SectorAssignments, SectorCatalog, SessionData, SessionInformation,
        StockWarnings, Symbol, TimeInterval, Timestamp, TradingSuspension, UnixSeconds, Volatility,
        WeightCaps, WholeOrderQuantity, default_market_order_policy,
        sector_assignment_from_compositions,
    };
    use order_execution::{
        ApprovedOrder, BrokerTrade, OrderOutcome, OrderRejectionReason, OrderUnknownCause,
    };
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};
    use testkit_ports::{
        ScriptedPresence, SteppedClock,
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

    fn push_gate_round(
        broker: &ScriptedBrokerRead,
        market: &ScriptedMarketInfo,
        master: &MasterInstrument,
        now: Timestamp,
        quote_and_power: (&str, &str),
    ) {
        let (quote_price, buying_power_krw) = quote_and_power;
        let quote = kr_quote(master, quote_price, now);
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

    struct StubRedrafter;

    impl ExpiredProposalRedrafter for StubRedrafter {
        fn redraft(
            &self,
            _expired: &Proposal<Draft>,
            _now: Timestamp,
        ) -> impl Future<Output = Result<RedraftOutcome, SystemPortError>> + Send {
            std::future::ready(Ok(RedraftOutcome::NoSignal))
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

    #[derive(Clone, Copy)]
    enum CommitMode {
        Atomic,
        FailTransaction(SystemPortError),
        AlreadyConsumedRace,
    }

    struct AtomicTxStub<'a> {
        consumption: &'a MemoryProposalConsumptionRepository,
        audit: &'a MemoryAuditLog,
        mode: CommitMode,
        prepared_records: RwLock<Vec<PreparedSubmissionRecord>>,
        response_records: RwLock<Vec<SubmissionResponseRecord>>,
    }

    impl<'a> AtomicTxStub<'a> {
        fn new(
            consumption: &'a MemoryProposalConsumptionRepository,
            audit: &'a MemoryAuditLog,
            mode: CommitMode,
        ) -> Self {
            Self {
                consumption,
                audit,
                mode,
                prepared_records: RwLock::new(Vec::new()),
                response_records: RwLock::new(Vec::new()),
            }
        }

        fn prepared(&self) -> Vec<PreparedSubmissionRecord> {
            match self.prepared_records.read() {
                Ok(guard) => guard.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        }

        fn responses(&self) -> Vec<SubmissionResponseRecord> {
            match self.response_records.read() {
                Ok(guard) => guard.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        }
    }

    impl PreSubmitTransactionPort for AtomicTxStub<'_> {
        async fn commit_consumption_and_submission(
            &self,
            record: &PreparedSubmissionRecord,
        ) -> Result<PreSubmitCommitOutcome, SystemPortError> {
            match self.mode {
                CommitMode::FailTransaction(err) => Err(err),
                CommitMode::AlreadyConsumedRace => Ok(PreSubmitCommitOutcome::AlreadyConsumed),
                CommitMode::Atomic => match self.consumption.consume(&record.proposal_id).await? {
                    application::ConsumptionOutcome::AlreadyConsumed => {
                        Ok(PreSubmitCommitOutcome::AlreadyConsumed)
                    }
                    application::ConsumptionOutcome::Recorded => {
                        let _audit = self
                            .audit
                            .append(&AuditEntry::new(
                                record.prepared_at,
                                AuditKind::SubmissionPrepared(record.client_order_id.clone()),
                            ))
                            .await?;
                        let mut lock = match self.prepared_records.write() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        lock.push(record.clone());
                        Ok(PreSubmitCommitOutcome::Committed)
                    }
                },
            }
        }

        fn record_submission_outcome(
            &self,
            record: &SubmissionResponseRecord,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            let mut lock = match self.response_records.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            lock.push(record.clone());
            std::future::ready(Ok(WriteCompleted))
        }
    }

    #[derive(Clone)]
    enum BrokerReplySpec {
        Accept(String),
        Reject(OrderRejectionReason),
        Unknown(OrderUnknownCause),
    }

    struct RecordingTradeBroker {
        replies: RwLock<VecDeque<BrokerReplySpec>>,
        submitted_keys: RwLock<Vec<ClientOrderId>>,
    }

    impl RecordingTradeBroker {
        fn with_replies(items: Vec<BrokerReplySpec>) -> Self {
            Self {
                replies: RwLock::new(items.into()),
                submitted_keys: RwLock::new(Vec::new()),
            }
        }

        fn submitted_keys(&self) -> Vec<ClientOrderId> {
            match self.submitted_keys.read() {
                Ok(guard) => guard.clone(),
                Err(poisoned) => poisoned.into_inner().clone(),
            }
        }
    }

    impl BrokerTrade for RecordingTradeBroker {
        fn submit(&self, order: ApprovedOrder) -> impl Future<Output = OrderOutcome> + Send {
            let mut keys = match self.submitted_keys.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            keys.push(order.client_order_id().clone());
            let mut replies = match self.replies.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let reply = replies
                .pop_front()
                .unwrap_or(BrokerReplySpec::Reject(OrderRejectionReason::OtherRejected));
            let outcome = match reply {
                BrokerReplySpec::Accept(id) => {
                    order.into_accepted(BrokerOrderId::try_new(&id).expect("주문번호"))
                }
                BrokerReplySpec::Reject(reason) => order.into_rejected(reason),
                BrokerReplySpec::Unknown(cause) => order.into_unknown(cause),
            };
            std::future::ready(outcome)
        }
    }

    fn settings_with_orders(permission: OrderPermission) -> MemorySettingsRepository {
        let settings = MemorySettingsRepository::default();
        settings
            .script_store_risk_settings()
            .push(MemoryReply::Memory);
        for _ in 0_u8..8 {
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

    fn make_bus_with_collector() -> (EventBus, Arc<RwLock<Vec<DomainEvent>>>) {
        let collected = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([
                DomainEventKind::ProposalApproved,
                DomainEventKind::ProposalRejected,
                DomainEventKind::OrderSubmitted,
                DomainEventKind::OrderRejected,
                DomainEventKind::OrderOutcomeUnknown,
            ]),
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

    struct ExecuteHarness {
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
        freeze: StubFreeze,
        clock: SteppedClock,
        bus: EventBus,
        events: Arc<RwLock<Vec<DomainEvent>>>,
        presence: ScriptedPresence,
        trade: RecordingTradeBroker,
        vault: UnknownOrderVault,
    }

    impl ExecuteHarness {
        fn new(
            draft: Proposal<Draft>,
            context: OrderGatePolicyContext,
            now: Timestamp,
            confirmations: Vec<OrderConfirmation>,
            broker_replies: Vec<BrokerReplySpec>,
        ) -> Self {
            let pid = draft.id().clone();
            let (bus, events) = make_bus_with_collector();
            let consumption = MemoryProposalConsumptionRepository::default();
            let reports = MemoryValidationReportRepository::default();
            let audit = MemoryAuditLog::default();
            for _ in 0_u8..12 {
                consumption.script_status().push(MemoryReply::Memory);
                consumption.script_consume().push(MemoryReply::Memory);
                reports.script_append().push(MemoryReply::Memory);
                audit.script_append().push(MemoryReply::Memory);
            }
            Self {
                guard: ApprovalGuard::default(),
                broker: ScriptedBrokerRead::default(),
                market: ScriptedMarketInfo::default(),
                settings: settings_with_orders(OrderPermission::Enabled),
                rates: StubRateRepo {
                    limit: None,
                    times: Vec::new(),
                },
                proposals: StubProposalRepo::with_draft(draft),
                consumption,
                reports,
                audit,
                contexts: StubContextPort {
                    contexts: BTreeMap::from([(pid, context)]),
                },
                freeze: StubFreeze(SymbolFreezeStatus::Unfrozen),
                clock: SteppedClock::new(now),
                bus,
                events,
                presence: ScriptedPresence::new(confirmations, []),
                trade: RecordingTradeBroker::with_replies(broker_replies),
                vault: UnknownOrderVault::new(),
            }
        }

        fn execute(
            &self,
            proposal_id: &ProposalId,
            tx_port: &AtomicTxStub<'_>,
        ) -> Result<ApprovalCommandOutcome, ApprovalCommandError> {
            let command = ApprovalCommand::new(
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
                    redrafter: &StubRedrafter,
                    freeze: &self.freeze,
                    clock: &self.clock,
                    events: &self.bus,
                },
                ApprovalExecutionPorts {
                    presence: &self.presence,
                    tx_port,
                    trade: &self.trade,
                    vault: &self.vault,
                },
            );
            run(command.request_order_approval(proposal_id))
        }
    }

    fn script_full_happy_broker(
        harness: &ExecuteHarness,
        master: &MasterInstrument,
        now: Timestamp,
        post_quote_price: &str,
        post_buying_power: &str,
    ) {
        harness
            .broker
            .script_quotes()
            .push(Ok(vec![kr_quote(master, "35000", now)]));
        push_gate_round(
            &harness.broker,
            &harness.market,
            master,
            now,
            ("35000", "5000000"),
        );
        push_gate_round(
            &harness.broker,
            &harness.market,
            master,
            now,
            (post_quote_price, post_buying_power),
        );
    }

    #[test]
    fn confirmed_order_submits_once_records_audit_and_blocks_reuse() {
        let now = kst(10, 0);
        let master = kr_master(now);
        let draft = make_kr_draft("ord-o18-ok-001", &master, now, "35000");
        let pid = draft.id().clone();
        let cid = pid.as_client_order_id().clone();
        let harness = ExecuteHarness::new(
            draft,
            make_context(&master, now),
            now,
            vec![OrderConfirmation::Confirmed],
            vec![BrokerReplySpec::Accept("BRK-1001".to_owned())],
        );
        script_full_happy_broker(&harness, &master, now, "35000", "5000000");
        let tx = AtomicTxStub::new(&harness.consumption, &harness.audit, CommitMode::Atomic);
        let outcome = harness.execute(&pid, &tx).expect("주문 승인 실행");
        assert_eq!(
            outcome,
            ApprovalCommandOutcome::Submitted {
                client_order_id: cid.clone(),
                broker_order_id: BrokerOrderId::try_new("BRK-1001").expect("주문번호"),
                audit_status: SubmissionAuditStatus::Recorded,
                delivery: application::DeliveryReport::default(),
            },
            "승인 통과 시 정확히 1건 전송"
        );
        assert_eq!(
            harness.trade.submitted_keys(),
            vec![cid.clone()],
            "결정적 키 1회 전송"
        );
        assert_eq!(tx.prepared().len(), 1, "전송 전 원장 기록 1건");
        assert_eq!(tx.responses().len(), 1, "응답 원장 기록 1건");
        assert_eq!(
            read_events(&harness.events),
            vec![
                DomainEvent::ProposalApproved(pid.clone()),
                DomainEvent::OrderSubmitted(cid),
            ],
            "승인·전송 이벤트 순서"
        );
        let second = harness.execute(&pid, &tx).expect("재사용 거부");
        assert_eq!(
            second,
            ApprovalCommandOutcome::Rejected(ApprovalCommandRejection::Prepare(
                PrepareRejection::AlreadyConsumed
            )),
            "소비된 제안 재승인 거부"
        );
        assert_eq!(
            harness.presence.order_calls().len(),
            1,
            "재요청 시 본인 확인 0회 추가"
        );
        assert_eq!(
            harness.trade.submitted_keys().len(),
            1,
            "재요청 시 전송 0건 추가"
        );
    }

    #[test]
    fn presence_denied_blocks_submission_and_activates_cooldown() {
        let now = kst(10, 0);
        let master = kr_master(now);
        let draft = make_kr_draft("ord-o18-deny-01", &master, now, "35000");
        let pid = draft.id().clone();
        let harness = ExecuteHarness::new(
            draft,
            make_context(&master, now),
            now,
            vec![OrderConfirmation::Denied],
            Vec::new(),
        );
        harness
            .broker
            .script_quotes()
            .push(Ok(vec![kr_quote(&master, "35000", now)]));
        push_gate_round(
            &harness.broker,
            &harness.market,
            &master,
            now,
            ("35000", "5000000"),
        );
        let tx = AtomicTxStub::new(&harness.consumption, &harness.audit, CommitMode::Atomic);
        assert_eq!(
            harness.execute(&pid, &tx).expect("거부 결과"),
            ApprovalCommandOutcome::Rejected(ApprovalCommandRejection::PresenceDenied),
            "본인 확인 거절"
        );
        assert!(
            harness.trade.submitted_keys().is_empty(),
            "거절 시 전송 0건"
        );
        assert!(tx.prepared().is_empty(), "거절 시 소비 0건");
        assert_eq!(
            read_events(&harness.events),
            vec![DomainEvent::ProposalRejected(pid.clone())],
            "ProposalRejected 발행"
        );
        assert_eq!(
            harness.execute(&pid, &tx).expect("쿨다운 거부"),
            ApprovalCommandOutcome::Rejected(ApprovalCommandRejection::Prepare(
                PrepareRejection::RejectionCooldown {
                    retry_at: at_kst(now.utc_seconds().as_i64().saturating_add(60)),
                }
            )),
            "거절 직후 재요청 차단"
        );
        assert_eq!(
            harness.guard.dialog_status(),
            DialogStatus::Idle,
            "창 잠금 해제"
        );
    }

    #[test]
    fn post_presence_price_move_and_funds_drop_submit_zero_orders() {
        let now = kst(10, 0);
        let master = kr_master(now);
        let draft_price = make_kr_draft("ord-o18-drift-1", &master, now, "35000");
        let pid_price = draft_price.id().clone();
        let h_price = ExecuteHarness::new(
            draft_price,
            make_context(&master, now),
            now,
            vec![OrderConfirmation::Confirmed],
            Vec::new(),
        );
        script_full_happy_broker(&h_price, &master, now, "44000", "5000000");
        let tx_price = AtomicTxStub::new(&h_price.consumption, &h_price.audit, CommitMode::Atomic);
        assert_eq!(
            h_price
                .execute(&pid_price, &tx_price)
                .expect("가격 이탈 거부"),
            ApprovalCommandOutcome::Rejected(ApprovalCommandRejection::PostPresenceRecheckFailed(
                ProposalExecutionError::PriceOutsideRange
            )),
            "확인 창 중 가격 이탈 시 재계산 없이 전송 0건"
        );
        assert!(h_price.trade.submitted_keys().is_empty(), "전송 0건");
        assert!(tx_price.prepared().is_empty(), "소비 0건");
        let draft_funds = make_kr_draft("ord-o18-funds-1", &master, now, "35000");
        let pid_funds = draft_funds.id().clone();
        let h_funds = ExecuteHarness::new(
            draft_funds,
            make_context(&master, now),
            now,
            vec![OrderConfirmation::Confirmed],
            Vec::new(),
        );
        script_full_happy_broker(&h_funds, &master, now, "35000", "1000");
        let tx_funds = AtomicTxStub::new(&h_funds.consumption, &h_funds.audit, CommitMode::Atomic);
        match h_funds
            .execute(&pid_funds, &tx_funds)
            .expect("잔고 감소 거부")
        {
            ApprovalCommandOutcome::Rejected(
                ApprovalCommandRejection::PostPresenceConditionsChanged(_),
            ) => (),
            other @ (ApprovalCommandOutcome::Rejected(_)
            | ApprovalCommandOutcome::Submitted { .. }
            | ApprovalCommandOutcome::BrokerRejected { .. }
            | ApprovalCommandOutcome::OutcomeUnknown { .. }) => {
                panic!("잔고 변화 거부 기대: {other:?}");
            }
        }
        assert!(
            h_funds.trade.submitted_keys().is_empty(),
            "잔고 감소 시 전송 0건"
        );
        assert!(tx_funds.prepared().is_empty(), "잔고 감소 시 소비 0건");
    }

    #[test]
    fn atomic_commit_failure_and_race_block_broker_submit() {
        let now = kst(10, 0);
        let master = kr_master(now);
        let draft = make_kr_draft("ord-o18-txfail-1", &master, now, "35000");
        let pid = draft.id().clone();
        let h_fail = ExecuteHarness::new(
            draft.clone(),
            make_context(&master, now),
            now,
            vec![OrderConfirmation::Confirmed],
            Vec::new(),
        );
        script_full_happy_broker(&h_fail, &master, now, "35000", "5000000");
        let storage_err = SystemPortError::new(
            SystemPort::AuditLog,
            SystemFailure::Unavailable(application::RetryDelay::Unknown),
        );
        let tx_fail = AtomicTxStub::new(
            &h_fail.consumption,
            &h_fail.audit,
            CommitMode::FailTransaction(storage_err),
        );
        assert_eq!(
            h_fail.execute(&pid, &tx_fail),
            Err(ApprovalCommandError::Storage(storage_err)),
            "원자 커밋 실패 시 오류 반환"
        );
        assert!(
            h_fail.trade.submitted_keys().is_empty(),
            "감사 커밋 실패 시 전송 0건"
        );
        let h_race = ExecuteHarness::new(
            draft,
            make_context(&master, now),
            now,
            vec![OrderConfirmation::Confirmed],
            Vec::new(),
        );
        script_full_happy_broker(&h_race, &master, now, "35000", "5000000");
        let tx_race = AtomicTxStub::new(
            &h_race.consumption,
            &h_race.audit,
            CommitMode::AlreadyConsumedRace,
        );
        assert_eq!(
            h_race.execute(&pid, &tx_race).expect("경합 거부"),
            ApprovalCommandOutcome::Rejected(ApprovalCommandRejection::AlreadyConsumed),
            "소비 고유 제약 경합 시 거부"
        );
        assert!(h_race.trade.submitted_keys().is_empty(), "경합 시 전송 0건");
    }

    #[test]
    fn high_value_rejection_never_retries_and_unknown_stores_retry_permit_selectively() {
        let now = kst(10, 0);
        let master = kr_master(now);
        let draft_hv = make_kr_draft("ord-o18-hv-001", &master, now, "35000");
        let pid_hv = draft_hv.id().clone();
        let cid_hv = pid_hv.as_client_order_id().clone();
        let h_hv = ExecuteHarness::new(
            draft_hv,
            make_context(&master, now),
            now,
            vec![OrderConfirmation::Confirmed],
            vec![BrokerReplySpec::Reject(
                OrderRejectionReason::ConfirmHighValueRequired,
            )],
        );
        script_full_happy_broker(&h_hv, &master, now, "35000", "5000000");
        let tx_hv = AtomicTxStub::new(&h_hv.consumption, &h_hv.audit, CommitMode::Atomic);
        assert_eq!(
            h_hv.execute(&pid_hv, &tx_hv).expect("고액 거절"),
            ApprovalCommandOutcome::BrokerRejected {
                client_order_id: cid_hv.clone(),
                reason: BrokerOrderRejectionReason::ConfirmHighValueRequired,
                audit_status: SubmissionAuditStatus::Recorded,
                delivery: application::DeliveryReport::default(),
            },
            "고액 확인 거절은 실패로 종료"
        );
        assert_eq!(h_hv.trade.submitted_keys().len(), 1, "고액 거절 재시도 0회");
        assert_eq!(
            h_hv.vault.retry_status(&cid_hv),
            UnknownOrderRetryStatus::Forbidden,
            "거절 주문은 재시도 보관 없음"
        );
        verify_unknown_vault_behavior(&master, now);
    }

    fn verify_unknown_vault_behavior(master: &MasterInstrument, now: Timestamp) {
        let draft_tr = make_kr_draft("ord-o18-unk-tr", master, now, "35000");
        let pid_tr = draft_tr.id().clone();
        let cid_tr = pid_tr.as_client_order_id().clone();
        let h_tr = ExecuteHarness::new(
            draft_tr,
            make_context(master, now),
            now,
            vec![OrderConfirmation::Confirmed],
            vec![BrokerReplySpec::Unknown(OrderUnknownCause::Transport)],
        );
        script_full_happy_broker(&h_tr, master, now, "35000", "5000000");
        let tx_tr = AtomicTxStub::new(&h_tr.consumption, &h_tr.audit, CommitMode::Atomic);
        assert_eq!(
            h_tr.execute(&pid_tr, &tx_tr).expect("전송 모름"),
            ApprovalCommandOutcome::OutcomeUnknown {
                client_order_id: cid_tr.clone(),
                cause: UnknownOrderCause::Transport,
                retry: UnknownOrderRetryStatus::Eligible,
                audit_status: SubmissionAuditStatus::Recorded,
                delivery: application::DeliveryReport::default(),
            },
            "전송 오류 미확정은 재시도 권한 보관"
        );
        assert_eq!(
            h_tr.vault.retry_status(&cid_tr),
            UnknownOrderRetryStatus::Eligible,
            "전송 오류 재시도 가능"
        );
        let draft_m = make_kr_draft("ord-o18-unk-mt", master, now, "35000");
        let pid_m = draft_m.id().clone();
        let cid_m = pid_m.as_client_order_id().clone();
        let h_m = ExecuteHarness::new(
            draft_m,
            make_context(master, now),
            now,
            vec![OrderConfirmation::Confirmed],
            vec![BrokerReplySpec::Unknown(OrderUnknownCause::Maintenance)],
        );
        script_full_happy_broker(&h_m, master, now, "35000", "5000000");
        let tx_m = AtomicTxStub::new(&h_m.consumption, &h_m.audit, CommitMode::Atomic);
        assert_eq!(
            h_m.execute(&pid_m, &tx_m).expect("점검 모름"),
            ApprovalCommandOutcome::OutcomeUnknown {
                client_order_id: cid_m.clone(),
                cause: UnknownOrderCause::Maintenance,
                retry: UnknownOrderRetryStatus::Forbidden,
                audit_status: SubmissionAuditStatus::Recorded,
                delivery: application::DeliveryReport::default(),
            },
            "점검 응답 미확정은 재시도 금지"
        );
        assert_eq!(
            h_m.vault.retry_status(&cid_m),
            UnknownOrderRetryStatus::Forbidden,
            "점검 미확정은 보관 없음"
        );
    }
}
