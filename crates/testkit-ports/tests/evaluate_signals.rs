#[cfg(test)]
mod tests {
    mod a27_fixtures;
    use a27_fixtures as fixtures;
    use application::{
        EstimateSubmissionRepository, EvaluateSignalInputs, EvaluateSignalOutcome, EvaluateSignals,
        EventBusBuilder, EventSubscriber, SignalEvaluationData, SignalRepository, SubmitEstimate,
        SubscriberFailure, SystemPortError,
    };
    use domain::{
        BuySignalOccurrenceId, Calibration, CalibrationSnapshot, DomainEvent, DomainEventKind,
        EstimateSubmissionRecord, InvestorProfile, Market, MasterInstrument, MasterInstrumentFacts,
        Policy, SectorAssignments, Timestamp,
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
            Poll::Pending => panic!("즉시 완료"),
        }
    }

    struct Receiver(Arc<RwLock<Vec<DomainEvent>>>);
    impl EventSubscriber for Receiver {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            match self.0.write() {
                Ok(mut events) => events.push(event.clone()),
                Err(poison) => poison.into_inner().push(event.clone()),
            }
            Ok(())
        }
    }

    struct Inputs(EvaluateSignalInputs);
    impl SignalEvaluationData for Inputs {
        fn inputs(
            &self,
            _record: &EstimateSubmissionRecord<application::AiRunId, application::ReasoningEffort>,
            _now: Timestamp,
        ) -> impl Future<Output = Result<EvaluateSignalInputs, SystemPortError>> + Send {
            std::future::ready(Ok(self.0.clone()))
        }
    }

    fn setup(source: &MasterInstrument) -> EvaluateSignalInputs {
        let now = fixtures::day(60);
        let head = fixtures::head(0);
        let table = Calibration::try_new(fixtures::calibration_key(), &[]).expect("표");
        let portfolio = domain::Portfolio::new(
            vec![],
            domain::BuyingPower::new(fixtures::money("10000", domain::Currency::Krw), now),
            domain::BuyingPower::new(fixtures::money("0", domain::Currency::Usd), now),
        )
        .expect("포트")
        .evaluate(None);
        let sector = domain::SectorId::try_new("tech").expect("섹터");
        let catalog = domain::SectorCatalog::from_response(
            vec![domain::SectorMetadataRow {
                id: sector.clone(),
                name: domain::DisplayInstrumentName::try_new("기술").expect("이름"),
                depth: domain::SectorDepth::Major,
                parent: None,
            }],
            now,
        )
        .expect("목록");
        let composition = domain::SectorComposition::from_stocks_response(
            sector,
            domain::KnownMarket::Kr,
            vec![domain::SectorStockRow {
                symbol: source.row().symbol.clone(),
                currency: domain::Currency::Krw,
                market_capitalization: Some(domain::NumericInput::try_new("100000").expect("시총")),
                trading_amount: None,
            }],
            now,
        )
        .expect("구성");
        let assignment =
            domain::sector_assignment_from_compositions(source, &catalog, &[composition])
                .expect("배정");
        let target = fixtures::eligible_for_master("102", source)
            .task()
            .reference()
            .clone();
        EvaluateSignalInputs {
            calibration: Some(CalibrationSnapshot::try_new(table, head.clone()).expect("스냅숏")),
            calibration_head: Some(head),
            valuation: Some(portfolio),
            policy: Policy::new(InvestorProfile::default()),
            assignments: SectorAssignments::try_new(vec![assignment]).expect("분류"),
            security: MasterInstrumentFacts::try_new(vec![source.clone()])
                .expect("마스터")
                .policy_security_facts(),
            target_price: Some(target),
            calendars: (60..=66)
                .map(|day| fixtures::calendar(day, &Market::Kr))
                .collect(),
            quantity_costs: None,
        }
    }

    fn repository() -> MemorySignalEvaluationRepository {
        let repository = MemorySignalEvaluationRepository::new(fixtures::assignments());
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("보정 위치");
        repository
    }

    fn submit(
        repository: &MemorySignalEvaluationRepository,
        name: &str,
        probability: &str,
        source: &MasterInstrument,
    ) {
        let events = EventBusBuilder::default().build();
        run(SubmitEstimate::new(repository, &events).execute(
            &fixtures::run_id(),
            vec![fixtures::input(name, probability, source)],
        ))
        .expect("제출");
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("조회");
        repository.save_sources(
            snapshot.submission.expect("제출 ID"),
            application::SignalSourceFacts {
                items: vec![fixtures::item()],
                disclosures: vec![],
                tools: vec![],
            },
        );
    }

    fn occurrence(name: &str) -> BuySignalOccurrenceId {
        BuySignalOccurrenceId::try_new(name).expect("발생 ID")
    }

    #[test]
    fn raises_once_then_retains_without_another_event() {
        let source = fixtures::master();
        match fixtures::decision(&source, "99", None) {
            domain::SignalDecision::Buy(_) => (),
            domain::SignalDecision::Sell(_) | domain::SignalDecision::None(_) => {
                panic!("매수 필요")
            }
        }
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let observed = Arc::new(RwLock::new(vec![]));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            [
                DomainEventKind::SignalRaised,
                DomainEventKind::SignalCleared,
            ]
            .into_iter()
            .collect(),
            Receiver(Arc::clone(&observed)),
        );
        let bus = builder.build();
        let inputs = Inputs(setup(&source));
        let usecase = EvaluateSignals::new(&repository, &inputs, &bus);
        let first = run(usecase.execute(
            &source.row().instrument_id,
            fixtures::day(60),
            occurrence("first"),
        ))
        .expect("평가");
        assert_eq!(reason(&first), None);
        run(usecase.execute(
            &source.row().instrument_id,
            fixtures::day(60),
            occurrence("second"),
        ))
        .expect("재평가");
        assert_eq!(
            *observed.read().expect("이벤트"),
            vec![DomainEvent::SignalRaised(
                source.row().instrument_id.clone()
            )]
        );
        assert_eq!(
            run(repository.current(&source.row().instrument_id))
                .expect("현재")
                .len(),
            1
        );
    }

    #[test]
    fn missing_calibration_clears_after_commit_and_keeps_the_reason() {
        let source = fixtures::master();
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let observed = Arc::new(RwLock::new(vec![]));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            [
                DomainEventKind::SignalRaised,
                DomainEventKind::SignalCleared,
            ]
            .into_iter()
            .collect(),
            Receiver(Arc::clone(&observed)),
        );
        let bus = builder.build();
        run(
            EvaluateSignals::new(&repository, &Inputs(setup(&source)), &bus).execute(
                &source.row().instrument_id,
                fixtures::day(60),
                occurrence("first"),
            ),
        )
        .expect("발생");
        let mut missing = setup(&source);
        missing.calibration = None;
        let outcome = run(
            EvaluateSignals::new(&repository, &Inputs(missing), &bus).execute(
                &source.row().instrument_id,
                fixtures::day(60),
                occurrence("other"),
            ),
        )
        .expect("해제");
        assert_eq!(
            reason(&outcome),
            Some(domain::NoSignalReason::Unavailable(
                domain::SignalUnavailable::MissingCalibration
            ))
        );
        assert_eq!(
            *observed.read().expect("이벤트"),
            vec![
                DomainEvent::SignalRaised(source.row().instrument_id.clone()),
                DomainEvent::SignalCleared(source.row().instrument_id.clone())
            ]
        );
        assert_eq!(
            run(repository.current(&source.row().instrument_id)).expect("현재"),
            Vec::new()
        );
    }

    fn events() -> (application::EventBus, Arc<RwLock<Vec<DomainEvent>>>) {
        let recorded = Arc::new(RwLock::new(vec![]));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            [
                DomainEventKind::SignalRaised,
                DomainEventKind::SignalCleared,
            ]
            .into_iter()
            .collect(),
            Receiver(Arc::clone(&recorded)),
        );
        (builder.build(), recorded)
    }

    fn reason(outcome: &EvaluateSignalOutcome) -> Option<domain::NoSignalReason> {
        match outcome {
            EvaluateSignalOutcome::Applied { reason, .. } => *reason,
            EvaluateSignalOutcome::NoSubmission | EvaluateSignalOutcome::SubmissionDiscarded => {
                panic!("평가 결과 필요")
            }
        }
    }

    fn quantity(outcome: &EvaluateSignalOutcome) -> Option<&domain::BuyQuantityDecision> {
        match outcome {
            EvaluateSignalOutcome::Applied { quantity, .. } => quantity.as_ref(),
            EvaluateSignalOutcome::NoSubmission | EvaluateSignalOutcome::SubmissionDiscarded => {
                panic!("평가 결과 필요")
            }
        }
    }

    #[test]
    fn new_submission_recomputes_and_preserves_older_record() {
        let source = fixtures::master();
        let repository = repository();
        let (bus, observed) = events();
        submit(&repository, "task-old", "99", &source);
        let inputs = Inputs(setup(&source));
        let evaluate = EvaluateSignals::new(&repository, &inputs, &bus);
        run(evaluate.execute(
            &source.row().instrument_id,
            fixtures::day(60),
            occurrence("first"),
        ))
        .expect("발생");
        submit(&repository, "task-new", "10", &source);
        let result = run(evaluate.execute(
            &source.row().instrument_id,
            fixtures::day(60),
            occurrence("next"),
        ))
        .expect("해제");
        assert_eq!(
            reason(&result),
            Some(domain::NoSignalReason::UnheldLowProbability)
        );
        let records =
            run(repository.submission_records(&source.row().instrument_id)).expect("제출");
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0].task_id().as_estimate_id(),
            &fixtures::task_id("task-old")
        );
        assert_eq!(
            records[1].task_id().as_estimate_id(),
            &fixtures::task_id("task-new")
        );
        assert_eq!(
            run(repository.current(&source.row().instrument_id)).expect("신호"),
            Vec::new()
        );
        assert_eq!(
            *observed.read().expect("이벤트"),
            vec![
                DomainEvent::SignalRaised(source.row().instrument_id.clone()),
                DomainEvent::SignalCleared(source.row().instrument_id.clone())
            ]
        );
    }

    fn configure_case(
        case: &str,
        input: &mut EvaluateSignalInputs,
        source: &MasterInstrument,
        repository: &MemorySignalEvaluationRepository,
    ) -> Timestamp {
        match case {
            "missing holdings" => {
                input.valuation = None;
                fixtures::day(60)
            }
            "missing price" => {
                input.target_price = None;
                fixtures::day(60)
            }
            "wrong price" => {
                input.target_price = Some(
                    fixtures::eligible_for_master("104", &fixtures::other_master())
                        .task()
                        .reference()
                        .clone(),
                );
                fixtures::day(60)
            }
            "future" => fixtures::day(59),
            "missing source" => {
                let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("조회");
                repository.save_sources(
                    snapshot.submission.expect("제출"),
                    application::SignalSourceFacts::default(),
                );
                fixtures::day(60)
            }
            _ => panic!("알 수 없는 테스트"),
        }
    }

    #[test]
    fn decision_failures_clear_the_old_signal_and_preserve_specific_reasons() {
        let source = fixtures::master();
        let cases = [
            (
                "missing holdings",
                domain::NoSignalReason::Unavailable(domain::SignalUnavailable::HoldingsQueryFailed),
            ),
            (
                "missing price",
                domain::NoSignalReason::Unavailable(domain::SignalUnavailable::MissingTargetPrice),
            ),
            (
                "wrong price",
                domain::NoSignalReason::Unavailable(domain::SignalUnavailable::InvalidTargetPrice),
            ),
            (
                "future",
                domain::NoSignalReason::Unavailable(
                    domain::SignalUnavailable::SubmissionFromFuture,
                ),
            ),
            ("missing source", domain::NoSignalReason::MissingSource),
        ];
        for (case, expected) in cases {
            let repository = repository();
            submit(&repository, "task-old", "99", &source);
            let (bus, observed) = events();
            run(
                EvaluateSignals::new(&repository, &Inputs(setup(&source)), &bus).execute(
                    &source.row().instrument_id,
                    fixtures::day(60),
                    occurrence("initial"),
                ),
            )
            .expect("발생");
            let mut input = setup(&source);
            let now = configure_case(case, &mut input, &source, &repository);
            let result = run(
                EvaluateSignals::new(&repository, &Inputs(input), &bus).execute(
                    &source.row().instrument_id,
                    now,
                    occurrence("next"),
                ),
            )
            .expect("재평가");
            assert_eq!(reason(&result), Some(expected), "{case}");
            assert_eq!(
                run(repository.current(&source.row().instrument_id)).expect("신호"),
                Vec::new(),
                "{case}"
            );
            assert_eq!(
                *observed.read().expect("이벤트"),
                vec![
                    DomainEvent::SignalRaised(source.row().instrument_id.clone()),
                    DomainEvent::SignalCleared(source.row().instrument_id.clone())
                ],
                "{case}"
            );
        }
    }

    #[test]
    fn stale_calibration_is_distinct_and_clears_a_signal() {
        let source = fixtures::master();
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let (bus, observed) = events();
        let mut input = setup(&source);
        run(
            EvaluateSignals::new(&repository, &Inputs(input.clone()), &bus).execute(
                &source.row().instrument_id,
                fixtures::day(60),
                occurrence("first"),
            ),
        )
        .expect("발생");
        repository
            .advance_calibration_head(fixtures::head(1))
            .expect("최신 원장");
        input.calibration_head = Some(fixtures::head(1));
        let result = run(
            EvaluateSignals::new(&repository, &Inputs(input), &bus).execute(
                &source.row().instrument_id,
                fixtures::day(60),
                occurrence("second"),
            ),
        )
        .expect("해제");
        assert_eq!(
            reason(&result),
            Some(domain::NoSignalReason::Unavailable(
                domain::SignalUnavailable::StaleCalibration
            ))
        );
        assert_eq!(observed.read().expect("이벤트").len(), 2);
    }

    #[test]
    fn storage_failure_never_publishes_or_persists_a_signal() {
        let source = fixtures::master();
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let error = SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::InvalidData,
        );
        repository.fail_next_commit(error);
        let (bus, observed) = events();
        let result = run(
            EvaluateSignals::new(&repository, &Inputs(setup(&source)), &bus).execute(
                &source.row().instrument_id,
                fixtures::day(60),
                occurrence("failed"),
            ),
        );
        assert_eq!(
            result,
            Err(application::EvaluateSignalError::Storage(error))
        );
        assert_eq!(*observed.read().expect("이벤트"), Vec::new());
        assert_eq!(
            run(repository.current(&source.row().instrument_id)).expect("신호"),
            Vec::new()
        );
    }

    struct RacingInputs {
        input: EvaluateSignalInputs,
        barrier: std::sync::Barrier,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl SignalEvaluationData for RacingInputs {
        fn inputs(
            &self,
            _record: &EstimateSubmissionRecord<application::AiRunId, application::ReasoningEffort>,
            _now: Timestamp,
        ) -> impl Future<Output = Result<EvaluateSignalInputs, SystemPortError>> + Send {
            if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < 2 {
                self.barrier.wait();
            }
            std::future::ready(Ok(self.input.clone()))
        }
    }

    #[test]
    fn simultaneous_evaluations_retry_without_duplicate_events() {
        let source = fixtures::master();
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let (bus, observed) = events();
        let inputs = RacingInputs {
            input: setup(&source),
            barrier: std::sync::Barrier::new(2),
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        std::thread::scope(|scope| {
            let one = scope.spawn(|| {
                run(EvaluateSignals::new(&repository, &inputs, &bus).execute(
                    &source.row().instrument_id,
                    fixtures::day(60),
                    occurrence("parallel-one"),
                ))
            });
            let two = scope.spawn(|| {
                run(EvaluateSignals::new(&repository, &inputs, &bus).execute(
                    &source.row().instrument_id,
                    fixtures::day(60),
                    occurrence("parallel-two"),
                ))
            });
            assert_eq!(reason(&one.join().expect("스레드").expect("평가")), None);
            assert_eq!(reason(&two.join().expect("스레드").expect("평가")), None);
        });
        assert_eq!(
            *observed.read().expect("이벤트"),
            vec![DomainEvent::SignalRaised(
                source.row().instrument_id.clone()
            )]
        );
        assert_eq!(
            run(repository.occurrences(
                &source.row().instrument_id,
                &source.order_instrument().expect("종목")
            ))
            .expect("이력")
            .len(),
            1
        );
    }

    #[test]
    fn other_instruments_have_independent_signals_and_events() {
        let first = fixtures::master();
        let other = fixtures::other_master();
        let repository = repository();
        submit(&repository, "task-old", "99", &first);
        submit(&repository, "task-other", "99", &other);
        let (bus, observed) = events();
        for (source, id) in [(&first, "one"), (&other, "two")] {
            run(
                EvaluateSignals::new(&repository, &Inputs(setup(source)), &bus).execute(
                    &source.row().instrument_id,
                    fixtures::day(60),
                    occurrence(id),
                ),
            )
            .expect("평가");
        }
        assert_eq!(
            run(repository.current(&first.row().instrument_id))
                .expect("첫 종목")
                .len(),
            1
        );
        assert_eq!(
            run(repository.current(&other.row().instrument_id))
                .expect("둘째 종목")
                .len(),
            1
        );
        assert_eq!(
            *observed.read().expect("이벤트"),
            vec![
                DomainEvent::SignalRaised(first.row().instrument_id.clone()),
                DomainEvent::SignalRaised(other.row().instrument_id.clone())
            ]
        );
    }

    #[test]
    fn absence_of_any_submission_has_no_event() {
        let repository = repository();
        let (bus, observed) = events();
        let source = fixtures::master();
        let outcome = run(
            EvaluateSignals::new(&repository, &Inputs(setup(&source)), &bus).execute(
                &source.row().instrument_id,
                fixtures::day(60),
                occurrence("unused"),
            ),
        )
        .expect("조회");
        assert_eq!(outcome, EvaluateSignalOutcome::NoSubmission);
        assert_eq!(*observed.read().expect("이벤트"), Vec::new());
    }

    fn with_costs(mut input: EvaluateSignalInputs) -> EvaluateSignalInputs {
        input.quantity_costs = Some(application::SignalQuantityCosts {
            fx: domain::OrderFxRates::missing(),
            commission: domain::CommissionRateFact::from_commissions_result(
                Market::Kr,
                domain::CostRate::try_new(fixtures::ratio("0")).expect("요율"),
                domain::CommissionValidity::try_new(None, None).expect("기간"),
            ),
        });
        input
    }

    #[test]
    fn old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise() {
        let source = fixtures::master();
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let (bus, _) = events();
        let inputs = Inputs(with_costs(setup(&source)));
        let usecase = EvaluateSignals::new(&repository, &inputs, &bus);
        let first = run(usecase.execute(
            &source.row().instrument_id,
            fixtures::day(60),
            occurrence("old"),
        ))
        .expect("발생");
        match quantity(&first) {
            Some(domain::BuyQuantityDecision::Plan(_)) => (),
            Some(domain::BuyQuantityDecision::None(_)) | None => panic!("수량 계획 필요"),
        }
        let mut missing = setup(&source);
        missing.calibration = None;
        run(
            EvaluateSignals::new(&repository, &Inputs(missing), &bus).execute(
                &source.row().instrument_id,
                fixtures::day(60),
                occurrence("unused"),
            ),
        )
        .expect("해제");
        let second = run(usecase.execute(
            &source.row().instrument_id,
            fixtures::day(60),
            occurrence("new"),
        ))
        .expect("재발생");
        assert_eq!(
            quantity(&second),
            Some(&domain::BuyQuantityDecision::None(
                domain::NoBuyQuantity::RepeatedSignal
            ))
        );
        assert_eq!(
            run(repository.occurrences(
                &source.row().instrument_id,
                &source.order_instrument().expect("종목")
            ))
            .expect("이력")
            .len(),
            2
        );
    }

    fn eligible_with_tax(
        source: &MasterInstrument,
        high: &str,
        tax: i128,
    ) -> domain::CostEligibleTask {
        let task = fixtures::eligible_for_master(high, source).task().clone();
        let rate =
            domain::CostRate::try_new(fixtures::ratio(&format!("0.{tax:028}"))).expect("세율");
        let zero = domain::CostRate::try_new(fixtures::ratio("0")).expect("비용");
        let period = domain::EstimateCostPeriod::from_calendar_result(&task, fixtures::day(66))
            .expect("기간");
        let cost = domain::TradeCost::from_facts(
            Some(domain::CommissionRateFact::from_commissions_result(
                Market::Kr,
                zero,
                domain::CommissionValidity::try_new(None, None).expect("기간"),
            )),
            Some(domain::TradeTaxFact::from_schedule_result(
                Market::Kr,
                rate,
                zero,
            )),
            period,
        )
        .expect("비용");
        domain::CostEligibleTask::assess(task, &cost).expect("적격")
    }

    fn boundary_for(
        source: &MasterInstrument,
        high: &str,
        percent: i32,
    ) -> Option<domain::CostEligibleTask> {
        use std::cmp::Ordering;
        let head = fixtures::head(0);
        let table = Calibration::try_new(fixtures::calibration_key(), &[]).expect("표");
        let snapshot = CalibrationSnapshot::try_new(table, head.clone()).expect("스냅숏");
        let mut lower = 0_i128;
        let mut upper = 30_000_000_000_000_000_000_000_000_i128;
        while lower <= upper {
            let middle = lower.checked_add(upper)?.checked_div(2)?;
            let task = eligible_with_tax(source, high, middle);
            let submitted = domain::calibrate_signal_submission(
                Some(&snapshot),
                &head,
                fixtures::submission(&task, &percent.to_string()),
            )
            .expect("보정");
            let margin = domain::Margin::for_appetite(domain::RiskAppetite::Neutral);
            let thresholds =
                domain::Thresholds::create(&task, submitted.calibration(), margin).expect("임계값");
            match thresholds
                .compare(submitted.calibration(), domain::ThresholdKind::Buy)
                .expect("비교")
            {
                Ordering::Equal => return Some(task),
                Ordering::Greater => lower = middle.checked_add(1)?,
                Ordering::Less => upper = middle.checked_sub(1)?,
            }
        }
        None
    }

    fn exact_boundary_task(source: &MasterInstrument) -> (domain::CostEligibleTask, i32) {
        ["102", "104", "106", "108", "110", "112", "114", "116"]
            .into_iter()
            .flat_map(|high| (43_i32..=47_i32).map(move |percent| (high, percent)))
            .find_map(|(high, percent)| {
                boundary_for(source, high, percent).map(|task| (task, percent))
            })
            .expect("정확히 일치하는 임계값 필요")
    }

    #[test]
    fn buy_threshold_below_equal_and_above_follow_the_domain_decision() {
        let source = fixtures::master();
        let (task, boundary) = exact_boundary_task(&source);
        for (probability, should_buy) in [
            (boundary.checked_sub(1).expect("아래"), false),
            (boundary, true),
            (boundary.checked_add(1).expect("위"), true),
        ] {
            let assigned = domain::EstimateAssignments::default()
                .issue(
                    fixtures::task_id("task-old"),
                    fixtures::run_id(),
                    task.clone(),
                )
                .expect("과제");
            let repository = MemorySignalEvaluationRepository::new(assigned);
            repository
                .advance_calibration_head(fixtures::head(0))
                .expect("위치");
            let mut submission = fixtures::input("task-old", &probability.to_string(), &source);
            submission.submission = fixtures::submission(&task, &probability.to_string());
            let quiet = EventBusBuilder::default().build();
            run(SubmitEstimate::new(&repository, &quiet)
                .execute(&fixtures::run_id(), vec![submission]))
            .expect("제출");
            let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("조회");
            repository.save_sources(
                snapshot.submission.expect("제출"),
                application::SignalSourceFacts {
                    items: vec![fixtures::item()],
                    disclosures: vec![],
                    tools: vec![],
                },
            );
            let (bus, observed) = events();
            let result = run(
                EvaluateSignals::new(&repository, &Inputs(setup(&source)), &bus).execute(
                    &source.row().instrument_id,
                    fixtures::day(60),
                    occurrence("boundary"),
                ),
            )
            .expect("평가");
            let actual = run(repository.current(&source.row().instrument_id)).expect("현재");
            assert_eq!(actual.len(), usize::from(should_buy), "{probability}");
            assert_eq!(
                observed.read().expect("이벤트").len(),
                usize::from(should_buy)
            );
            assert_eq!(
                reason(&result),
                if should_buy {
                    None
                } else {
                    Some(domain::NoSignalReason::ConditionsUnmet)
                }
            );
        }
    }

    #[derive(Clone, Copy)]
    enum CommitBehavior {
        AlwaysConflict,
        Discarded,
    }
    struct CommitOverride<'a> {
        inner: &'a MemorySignalEvaluationRepository,
        calls: std::sync::atomic::AtomicUsize,
        behavior: CommitBehavior,
    }
    impl EstimateSubmissionRepository for CommitOverride<'_> {
        async fn submission_records(
            &self,
            instrument: &domain::InstrumentId,
        ) -> Result<
            Vec<EstimateSubmissionRecord<application::AiRunId, application::ReasoningEffort>>,
            SystemPortError,
        > {
            self.inner.submission_records(instrument).await
        }
        async fn assignments(
            &self,
        ) -> Result<domain::EstimateAssignments<application::AiRunId>, SystemPortError> {
            self.inner.assignments().await
        }
        async fn commit_submission(
            &self,
            previous: &domain::EstimateAssignments<application::AiRunId>,
            next: &domain::EstimateAssignments<application::AiRunId>,
            records: &[EstimateSubmissionRecord<
                application::AiRunId,
                application::ReasoningEffort,
            >],
        ) -> Result<application::EstimateCommitOutcome, SystemPortError> {
            self.inner.commit_submission(previous, next, records).await
        }
    }
    impl SignalRepository for CommitOverride<'_> {
        async fn snapshot(
            &self,
            instrument: &domain::InstrumentId,
        ) -> Result<application::SignalSnapshot, SystemPortError> {
            self.inner.snapshot(instrument).await
        }
        fn commit(
            &self,
            _instrument: &domain::InstrumentId,
            _expected: &application::SignalSnapshot,
            _mutation: &application::SignalMutation,
        ) -> impl Future<Output = Result<application::SignalCommitOutcome, SystemPortError>> + Send
        {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::future::ready(Ok(match self.behavior {
                CommitBehavior::AlwaysConflict => application::SignalCommitOutcome::Conflict,
                CommitBehavior::Discarded => application::SignalCommitOutcome::SubmissionDiscarded,
            }))
        }
        async fn occurrences(
            &self,
            instrument: &domain::InstrumentId,
            identity: &domain::ResolvedOrderInstrument,
        ) -> Result<Vec<domain::BuySignalRecord>, SystemPortError> {
            self.inner.occurrences(instrument, identity).await
        }
        async fn sources(
            &self,
            task: &domain::IssuedEstimateTaskId,
        ) -> Result<application::SignalSourceFacts, SystemPortError> {
            self.inner.sources(task).await
        }
        async fn current(
            &self,
            instrument: &domain::InstrumentId,
        ) -> Result<Vec<domain::RaisedSignal>, SystemPortError> {
            self.inner.current(instrument).await
        }
    }

    #[test]
    fn conflicts_are_bounded_and_discarded_submissions_do_not_retry() {
        let source = fixtures::master();
        for (behavior, count) in [
            (CommitBehavior::AlwaysConflict, 3),
            (CommitBehavior::Discarded, 1),
        ] {
            let repository = repository();
            submit(&repository, "task-old", "99", &source);
            let wrapper = CommitOverride {
                inner: &repository,
                calls: std::sync::atomic::AtomicUsize::new(0),
                behavior,
            };
            let (bus, observed) = events();
            let result = run(
                EvaluateSignals::new(&wrapper, &Inputs(setup(&source)), &bus).execute(
                    &source.row().instrument_id,
                    fixtures::day(60),
                    occurrence("retry"),
                ),
            );
            match behavior {
                CommitBehavior::AlwaysConflict => assert_eq!(
                    result,
                    Err(application::EvaluateSignalError::ConcurrentUpdate)
                ),
                CommitBehavior::Discarded => {
                    assert_eq!(result, Ok(EvaluateSignalOutcome::SubmissionDiscarded));
                }
            }
            assert_eq!(
                wrapper.calls.load(std::sync::atomic::Ordering::SeqCst),
                count
            );
            assert_eq!(*observed.read().expect("이벤트"), Vec::new());
        }
    }

    struct NewSubmissionInputs<'a> {
        repository: &'a MemorySignalEvaluationRepository,
        source: MasterInstrument,
        input: EvaluateSignalInputs,
        calls: std::sync::atomic::AtomicUsize,
    }
    impl SignalEvaluationData for NewSubmissionInputs<'_> {
        fn inputs(
            &self,
            _record: &EstimateSubmissionRecord<application::AiRunId, application::ReasoningEffort>,
            _now: Timestamp,
        ) -> impl Future<Output = Result<EvaluateSignalInputs, SystemPortError>> + Send {
            if self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                submit(self.repository, "task-new", "10", &self.source);
            }
            std::future::ready(Ok(self.input.clone()))
        }
    }

    #[test]
    fn concurrent_new_submission_forces_a_fresh_read_before_commit() {
        let source = fixtures::master();
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let (bus, observed) = events();
        let inputs = NewSubmissionInputs {
            repository: &repository,
            source: source.clone(),
            input: setup(&source),
            calls: std::sync::atomic::AtomicUsize::new(0),
        };
        let result = run(EvaluateSignals::new(&repository, &inputs, &bus).execute(
            &source.row().instrument_id,
            fixtures::day(60),
            occurrence("race"),
        ))
        .expect("평가");
        assert_eq!(
            reason(&result),
            Some(domain::NoSignalReason::UnheldLowProbability)
        );
        assert_eq!(inputs.calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(
            run(repository.submission_records(&source.row().instrument_id))
                .expect("제출")
                .len(),
            2
        );
        assert_eq!(
            run(repository.current(&source.row().instrument_id)).expect("현재"),
            Vec::new()
        );
        assert_eq!(*observed.read().expect("이벤트"), Vec::new());
    }

    #[test]
    fn actual_discarded_submission_cannot_be_raised_again() {
        let source = fixtures::master();
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let (bus, observed) = events();
        let inputs = Inputs(setup(&source));
        let evaluate = EvaluateSignals::new(&repository, &inputs, &bus);
        run(evaluate.execute(
            &source.row().instrument_id,
            fixtures::day(60),
            occurrence("first"),
        ))
        .expect("발생");
        let snapshot = run(repository.snapshot(&source.row().instrument_id)).expect("조회");
        let mutation = application::SignalMutation::Discard(Box::new(
            domain::SignalTransition::Cleared(source.row().instrument_id.clone()),
        ));
        let outcome = run(repository.commit(&source.row().instrument_id, &snapshot, &mutation))
            .expect("폐기");
        match outcome {
            application::SignalCommitOutcome::Applied(_) => (),
            application::SignalCommitOutcome::Conflict
            | application::SignalCommitOutcome::SubmissionDiscarded => panic!("폐기 실패"),
        }
        let result = run(evaluate.execute(
            &source.row().instrument_id,
            fixtures::day(60),
            occurrence("second"),
        ))
        .expect("재평가");
        assert_eq!(result, EvaluateSignalOutcome::SubmissionDiscarded);
        assert_eq!(
            run(repository.current(&source.row().instrument_id)).expect("현재"),
            Vec::new()
        );
        assert_eq!(
            *observed.read().expect("이벤트"),
            vec![DomainEvent::SignalRaised(
                source.row().instrument_id.clone()
            )]
        );
    }

    #[test]
    fn cash_failure_is_preserved_from_domain_without_creating_a_buy() {
        let source = fixtures::master();
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let failure = domain::CashObservationFailure::Unavailable {
            cause: domain::CashUnavailableCause::Network,
            retry: domain::CashRetryDelay::Unknown,
        };
        let now = fixtures::day(60);
        let portfolio = domain::portfolio_from_cash_observations(
            vec![],
            domain::CashObservation::Observed(domain::BuyingPower::new(
                fixtures::money("10000", domain::Currency::Krw),
                now,
            )),
            domain::CashObservation::Failed {
                currency: domain::Currency::Usd,
                reason: failure,
                observed_at: now,
            },
        )
        .expect("포트")
        .evaluate(None);
        let mut inputs = setup(&source);
        inputs.valuation = Some(portfolio);
        let (bus, observed) = events();
        let result = run(
            EvaluateSignals::new(&repository, &Inputs(inputs), &bus).execute(
                &source.row().instrument_id,
                now,
                occurrence("cash"),
            ),
        )
        .expect("평가");
        assert_eq!(
            reason(&result),
            Some(domain::NoSignalReason::Unavailable(
                domain::SignalUnavailable::CashObservationFailed(
                    domain::CashObservationFailures::Usd(failure)
                )
            ))
        );
        assert_eq!(*observed.read().expect("이벤트"), Vec::new());
        assert_eq!(
            run(repository.current(&source.row().instrument_id)).expect("신호"),
            Vec::new()
        );
    }

    #[test]
    fn mismatched_calibration_is_reported_without_a_signal() {
        let source = fixtures::master();
        let repository = repository();
        submit(&repository, "task-old", "99", &source);
        let mut input = setup(&source);
        let other_key = fixtures::calibration_key_for(domain::HorizonSelection::WeeksToMonths);
        let head = domain::CalibrationLedgerHead::new(
            other_key.clone(),
            domain::CalibrationPosition::new(0),
        );
        input.calibration = Some(
            CalibrationSnapshot::try_new(Calibration::try_new(other_key, &[]).expect("표"), head)
                .expect("스냅숏"),
        );
        let (bus, observed) = events();
        let result = run(
            EvaluateSignals::new(&repository, &Inputs(input), &bus).execute(
                &source.row().instrument_id,
                fixtures::day(60),
                occurrence("mismatch"),
            ),
        )
        .expect("평가");
        assert_eq!(
            reason(&result),
            Some(domain::NoSignalReason::Unavailable(
                domain::SignalUnavailable::CalibrationCombinationMismatch
            ))
        );
        assert_eq!(*observed.read().expect("이벤트"), Vec::new());
    }

    #[test]
    fn threshold_margin_follows_the_current_risk_appetite() {
        let source = fixtures::master();
        for (appetite, expected) in [
            (domain::RiskAppetite::Neutral, 1),
            (domain::RiskAppetite::Conservative, 0),
        ] {
            let repository = repository();
            submit(&repository, "task-old", "45", &source);
            let mut input = setup(&source);
            input.policy = Policy::new(InvestorProfile::default().with_risk_appetite(appetite));
            let (bus, _) = events();
            run(
                EvaluateSignals::new(&repository, &Inputs(input), &bus).execute(
                    &source.row().instrument_id,
                    fixtures::day(60),
                    occurrence("risk"),
                ),
            )
            .expect("평가");
            assert_eq!(
                run(repository.current(&source.row().instrument_id))
                    .expect("신호")
                    .len(),
                expected
            );
        }
    }
}
