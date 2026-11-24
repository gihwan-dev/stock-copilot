use application::{
    AuditEntry, AuditKind, AuditLog, ConditionalDraftOutcome, ConsumptionOutcome,
    CurrentDraftOutcome, DraftCheck, DraftStoreCandidate, EventBusBuilder, OrderGatePolicyContext,
    OrderRateRepository, ProposalConsumptionRepository, ProposalRepository, QuoteObservation,
    SettingsRepository, SignalSnapshot, SystemPortError, WriteCompleted,
};
use approval_command::ports::{
    ExpiredProposalRedrafter, PreSubmitCommitOutcome, PreSubmitTransactionPort,
    PreparedSubmissionRecord, ProposalContextPort, RedraftOutcome, SubmissionResponseRecord,
    SymbolFreezePort, SymbolFreezeStatus,
};
use approval_command::service::{ApprovalService, ApprovalServiceParts, ApprovalServiceTypes};
use domain::{
    AccountId, AdjustmentBasis, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
    BrokerAccountTypeCode, BrokerOrderId, BrokerOrderSet, BuyingPower, CalendarDate, CalendarYear,
    Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime, ClientOrderId,
    CommissionRateFact, CommissionValidity, CommonShare, CostRate, Currency, DayNumber,
    DisplayInstrumentName, Draft, DraftRestoreEvidence, EstimateContext, EstimateTask, Horizon,
    HourlyOrderLimit, HoursPermission, HoursPermissionState, InstrumentId, KoreanMasterDetail,
    KrOrderPrice, LiquidationTrading, ListingStatus, Market, MarketCalendar, MarketSession,
    MasterCode, MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType,
    Money, MonthNumber, Nanosecond, NumericInput, NxtTrading, OffsetSeconds, Ohlcv,
    OpenOrderCoverage, OrderIntent, OrderPermission, OrderSide, OrderTerms, Portfolio, Price,
    PriceAdjustment, PriceLimits, PriceLimitsInput, Proposal, ProposalId, ProposalLifetime,
    ProposalPriceRange, Quantity, Quote, RiskAppetite, RiskSettings, SavedMarketOrderSetting,
    SectorAssignments, SectorCatalog, SessionData, SessionInformation, StockWarnings, Symbol,
    TimeInterval, Timestamp, TradingSuspension, UnixSeconds, Volatility, WeightCaps,
    WholeOrderQuantity, default_market_order_policy, sector_assignment_from_compositions,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, RwLock};
use testkit_ports::memory::{MemoryReply, MemorySettingsRepository};
use testkit_ports::proposal_memory::{
    MemoryProposalConsumptionRepository, MemoryValidationReportRepository,
};
use testkit_ports::scripted::{ScriptedBrokerRead, ScriptedMarketInfo};
use testkit_ports::system_fakes::MemoryAuditLog;
use testkit_ports::{ScriptedPresence, SteppedClock};
use trade_fakes::{BrokerReply, RecordingBroker};

const DAY_BASE: i64 = 1_775_055_600;

pub(crate) const PROPOSAL: &str = "ord-o21-smoke-001";

fn at_kst(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

pub(crate) fn now() -> Timestamp {
    at_kst(DAY_BASE.saturating_add(36_000))
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

fn price(value: &str) -> Price {
    Price::try_new(money(value, Currency::Krw)).expect("가격")
}

fn quantity(value: &str) -> Quantity {
    Quantity::parse(&NumericInput::try_new(value).expect("숫자")).expect("수량")
}

fn master() -> MasterInstrument {
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
        observed_at: now(),
    })
    .expect("마스터")
}

fn volatility(master: &MasterInstrument) -> Volatility {
    let adjustment = PriceAdjustment::Adjusted(AdjustmentBasis::new(now()));
    let candles: Vec<Candle> = (0_i64..=60_i64)
        .map(|index| {
            let offset_days = 60_i64.saturating_sub(index);
            let stamp = at_kst(DAY_BASE.saturating_sub(offset_days.saturating_mul(86_400)));
            let close = if index.rem_euclid(2) == 0 {
                price("34500")
            } else {
                price("35500")
            };
            let values = Ohlcv::try_new(close, close, close, close, quantity("1000")).expect("봉");
            Candle::try_new(
                master.row().instrument_id.clone(),
                CandleTime::try_new(stamp, CandlePeriod::Daily).expect("일봉 시각"),
                adjustment,
                values,
                CandleObservation::new(now(), CandleStatus::Finalized),
            )
            .expect("일봉")
        })
        .collect();
    Volatility::calculate(&candles).expect("변동성")
}

