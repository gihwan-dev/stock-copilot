#[cfg(test)]
mod tests {
    mod a27_fixtures;
    use a27_fixtures as fixtures;
    use application::{
        DraftProposal, DraftProposalData, DraftProposalError, DraftProposalInputs,
        DraftProposalOutcome, EvaluateSignalInputs, EvaluateSignals, EventBusBuilder,
        EventSubscriber, ProposalRepository, ReestimateScheduler, SignalEvaluationData,
        SignalQuantityCosts, SignalRepository, StoredSignal, SubmitEstimate, SubscriberFailure,
        SystemPortError,
    };
    use domain::{
        AccountId, BuySignalOccurrenceId, Calibration, CalibrationSnapshot, CheckResult,
        ClientOrderId, CommissionRateFact, CommissionValidity, CostRate, Currency, InstrumentId,
        InvestorProfile, IssuedEstimateTaskId, Market, MasterInstrument, MasterInstrumentFacts,
        OrderFxRates, Orderbook, OrderbookInput, OrderbookLevelInput, Policy, PriceLimits,
        PriceLimitsInput, ProposalId, SectorAssignments, SectorCatalog, SectorComposition,
        SectorDepth, SectorId, SectorMetadataRow, SectorStockRow, Timestamp, ValidationCheck,
    };
    use std::{
        future::Future,
        sync::{Arc, Barrier, RwLock},
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::MemorySignalEvaluationRepository;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("가짜는 즉시 완료"),
        }
    }

    fn inputs(source: &MasterInstrument, now: Timestamp) -> EvaluateSignalInputs {
        let head = fixtures::head(0);
        let table = Calibration::try_new(fixtures::calibration_key(), &[]).expect("표");
        let portfolio = domain::Portfolio::new(
            vec![],
            domain::BuyingPower::new(fixtures::money("10000", Currency::Krw), now),
            domain::BuyingPower::new(fixtures::money("0", Currency::Usd), now),
        )
        .expect("포트")
        .evaluate(None);
        let assignments = sectors(source, now);
        let rate = CostRate::try_new(fixtures::ratio("0")).expect("수수료율");
        EvaluateSignalInputs {
            calibration: Some(CalibrationSnapshot::try_new(table, head.clone()).expect("스냅숏")),
            calibration_head: Some(head),
            valuation: Some(portfolio),
            policy: Policy::new(InvestorProfile::default()),
            assignments,
            security: MasterInstrumentFacts::try_new(vec![source.clone()])
                .expect("마스터")
                .policy_security_facts(),
            target_price: Some(
                source
                    .price_observation(fixtures::price("100"), fixtures::local(now))
                    .expect("시세"),
            ),
            calendars: (60..=66)
                .map(|day| fixtures::calendar(day, &Market::Kr))
                .collect(),
            quantity_costs: Some(SignalQuantityCosts {
                fx: OrderFxRates::missing(),
                commission: CommissionRateFact::from_commissions_result(
                    Market::Kr,
                    rate,
                    CommissionValidity::try_new(None, None).expect("유효"),
                ),
            }),
        }
    }

    fn sectors(source: &MasterInstrument, now: Timestamp) -> SectorAssignments {
        let sector = SectorId::try_new("tech").expect("분류");
        let catalog = SectorCatalog::from_response(
            vec![SectorMetadataRow {
                id: sector.clone(),
                name: domain::DisplayInstrumentName::try_new("기술").expect("이름"),
                depth: SectorDepth::Major,
                parent: None,
            }],
            now,
        )
        .expect("목록");
        let composition = SectorComposition::from_stocks_response(
            sector,
            domain::KnownMarket::Kr,
            vec![SectorStockRow {
                symbol: source.row().symbol.clone(),
                currency: Currency::Krw,
                market_capitalization: Some(domain::NumericInput::try_new("100000").expect("입력")),
                trading_amount: None,
            }],
            now,
        )
        .expect("구성");
        let assignment =
            domain::sector_assignment_from_compositions(source, &catalog, &[composition])
                .expect("배정");
        SectorAssignments::try_new(vec![assignment]).expect("분류")
    }

    fn market(source: &MasterInstrument, now: Timestamp) -> DraftProposalInputs {
        let instrument = source.quote_instrument().expect("시세 종목");
        let book = Orderbook::try_new(OrderbookInput {
            instrument: instrument.clone(),
            currency: Currency::Krw,
            asks: vec![OrderbookLevelInput {
                price: fixtures::price("100"),
                volume: fixtures::quantity("23"),
            }],
            bids: vec![OrderbookLevelInput {
                price: fixtures::price("99"),
                volume: fixtures::quantity("17"),
            }],
            timestamp: Some(now),
            collected_at: now,
        })
        .expect("호가");
        let limits = PriceLimits::try_new(PriceLimitsInput {
            instrument,
            currency: Currency::Krw,
            lower: Some(fixtures::price("50")),
            upper: Some(fixtures::price("200")),
            timestamp: now,
            collected_at: now,
        })
        .expect("가격 제한");
        DraftProposalInputs {
            evaluation: inputs(source, now),
            master: source.clone(),
            book,
            limits,
        }
    }

    struct Data(RwLock<DraftProposalInputs>);
    impl Data {
        fn new(value: DraftProposalInputs) -> Self {
            Self(RwLock::new(value))
        }
    }
    impl DraftProposalData for Data {
        fn inputs(
            &self,
            _instrument: &InstrumentId,
            _account: &AccountId,
            _current: &StoredSignal,
            _now: Timestamp,
        ) -> impl Future<Output = Result<DraftProposalInputs, SystemPortError>> + Send {
            std::future::ready(Ok(self.0.read().expect("입력").clone()))
        }
    }
    struct PairedData<'a> {
        inputs: DraftProposalInputs,
        barrier: &'a Barrier,
    }
    impl DraftProposalData for PairedData<'_> {
        fn inputs(
            &self,
            _instrument: &InstrumentId,
            _account: &AccountId,
            _current: &StoredSignal,
            _now: Timestamp,
        ) -> impl Future<Output = Result<DraftProposalInputs, SystemPortError>> + Send {
            self.barrier.wait();
            std::future::ready(Ok(self.inputs.clone()))
        }
    }
    impl SignalEvaluationData for Data {
        fn inputs(
            &self,
            _record: &domain::EstimateSubmissionRecord<
                application::AiRunId,
                application::ReasoningEffort,
            >,
            _now: Timestamp,
        ) -> impl Future<Output = Result<EvaluateSignalInputs, SystemPortError>> + Send {
            std::future::ready(Ok(self.0.read().expect("입력").evaluation.clone()))
        }
    }
    struct AdvancingData<'a> {
        inputs: DraftProposalInputs,
        clock: &'a testkit_ports::SteppedClock,
        to: Timestamp,
    }
    impl DraftProposalData for AdvancingData<'_> {
        fn inputs(
            &self,
            _instrument: &InstrumentId,
            _account: &AccountId,
            _current: &StoredSignal,
            _now: Timestamp,
        ) -> impl Future<Output = Result<DraftProposalInputs, SystemPortError>> + Send {
            self.clock.advance_to(self.to).expect("시간 전진");
            std::future::ready(Ok(self.inputs.clone()))
        }
    }
    struct HeadAdvancingData<'a> {
        inputs: DraftProposalInputs,
        repository: &'a MemorySignalEvaluationRepository,
        next: RwLock<Option<domain::CalibrationLedgerHead>>,
    }
    impl DraftProposalData for HeadAdvancingData<'_> {
        fn inputs(
            &self,
            _instrument: &InstrumentId,
            _account: &AccountId,
            _current: &StoredSignal,
            _now: Timestamp,
        ) -> impl Future<Output = Result<DraftProposalInputs, SystemPortError>> + Send {
            let next = self.next.write().expect("원장").take();
            if let Some(head) = next {
                self.repository
                    .advance_calibration_head(head)
                    .expect("위치 전진");
            }
            std::future::ready(Ok(self.inputs.clone()))
        }
    }
    #[derive(Default)]
    struct Scheduler {
        calls: RwLock<Vec<(InstrumentId, IssuedEstimateTaskId)>>,
        error: Option<SystemPortError>,
    }
    impl ReestimateScheduler for Scheduler {
        fn schedule(
            &self,
            instrument: &InstrumentId,
            task: &IssuedEstimateTaskId,
        ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send
        {
            self.calls
                .write()
                .expect("호출")
                .push((instrument.clone(), task.clone()));
            std::future::ready(self.error.map_or(Ok(application::WriteCompleted), Err))
        }
    }
    struct DraftHarness<'a> {
        repository: &'a MemorySignalEvaluationRepository,
        data: &'a Data,
        scheduler: &'a Scheduler,
        bus: &'a application::EventBus,
    }
    impl DraftHarness<'_> {
        fn execute(
            &self,
            instrument: &InstrumentId,
            at: Timestamp,
            id: ProposalId,
        ) -> Result<DraftProposalOutcome, DraftProposalError> {
            run(DraftProposal::new(
                self.repository,
                self.data,
                self.scheduler,
                &testkit_ports::SteppedClock::new(at),
                self.bus,
            )
            .execute(instrument, &AccountId::new(4321), id))
        }
    }
    struct Receiver(Arc<RwLock<Vec<domain::DomainEvent>>>);
    impl EventSubscriber for Receiver {
        fn on_event(&self, event: &domain::DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .write()
                .map_err(|_error| SubscriberFailure::Unavailable)?
                .push(event.clone());
            Ok(())
        }
    }
    struct StoredObserver {
        repository: Arc<MemorySignalEvaluationRepository>,
        observed: Arc<RwLock<Vec<ProposalId>>>,
    }
    impl EventSubscriber for StoredObserver {
        fn on_event(&self, event: &domain::DomainEvent) -> Result<(), SubscriberFailure> {
            let domain::DomainEvent::ProposalCreated(id) = event else {
                return Err(SubscriberFailure::Unavailable);
            };
            let stored =
                run(self.repository.draft(id)).map_err(|_error| SubscriberFailure::Unavailable)?;
            if stored.is_none() {
                return Err(SubscriberFailure::Unavailable);
            }
            self.observed
                .write()
                .map_err(|_error| SubscriberFailure::Unavailable)?
                .push(id.clone());
            Ok(())
        }
    }
    fn event_bus() -> (application::EventBus, Arc<RwLock<Vec<domain::DomainEvent>>>) {
        let events = Arc::new(RwLock::new(vec![]));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            [
                domain::DomainEventKind::ProposalCreated,
                domain::DomainEventKind::SignalCleared,
            ]
            .into_iter()
            .collect(),
            Receiver(Arc::clone(&events)),
        );
        (builder.build(), events)
    }
    fn seeded(source: &MasterInstrument, data: &Data) -> MemorySignalEvaluationRepository {
        let repository = MemorySignalEvaluationRepository::new(fixtures::assignments());
        repository.select_account(AccountId::new(4321));
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("위치");
        let bus = EventBusBuilder::default().build();
        submit(&repository, &bus, "task-old", source);
        run(EvaluateSignals::new(&repository, data, &bus).execute(
            &source.row().instrument_id,
            fixtures::day(60),
            BuySignalOccurrenceId::try_new("first-occurrence").expect("발생"),
        ))
        .expect("평가");
        repository
    }
    fn submit(
        repository: &MemorySignalEvaluationRepository,
        bus: &application::EventBus,
        name: &str,
        source: &MasterInstrument,
    ) {
        run(SubmitEstimate::new(repository, bus).execute(
            &fixtures::run_id(),
            vec![fixtures::input(name, "99", source)],
        ))
        .expect("제출");
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        repository.save_sources(
            snapshot.submission.expect("과제"),
            application::SignalSourceFacts {
                items: vec![fixtures::item()],
                disclosures: vec![],
                tools: vec![],
            },
        );
    }
    fn proposal_id(value: &str) -> ProposalId {
        ProposalId::new(ClientOrderId::try_new(value).expect("주문 ID"))
    }

    fn current_observation(data: &Data) -> domain::PriceObservation {
        data.0
            .read()
            .expect("입력")
            .evaluation
            .target_price
            .clone()
            .expect("시세")
    }

    fn independent_draft(
        source: &MasterInstrument,
        data: &Data,
        now: Timestamp,
        id: ProposalId,
    ) -> Box<domain::Proposal<domain::Draft>> {
        let repository = seeded(source, data);
        let bus = EventBusBuilder::default().build();
        let Ok(DraftProposalOutcome::Created { proposal, .. }) = run(DraftProposal::new(
            &repository,
            data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(&source.row().instrument_id, &AccountId::new(4321), id)) else {
            panic!("독립 초안")
        };
        proposal
    }

    fn current_buy_plan(
        repository: &MemorySignalEvaluationRepository,
        source: &MasterInstrument,
        data: &Data,
        now: Timestamp,
    ) -> domain::BuyQuantityPlan {
        let bus = EventBusBuilder::default().build();
        let result = run(EvaluateSignals::new(repository, data, &bus).execute(
            &source.row().instrument_id,
            now,
            BuySignalOccurrenceId::try_new("later-occurrence").expect("ID"),
        ))
        .expect("재평가");
        let application::EvaluateSignalOutcome::Applied {
            quantity: Some(domain::BuyQuantityDecision::Plan(plan)),
            ..
        } = result
        else {
            panic!("매수 수량 계획 필요")
        };
        *plan
    }

    fn calculated_limit(
        source: &MasterInstrument,
        now: Timestamp,
        ask: &str,
        side: domain::OrderSide,
    ) -> domain::CalculatedLimitPrice {
        let mut facts = market(source, now);
        facts.book = Orderbook::try_new(OrderbookInput {
            instrument: source.quote_instrument().expect("종목"),
            currency: Currency::Krw,
            asks: vec![OrderbookLevelInput {
                price: fixtures::price(ask),
                volume: fixtures::quantity("23"),
            }],
            bids: vec![OrderbookLevelInput {
                price: fixtures::price("98"),
                volume: fixtures::quantity("17"),
            }],
            timestamp: Some(now),
            collected_at: now,
        })
        .expect("호가");
        let table = domain::TickTable::from_master(source).expect("호가 단위");
        domain::calculate_limit_price(&facts.book, Some(&table), &facts.limits, side, now)
            .expect("지정가")
    }

    #[test]
    fn buy_intent_rejects_wrong_direction_internal_id_and_higher_limit() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let plan = current_buy_plan(&repository, &source, &data, now);
        let key = ClientOrderId::try_new("limit-test").expect("ID");
        let sell = calculated_limit(&source, now, "100", domain::OrderSide::Sell);
        assert_eq!(
            application::buy_limit_intent(&plan, &sell, AccountId::new(4321), key.clone()),
            Err(application::BuyLimitIntentError::Direction)
        );
        let mut row = source.row().clone();
        row.instrument_id = fixtures::id("other-id");
        let foreign = MasterInstrument::try_new(row).expect("다른 ID");
        let other = calculated_limit(&foreign, now, "100", domain::OrderSide::Buy);
        assert_eq!(
            application::buy_limit_intent(&plan, &other, AccountId::new(4321), key.clone()),
            Err(application::BuyLimitIntentError::Instrument)
        );
        let too_high = calculated_limit(&source, now, "101", domain::OrderSide::Buy);
        assert_eq!(
            application::buy_limit_intent(&plan, &too_high, AccountId::new(4321), key),
            Err(application::BuyLimitIntentError::RecalculateRequired)
        );
    }

    #[test]
    fn no_current_signal_returns_without_a_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = MemorySignalEvaluationRepository::new(fixtures::assignments());
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let outcome = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("no-signal"),
        ));
        let Ok(DraftProposalOutcome::NoSignal) = outcome else {
            panic!("신호 없음")
        };
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
    }

    #[test]
    fn proposal_created_subscriber_can_read_the_saved_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = Arc::new(seeded(&source, &data));
        let observed = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            std::iter::once(domain::DomainEventKind::ProposalCreated).collect(),
            StoredObserver {
                repository: Arc::clone(&repository),
                observed: Arc::clone(&observed),
            },
        );
        let result = run(DraftProposal::new(
            repository.as_ref(),
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &builder.build(),
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("event-order"),
        ))
        .expect("초안");
        let DraftProposalOutcome::Created { delivery, .. } = result else {
            panic!("초안")
        };
        assert_eq!(delivery.failures, []);
        assert_eq!(
            observed.read().expect("수신").as_slice(),
            &[proposal_id("event-order")]
        );
    }

    fn assert_open_fields(proposal: &domain::Proposal<domain::Draft>, now: Timestamp) {
        assert_eq!(proposal.id(), &proposal_id("draft-4321"));
        assert_eq!(proposal.intent().account_id(), &AccountId::new(4321));
        assert_eq!(proposal.created_at(), now);
        assert_eq!(
            proposal.expires_at(),
            fixtures::at(now.utc_seconds().as_i64().checked_add(600).expect("만료"))
        );
        assert_eq!(
            proposal.intent().symbol(),
            &domain::Symbol::try_new("005930").expect("코드")
        );
        assert_eq!(
            proposal.intent().terms(),
            &domain::OrderTerms::KrLimit {
                side: domain::OrderSide::Buy,
                quantity: domain::WholeOrderQuantity::try_new(fixtures::quantity("1"))
                    .expect("수량"),
                price: domain::KrOrderPrice::try_new(fixtures::price("100")).expect("가격"),
            }
        );
    }

    #[test]
    fn open_market_saves_draft_and_public_validation_remains_closed() {
        let source = fixtures::master();
        let domain::SignalDecision::Buy(_) = fixtures::decision(&source, "99", None) else {
            panic!("예상한 결과가 아님")
        };
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let bus = EventBusBuilder::default().build();
        let outcome = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("draft-4321"),
        ))
        .expect("초안");
        let DraftProposalOutcome::Created { proposal, .. } = outcome else {
            panic!("초안 필요")
        };
        assert_open_fields(&proposal, now);
        let observation = data
            .0
            .read()
            .expect("입력")
            .evaluation
            .target_price
            .clone()
            .expect("시세");
        let report = proposal
            .validate(now, observation)
            .expect_err("게이트 미확인");
        assert_eq!(
            report.result(ValidationCheck::Instrument),
            CheckResult::Unknown
        );
        assert_eq!(
            report.result(ValidationCheck::MarketHours),
            CheckResult::Unknown
        );
        assert_eq!(
            report.result(ValidationCheck::AvailableFundsOrQuantity),
            CheckResult::Unknown
        );
    }

    #[test]
    fn same_instrument_and_account_reuses_unexpired_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let clock = testkit_ports::SteppedClock::new(now);
        let (bus, events) = event_bus();
        let usecase = DraftProposal::new(&repository, &data, &scheduler, &clock, &bus);
        let first_id = proposal_id("active-first");
        let second_id = proposal_id("active-second");
        let Ok(DraftProposalOutcome::Created { proposal, .. }) = run(usecase.execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            first_id.clone(),
        )) else {
            panic!("첫 초안")
        };
        assert_eq!(proposal.id(), &first_id);
        data.0.write().expect("입력").evaluation.quantity_costs = None;
        let Ok(DraftProposalOutcome::Existing { proposal_id }) = run(usecase.execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            second_id.clone(),
        )) else {
            panic!("기존 초안")
        };
        assert_eq!(proposal_id, first_id);
        assert!(run(repository.draft(&second_id)).expect("조회").is_none());
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[domain::DomainEvent::ProposalCreated(first_id)]
        );
    }

    fn two_sectors(
        source: &MasterInstrument,
        other: &MasterInstrument,
        now: Timestamp,
    ) -> SectorAssignments {
        let tech = SectorId::try_new("tech").expect("기술");
        let finance = SectorId::try_new("finance").expect("금융");
        let catalog = SectorCatalog::from_response(
            vec![
                SectorMetadataRow {
                    id: tech.clone(),
                    name: domain::DisplayInstrumentName::try_new("기술").expect("이름"),
                    depth: SectorDepth::Major,
                    parent: None,
                },
                SectorMetadataRow {
                    id: finance.clone(),
                    name: domain::DisplayInstrumentName::try_new("금융").expect("이름"),
                    depth: SectorDepth::Major,
                    parent: None,
                },
            ],
            now,
        )
        .expect("목록");
        let comp = |id, symbol: &domain::Symbol| {
            SectorComposition::from_stocks_response(
                id,
                domain::KnownMarket::Kr,
                vec![SectorStockRow {
                    symbol: symbol.clone(),
                    currency: Currency::Krw,
                    market_capitalization: Some(
                        domain::NumericInput::try_new("100000").expect("입력"),
                    ),
                    trading_amount: None,
                }],
                now,
            )
            .expect("구성")
        };
        let compositions = [
            comp(tech, &source.row().symbol),
            comp(finance, &other.row().symbol),
        ];
        SectorAssignments::try_new(vec![
            domain::sector_assignment_from_compositions(source, &catalog, &compositions)
                .expect("배정"),
            domain::sector_assignment_from_compositions(other, &catalog, &compositions)
                .expect("타 배정"),
        ])
        .expect("분류")
    }

    fn low_cash_same_total_market(
        source: &MasterInstrument,
        now: Timestamp,
    ) -> DraftProposalInputs {
        let mut facts = market(source, now);
        let mut other_row = source.row().clone();
        other_row.instrument_id = fixtures::id("other-holding");
        other_row.symbol = domain::Symbol::try_new("000660").expect("코드");
        let other = MasterInstrument::try_new(other_row).expect("다른 종목");
        facts.evaluation.assignments = two_sectors(source, &other, now);
        facts.evaluation.security =
            MasterInstrumentFacts::try_new(vec![source.clone(), other.clone()])
                .expect("마스터")
                .policy_security_facts();
        let holding = domain::Holding::new(
            other.order_instrument().expect("주문 종목"),
            fixtures::quantity("1"),
            domain::AcquisitionPrice::try_new(fixtures::money("9950", Currency::Krw))
                .expect("매입가"),
            domain::HoldingObservation::new(Some(fixtures::price("9950")), None, now),
        )
        .expect("보유");
        facts.evaluation.valuation = Some(
            domain::Portfolio::new(
                vec![holding],
                domain::BuyingPower::new(fixtures::money("50", Currency::Krw), now),
                domain::BuyingPower::new(fixtures::money("0", Currency::Usd), now),
            )
            .expect("포트")
            .evaluate(None),
        );
        facts
    }

    #[test]
    fn different_account_supersedes_old_draft_even_with_same_policy_limit() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let clock = testkit_ports::SteppedClock::new(now);
        let (bus, events) = event_bus();
        let usecase = DraftProposal::new(&repository, &data, &scheduler, &clock, &bus);
        let first_id = proposal_id("acct-4321");
        let Ok(DraftProposalOutcome::Created { proposal, .. }) = run(usecase.execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            first_id.clone(),
        )) else {
            panic!("첫 계좌 초안")
        };
        assert_eq!(proposal.intent().account_id(), &AccountId::new(4321));
        repository.select_account(AccountId::new(8765));
        *data.0.write().expect("입력") = low_cash_same_total_market(&source, now);
        let second_id = proposal_id("acct-8765-low-cash");
        let second = run(usecase.execute(
            &source.row().instrument_id,
            &AccountId::new(8765),
            second_id.clone(),
        ));
        let Err(DraftProposalError::Quantity(domain::NoBuyQuantity::InsufficientBuyingPower)) =
            second
        else {
            panic!("현금 부족 계좌는 이전 계좌 초안을 재사용하지 않아야 한다: {second:?}")
        };
        assert!(run(repository.draft(&first_id)).expect("대체").is_none());
        repository.select_account(AccountId::new(4321));
        assert!(run(repository.draft(&first_id)).expect("복귀").is_none());
        repository.select_account(AccountId::new(8765));
        assert!(run(repository.draft(&second_id)).expect("조회").is_none());
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[domain::DomainEvent::ProposalCreated(first_id)]
        );
    }

    #[test]
    fn different_account_creates_new_draft_and_supersedes_previous_account() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let clock = testkit_ports::SteppedClock::new(now);
        let (bus, events) = event_bus();
        let usecase = DraftProposal::new(&repository, &data, &scheduler, &clock, &bus);
        let first_id = proposal_id("switch-4321");
        let second_id = proposal_id("switch-8765");
        let Ok(DraftProposalOutcome::Created { .. }) = run(usecase.execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            first_id.clone(),
        )) else {
            panic!("첫 계좌 초안")
        };
        repository.select_account(AccountId::new(8765));
        let Ok(DraftProposalOutcome::Created { proposal, .. }) = run(usecase.execute(
            &source.row().instrument_id,
            &AccountId::new(8765),
            second_id.clone(),
        )) else {
            panic!("전환 계좌 초안")
        };
        assert_eq!(proposal.id(), &second_id);
        assert_eq!(proposal.intent().account_id(), &AccountId::new(8765));
        assert!(run(repository.draft(&first_id)).expect("이전").is_none());
        assert!(run(repository.draft(&second_id)).expect("현재").is_some());
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[
                domain::DomainEvent::ProposalCreated(first_id),
                domain::DomainEvent::ProposalCreated(second_id),
            ]
        );
    }

    #[test]
    fn concurrent_drafts_for_one_instrument_create_and_emit_once() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let seed_data = Data::new(market(&source, now));
        let repository = seeded(&source, &seed_data);
        let barrier = Barrier::new(2);
        let data = PairedData {
            inputs: market(&source, now),
            barrier: &barrier,
        };
        let scheduler = Scheduler::default();
        let clock = testkit_ports::SteppedClock::new(now);
        let (bus, events) = event_bus();
        let first_id = proposal_id("race-first");
        let second_id = proposal_id("race-second");
        let results =
            std::thread::scope(|scope| {
                let run_one =
                    |id| {
                        run(
                            DraftProposal::new(&repository, &data, &scheduler, &clock, &bus)
                                .execute(&source.row().instrument_id, &AccountId::new(4321), id),
                        )
                        .expect("결과")
                    };
                let first_key = first_id.clone();
                let second_key = second_id.clone();
                let first = scope.spawn(move || run_one(first_key));
                let second = scope.spawn(move || run_one(second_key));
                (
                    first.join().expect("첫 호출"),
                    second.join().expect("두 호출"),
                )
            });
        let (created, existing) = match &results {
            (
                DraftProposalOutcome::Created { proposal, .. },
                DraftProposalOutcome::Existing { proposal_id },
            )
            | (
                DraftProposalOutcome::Existing { proposal_id },
                DraftProposalOutcome::Created { proposal, .. },
            ) => (proposal.id(), proposal_id),
            _ => panic!("한 초안과 기존 ID가 필요하다"),
        };
        assert_eq!(created, existing);
        let other = if created == &first_id {
            &second_id
        } else {
            &first_id
        };
        assert!(run(repository.draft(created)).expect("저장").is_some());
        assert!(run(repository.draft(other)).expect("조회").is_none());
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[domain::DomainEvent::ProposalCreated(created.clone())]
        );
    }

    #[test]
    fn concurrent_conditional_stores_atomically_choose_one_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let first = independent_draft(&source, &data, now, proposal_id("atomic-first"));
        let second = independent_draft(&source, &data, now, proposal_id("atomic-second"));
        let repository = seeded(&source, &data);
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("신호");
        let observation = current_observation(&data);
        let barrier = Barrier::new(3);
        let attempt = |proposal: &domain::Proposal<domain::Draft>| {
            barrier.wait();
            run(repository.store_current_draft(
                &source.row().instrument_id,
                &snapshot,
                proposal,
                application::DraftCheck {
                    account: &AccountId::new(4321),
                    at: now,
                    observation: &observation,
                },
            ))
            .expect("저장")
        };
        let results = std::thread::scope(|scope| {
            let one = scope.spawn(|| attempt(&first));
            let two = scope.spawn(|| attempt(&second));
            barrier.wait();
            (one.join().expect("첫 호출"), two.join().expect("두 호출"))
        });
        let winner = match &results {
            (
                application::ConditionalDraftOutcome::Stored,
                application::ConditionalDraftOutcome::Existing(id),
            ) => {
                assert_eq!(id, first.id());
                first.id()
            }
            (
                application::ConditionalDraftOutcome::Existing(id),
                application::ConditionalDraftOutcome::Stored,
            ) => {
                assert_eq!(id, second.id());
                second.id()
            }
            _ => panic!("한 저장과 같은 초안 ID가 필요하다"),
        };
        let other = if winner == first.id() {
            second.id()
        } else {
            first.id()
        };
        assert!(run(repository.draft(winner)).expect("저장").is_some());
        assert!(run(repository.draft(other)).expect("조회").is_none());
        assert_eq!(repository.draft_attempts(), 2);
    }

    #[test]
    fn unkeyed_store_cannot_bypass_instrument_uniqueness() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let proposal = independent_draft(&source, &data, now, proposal_id("unkeyed"));
        let repository = seeded(&source, &data);
        let error = SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        );
        assert_eq!(run(repository.store_draft(&proposal)), Err(error));
        assert!(
            run(repository.draft(proposal.id()))
                .expect("초안")
                .is_none()
        );
    }

    #[test]
    fn different_internal_instruments_can_have_separate_drafts() {
        let first = fixtures::master();
        let second = fixtures::other_master();
        let now = fixtures::day(60);
        let first_data = Data::new(market(&first, now));
        let repository = seeded(&first, &first_data);
        let scheduler = Scheduler::default();
        let clock = testkit_ports::SteppedClock::new(now);
        let (bus, events) = event_bus();
        let Ok(DraftProposalOutcome::Created { proposal: one, .. }) =
            run(
                DraftProposal::new(&repository, &first_data, &scheduler, &clock, &bus).execute(
                    &first.row().instrument_id,
                    &AccountId::new(4321),
                    proposal_id("instrument-first"),
                ),
            )
        else {
            panic!("첫 종목 초안")
        };
        let second_data = Data::new(market(&second, now));
        let evaluation_bus = EventBusBuilder::default().build();
        submit(&repository, &evaluation_bus, "task-other", &second);
        run(
            EvaluateSignals::new(&repository, &second_data, &evaluation_bus).execute(
                &second.row().instrument_id,
                now,
                BuySignalOccurrenceId::try_new("other-occurrence").expect("발생"),
            ),
        )
        .expect("두 번째 신호");
        let Ok(DraftProposalOutcome::Created { proposal: two, .. }) =
            run(
                DraftProposal::new(&repository, &second_data, &scheduler, &clock, &bus).execute(
                    &second.row().instrument_id,
                    &AccountId::new(4321),
                    proposal_id("instrument-second"),
                ),
            )
        else {
            panic!("다른 종목 초안")
        };
        assert_ne!(one.id(), two.id());
        assert!(run(repository.draft(one.id())).expect("첫 조회").is_some());
        assert!(
            run(repository.draft(two.id()))
                .expect("두 번째 조회")
                .is_some()
        );
        assert_eq!(events.read().expect("이벤트").len(), 2);
    }

    #[test]
    fn new_signal_snapshot_cannot_reuse_the_previous_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let clock = testkit_ports::SteppedClock::new(now);
        let (bus, events) = event_bus();
        let old_id = proposal_id("old-snapshot");
        let Ok(DraftProposalOutcome::Created { .. }) =
            run(
                DraftProposal::new(&repository, &data, &scheduler, &clock, &bus).execute(
                    &source.row().instrument_id,
                    &AccountId::new(4321),
                    old_id.clone(),
                ),
            )
        else {
            panic!("기존 초안")
        };
        let evaluation_bus = EventBusBuilder::default().build();
        submit(&repository, &evaluation_bus, "task-new", &source);
        assert!(
            run(repository.draft(&old_id))
                .expect("오래된 초안")
                .is_none()
        );
        run(
            EvaluateSignals::new(&repository, &data, &evaluation_bus).execute(
                &source.row().instrument_id,
                now,
                BuySignalOccurrenceId::try_new("updated-occurrence").expect("발생"),
            ),
        )
        .expect("재평가");
        let new_id = proposal_id("new-snapshot");
        let Ok(DraftProposalOutcome::Created { proposal, .. }) =
            run(
                DraftProposal::new(&repository, &data, &scheduler, &clock, &bus).execute(
                    &source.row().instrument_id,
                    &AccountId::new(4321),
                    new_id.clone(),
                ),
            )
        else {
            panic!("새 신호의 초안")
        };
        assert_eq!(proposal.id(), &new_id);
        assert!(run(repository.draft(&old_id)).expect("이전 초안").is_none());
        assert_eq!(events.read().expect("이벤트").len(), 2);
    }

    #[test]
    fn closed_and_unknown_market_never_write_or_discard() {
        let source = fixtures::master();
        let data = Data::new(market(&source, fixtures::day(60)));
        let repository = seeded(&source, &data);
        let before = run(repository.snapshot(&source.row().instrument_id)).expect("신호");
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let closed = fixtures::at(*fixtures::day(60).utc_seconds().as_i64() + 3601);
        let Ok(DraftProposalOutcome::MarketClosed) = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(closed),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("closed"),
        )) else {
            panic!("예상한 결과가 아님")
        };
        let Err(DraftProposalError::MarketUnknown) = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(fixtures::day(80)),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("unknown"),
        )) else {
            panic!("예상한 결과가 아님")
        };
        assert_eq!(
            run(repository.snapshot(&source.row().instrument_id)).expect("상태"),
            before
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
        assert!(scheduler.calls.read().expect("예약").is_empty());
    }

    #[test]
    fn conflicting_market_calendars_fail_closed_without_changing_signal() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let before = run(repository.snapshot(&source.row().instrument_id)).expect("신호");
        let closed = domain::MarketSession::try_new(
            now.offset_calendar_date(),
            domain::SessionInformation::Kr {
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
        )
        .expect("휴장 세션");
        let conflicting = domain::MarketCalendar::try_new(
            fixtures::session(59, &Market::Kr),
            closed,
            fixtures::session(61, &Market::Kr),
        )
        .expect("달력");
        data.0
            .write()
            .expect("입력")
            .evaluation
            .calendars
            .push(conflicting);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let result = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("conflicting-calendars"),
        ));
        let Err(DraftProposalError::MarketUnknown) = result else {
            panic!("불일치한 장 상태는 알 수 없어야 한다")
        };
        assert_eq!(
            run(repository.snapshot(&source.row().instrument_id)).expect("상태"),
            before
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
        assert!(scheduler.calls.read().expect("예약").is_empty());
    }

    fn invalid_prices(
        source: &MasterInstrument,
        now: Timestamp,
    ) -> [(Option<domain::PriceObservation>, domain::SignalUnavailable); 4] {
        let old = fixtures::at(now.utc_seconds().as_i64().checked_sub(5).expect("시각"));
        let no_id = domain::PriceObservation::from_quote_parts(
            source.order_instrument().expect("종목"),
            fixtures::price("100"),
            now,
        )
        .expect("시세");
        let mut row = source.row().clone();
        row.instrument_id = fixtures::id("other-id");
        let foreign = MasterInstrument::try_new(row).expect("다른 ID");
        [
            (None, domain::SignalUnavailable::MissingTargetPrice),
            (
                Some(
                    source
                        .price_observation(fixtures::price("100"), fixtures::local(old))
                        .expect("오래된 시세"),
                ),
                domain::SignalUnavailable::StaleTargetPrice,
            ),
            (Some(no_id), domain::SignalUnavailable::InvalidTargetPrice),
            (
                Some(
                    foreign
                        .price_observation(fixtures::price("100"), fixtures::local(now))
                        .expect("다른 ID"),
                ),
                domain::SignalUnavailable::InvalidTargetPrice,
            ),
        ]
    }

    #[test]
    fn missing_stale_unidentified_and_foreign_prices_fail_closed() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        for (price, reason) in invalid_prices(&source, now) {
            data.0.write().expect("입력").evaluation.target_price = price;
            let result = run(DraftProposal::new(
                &repository,
                &data,
                &scheduler,
                &testkit_ports::SteppedClock::new(now),
                &bus,
            )
            .execute(
                &source.row().instrument_id,
                &AccountId::new(4321),
                proposal_id("bad-price"),
            ));
            let Err(DraftProposalError::Observation(actual)) = result else {
                panic!("예상한 결과가 아님")
            };
            assert_eq!(actual, reason);
        }
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
        assert!(scheduler.calls.read().expect("예약").is_empty());
        assert!(
            run(repository.snapshot(&source.row().instrument_id))
                .expect("상태")
                .current
                .is_some()
        );
    }

    #[test]
    fn outside_estimate_range_discards_signal_before_scheduling_and_emits_clear() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        data.0.write().expect("입력").evaluation.target_price = Some(
            source
                .price_observation(fixtures::price("200"), fixtures::local(now))
                .expect("시세"),
        );
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let Ok(DraftProposalOutcome::ReestimateScheduled { .. }) = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("outside"),
        )) else {
            panic!("예상한 결과가 아님")
        };
        assert!(
            run(repository.snapshot(&source.row().instrument_id))
                .expect("상태")
                .current
                .is_none()
        );
        assert_eq!(
            scheduler.calls.read().expect("예약").as_slice(),
            &[(
                source.row().instrument_id.clone(),
                snapshot.current.expect("신호").submission
            )]
        );
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[domain::DomainEvent::SignalCleared(
                source.row().instrument_id.clone()
            )]
        );
        assert_eq!(repository.draft_attempts(), 0);
    }

    #[test]
    fn failed_draft_save_emits_no_event_and_keeps_signal() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let before = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        let error = SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        );
        repository.fail_next_draft(error);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let Err(DraftProposalError::Storage(actual)) = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("failure"),
        )) else {
            panic!("예상한 결과가 아님")
        };
        assert_eq!(actual, error);
        assert_eq!(
            run(repository.snapshot(&source.row().instrument_id)).expect("상태"),
            before
        );
        assert!(
            run(repository.draft(&proposal_id("failure")))
                .expect("조회")
                .is_none()
        );
        assert!(events.read().expect("이벤트").is_empty());
    }

    #[test]
    fn clock_advancing_during_input_read_rejects_stale_price() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let initial = market(&source, now);
        let data = Data::new(initial.clone());
        let repository = seeded(&source, &data);
        let clock = testkit_ports::SteppedClock::new(now);
        let slow = AdvancingData {
            inputs: initial,
            clock: &clock,
            to: fixtures::at(*now.utc_seconds().as_i64() + 5),
        };
        let (bus, events) = event_bus();
        let result =
            run(
                DraftProposal::new(&repository, &slow, &Scheduler::default(), &clock, &bus)
                    .execute(
                        &source.row().instrument_id,
                        &AccountId::new(4321),
                        proposal_id("slow-read"),
                    ),
            );
        let Err(DraftProposalError::Observation(domain::SignalUnavailable::StaleTargetPrice)) =
            result
        else {
            panic!("오래된 시세를 거부해야 한다")
        };
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
    }

    #[test]
    fn calibration_head_advancing_without_signal_revision_never_saves_stale_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let original = market(&source, now);
        let data = Data::new(original.clone());
        let repository = seeded(&source, &data);
        let before = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        let reads = repository.snapshot_reads();
        let changed = HeadAdvancingData {
            inputs: original,
            repository: &repository,
            next: RwLock::new(Some(fixtures::head(1))),
        };
        let (bus, events) = event_bus();
        let result = run(DraftProposal::new(
            &repository,
            &changed,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("changed-head"),
        ));
        let Err(DraftProposalError::ConcurrentUpdate) = result else {
            panic!("변경된 원장에는 저장하면 안 된다")
        };
        assert_eq!(repository.snapshot_reads() - reads, 3);
        assert_eq!(
            run(repository.snapshot(&source.row().instrument_id)).expect("상태"),
            before
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert!(
            run(repository.draft(&proposal_id("changed-head")))
                .expect("초안")
                .is_none()
        );
        assert!(events.read().expect("이벤트").is_empty());
    }

    fn sell_fixture(
        source: &MasterInstrument,
        now: Timestamp,
    ) -> (Data, MemorySignalEvaluationRepository) {
        let mut facts = market(source, now);
        let holding = domain::Holding::new(
            source.order_instrument().expect("종목"),
            fixtures::quantity("17"),
            domain::AcquisitionPrice::try_new(fixtures::money("87", Currency::Krw))
                .expect("매입가"),
            domain::HoldingObservation::new(Some(fixtures::price("100")), None, now),
        )
        .expect("보유");
        facts.evaluation.valuation = Some(
            domain::Portfolio::new(
                vec![holding],
                domain::BuyingPower::new(fixtures::money("10000", Currency::Krw), now),
                domain::BuyingPower::new(fixtures::money("0", Currency::Usd), now),
            )
            .expect("포트")
            .evaluate(None),
        );
        let data = Data::new(facts);
        let repository = MemorySignalEvaluationRepository::new(fixtures::assignments());
        repository.select_account(AccountId::new(4321));
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("위치");
        let bus = EventBusBuilder::default().build();
        run(SubmitEstimate::new(&repository, &bus).execute(
            &fixtures::run_id(),
            vec![fixtures::input("task-old", "1", source)],
        ))
        .expect("제출");
        let before = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        repository.save_sources(
            before.submission.expect("과제"),
            application::SignalSourceFacts {
                items: vec![fixtures::item()],
                disclosures: vec![],
                tools: vec![],
            },
        );
        run(EvaluateSignals::new(&repository, &data, &bus).execute(
            &source.row().instrument_id,
            now,
            BuySignalOccurrenceId::try_new("unused").expect("ID"),
        ))
        .expect("매도 평가");
        (data, repository)
    }

    #[test]
    fn sell_signal_stays_without_draft_until_sell_path_is_implemented() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let (data, repository) = sell_fixture(&source, now);
        let signal = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        let Some(StoredSignal {
            signal: domain::RaisedSignal::Sell(_),
            ..
        }) = &signal.current
        else {
            panic!("매도 신호가 필요하다")
        };
        let scheduler = Scheduler::default();
        let (draft_bus, events) = event_bus();
        let result = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &draft_bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("sell"),
        ));
        let Err(DraftProposalError::SellDraftUnavailable) = result else {
            panic!("매도 경로 미구현")
        };
        assert_eq!(
            run(repository.snapshot(&source.row().instrument_id)).expect("상태"),
            signal
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
    }

    #[test]
    fn sell_reversal_does_not_return_a_previous_buy_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let clock = testkit_ports::SteppedClock::new(now);
        let (bus, events) = event_bus();
        let old_id = proposal_id("before-sell");
        let Ok(DraftProposalOutcome::Created { .. }) =
            run(
                DraftProposal::new(&repository, &data, &scheduler, &clock, &bus).execute(
                    &source.row().instrument_id,
                    &AccountId::new(4321),
                    old_id.clone(),
                ),
            )
        else {
            panic!("기존 매수 초안")
        };
        let (sell_data, _) = sell_fixture(&source, now);
        *data.0.write().expect("입력") = sell_data.0.read().expect("매도 입력").clone();
        let evaluation_bus = EventBusBuilder::default().build();
        run(SubmitEstimate::new(&repository, &evaluation_bus).execute(
            &fixtures::run_id(),
            vec![fixtures::input("task-new", "1", &source)],
        ))
        .expect("제출");
        let submitted = run(repository.snapshot(&source.row().instrument_id)).expect("신호");
        repository.save_sources(
            submitted.submission.expect("과제"),
            application::SignalSourceFacts {
                items: vec![fixtures::item()],
                disclosures: vec![],
                tools: vec![],
            },
        );
        run(
            EvaluateSignals::new(&repository, &data, &evaluation_bus).execute(
                &source.row().instrument_id,
                now,
                BuySignalOccurrenceId::try_new("sell-reversal").expect("발생"),
            ),
        )
        .expect("매도 평가");
        let result = run(
            DraftProposal::new(&repository, &data, &scheduler, &clock, &bus).execute(
                &source.row().instrument_id,
                &AccountId::new(4321),
                proposal_id("after-sell"),
            ),
        );
        let Err(DraftProposalError::SellDraftUnavailable) = result else {
            panic!("매도 경로 미구현")
        };
        assert!(run(repository.draft(&old_id)).expect("이전 초안").is_none());
        assert_eq!(events.read().expect("이벤트").len(), 1);
    }

    #[test]
    fn conflicts_reread_entire_snapshot_and_stop_after_three_attempts() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let before_failure = repository.snapshot_reads();
        repository.conflict_next_drafts(2);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let Ok(DraftProposalOutcome::Created { .. }) = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("retry-ok"),
        )) else {
            panic!("예상한 결과가 아님")
        };
        assert_eq!(repository.snapshot_reads() - before_failure, 3);
        assert_eq!(repository.draft_attempts(), 3);
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[domain::DomainEvent::ProposalCreated(proposal_id(
                "retry-ok"
            ))]
        );
        let conflicting_repository = seeded(&source, &data);
        conflicting_repository.conflict_next_drafts(3);
        let before = conflicting_repository.snapshot_reads();
        let Err(DraftProposalError::ConcurrentUpdate) = run(DraftProposal::new(
            &conflicting_repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("retry-stop"),
        )) else {
            panic!("예상한 결과가 아님")
        };
        assert_eq!(conflicting_repository.snapshot_reads() - before, 3);
        assert!(
            run(conflicting_repository.draft(&proposal_id("retry-stop")))
                .expect("조회")
                .is_none()
        );
        assert_eq!(events.read().expect("이벤트").len(), 1);
    }

    #[test]
    fn changed_master_identity_does_not_turn_old_signal_into_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let mut row = source.row().clone();
        row.symbol = domain::Symbol::try_new("000660").expect("새 코드");
        data.0.write().expect("입력").master = MasterInstrument::try_new(row).expect("새 마스터");
        let (bus, events) = event_bus();
        let Err(DraftProposalError::InstrumentMismatch) = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("changed"),
        )) else {
            panic!("예상한 결과가 아님")
        };
        assert_eq!(repository.draft_attempts(), 0);
        assert!(
            run(repository.snapshot(&source.row().instrument_id))
                .expect("상태")
                .current
                .is_some()
        );
        assert!(events.read().expect("이벤트").is_empty());
    }

    fn identity_repository(
        old: &MasterInstrument,
        changed: &MasterInstrument,
    ) -> MemorySignalEvaluationRepository {
        let old_data = Data::new(market(old, fixtures::day(60)));
        let assignments = domain::EstimateAssignments::default()
            .issue(
                fixtures::task_id("task-old"),
                fixtures::run_id(),
                fixtures::eligible_for_master("102", old),
            )
            .expect("발급")
            .issue(
                fixtures::task_id("task-new"),
                fixtures::run_id(),
                fixtures::eligible_for_master("102", changed),
            )
            .expect("발급");
        let repository = MemorySignalEvaluationRepository::new(assignments);
        repository.select_account(AccountId::new(4321));
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("위치");
        let bus = EventBusBuilder::default().build();
        submit(&repository, &bus, "task-old", old);
        run(EvaluateSignals::new(&repository, &old_data, &bus).execute(
            &old.row().instrument_id,
            fixtures::day(60),
            BuySignalOccurrenceId::try_new("old-occurrence").expect("ID"),
        ))
        .expect("기존 신호");
        submit(&repository, &bus, "task-new", changed);
        repository
    }

    #[test]
    fn failed_a13_identity_transition_leaves_old_signal_unusable_for_drafting() {
        let old = fixtures::master();
        let now = fixtures::day(60);
        let mut row = old.row().clone();
        row.symbol = domain::Symbol::try_new("000660").expect("새 코드");
        let changed = MasterInstrument::try_new(row).expect("새 마스터");
        let repository = identity_repository(&old, &changed);
        let old_signal = run(repository.snapshot(&old.row().instrument_id))
            .expect("스냅숏")
            .current;
        let bus = EventBusBuilder::default().build();
        let changed_data = Data::new(market(&changed, now));
        let Err(application::EvaluateSignalError::Transition(
            domain::SignalTransitionError::InstrumentMismatch,
        )) = run(
            EvaluateSignals::new(&repository, &changed_data, &bus).execute(
                &old.row().instrument_id,
                now,
                BuySignalOccurrenceId::try_new("new-occurrence").expect("ID"),
            ),
        )
        else {
            panic!("예상한 결과가 아님")
        };
        assert_eq!(
            run(repository.snapshot(&old.row().instrument_id))
                .expect("스냅숏")
                .current,
            old_signal
        );
        let (draft_bus, events) = event_bus();
        let Err(DraftProposalError::SignalChanged) = run(DraftProposal::new(
            &repository,
            &changed_data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &draft_bus,
        )
        .execute(
            &old.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("stale-after-a13"),
        )) else {
            panic!("예상한 결과가 아님")
        };
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
    }

    #[test]
    fn closed_market_takes_precedence_over_buy_restriction() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let before = run(repository.snapshot(&source.row().instrument_id)).expect("신호");
        let mut row = source.row().clone();
        row.korean_detail.as_mut().expect("국내").liquidation =
            domain::LiquidationTrading::Liquidation;
        data.0.write().expect("입력").master = MasterInstrument::try_new(row).expect("마스터");
        let closed = fixtures::at(*now.utc_seconds().as_i64() + 3601);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let result = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(closed),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("closed-liquidation"),
        ));
        let Ok(DraftProposalOutcome::MarketClosed) = result else {
            panic!("닫힌 장이어야 한다")
        };
        assert_eq!(
            run(repository.snapshot(&source.row().instrument_id)).expect("상태"),
            before
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
        assert!(scheduler.calls.read().expect("예약").is_empty());
    }

    #[test]
    fn unevaluated_new_submission_is_distinguished_from_write_conflict() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let bus = EventBusBuilder::default().build();
        submit(&repository, &bus, "task-new", &source);
        let before = run(repository.snapshot(&source.row().instrument_id)).expect("신호");
        let reads = repository.snapshot_reads();
        let (draft_bus, events) = event_bus();
        let result = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &draft_bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("unevaluated"),
        ));
        let Err(DraftProposalError::SignalChanged) = result else {
            panic!("이전 신호와 새 제출의 불일치여야 한다")
        };
        assert_eq!(repository.snapshot_reads() - reads, 3);
        assert_eq!(
            run(repository.snapshot(&source.row().instrument_id)).expect("상태"),
            before
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
    }

    #[test]
    fn stale_snapshot_cannot_commit_even_if_a_valid_draft_exists() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let stale = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        let bus = EventBusBuilder::default().build();
        let result = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("stale-copy"),
        ))
        .expect("초안");
        let DraftProposalOutcome::Created { proposal, .. } = result else {
            panic!("초안")
        };
        run(application::SignalInvalidationRepository::invalidate(
            &repository,
            &source.row().instrument_id,
            &stale,
        ))
        .expect("비활성화");
        let observation = current_observation(&data);
        assert_eq!(
            run(repository.store_current_draft(
                &source.row().instrument_id,
                &stale,
                &proposal,
                application::DraftCheck {
                    account: &AccountId::new(4321),
                    at: now,
                    observation: &observation,
                },
            )),
            Ok(application::ConditionalDraftOutcome::Conflict)
        );
        assert_eq!(
            run(repository.current_draft(
                &source.row().instrument_id,
                &stale,
                application::DraftCheck {
                    account: &AccountId::new(4321),
                    at: now,
                    observation: &observation,
                },
            )),
            Ok(application::CurrentDraftOutcome::Conflict)
        );
        assert!(
            run(repository.snapshot(&source.row().instrument_id))
                .expect("상태")
                .current
                .is_none()
        );
    }

    #[test]
    fn current_draft_rejects_same_symbol_with_a_different_internal_id() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        let mut row = source.row().clone();
        row.instrument_id = fixtures::id("different-internal-id");
        let other = MasterInstrument::try_new(row).expect("다른 내부 ID");
        let observation = other
            .price_observation(fixtures::price("100"), fixtures::local(now))
            .expect("관측");
        let result = run(repository.current_draft(
            &source.row().instrument_id,
            &snapshot,
            application::DraftCheck {
                account: &AccountId::new(4321),
                at: now,
                observation: &observation,
            },
        ));
        assert_eq!(
            result,
            Ok(application::CurrentDraftOutcome::Invalid(
                domain::ProposalExecutionError::ObservationInvalid,
            ))
        );
        assert_eq!(repository.draft_attempts(), 0);
    }

    #[test]
    fn earlier_clock_never_replaces_a_future_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let later = fixtures::at(*now.utc_seconds().as_i64() + 1);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        *data.0.write().expect("입력") = market(&source, later);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let harness = DraftHarness {
            repository: &repository,
            data: &data,
            scheduler: &scheduler,
            bus: &bus,
        };
        let id = proposal_id("future-draft");
        let Ok(DraftProposalOutcome::Created { .. }) =
            harness.execute(&source.row().instrument_id, later, id.clone())
        else {
            panic!("초안")
        };
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        let observation = source
            .price_observation(fixtures::price("100"), fixtures::local(now))
            .expect("이전 관측");
        let result = run(repository.current_draft(
            &source.row().instrument_id,
            &snapshot,
            application::DraftCheck {
                account: &AccountId::new(4321),
                at: now,
                observation: &observation,
            },
        ));
        assert_eq!(
            result,
            Ok(application::CurrentDraftOutcome::Invalid(
                domain::ProposalExecutionError::ClockRollback,
            ))
        );
        assert!(run(repository.draft(&id)).expect("보존").is_some());
        assert_eq!(events.read().expect("이벤트").len(), 1);
    }

    #[test]
    fn scheduler_failure_is_returned_after_the_discard_without_creating_a_proposal() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        data.0.write().expect("입력").evaluation.target_price = Some(
            source
                .price_observation(fixtures::price("200"), fixtures::local(now))
                .expect("시세"),
        );
        let error = SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        );
        let scheduler = Scheduler {
            error: Some(error),
            ..Scheduler::default()
        };
        let (bus, events) = event_bus();
        let Err(DraftProposalError::Schedule { error: actual, .. }) = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("schedule-error"),
        )) else {
            panic!("예상한 결과가 아님")
        };
        assert_eq!(actual, error);
        assert!(
            run(repository.snapshot(&source.row().instrument_id))
                .expect("상태")
                .current
                .is_none()
        );
        assert_eq!(scheduler.calls.read().expect("예약").len(), 1);
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[domain::DomainEvent::SignalCleared(
                source.row().instrument_id.clone()
            )]
        );
        assert_eq!(repository.draft_attempts(), 0);
    }

    fn later_market(
        source: &MasterInstrument,
        later: Timestamp,
    ) -> (DraftProposalInputs, domain::PriceObservation) {
        let mut fresh = market(source, later);
        let observation = source
            .price_observation(fixtures::price("99.99"), fixtures::local(later))
            .expect("새 시세");
        fresh.evaluation.target_price = Some(observation.clone());
        fresh.book = Orderbook::try_new(OrderbookInput {
            instrument: source.quote_instrument().expect("종목"),
            currency: Currency::Krw,
            asks: vec![OrderbookLevelInput {
                price: fixtures::price("99"),
                volume: fixtures::quantity("23"),
            }],
            bids: vec![OrderbookLevelInput {
                price: fixtures::price("98"),
                volume: fixtures::quantity("17"),
            }],
            timestamp: Some(later),
            collected_at: later,
        })
        .expect("새 호가");
        (fresh, observation)
    }

    fn market_at_price(
        source: &MasterInstrument,
        now: Timestamp,
        ask: &str,
        bid: &str,
    ) -> DraftProposalInputs {
        let mut result = market(source, now);
        result.evaluation.valuation = Some(
            domain::Portfolio::new(
                vec![],
                domain::BuyingPower::new(fixtures::money("100000", Currency::Krw), now),
                domain::BuyingPower::new(fixtures::money("0", Currency::Usd), now),
            )
            .expect("포트")
            .evaluate(None),
        );
        result.evaluation.target_price = Some(
            source
                .price_observation(fixtures::price(ask), fixtures::local(now))
                .expect("시세"),
        );
        result.book = Orderbook::try_new(OrderbookInput {
            instrument: source.quote_instrument().expect("종목"),
            currency: Currency::Krw,
            asks: vec![OrderbookLevelInput {
                price: fixtures::price(ask),
                volume: fixtures::quantity("23"),
            }],
            bids: vec![OrderbookLevelInput {
                price: fixtures::price(bid),
                volume: fixtures::quantity("17"),
            }],
            timestamp: Some(now),
            collected_at: now,
        })
        .expect("호가");
        result
    }

    #[test]
    fn price_expired_draft_is_replaced_and_cannot_reappear() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market_at_price(&source, now, "102", "101"));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let old_id = proposal_id("price-old");
        let first = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            old_id.clone(),
        ));
        let Ok(DraftProposalOutcome::Created { proposal: old, .. }) = first else {
            panic!("기존 초안")
        };
        let later = fixtures::at(*now.utc_seconds().as_i64() + 1);
        let fresh = market_at_price(&source, later, "98", "97");
        let observation = fresh.evaluation.target_price.clone().expect("시세");
        assert_eq!(
            old.report(later, &observation)
                .result(ValidationCheck::PriceRange),
            CheckResult::Failed
        );
        *data.0.write().expect("입력") = fresh;
        let new_id = proposal_id("price-new");
        let Ok(DraftProposalOutcome::Created { proposal: new, .. }) = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(later),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            new_id.clone(),
        )) else {
            panic!("새 초안")
        };
        assert_eq!(new.id(), &new_id);
        assert!(run(repository.draft(&old_id)).expect("이전 초안").is_none());
        assert_eq!(events.read().expect("이벤트").len(), 2);
    }

    #[test]
    fn price_expiry_survives_failed_replacement_and_price_reversal() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market_at_price(&source, now, "102", "101"));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let harness = DraftHarness {
            repository: &repository,
            data: &data,
            scheduler: &scheduler,
            bus: &bus,
        };
        let old_id = proposal_id("expired-before-failure");
        let Ok(DraftProposalOutcome::Created { .. }) =
            harness.execute(&source.row().instrument_id, now, old_id.clone())
        else {
            panic!("첫 초안")
        };
        let moved = fixtures::at(*now.utc_seconds().as_i64() + 1);
        *data.0.write().expect("입력") = market_at_price(&source, moved, "98", "97");
        let error = SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        );
        repository.fail_next_draft(error);
        let Err(DraftProposalError::Storage(actual)) = harness.execute(
            &source.row().instrument_id,
            moved,
            proposal_id("failed-replacement"),
        ) else {
            panic!("저장 실패")
        };
        assert_eq!(actual, error);
        assert!(run(repository.draft(&old_id)).expect("만료 초안").is_none());
        let reverted = fixtures::at(*now.utc_seconds().as_i64() + 2);
        *data.0.write().expect("입력") = market_at_price(&source, reverted, "102", "101");
        let Ok(DraftProposalOutcome::Created { proposal, .. }) = harness.execute(
            &source.row().instrument_id,
            reverted,
            proposal_id("after-reversal"),
        ) else {
            panic!("되돌아온 가격의 새 초안")
        };
        assert_eq!(proposal.id(), &proposal_id("after-reversal"));
        assert_eq!(events.read().expect("이벤트").len(), 2);
    }

    #[test]
    fn expired_draft_keeps_signal_and_rebuilds_with_new_observation() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let first = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("old"),
        ))
        .expect("초안");
        let DraftProposalOutcome::Created { proposal: old, .. } = first else {
            panic!("초안")
        };
        let later = fixtures::at(*now.utc_seconds().as_i64() + 601);
        let (fresh, observation) = later_market(&source, later);
        *data.0.write().expect("입력") = fresh;
        let second = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(later),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("new"),
        ))
        .expect("새 초안");
        let DraftProposalOutcome::Created { proposal: new, .. } = second else {
            panic!("초안")
        };
        assert_eq!(
            old.report(later, &observation)
                .result(ValidationCheck::Expiry),
            CheckResult::Failed
        );
        assert_eq!(
            new.report(later, &observation)
                .result(ValidationCheck::Expiry),
            CheckResult::Passed
        );
        assert_ne!(old.price_range(), new.price_range());
        assert_eq!(new.created_at(), later);
        let current = run(repository.snapshot(&source.row().instrument_id))
            .expect("상태")
            .current;
        assert!(current.is_some());
        assert!(scheduler.calls.read().expect("예약").is_empty());
        assert_eq!(events.read().expect("이벤트").len(), 2);
    }

    #[test]
    fn exact_expiry_allows_one_new_draft_with_latest_price() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let harness = DraftHarness {
            repository: &repository,
            data: &data,
            scheduler: &scheduler,
            bus: &bus,
        };
        let first_id = proposal_id("boundary-old");
        let Ok(DraftProposalOutcome::Created { proposal: old, .. }) =
            harness.execute(&source.row().instrument_id, now, first_id.clone())
        else {
            panic!("첫 초안")
        };
        let before = fixtures::at(*now.utc_seconds().as_i64() + 599);
        let (before_input, _) = later_market(&source, before);
        *data.0.write().expect("입력") = before_input;
        let Ok(DraftProposalOutcome::Existing {
            proposal_id: existing_id,
        }) = harness.execute(
            &source.row().instrument_id,
            before,
            proposal_id("boundary-before"),
        )
        else {
            panic!("만료 전 기존 초안")
        };
        assert_eq!(existing_id, first_id);
        let expiry = old.expires_at();
        let (expiry_input, observation) = later_market(&source, expiry);
        *data.0.write().expect("입력") = expiry_input;
        let Ok(DraftProposalOutcome::Created { proposal: new, .. }) = harness.execute(
            &source.row().instrument_id,
            expiry,
            proposal_id("boundary-new"),
        ) else {
            panic!("만료 직후 새 초안")
        };
        assert_eq!(new.created_at(), expiry);
        assert_eq!(
            old.report(expiry, &observation)
                .result(ValidationCheck::Expiry),
            CheckResult::Failed
        );
        assert_eq!(
            new.report(expiry, &observation)
                .result(ValidationCheck::Expiry),
            CheckResult::Passed
        );
        assert_ne!(old.price_range(), new.price_range());
        assert_eq!(events.read().expect("이벤트").len(), 2);
    }

    #[test]
    fn store_current_draft_rejects_opposite_side_for_signal() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let buy_data = Data::new(market(&source, now));
        let buy_draft = independent_draft(&source, &buy_data, now, proposal_id("buy-for-sell"));
        let (sell_data, sell_repository) = sell_fixture(&source, now);
        let sell_snapshot =
            run(sell_repository.snapshot(&source.row().instrument_id)).expect("매도");
        let observation = current_observation(&sell_data);
        let error = SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        );
        assert_eq!(
            run(sell_repository.store_current_draft(
                &source.row().instrument_id,
                &sell_snapshot,
                &buy_draft,
                application::DraftCheck {
                    account: &AccountId::new(4321),
                    at: now,
                    observation: &observation,
                },
            )),
            Err(error)
        );
        assert!(
            run(sell_repository.draft(buy_draft.id()))
                .expect("조회")
                .is_none()
        );
    }

    #[test]
    fn store_current_draft_rejects_mismatched_account() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let draft = independent_draft(&source, &data, now, proposal_id("wrong-account"));
        let repository = seeded(&source, &data);
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("상태");
        let observation = current_observation(&data);
        let error = SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        );
        assert_eq!(
            run(repository.store_current_draft(
                &source.row().instrument_id,
                &snapshot,
                &draft,
                application::DraftCheck {
                    account: &AccountId::new(8765),
                    at: now,
                    observation: &observation,
                },
            )),
            Err(error)
        );
        assert!(run(repository.draft(draft.id())).expect("조회").is_none());
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum PreemptAt {
        Read,
        Store,
    }
    struct WinnerDraft {
        instrument: InstrumentId,
        snapshot: application::SignalSnapshot,
        proposal: domain::Proposal<domain::Draft>,
        observation: domain::PriceObservation,
        at: Timestamp,
    }
    struct PreemptingRepository<'a> {
        inner: &'a MemorySignalEvaluationRepository,
        clock: &'a testkit_ports::SteppedClock,
        winner: RwLock<Option<WinnerDraft>>,
        preempt_at: PreemptAt,
        advance_to: Option<Timestamp>,
    }
    impl PreemptingRepository<'_> {
        async fn apply_pending(&self, pending: Option<WinnerDraft>) -> Result<(), SystemPortError> {
            let Some(item) = pending else {
                return Ok(());
            };
            let check = application::DraftCheck {
                account: item.proposal.intent().account_id(),
                at: item.at,
                observation: &item.observation,
            };
            let stored = self
                .inner
                .store_current_draft(&item.instrument, &item.snapshot, &item.proposal, check)
                .await?;
            assert_eq!(stored, application::ConditionalDraftOutcome::Stored);
            if let Some(target) = self.advance_to {
                self.clock.advance_to(target).expect("시계 전진");
            }
            Ok(())
        }
        fn take_for(&self, stage: PreemptAt) -> Option<WinnerDraft> {
            if self.preempt_at == stage {
                self.winner.write().expect("선점").take()
            } else {
                None
            }
        }
    }
    impl SignalRepository for PreemptingRepository<'_> {
        fn snapshot(
            &self,
            instrument: &InstrumentId,
        ) -> impl Future<Output = Result<application::SignalSnapshot, SystemPortError>> + Send
        {
            self.inner.snapshot(instrument)
        }
        fn commit(
            &self,
            instrument: &InstrumentId,
            expected: &application::SignalSnapshot,
            mutation: &application::SignalMutation,
        ) -> impl Future<Output = Result<application::SignalCommitOutcome, SystemPortError>> + Send
        {
            self.inner.commit(instrument, expected, mutation)
        }
        fn occurrences(
            &self,
            instrument: &InstrumentId,
            identity: &domain::ResolvedOrderInstrument,
        ) -> impl Future<Output = Result<Vec<domain::BuySignalRecord>, SystemPortError>> + Send
        {
            self.inner.occurrences(instrument, identity)
        }
        fn sources(
            &self,
            task: &IssuedEstimateTaskId,
        ) -> impl Future<Output = Result<application::SignalSourceFacts, SystemPortError>> + Send
        {
            self.inner.sources(task)
        }
        fn current(
            &self,
            instrument: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<domain::RaisedSignal>, SystemPortError>> + Send
        {
            self.inner.current(instrument)
        }
    }
    impl application::EstimateSubmissionRepository for PreemptingRepository<'_> {
        fn submission_records(
            &self,
            instrument: &InstrumentId,
        ) -> impl Future<
            Output = Result<
                Vec<
                    domain::EstimateSubmissionRecord<
                        application::AiRunId,
                        application::ReasoningEffort,
                    >,
                >,
                SystemPortError,
            >,
        > + Send {
            self.inner.submission_records(instrument)
        }
        fn assignments(
            &self,
        ) -> impl Future<
            Output = Result<domain::EstimateAssignments<application::AiRunId>, SystemPortError>,
        > + Send {
            self.inner.assignments()
        }
        fn commit_submission(
            &self,
            previous: &domain::EstimateAssignments<application::AiRunId>,
            next: &domain::EstimateAssignments<application::AiRunId>,
            records: &[domain::EstimateSubmissionRecord<
                application::AiRunId,
                application::ReasoningEffort,
            >],
        ) -> impl Future<Output = Result<application::EstimateCommitOutcome, SystemPortError>> + Send
        {
            self.inner.commit_submission(previous, next, records)
        }
    }
    impl ProposalRepository for PreemptingRepository<'_> {
        fn store_draft(
            &self,
            proposal: &domain::Proposal<domain::Draft>,
        ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send
        {
            self.inner.store_draft(proposal)
        }
        fn draft(
            &self,
            proposal: &ProposalId,
        ) -> impl Future<Output = Result<Option<domain::Proposal<domain::Draft>>, SystemPortError>> + Send
        {
            self.inner.draft(proposal)
        }
        fn current_draft(
            &self,
            instrument: &InstrumentId,
            expected: &application::SignalSnapshot,
            check: application::DraftCheck<'_>,
        ) -> impl Future<Output = Result<application::CurrentDraftOutcome, SystemPortError>> + Send
        {
            let pending = self.take_for(PreemptAt::Read);
            async move {
                self.apply_pending(pending).await?;
                self.inner.current_draft(instrument, expected, check).await
            }
        }
        fn store_current_draft(
            &self,
            instrument: &InstrumentId,
            expected: &application::SignalSnapshot,
            proposal: &domain::Proposal<domain::Draft>,
            check: application::DraftCheck<'_>,
        ) -> impl Future<Output = Result<application::ConditionalDraftOutcome, SystemPortError>> + Send
        {
            let pending = self.take_for(PreemptAt::Store);
            async move {
                self.apply_pending(pending).await?;
                self.inner
                    .store_current_draft(instrument, expected, proposal, check)
                    .await
            }
        }
    }

    fn draft_at_later_time(
        source: &MasterInstrument,
        seed_data: &Data,
        later_data: &Data,
        later: Timestamp,
        id: ProposalId,
    ) -> domain::Proposal<domain::Draft> {
        let repository = seeded(source, seed_data);
        let bus = EventBusBuilder::default().build();
        let Ok(DraftProposalOutcome::Created { proposal, .. }) = run(DraftProposal::new(
            &repository,
            later_data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(later),
            &bus,
        )
        .execute(&source.row().instrument_id, &AccountId::new(4321), id)) else {
            panic!("선점 초안")
        };
        *proposal
    }

    #[test]
    fn concurrent_monotonic_clock_race_returns_existing_and_rollback_fails_closed() {
        let source = fixtures::master();
        let id = &source.row().instrument_id;
        let now = fixtures::day(60);
        let winner_at = fixtures::at(now.utc_seconds().as_i64().checked_add(1).expect("시각"));
        let retry_at = fixtures::at(now.utc_seconds().as_i64().checked_add(2).expect("시각"));
        let data = Data::new(market(&source, now));
        let winner_id = proposal_id("monotonic-winner");
        let winner_data = Data::new(market(&source, winner_at));
        let winner =
            draft_at_later_time(&source, &data, &winner_data, winner_at, winner_id.clone());
        let repository = seeded(&source, &data);
        let snapshot = run(repository.snapshot(id)).expect("스냅숏");
        let clock = testkit_ports::SteppedClock::new(now);
        let racing = PreemptingRepository {
            inner: &repository,
            clock: &clock,
            winner: RwLock::new(Some(WinnerDraft {
                instrument: id.clone(),
                snapshot,
                proposal: winner,
                observation: current_observation(&winner_data),
                at: winner_at,
            })),
            preempt_at: PreemptAt::Store,
            advance_to: Some(retry_at),
        };
        let (bus, events) = event_bus();
        let scheduler = Scheduler::default();
        let loser = proposal_id("monotonic-loser");
        let outcome = run(
            DraftProposal::new(&racing, &data, &scheduler, &clock, &bus).execute(
                id,
                &AccountId::new(4321),
                loser.clone(),
            ),
        )
        .expect("재시도 뒤 기존 초안");
        let DraftProposalOutcome::Existing { proposal_id: seen } = outcome else {
            panic!("선점된 기존 초안 ID가 필요하다")
        };
        assert_eq!(seen, winner_id);
        assert!(events.read().expect("이벤트").is_empty());
        assert!(run(repository.draft(&loser)).expect("조회").is_none());
        let reads_before = repository.snapshot_reads();
        let harness = DraftHarness {
            repository: &repository,
            data: &data,
            scheduler: &scheduler,
            bus: &bus,
        };
        let rollback = harness.execute(id, now, proposal_id("rollback-loser"));
        assert_eq!(
            rollback.expect_err("시계 역행 거부"),
            DraftProposalError::DraftStatus(domain::ProposalExecutionError::ClockRollback)
        );
        assert_eq!(
            repository.snapshot_reads().checked_sub(reads_before),
            Some(3)
        );
        assert!(run(repository.draft(&winner_id)).expect("보존").is_some());
    }

    #[test]
    fn read_path_monotonic_clock_race_retries_to_existing() {
        let source = fixtures::master();
        let id = &source.row().instrument_id;
        let now = fixtures::day(60);
        let winner_at = fixtures::at(now.utc_seconds().as_i64().checked_add(1).expect("시각"));
        let retry_at = fixtures::at(now.utc_seconds().as_i64().checked_add(2).expect("시각"));
        let data = Data::new(market(&source, now));
        let winner_id = proposal_id("read-race-winner");
        let winner_data = Data::new(market(&source, winner_at));
        let winner =
            draft_at_later_time(&source, &data, &winner_data, winner_at, winner_id.clone());
        let repository = seeded(&source, &data);
        let snapshot = run(repository.snapshot(id)).expect("스냅숏");
        let reads_before = repository.snapshot_reads();
        let clock = testkit_ports::SteppedClock::new(now);
        let racing = PreemptingRepository {
            inner: &repository,
            clock: &clock,
            winner: RwLock::new(Some(WinnerDraft {
                instrument: id.clone(),
                snapshot,
                proposal: winner,
                observation: current_observation(&winner_data),
                at: winner_at,
            })),
            preempt_at: PreemptAt::Read,
            advance_to: Some(retry_at),
        };
        let (bus, events) = event_bus();
        let loser = proposal_id("read-race-loser");
        let Ok(DraftProposalOutcome::Existing { proposal_id: seen }) = run(DraftProposal::new(
            &racing,
            &data,
            &Scheduler::default(),
            &clock,
            &bus,
        )
        .execute(id, &AccountId::new(4321), loser.clone())) else {
            panic!("조회 경로 재시도 뒤 기존 초안 ID가 필요하다")
        };
        assert_eq!(seen, winner_id);
        assert_eq!(
            repository.snapshot_reads().checked_sub(reads_before),
            Some(2)
        );
        assert_eq!(repository.draft_attempts(), 1);
        assert!(events.read().expect("이벤트").is_empty());
        assert!(run(repository.draft(&loser)).expect("조회").is_none());
    }

    #[test]
    fn persistent_read_path_clock_rollback_retries_three_times() {
        let source = fixtures::master();
        let id = &source.row().instrument_id;
        let now = fixtures::day(60);
        let winner_at = fixtures::at(now.utc_seconds().as_i64().checked_add(1).expect("시각"));
        let data = Data::new(market(&source, now));
        let winner_id = proposal_id("read-stuck-winner");
        let winner_data = Data::new(market(&source, winner_at));
        let winner =
            draft_at_later_time(&source, &data, &winner_data, winner_at, winner_id.clone());
        let repository = seeded(&source, &data);
        let snapshot = run(repository.snapshot(id)).expect("스냅숏");
        let check = application::DraftCheck {
            account: &AccountId::new(4321),
            at: winner_at,
            observation: &current_observation(&winner_data),
        };
        assert_eq!(
            run(repository.store_current_draft(id, &snapshot, &winner, check)),
            Ok(application::ConditionalDraftOutcome::Stored)
        );
        let reads_before = repository.snapshot_reads();
        let (bus, events) = event_bus();
        let err = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            id,
            &AccountId::new(4321),
            proposal_id("read-rollback-stuck"),
        ))
        .expect_err("지속 시계 역행 거부");
        assert_eq!(
            err,
            DraftProposalError::DraftStatus(domain::ProposalExecutionError::ClockRollback)
        );
        assert_eq!(
            repository.snapshot_reads().checked_sub(reads_before),
            Some(3)
        );
        assert_eq!(repository.draft_attempts(), 1);
        assert!(run(repository.draft(&winner_id)).expect("보존").is_some());
        assert!(events.read().expect("이벤트").is_empty());
    }

    struct ScriptedClock {
        ticks: RwLock<std::collections::VecDeque<Timestamp>>,
        fallback: Timestamp,
    }
    impl ScriptedClock {
        fn new(ticks: impl IntoIterator<Item = Timestamp>, fallback: Timestamp) -> Self {
            Self {
                ticks: RwLock::new(ticks.into_iter().collect()),
                fallback,
            }
        }
    }
    impl application::Clock for ScriptedClock {
        fn now(&self) -> Timestamp {
            self.ticks
                .write()
                .expect("시각")
                .pop_front()
                .unwrap_or(self.fallback)
        }
    }

    #[test]
    fn pre_store_recheck_rejects_late_market_close_and_stale_price() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let closed = fixtures::at(now.utc_seconds().as_i64().checked_add(3601).expect("마감"));
        let stale = fixtures::at(now.utc_seconds().as_i64().checked_add(5).expect("만료"));
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let close_clock = ScriptedClock::new([now, now, now, closed], closed);
        let Ok(DraftProposalOutcome::MarketClosed) =
            run(
                DraftProposal::new(&repository, &data, &scheduler, &close_clock, &bus).execute(
                    &source.row().instrument_id,
                    &AccountId::new(4321),
                    proposal_id("late-close"),
                ),
            )
        else {
            panic!("저장 직전 장 마감은 닫힌 장이어야 한다")
        };
        assert_eq!(repository.draft_attempts(), 0);
        let stale_clock = ScriptedClock::new([now, now, now, stale], stale);
        assert_eq!(
            run(
                DraftProposal::new(&repository, &data, &scheduler, &stale_clock, &bus,).execute(
                    &source.row().instrument_id,
                    &AccountId::new(4321),
                    proposal_id("late-stale"),
                )
            )
            .expect_err("저장 직전 오래된 시세 거부"),
            DraftProposalError::Observation(domain::SignalUnavailable::StaleTargetPrice)
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
    }

    #[test]
    fn pre_store_check_current_rejects_local_clock_rollback_without_storing() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let later = fixtures::at(now.utc_seconds().as_i64().checked_add(1).expect("시각"));
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let reads_before = repository.snapshot_reads();
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let clock = ScriptedClock::new([now, now, later, now], now);
        assert_eq!(
            run(
                DraftProposal::new(&repository, &data, &scheduler, &clock, &bus).execute(
                    &source.row().instrument_id,
                    &AccountId::new(4321),
                    proposal_id("local-rollback"),
                )
            )
            .expect_err("저장 직전 로컬 시계 역행 거부"),
            DraftProposalError::DraftStatus(domain::ProposalExecutionError::ClockRollback)
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert_eq!(
            repository.snapshot_reads().checked_sub(reads_before),
            Some(1)
        );
        assert!(events.read().expect("이벤트").is_empty());
    }

    fn calendars_with_day60(
        market_kind: &Market,
        custom: &domain::MarketSession,
        range: std::ops::RangeInclusive<i64>,
    ) -> Vec<domain::MarketCalendar> {
        let row = |day: i64| {
            if day == 60 {
                custom.clone()
            } else {
                fixtures::session(day, market_kind)
            }
        };
        range
            .map(|day| {
                let prev = day.checked_sub(1).expect("전일");
                let next = day.checked_add(1).expect("익일");
                domain::MarketCalendar::try_new(row(prev), row(day), row(next)).expect("달력")
            })
            .collect()
    }

    #[test]
    fn kr_extended_open_sessions_create_drafts() {
        let now = fixtures::day(60);
        let kr_source = fixtures::master();
        let base_kr = fixtures::session(60, &Market::Kr);
        let kr_data =
            domain::SessionData::try_new(base_kr.sessions()[0].session().interval(), None)
                .expect("국내 세션");
        let kr_cases = [
            (
                domain::MarketState::PreMarket,
                domain::SessionInformation::Kr {
                    pre_market: Some(kr_data),
                    regular_market: None,
                    after_market: None,
                },
                "kr-pre",
            ),
            (
                domain::MarketState::AfterMarket,
                domain::SessionInformation::Kr {
                    pre_market: None,
                    regular_market: None,
                    after_market: Some(kr_data),
                },
                "kr-after",
            ),
        ];
        for (expected_state, info, id_label) in kr_cases {
            let custom = domain::MarketSession::try_new(base_kr.date(), info).expect("세션");
            let mut inputs = market(&kr_source, now);
            inputs.evaluation.calendars = calendars_with_day60(&Market::Kr, &custom, 60..=66);
            assert_eq!(
                domain::market_state_at(&inputs.evaluation.calendars, &Market::Kr, now),
                expected_state
            );
            let data = Data::new(inputs);
            let repository = seeded(&kr_source, &data);
            let (bus, _) = event_bus();
            let Ok(DraftProposalOutcome::Created { .. }) = run(DraftProposal::new(
                &repository,
                &data,
                &Scheduler::default(),
                &testkit_ports::SteppedClock::new(now),
                &bus,
            )
            .execute(
                &kr_source.row().instrument_id,
                &AccountId::new(4321),
                proposal_id(id_label),
            )) else {
                panic!("확장 세션은 초안을 만들어야 한다: {id_label}")
            };
        }
    }

    #[test]
    fn us_day_market_creates_draft() {
        let now = fixtures::day(60);
        let us_source = us_master(now);
        let base_us = fixtures::session(60, &Market::Us);
        let us_data =
            domain::SessionData::try_new(base_us.sessions()[0].session().interval(), None)
                .expect("미국 세션");
        let us_day = domain::MarketSession::try_new(
            base_us.date(),
            domain::SessionInformation::Us {
                day_market: Some(us_data),
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
        )
        .expect("데이마켓");
        let mut us_inputs = us_market(&us_source, now);
        us_inputs.evaluation.calendars = calendars_with_day60(&Market::Us, &us_day, 59..=67);
        assert_eq!(
            domain::market_state_at(&us_inputs.evaluation.calendars, &Market::Us, now),
            domain::MarketState::DayMarket
        );
        let data = Data::new(us_inputs);
        let repository = seed_us_repository(&us_source, &data, now);
        let (bus, _) = event_bus();
        let Ok(DraftProposalOutcome::Created { .. }) = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &us_source.row().instrument_id,
            &AccountId::new(7777),
            proposal_id("us-day"),
        )) else {
            panic!("미국 데이마켓은 초안을 만들어야 한다")
        };
    }

    fn us_master(now: Timestamp) -> MasterInstrument {
        MasterInstrument::try_new(domain::MasterInstrumentRow {
            listing_market: domain::MasterCode::try_new("NASDAQ").expect("시장"),
            instrument_id: fixtures::id("sample-us"),
            symbol: domain::Symbol::try_new("AAPL").expect("코드"),
            name: domain::DisplayInstrumentName::try_new("미국 종목").expect("이름"),
            security_type: domain::MasterSecurityType::from_code(
                domain::MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Usd,
            listing_status: domain::ListingStatus::Active,
            common_share: domain::CommonShare::Common,
            korean_detail: None,
            observed_at: now,
        })
        .expect("미국 마스터")
    }

    fn us_local(time: Timestamp) -> Timestamp {
        Timestamp::from_parts(
            time.utc_seconds(),
            time.nanosecond(),
            domain::OffsetSeconds::new(-18_000),
        )
        .expect("미국 시각")
    }

    fn us_price(value: &str) -> domain::Price {
        domain::Price::parse(
            &domain::NumericInput::try_new(value).expect("입력"),
            Currency::Usd,
        )
        .expect("달러 가격")
    }

    fn us_candles(source: &MasterInstrument) -> Vec<domain::Candle> {
        (-1_i64..60)
            .map(|index| {
                let fixture_price = us_price(if index % 2 == 0 { "100" } else { "102" });
                let midnight = us_local(fixtures::at(
                    fixtures::day(index)
                        .utc_seconds()
                        .as_i64()
                        .checked_add(18_000)
                        .expect("현지 자정"),
                ));
                domain::Candle::try_new(
                    source.row().instrument_id.clone(),
                    domain::CandleTime::try_new(midnight, domain::CandlePeriod::Daily)
                        .expect("일봉 시각"),
                    domain::PriceAdjustment::Adjusted(domain::AdjustmentBasis::new(fixtures::day(
                        61,
                    ))),
                    domain::Ohlcv::try_new(
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        fixtures::quantity("13"),
                    )
                    .expect("OHLCV"),
                    domain::CandleObservation::new(
                        fixtures::day(61),
                        domain::CandleStatus::Finalized,
                    ),
                )
                .expect("일봉")
            })
            .collect()
    }

    fn us_eligible(source: &MasterInstrument) -> domain::CostEligibleTask {
        let volatility = domain::Volatility::calculate(&us_candles(source)).expect("변동성");
        let task = domain::EstimateTask::create(
            domain::EstimateContext::new(
                source
                    .price_observation(us_price("100"), us_local(fixtures::day(60)))
                    .expect("관측"),
                fixtures::day(60),
            ),
            volatility,
            domain::Horizon::new(domain::HorizonSelection::FewDays),
            domain::HoursPermission::default(),
            domain::RiskAppetite::Neutral,
        )
        .expect("과제");
        let zero = CostRate::try_new(fixtures::ratio("0")).expect("비용");
        let end = fixtures::at(
            fixtures::day(65)
                .utc_seconds()
                .as_i64()
                .checked_add(3600)
                .expect("끝"),
        );
        let period = domain::EstimateCostPeriod::from_calendar_result(&task, end).expect("만기");
        let cost = domain::TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                Market::Us,
                zero,
                CommissionValidity::try_new(None, None).expect("유효"),
            )),
            Some(domain::TradeTaxFact::from_schedule_result(
                Market::Us,
                zero,
                zero,
            )),
            period,
        )
        .expect("비용");
        domain::CostEligibleTask::assess(task, &cost).expect("적격")
    }

    fn us_fx_rates(now: Timestamp) -> (domain::FxRate, domain::FxRate) {
        let valid = domain::TimeInterval::try_new(
            fixtures::at(now.utc_seconds().as_i64().checked_sub(60).expect("시작")),
            fixtures::at(now.utc_seconds().as_i64().checked_add(3600).expect("끝")),
        )
        .expect("환율 구간");
        let buy_fx = domain::FxRate::parse(
            &domain::NumericInput::try_new("1400").expect("입력"),
            domain::FxPair::UsdKrw,
            domain::FxRateKind::Buy,
            valid,
            domain::FxSource::TossOpenApi,
        )
        .expect("매수 환율");
        let mid_fx = domain::FxRate::parse(
            &domain::NumericInput::try_new("1390").expect("입력"),
            domain::FxPair::UsdKrw,
            domain::FxRateKind::Mid,
            valid,
            domain::FxSource::TossOpenApi,
        )
        .expect("기준 환율");
        (buy_fx, mid_fx)
    }

    fn us_sectors(source: &MasterInstrument, now: Timestamp) -> SectorAssignments {
        let sector = SectorId::try_new("us-tech").expect("분류");
        let catalog = SectorCatalog::from_response(
            vec![SectorMetadataRow {
                id: sector.clone(),
                name: domain::DisplayInstrumentName::try_new("미국 기술").expect("이름"),
                depth: SectorDepth::Major,
                parent: None,
            }],
            now,
        )
        .expect("목록");
        let composition = SectorComposition::from_stocks_response(
            sector,
            domain::KnownMarket::Us,
            vec![SectorStockRow {
                symbol: source.row().symbol.clone(),
                currency: Currency::Krw,
                market_capitalization: Some(domain::NumericInput::try_new("100000").expect("입력")),
                trading_amount: None,
            }],
            now,
        )
        .expect("구성");
        let assignment =
            domain::sector_assignment_from_compositions(source, &catalog, &[composition])
                .expect("배정");
        SectorAssignments::try_new(vec![assignment]).expect("분류")
    }

    fn us_book_and_limits(source: &MasterInstrument, now: Timestamp) -> (Orderbook, PriceLimits) {
        let instrument = source.quote_instrument().expect("시세 종목");
        let book = Orderbook::try_new(OrderbookInput {
            instrument: instrument.clone(),
            currency: Currency::Usd,
            asks: vec![OrderbookLevelInput {
                price: us_price("100"),
                volume: fixtures::quantity("23"),
            }],
            bids: vec![OrderbookLevelInput {
                price: us_price("99"),
                volume: fixtures::quantity("17"),
            }],
            timestamp: Some(now),
            collected_at: now,
        })
        .expect("호가");
        let limits = PriceLimits::try_new(PriceLimitsInput {
            instrument,
            currency: Currency::Usd,
            lower: None,
            upper: None,
            timestamp: now,
            collected_at: now,
        })
        .expect("가격 제한");
        (book, limits)
    }

    fn us_market(source: &MasterInstrument, now: Timestamp) -> DraftProposalInputs {
        let (buy_fx, mid_fx) = us_fx_rates(now);
        let head = fixtures::head(0);
        let table = Calibration::try_new(fixtures::calibration_key(), &[]).expect("표");
        let portfolio = domain::Portfolio::new(
            vec![],
            domain::BuyingPower::new(fixtures::money("0", Currency::Krw), now),
            domain::BuyingPower::new(fixtures::money("11000", Currency::Usd), now),
        )
        .expect("포트")
        .evaluate(Some(mid_fx));
        let (book, limits) = us_book_and_limits(source, now);
        DraftProposalInputs {
            evaluation: EvaluateSignalInputs {
                calibration: Some(
                    CalibrationSnapshot::try_new(table, head.clone()).expect("스냅숏"),
                ),
                calibration_head: Some(head),
                valuation: Some(portfolio),
                policy: Policy::new(InvestorProfile::default()),
                assignments: us_sectors(source, now),
                security: MasterInstrumentFacts::try_new(vec![source.clone()])
                    .expect("마스터")
                    .policy_security_facts(),
                target_price: Some(
                    source
                        .price_observation(us_price("100"), us_local(now))
                        .expect("시세"),
                ),
                calendars: (59..=67)
                    .map(|day| fixtures::calendar(day, &Market::Us))
                    .collect(),
                quantity_costs: Some(SignalQuantityCosts {
                    fx: OrderFxRates::from_broker_fx_result(buy_fx, mid_fx),
                    commission: CommissionRateFact::from_commissions_result(
                        Market::Us,
                        CostRate::try_new(fixtures::ratio("0")).expect("수수료율"),
                        CommissionValidity::try_new(None, None).expect("유효"),
                    ),
                }),
            },
            master: source.clone(),
            book,
            limits,
        }
    }

    fn seed_us_repository(
        source: &MasterInstrument,
        data: &Data,
        now: Timestamp,
    ) -> MemorySignalEvaluationRepository {
        let eligible = us_eligible(source);
        let assignments = domain::EstimateAssignments::default()
            .issue(
                fixtures::task_id("task-us"),
                fixtures::run_id(),
                eligible.clone(),
            )
            .expect("발급");
        let repository = MemorySignalEvaluationRepository::new(assignments);
        repository.select_account(AccountId::new(7777));
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("위치");
        let bus = EventBusBuilder::default().build();
        let part = domain::ModelIdentityPart::try_new("model-v1").expect("모델");
        run(SubmitEstimate::new(&repository, &bus).execute(
            &fixtures::run_id(),
            vec![application::EstimateSubmissionInput {
                task_id: fixtures::task_id("task-us"),
                submission: fixtures::submission(&eligible, "99"),
                narrative: domain::EstimateNarrative {
                    rationale: domain::UntrustedText::try_new("근거").expect("근거"),
                    counterargument: domain::UntrustedText::try_new("반론").expect("반론"),
                },
                metadata: domain::EstimateMetadata {
                    effort: application::ReasoningEffort::High,
                    cli_version: part.clone(),
                    prompt: domain::EstimatePrompt {
                        id: domain::ScorecardPromptId::try_new("prompt").expect("ID"),
                        version: part,
                        hash: domain::PromptHash::try_new(
                            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
                        )
                        .expect("해시"),
                    },
                    evidence_bundle: domain::EvidenceBundleId::try_new("bundle-us").expect("묶음"),
                },
            }],
        ))
        .expect("제출");
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("스냅숏");
        repository.save_sources(
            snapshot.submission.expect("과제"),
            application::SignalSourceFacts {
                items: vec![fixtures::item()],
                disclosures: vec![],
                tools: vec![],
            },
        );
        let evaluated = run(EvaluateSignals::new(&repository, data, &bus).execute(
            &source.row().instrument_id,
            now,
            BuySignalOccurrenceId::try_new("us-occurrence").expect("발생"),
        ))
        .expect("평가");
        let application::EvaluateSignalOutcome::Applied { reason: None, .. } = evaluated else {
            panic!("미국 신호 필요: {evaluated:?}")
        };
        repository
    }

    #[test]
    fn us_buy_signal_creates_us_limit_draft() {
        let now = fixtures::day(60);
        let source = us_master(now);
        let data = Data::new(us_market(&source, now));
        let repository = seed_us_repository(&source, &data, now);
        let (draft_bus, events) = event_bus();
        let id = proposal_id("us-draft-1");
        let outcome = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &draft_bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(7777),
            id.clone(),
        ))
        .expect("미국 초안");
        let DraftProposalOutcome::Created { proposal, .. } = outcome else {
            panic!("미국 매수 초안 필요")
        };
        assert_eq!(proposal.id(), &id);
        assert_eq!(proposal.intent().account_id(), &AccountId::new(7777));
        assert_eq!(
            proposal.intent().terms(),
            &domain::OrderTerms::UsLimit {
                side: domain::OrderSide::Buy,
                quantity: domain::WholeOrderQuantity::try_new(fixtures::quantity("1"))
                    .expect("수량"),
                price: domain::UsOrderPrice::try_new(us_price("100")).expect("가격"),
            }
        );
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[domain::DomainEvent::ProposalCreated(id)]
        );
    }

    struct InterleavingAccountData<'a> {
        base: DraftProposalInputs,
        repository: &'a MemorySignalEvaluationRepository,
        scheduler: &'a Scheduler,
        bus: &'a application::EventBus,
        now: Timestamp,
        instrument: InstrumentId,
        switched: RwLock<Option<Result<DraftProposalOutcome, DraftProposalError>>>,
    }
    impl DraftProposalData for InterleavingAccountData<'_> {
        fn inputs(
            &self,
            _instrument: &InstrumentId,
            account: &AccountId,
            _current: &StoredSignal,
            _now: Timestamp,
        ) -> impl Future<Output = Result<DraftProposalInputs, SystemPortError>> + Send {
            if account == &AccountId::new(4321) && self.switched.read().expect("전환").is_none() {
                self.repository.select_account(AccountId::new(8765));
                let newer = run(DraftProposal::new(
                    self.repository,
                    self,
                    self.scheduler,
                    &testkit_ports::SteppedClock::new(self.now),
                    self.bus,
                )
                .execute(
                    &self.instrument,
                    &AccountId::new(8765),
                    proposal_id("fresh-8765"),
                ));
                *self.switched.write().expect("결과") = Some(newer);
            }
            std::future::ready(Ok(self.base.clone()))
        }
    }

    #[test]
    fn stale_account_request_after_account_switch_never_supersedes_new_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let seed_data = Data::new(market(&source, now));
        let repository = seeded(&source, &seed_data);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let interleaving = InterleavingAccountData {
            base: market(&source, now),
            repository: &repository,
            scheduler: &scheduler,
            bus: &bus,
            now,
            instrument: source.row().instrument_id.clone(),
            switched: RwLock::new(None),
        };
        let older = run(DraftProposal::new(
            &repository,
            &interleaving,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("stale-4321"),
        ));
        assert_eq!(
            older.expect_err("옛 계좌"),
            DraftProposalError::AccountChanged
        );
        assert_eq!(repository.draft_attempts(), 1);
        let newer = interleaving
            .switched
            .write()
            .expect("결과")
            .take()
            .expect("새 계좌 실행");
        let Ok(DraftProposalOutcome::Created { proposal, .. }) = newer else {
            panic!("새 계좌 초안이 생성돼야 한다")
        };
        assert_eq!(proposal.id(), &proposal_id("fresh-8765"));
        assert_eq!(proposal.intent().account_id(), &AccountId::new(8765));
        assert!(
            run(repository.draft(&proposal_id("fresh-8765")))
                .expect("새 초안")
                .is_some()
        );
        assert!(
            run(repository.draft(&proposal_id("stale-4321")))
                .expect("옛 초안")
                .is_none()
        );
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[domain::DomainEvent::ProposalCreated(proposal_id(
                "fresh-8765"
            ))]
        );
    }

    struct SwitchBeforeStoreRepository<'a> {
        inner: &'a MemorySignalEvaluationRepository,
        switch_to: AccountId,
    }
    impl SignalRepository for SwitchBeforeStoreRepository<'_> {
        fn snapshot(
            &self,
            instrument: &InstrumentId,
        ) -> impl Future<Output = Result<application::SignalSnapshot, SystemPortError>> + Send
        {
            self.inner.snapshot(instrument)
        }
        fn commit(
            &self,
            instrument: &InstrumentId,
            expected: &application::SignalSnapshot,
            mutation: &application::SignalMutation,
        ) -> impl Future<Output = Result<application::SignalCommitOutcome, SystemPortError>> + Send
        {
            self.inner.commit(instrument, expected, mutation)
        }
        fn occurrences(
            &self,
            instrument: &InstrumentId,
            identity: &domain::ResolvedOrderInstrument,
        ) -> impl Future<Output = Result<Vec<domain::BuySignalRecord>, SystemPortError>> + Send
        {
            self.inner.occurrences(instrument, identity)
        }
        fn sources(
            &self,
            task: &IssuedEstimateTaskId,
        ) -> impl Future<Output = Result<application::SignalSourceFacts, SystemPortError>> + Send
        {
            self.inner.sources(task)
        }
        fn current(
            &self,
            instrument: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<domain::RaisedSignal>, SystemPortError>> + Send
        {
            self.inner.current(instrument)
        }
    }
    impl application::EstimateSubmissionRepository for SwitchBeforeStoreRepository<'_> {
        fn submission_records(
            &self,
            instrument: &InstrumentId,
        ) -> impl Future<
            Output = Result<
                Vec<
                    domain::EstimateSubmissionRecord<
                        application::AiRunId,
                        application::ReasoningEffort,
                    >,
                >,
                SystemPortError,
            >,
        > + Send {
            self.inner.submission_records(instrument)
        }
        fn assignments(
            &self,
        ) -> impl Future<
            Output = Result<domain::EstimateAssignments<application::AiRunId>, SystemPortError>,
        > + Send {
            self.inner.assignments()
        }
        fn commit_submission(
            &self,
            previous: &domain::EstimateAssignments<application::AiRunId>,
            next: &domain::EstimateAssignments<application::AiRunId>,
            records: &[domain::EstimateSubmissionRecord<
                application::AiRunId,
                application::ReasoningEffort,
            >],
        ) -> impl Future<Output = Result<application::EstimateCommitOutcome, SystemPortError>> + Send
        {
            self.inner.commit_submission(previous, next, records)
        }
    }
    impl ProposalRepository for SwitchBeforeStoreRepository<'_> {
        fn store_draft(
            &self,
            proposal: &domain::Proposal<domain::Draft>,
        ) -> impl Future<Output = Result<application::WriteCompleted, SystemPortError>> + Send
        {
            self.inner.store_draft(proposal)
        }
        fn draft(
            &self,
            proposal: &ProposalId,
        ) -> impl Future<Output = Result<Option<domain::Proposal<domain::Draft>>, SystemPortError>> + Send
        {
            self.inner.draft(proposal)
        }
        fn current_draft(
            &self,
            instrument: &InstrumentId,
            expected: &application::SignalSnapshot,
            check: application::DraftCheck<'_>,
        ) -> impl Future<Output = Result<application::CurrentDraftOutcome, SystemPortError>> + Send
        {
            self.inner.current_draft(instrument, expected, check)
        }
        fn store_current_draft(
            &self,
            instrument: &InstrumentId,
            expected: &application::SignalSnapshot,
            proposal: &domain::Proposal<domain::Draft>,
            check: application::DraftCheck<'_>,
        ) -> impl Future<Output = Result<application::ConditionalDraftOutcome, SystemPortError>> + Send
        {
            self.inner.select_account(self.switch_to.clone());
            self.inner
                .store_current_draft(instrument, expected, proposal, check)
        }
    }

    #[test]
    fn account_switch_before_store_rejects_and_hides_old_draft() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let (bus, events) = event_bus();
        let before_reads = repository.snapshot_reads();
        let switching = SwitchBeforeStoreRepository {
            inner: &repository,
            switch_to: AccountId::new(8765),
        };
        let switched_err = run(DraftProposal::new(
            &switching,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("before-store-switch"),
        ))
        .expect_err("저장 직전 계좌 전환 거부");
        assert_eq!(switched_err, DraftProposalError::AccountChanged);
        assert_eq!(repository.snapshot_reads() - before_reads, 1);
        assert!(
            run(repository.draft(&proposal_id("before-store-switch")))
                .expect("조회")
                .is_none()
        );
        assert!(events.read().expect("이벤트").is_empty());
        let saved_id = proposal_id("saved-8765");
        let Ok(DraftProposalOutcome::Created { .. }) = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(8765),
            saved_id.clone(),
        )) else {
            panic!("선택 계좌 초안")
        };
        assert!(run(repository.draft(&saved_id)).expect("조회").is_some());
        repository.select_account(AccountId::new(4321));
        assert!(
            run(repository.draft(&saved_id))
                .expect("전환 은닉")
                .is_none()
        );
    }

    #[test]
    fn unselected_account_fails_closed() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let (bus, events) = event_bus();
        let unselected = MemorySignalEvaluationRepository::new(fixtures::assignments());
        unselected
            .advance_calibration_head(fixtures::head(0))
            .expect("위치");
        let seed_bus = EventBusBuilder::default().build();
        submit(&unselected, &seed_bus, "task-old", &source);
        run(EvaluateSignals::new(&unselected, &data, &seed_bus).execute(
            &source.row().instrument_id,
            now,
            BuySignalOccurrenceId::try_new("unselected-occ").expect("발생"),
        ))
        .expect("평가");
        let err = run(DraftProposal::new(
            &unselected,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("no-selected-account"),
        ))
        .expect_err("미선택 계좌 거부");
        assert_eq!(err, DraftProposalError::AccountChanged);
        assert_eq!(unselected.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
    }

    fn extended_day65_calendars(
        day65_start: Timestamp,
        regular_end: Timestamp,
        after_end: Timestamp,
    ) -> Vec<domain::MarketCalendar> {
        let base_day65 = fixtures::session(65, &Market::Kr);
        let regular = domain::SessionData::try_new(
            domain::TimeInterval::try_new(day65_start, regular_end).expect("정규장 구간"),
            None,
        )
        .expect("정규장");
        let after = domain::SessionData::try_new(
            domain::TimeInterval::try_new(regular_end, after_end).expect("애프터 구간"),
            None,
        )
        .expect("애프터");
        let day65_extended = domain::MarketSession::try_new(
            base_day65.date(),
            domain::SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(regular),
                after_market: Some(after),
            },
        )
        .expect("연장 세션");
        let row = |day: i64| {
            if day == 65 {
                day65_extended.clone()
            } else {
                fixtures::session(day, &Market::Kr)
            }
        };
        (60..=72_i64)
            .map(|day| {
                let prev = day.checked_sub(1).expect("전일");
                let next = day.checked_add(1).expect("익일");
                domain::MarketCalendar::try_new(row(prev), row(day), row(next)).expect("달력")
            })
            .collect()
    }

    #[test]
    fn pre_store_recheck_rejects_signal_expired_within_price_freshness() {
        let source = fixtures::master();
        let seed_data = Data::new(market(&source, fixtures::day(60)));
        let repository = seeded(&source, &seed_data);
        let day65_start = fixtures::day(65);
        let regular_end = fixtures::at(
            day65_start
                .utc_seconds()
                .as_i64()
                .checked_add(3600)
                .expect("정규장 종료"),
        );
        let after_end = fixtures::at(
            day65_start
                .utc_seconds()
                .as_i64()
                .checked_add(7200)
                .expect("애프터 종료"),
        );
        let t0 = fixtures::at(
            regular_end
                .utc_seconds()
                .as_i64()
                .checked_sub(2)
                .expect("종료 2초 전"),
        );
        let t1 = fixtures::at(
            regular_end
                .utc_seconds()
                .as_i64()
                .checked_sub(1)
                .expect("종료 1초 전"),
        );
        let mut facts = market(&source, t0);
        facts.evaluation.calendars = extended_day65_calendars(day65_start, regular_end, after_end);
        let data = Data::new(facts);
        let (bus, events) = event_bus();
        let clock = ScriptedClock::new([t0, t1, t1, regular_end], regular_end);
        let err = run(
            DraftProposal::new(&repository, &data, &Scheduler::default(), &clock, &bus).execute(
                &source.row().instrument_id,
                &AccountId::new(4321),
                proposal_id("late-expired-signal"),
            ),
        )
        .expect_err("저장 직전 만료 신호 거부");
        assert_eq!(
            err,
            DraftProposalError::SignalUnavailable(domain::NoSignalReason::Expired)
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
    }

    #[test]
    fn sell_signal_outside_price_range_discards_and_schedules_reestimate() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let (data, repository) = sell_fixture(&source, now);
        data.0.write().expect("입력").evaluation.target_price = Some(
            source
                .price_observation(fixtures::price("200"), fixtures::local(now))
                .expect("이탈 시세"),
        );
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let outcome = run(DraftProposal::new(
            &repository,
            &data,
            &scheduler,
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("sell-out-of-range"),
        ))
        .expect("범위 밖 매도 폐기");
        let DraftProposalOutcome::ReestimateScheduled { .. } = outcome else {
            panic!("재추정 예약이어야 한다")
        };
        assert!(
            run(repository.snapshot(&source.row().instrument_id))
                .expect("상태")
                .current
                .is_none()
        );
        assert_eq!(scheduler.calls.read().expect("예약").len(), 1);
        assert_eq!(
            events.read().expect("이벤트").as_slice(),
            &[domain::DomainEvent::SignalCleared(
                source.row().instrument_id.clone()
            )]
        );
    }

    struct DraftSpec {
        account: AccountId,
        side: domain::OrderSide,
        created_at: Timestamp,
        id: ProposalId,
    }

    fn custom_draft(
        source: &MasterInstrument,
        snapshot: &application::SignalSnapshot,
        spec: DraftSpec,
    ) -> domain::Proposal<domain::Draft> {
        let current = snapshot.current.as_ref().expect("신호");
        let task = match &current.signal {
            domain::RaisedSignal::Buy(value) => value.facts().task(),
            domain::RaisedSignal::Sell(value) => value.facts().task(),
        };
        let observation = source
            .price_observation(fixtures::price("100"), fixtures::local(spec.created_at))
            .expect("시세");
        let range = domain::ProposalPriceRange::from_estimate_task(task.range_facts(), observation)
            .expect("범위");
        let terms = domain::OrderTerms::KrLimit {
            side: spec.side,
            quantity: domain::WholeOrderQuantity::try_new(fixtures::quantity("1")).expect("수량"),
            price: domain::KrOrderPrice::try_new(fixtures::price("100")).expect("가격"),
        };
        let intent = domain::OrderIntent::new(
            spec.account,
            source.order_instrument().expect("주문 종목"),
            spec.id.as_client_order_id().clone(),
            terms,
        )
        .expect("의도");
        domain::Proposal::new(
            spec.id,
            intent,
            spec.created_at,
            range,
            domain::ProposalLifetime::default(),
        )
        .expect("제안")
    }

    fn invalid_store_specs(
        check: application::DraftCheck<'_>,
        existing_id: &ProposalId,
    ) -> [DraftSpec; 3] {
        let mismatched = if check.account == &AccountId::new(8765) {
            AccountId::new(4321)
        } else {
            AccountId::new(8765)
        };
        [
            DraftSpec {
                account: mismatched,
                side: domain::OrderSide::Buy,
                created_at: check.at,
                id: proposal_id("bad-acct"),
            },
            DraftSpec {
                account: check.account.clone(),
                side: domain::OrderSide::Sell,
                created_at: check.at,
                id: proposal_id("bad-side"),
            },
            DraftSpec {
                account: check.account.clone(),
                side: domain::OrderSide::Buy,
                created_at: check.at,
                id: existing_id.clone(),
            },
        ]
    }

    fn assert_four_invalid_stores_rejected(
        source: &MasterInstrument,
        repository: &MemorySignalEvaluationRepository,
        snapshot: &application::SignalSnapshot,
        check: application::DraftCheck<'_>,
        existing_id: &ProposalId,
    ) {
        let invalid_err = SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        );
        for spec in invalid_store_specs(check, existing_id) {
            let draft = custom_draft(source, snapshot, spec);
            assert_eq!(
                run(repository.store_current_draft(
                    &source.row().instrument_id,
                    snapshot,
                    &draft,
                    check,
                )),
                Err(invalid_err)
            );
        }
        let future_time = fixtures::at(
            check
                .at
                .utc_seconds()
                .as_i64()
                .checked_add(10)
                .expect("미래 시각"),
        );
        let future_draft = custom_draft(
            source,
            snapshot,
            DraftSpec {
                account: check.account.clone(),
                side: domain::OrderSide::Buy,
                created_at: future_time,
                id: proposal_id("future-rollback"),
            },
        );
        assert_eq!(
            run(repository.store_current_draft(
                &source.row().instrument_id,
                snapshot,
                &future_draft,
                check,
            )),
            Ok(application::ConditionalDraftOutcome::Invalid(
                domain::ProposalExecutionError::ClockRollback,
            ))
        );
    }

    #[test]
    fn store_current_draft_preserves_existing_draft_on_validation_errors() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let (bus, _) = event_bus();
        let valid_id = proposal_id("atomic-valid-4321");
        let Ok(DraftProposalOutcome::Created { .. }) = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            valid_id.clone(),
        )) else {
            panic!("유효 초안")
        };
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("스냅숏");
        let observation = current_observation(&data);
        repository.select_account(AccountId::new(8765));
        let check_8765 = application::DraftCheck {
            account: &AccountId::new(8765),
            at: now,
            observation: &observation,
        };
        assert_four_invalid_stores_rejected(&source, &repository, &snapshot, check_8765, &valid_id);
        repository.select_account(AccountId::new(4321));
        assert!(run(repository.draft(&valid_id)).expect("보존").is_some());
    }

    #[test]
    fn store_current_draft_rejects_invalid_proposal_even_when_same_account_draft_exists() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let (bus, _) = event_bus();
        let valid_id = proposal_id("same-acct-valid");
        let Ok(DraftProposalOutcome::Created { .. }) = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            valid_id.clone(),
        )) else {
            panic!("유효 초안")
        };
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("스냅숏");
        let observation = current_observation(&data);
        let check = application::DraftCheck {
            account: &AccountId::new(4321),
            at: now,
            observation: &observation,
        };
        assert_four_invalid_stores_rejected(&source, &repository, &snapshot, check, &valid_id);
        assert!(run(repository.draft(&valid_id)).expect("보존").is_some());
    }

    #[test]
    fn recheck_unknown_market_fails_closed_instead_of_market_closed() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let scheduler = Scheduler::default();
        let (bus, events) = event_bus();
        let unknown_time = fixtures::day(80);
        let clock = ScriptedClock::new([now, unknown_time], unknown_time);
        let err = run(
            DraftProposal::new(&repository, &data, &scheduler, &clock, &bus).execute(
                &source.row().instrument_id,
                &AccountId::new(4321),
                proposal_id("recheck-unknown"),
            ),
        )
        .expect_err("재판정 불명 장 거부");
        assert_eq!(err, DraftProposalError::MarketUnknown);
        assert_eq!(repository.draft_attempts(), 0);
        assert!(
            run(repository.snapshot(&source.row().instrument_id))
                .expect("신호 보존")
                .current
                .is_some()
        );
        assert!(events.read().expect("이벤트").is_empty());
    }

    #[test]
    fn draft_lookup_hides_proposal_after_calibration_head_advances() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let (bus, _) = event_bus();
        let id = proposal_id("hide-after-head");
        let Ok(DraftProposalOutcome::Created { .. }) = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            id.clone(),
        )) else {
            panic!("초안 생성")
        };
        assert!(run(repository.draft(&id)).expect("조회").is_some());
        repository
            .advance_calibration_head(fixtures::head(1))
            .expect("보정 원장 전진");
        assert!(run(repository.draft(&id)).expect("전진 후 은닉").is_none());
    }

    #[test]
    fn store_current_draft_rejects_expired_proposal_directly() {
        let source = fixtures::master();
        let created_at = fixtures::day(60);
        let later = fixtures::at(
            created_at
                .utc_seconds()
                .as_i64()
                .checked_add(601)
                .expect("만료 시각"),
        );
        let data = Data::new(market(&source, created_at));
        let repository = seeded(&source, &data);
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("스냅숏");
        let expired_id = proposal_id("direct-expired");
        let expired = custom_draft(
            &source,
            &snapshot,
            DraftSpec {
                account: AccountId::new(4321),
                side: domain::OrderSide::Buy,
                created_at,
                id: expired_id.clone(),
            },
        );
        let later_obs = source
            .price_observation(fixtures::price("100"), fixtures::local(later))
            .expect("이후 시세");
        assert_eq!(
            run(repository.store_current_draft(
                &source.row().instrument_id,
                &snapshot,
                &expired,
                application::DraftCheck {
                    account: &AccountId::new(4321),
                    at: later,
                    observation: &later_obs,
                },
            )),
            Ok(application::ConditionalDraftOutcome::Invalid(
                domain::ProposalExecutionError::Expired,
            ))
        );
        assert!(run(repository.draft(&expired_id)).expect("조회").is_none());
    }

    #[test]
    fn reused_symbol_with_different_master_instrument_id_fails_closed() {
        let source = fixtures::master();
        let now = fixtures::day(60);
        let data = Data::new(market(&source, now));
        let repository = seeded(&source, &data);
        let mut row = source.row().clone();
        row.instrument_id = fixtures::id("kr-005930-reissued");
        data.0.write().expect("입력").master =
            MasterInstrument::try_new(row).expect("재발급 마스터");
        let (bus, events) = event_bus();
        let err = run(DraftProposal::new(
            &repository,
            &data,
            &Scheduler::default(),
            &testkit_ports::SteppedClock::new(now),
            &bus,
        )
        .execute(
            &source.row().instrument_id,
            &AccountId::new(4321),
            proposal_id("reissued-master-id"),
        ))
        .expect_err("동일 심볼 다른 내부 ID 거부");
        assert_eq!(err, DraftProposalError::InstrumentMismatch);
        assert_eq!(repository.draft_attempts(), 0);
        assert!(
            run(repository.snapshot(&source.row().instrument_id))
                .expect("신호 보존")
                .current
                .is_some()
        );
        assert!(events.read().expect("이벤트").is_empty());
    }

    fn buy_plan_for(source: &MasterInstrument, now: Timestamp) -> domain::BuyQuantityPlan {
        let data = Data::new(market(source, now));
        let assignments = domain::EstimateAssignments::default()
            .issue(
                fixtures::task_id("task-old"),
                fixtures::run_id(),
                fixtures::eligible_for_master("102", source),
            )
            .expect("발급");
        let repository = MemorySignalEvaluationRepository::new(assignments);
        repository.select_account(AccountId::new(4321));
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("위치");
        let bus = EventBusBuilder::default().build();
        submit(&repository, &bus, "task-old", source);
        current_buy_plan(&repository, source, &data, now)
    }

    fn us_limit_for(source: &MasterInstrument, now: Timestamp) -> domain::CalculatedLimitPrice {
        let (book, limits) = us_book_and_limits(source, now);
        let table = domain::TickTable::from_master(source).expect("미국 호가 단위");
        domain::calculate_limit_price(&book, Some(&table), &limits, domain::OrderSide::Buy, now)
            .expect("미국 지정가")
    }

    #[test]
    fn buy_intent_rejects_each_single_instrument_field_mismatch() {
        let now = fixtures::day(60);
        let mut base_row = fixtures::master().row().clone();
        base_row.symbol = domain::Symbol::try_new("A00593").expect("공통 코드");
        let source = MasterInstrument::try_new(base_row).expect("기준 마스터");
        let plan = buy_plan_for(&source, now);
        let key = ClientOrderId::try_new("single-field-mismatch").expect("ID");
        let plan_id = plan.signal().facts().task().volatility().instrument();

        let mut id_row = source.row().clone();
        id_row.instrument_id = fixtures::id("different-internal-id");
        let id_only = MasterInstrument::try_new(id_row).expect("내부 ID만 다름");
        let id_limit = calculated_limit(&id_only, now, "100", domain::OrderSide::Buy);
        assert_ne!(id_limit.instrument().instrument_id(), plan_id);
        assert_eq!(id_limit.instrument().symbol(), plan.instrument().symbol());
        assert_eq!(
            Market::from(*id_limit.instrument().market()),
            plan.instrument().market()
        );
        assert_eq!(
            application::buy_limit_intent(&plan, &id_limit, AccountId::new(4321), key.clone()),
            Err(application::BuyLimitIntentError::Instrument)
        );

        let mut symbol_row = source.row().clone();
        symbol_row.symbol = domain::Symbol::try_new("B00593").expect("다른 코드");
        let symbol_only = MasterInstrument::try_new(symbol_row).expect("심볼만 다름");
        let symbol_limit = calculated_limit(&symbol_only, now, "100", domain::OrderSide::Buy);
        assert_eq!(symbol_limit.instrument().instrument_id(), plan_id);
        assert_ne!(
            symbol_limit.instrument().symbol(),
            plan.instrument().symbol()
        );
        assert_eq!(
            Market::from(*symbol_limit.instrument().market()),
            plan.instrument().market()
        );
        assert_eq!(
            application::buy_limit_intent(&plan, &symbol_limit, AccountId::new(4321), key.clone()),
            Err(application::BuyLimitIntentError::Instrument)
        );

        let mut market_row = source.row().clone();
        market_row.listing_market = domain::MasterCode::try_new("NASDAQ").expect("미국 시장");
        market_row.currency = Currency::Usd;
        market_row.korean_detail = None;
        let market_only = MasterInstrument::try_new(market_row).expect("시장만 다름");
        let market_limit = us_limit_for(&market_only, now);
        assert_eq!(market_limit.instrument().instrument_id(), plan_id);
        assert_eq!(
            market_limit.instrument().symbol(),
            plan.instrument().symbol()
        );
        assert_ne!(
            Market::from(*market_limit.instrument().market()),
            plan.instrument().market()
        );
        assert_eq!(
            application::buy_limit_intent(&plan, &market_limit, AccountId::new(4321), key),
            Err(application::BuyLimitIntentError::Instrument)
        );
    }
}
