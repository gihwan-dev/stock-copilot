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
        sync::{Arc, RwLock},
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
        assert_eq!(
            run(repository.snapshot(&source.row().instrument_id)).expect("상태"),
            before
        );
        assert_eq!(repository.draft_attempts(), 3);
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
    fn sell_signal_stays_without_draft_while_sell_target_is_undefined() {
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
        let Err(DraftProposalError::SellTargetUndefined) = result else {
            panic!("정의 대기")
        };
        assert_eq!(
            run(repository.snapshot(&source.row().instrument_id)).expect("상태"),
            signal
        );
        assert_eq!(repository.draft_attempts(), 0);
        assert!(events.read().expect("이벤트").is_empty());
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
        repository.conflict_next_drafts(3);
        let before = repository.snapshot_reads();
        let Err(DraftProposalError::ConcurrentUpdate) = run(DraftProposal::new(
            &repository,
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
        assert_eq!(repository.snapshot_reads() - before, 3);
        assert!(
            run(repository.draft(&proposal_id("retry-stop")))
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
        let Err(DraftProposalError::ConcurrentUpdate) = run(DraftProposal::new(
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
        assert_eq!(
            run(repository.store_current_draft(&source.row().instrument_id, &stale, &proposal)),
            Ok(application::ConditionalDraftOutcome::Conflict)
        );
        assert!(
            run(repository.snapshot(&source.row().instrument_id))
                .expect("상태")
                .current
                .is_none()
        );
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
}