fn draft(master: &MasterInstrument) -> Proposal<Draft> {
    let client_id = ClientOrderId::try_new(PROPOSAL).expect("주문 키");
    let obs = master
        .price_observation(price("35000"), now())
        .expect("시세 관측");
    let task = EstimateTask::create(
        EstimateContext::new(obs.clone(), now()),
        volatility(master),
        Horizon::default(),
        HoursPermission::default(),
        RiskAppetite::Neutral,
    )
    .expect("추정 과제");
    let range = ProposalPriceRange::from_estimate_task(task.range_facts(), obs).expect("범위");
    let intent = OrderIntent::new(
        AccountId::new(42),
        master.order_instrument().expect("종목"),
        client_id.clone(),
        OrderTerms::KrLimit {
            side: OrderSide::Buy,
            quantity: WholeOrderQuantity::try_new(quantity("2")).expect("정수 수량"),
            price: KrOrderPrice::try_new(price("35000")).expect("지정가"),
        },
    )
    .expect("주문 의도");
    Proposal::new(
        ProposalId::new(client_id),
        intent,
        now(),
        range,
        ProposalLifetime::default(),
    )
    .expect("초안")
}

fn context(master: &MasterInstrument) -> OrderGatePolicyContext {
    let portfolio = Portfolio::new(
        Vec::new(),
        BuyingPower::new(money("5000000", Currency::Krw), now()),
        BuyingPower::new(money("0", Currency::Usd), now()),
    )
    .expect("포트폴리오")
    .evaluate(None);
    let catalog = SectorCatalog::from_response(Vec::new(), now()).expect("섹터");
    let assignment = sector_assignment_from_compositions(master, &catalog, &[]).expect("배정");
    OrderGatePolicyContext {
        master: master.clone(),
        market_order_policy: default_market_order_policy(domain::Policy::new(
            domain::InvestorProfile::default(),
        )),
        valuation: portfolio,
        assignments: SectorAssignments::try_new(vec![assignment]).expect("배정 목록"),
        security_facts: MasterInstrumentFacts::try_new(vec![master.clone()])
            .expect("마스터 집합")
            .policy_security_facts(),
    }
}

fn calendar() -> MarketCalendar {
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

fn quote(master: &MasterInstrument) -> QuoteObservation {
    QuoteObservation {
        instrument: master.quote_instrument().expect("시세 종목"),
        result: Ok(Quote::try_new(
            master.row().symbol.clone(),
            Currency::Krw,
            Some(price("35000")),
            Some(now()),
            now(),
        )
        .expect("시세")),
    }
}

fn push_gate_round(
    broker: &ScriptedBrokerRead,
    market: &ScriptedMarketInfo,
    master: &MasterInstrument,
) {
    broker.script_quotes().push(Ok(vec![quote(master)]));
    broker
        .script_accounts()
        .push(Ok(BrokerAccountRows::try_new(vec![
            BrokerAccountRow::from_broker_account_result(
                AccountId::new(42),
                BrokerAccountTypeCode::new("BROKERAGE"),
                BrokerAccountNumberInput::new("111122223333".to_owned()),
                now(),
            )
            .expect("계좌 행"),
        ])
        .expect("계좌 목록")));
    broker
        .script_price_limits()
        .push(Ok(PriceLimits::try_new(PriceLimitsInput {
            instrument: master.quote_instrument().expect("시세 종목"),
            currency: Currency::Krw,
            lower: Some(price("25000")),
            upper: Some(price("45000")),
            timestamp: now(),
            collected_at: now(),
        })
        .expect("가격 제한")));
    broker
        .script_open_orders()
        .push(Ok(BrokerOrderSet::from_open_lookup_result(
            AccountId::new(42),
            Vec::new(),
            OpenOrderCoverage::Complete,
            now(),
        )
        .expect("미체결")));
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
    broker
        .script_buying_power()
        .push(Ok(BuyingPower::new(money("5000000", Currency::Krw), now())));
    broker
        .script_stock_warnings()
        .push(Ok(StockWarnings::from_lookup_result(
            master.order_instrument().expect("종목"),
            Vec::new(),
            now(),
        )));
    market.script_calendar().push(Ok(calendar()));
}

pub(crate) struct NoRates;

impl OrderRateRepository for NoRates {
    fn hourly_limit(
        &self,
    ) -> impl Future<Output = Result<Option<HourlyOrderLimit>, SystemPortError>> + Send {
        std::future::ready(Ok(None))
    }

    fn recent_submission_times(
        &self,
        _since: Timestamp,
    ) -> impl Future<Output = Result<Vec<Timestamp>, SystemPortError>> + Send {
        std::future::ready(Ok(Vec::new()))
    }
}

pub(crate) struct FixedProposals(BTreeMap<ProposalId, Proposal<Draft>>);

impl ProposalRepository for FixedProposals {
    fn store_draft(
        &self,
        _proposal: &Proposal<Draft>,
        _evidence: &DraftRestoreEvidence,
        _master: &MasterInstrument,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Ok(WriteCompleted))
    }

    fn draft(
        &self,
        proposal: &ProposalId,
        _master: &MasterInstrument,
    ) -> impl Future<Output = Result<Option<Proposal<Draft>>, SystemPortError>> + Send {
        std::future::ready(Ok(self.0.get(proposal).cloned()))
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

pub(crate) struct FixedContexts(BTreeMap<ProposalId, OrderGatePolicyContext>);

impl ProposalContextPort for FixedContexts {
    fn policy_context(
        &self,
        proposal_id: &ProposalId,
    ) -> impl Future<Output = Result<Option<OrderGatePolicyContext>, SystemPortError>> + Send {
        std::future::ready(Ok(self.0.get(proposal_id).cloned()))
    }
}

pub(crate) struct NoRedraft;

impl ExpiredProposalRedrafter for NoRedraft {
    fn redraft(
        &self,
        _expired: &Proposal<Draft>,
        _now: Timestamp,
    ) -> impl Future<Output = Result<RedraftOutcome, SystemPortError>> + Send {
        std::future::ready(Ok(RedraftOutcome::NoSignal))
    }
}

pub(crate) struct NeverFrozen;

impl SymbolFreezePort for NeverFrozen {
    fn freeze_status(
        &self,
        _account: &AccountId,
        _instrument: &domain::ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<SymbolFreezeStatus, SystemPortError>> + Send {
        std::future::ready(Ok(SymbolFreezeStatus::Unfrozen))
    }
}

pub(crate) struct LedgerTx {
    consumption: Arc<MemoryProposalConsumptionRepository>,
    audit: Arc<MemoryAuditLog>,
    prepared: RwLock<Vec<PreparedSubmissionRecord>>,
}

impl LedgerTx {
    pub(crate) fn prepared(&self) -> Vec<PreparedSubmissionRecord> {
        match self.prepared.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl PreSubmitTransactionPort for LedgerTx {
    async fn commit_consumption_and_submission(
        &self,
        record: &PreparedSubmissionRecord,
    ) -> Result<PreSubmitCommitOutcome, SystemPortError> {
        match self.consumption.consume(&record.proposal_id).await? {
            ConsumptionOutcome::AlreadyConsumed => Ok(PreSubmitCommitOutcome::AlreadyConsumed),
            ConsumptionOutcome::Recorded => {
                let _audit = self
                    .audit
                    .append(&AuditEntry::new(
                        record.prepared_at,
                        AuditKind::SubmissionPrepared(record.client_order_id.clone()),
                    ))
                    .await?;
                match self.prepared.write() {
                    Ok(mut guard) => guard.push(record.clone()),
                    Err(poisoned) => poisoned.into_inner().push(record.clone()),
                }
                Ok(PreSubmitCommitOutcome::Committed)
            }
        }
    }

    fn record_submission_outcome(
        &self,
        _record: &SubmissionResponseRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Ok(WriteCompleted))
    }
}

pub(crate) struct PathTypes;

impl ApprovalServiceTypes for PathTypes {
    type Broker = ScriptedBrokerRead;
    type Market = ScriptedMarketInfo;
    type Settings = MemorySettingsRepository;
    type Rates = NoRates;
    type Proposals = FixedProposals;
    type Consumption = MemoryProposalConsumptionRepository;
    type Reports = MemoryValidationReportRepository;
    type Audit = MemoryAuditLog;
    type Contexts = FixedContexts;
    type Redrafter = NoRedraft;
    type Freeze = NeverFrozen;
    type Clock = SteppedClock;
    type Presence = ScriptedPresence;
    type TxPort = LedgerTx;
    type Trade = RecordingBroker;
}

pub(crate) type ApprovalServiceOf = ApprovalService<PathTypes>;

pub(crate) struct PathFixture {
    pub(crate) service: Arc<ApprovalService<PathTypes>>,
    pub(crate) trade: Arc<RecordingBroker>,
    pub(crate) tx: Arc<LedgerTx>,
}

fn enabled_settings() -> MemorySettingsRepository {
    let settings = MemorySettingsRepository::default();
    settings
        .script_store_risk_settings()
        .push(MemoryReply::Memory);
    for _ in 0_u8..8 {
        settings.script_risk_settings().push(MemoryReply::Memory);
    }
    let risk = RiskSettings::new(
        OrderPermission::Enabled,
        SavedMarketOrderSetting::Disabled,
        HoursPermissionState::Disabled,
        WeightCaps::default(),
        RiskAppetite::Neutral,
    );
    let stored = tauri::async_runtime::block_on(settings.store_risk_settings(&risk));
    assert_eq!(stored, Ok(WriteCompleted), "설정 저장");
    settings
}

pub(crate) fn fixture(presence: Arc<ScriptedPresence>, accept: &str) -> PathFixture {
    let master = master();
    let proposal = draft(&master);
    let id = proposal.id().clone();
    let broker = Arc::new(ScriptedBrokerRead::default());
    let market = Arc::new(ScriptedMarketInfo::default());
    broker.script_quotes().push(Ok(vec![quote(&master)]));
    push_gate_round(&broker, &market, &master);
    push_gate_round(&broker, &market, &master);
    let consumption = Arc::new(MemoryProposalConsumptionRepository::default());
    let reports = Arc::new(MemoryValidationReportRepository::default());
    let audit = Arc::new(MemoryAuditLog::default());
    for _ in 0_u8..12 {
        consumption.script_status().push(MemoryReply::Memory);
        consumption.script_consume().push(MemoryReply::Memory);
        reports.script_append().push(MemoryReply::Memory);
        audit.script_append().push(MemoryReply::Memory);
    }
    let trade = Arc::new(RecordingBroker::with_replies([BrokerReply::Accept(
        BrokerOrderId::try_new(accept).expect("주문번호"),
    )]));
    let tx = Arc::new(LedgerTx {
        consumption: Arc::clone(&consumption),
        audit: Arc::clone(&audit),
        prepared: RwLock::new(Vec::new()),
    });
    let service = Arc::new(ApprovalService::new(ApprovalServiceParts::<PathTypes> {
        broker,
        market,
        settings: Arc::new(enabled_settings()),
        rates: Arc::new(NoRates),
        proposals: Arc::new(FixedProposals(BTreeMap::from([(id.clone(), proposal)]))),
        consumption,
        reports,
        audit,
        contexts: Arc::new(FixedContexts(BTreeMap::from([(id, context(&master))]))),
        redrafter: Arc::new(NoRedraft),
        freeze: Arc::new(NeverFrozen),
        clock: Arc::new(SteppedClock::new(now())),
        presence,
        tx_port: Arc::clone(&tx),
        trade: Arc::clone(&trade),
        events: EventBusBuilder::default().build(),
    }));
    PathFixture { service, trade, tx }
}
