#[cfg(test)]
mod tests {
    use application::{
        AiRunId, EstimateCommitOutcome, EstimateSubmissionInput, EstimateSubmissionRepository,
        EventBusBuilder, EventSubscriber, ReasoningEffort, SubmitEstimate, SubmitEstimateError,
        SubscriberFailure, SystemFailure, SystemPort, SystemPortError,
    };
    use domain::{
        AdjustmentBasis, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        CommissionRateFact, CommissionValidity, CommonShare, CostEligibleTask, CostRate, Currency,
        DisplayInstrumentName, EstimateContext, EstimateCostPeriod, EstimateTask, Horizon,
        HorizonSelection, HoursPermission, InstrumentId, KoreanMasterDetail, LiquidationTrading,
        ListingStatus, Market, MasterCode, MasterInstrument, MasterInstrumentRow,
        MasterSecurityType, Nanosecond, NumericInput, NxtTrading, OffsetSeconds, Ohlcv, Price,
        PriceAdjustment, Quantity, Ratio, RiskAppetite, Symbol, Timestamp, TradeCost, TradeTaxFact,
        TradingSuspension, UnixSeconds, Volatility,
    };
    use domain::{
        AiProbability, AiProbabilityInput, CalibrationEstimateId, CalibrationKey, CalibrationModel,
        DomainEvent, DomainEventKind, EstimateAssignmentError, EstimateAssignments,
        EstimateMetadata, EstimateNarrative, EstimatePrompt, EstimateSourceStatus,
        EstimateSubmissionRecord, EvidenceBundleId, ModelIdentityPart, PromptHash,
        SignalSubmission, SignalTaskKey, UntrustedText,
    };
    use std::{
        future::Future,
        sync::{Arc, Barrier, RwLock},
        task::{Context, Poll, Waker},
    };
    fn price(value: &str) -> Price {
        Price::parse(&NumericInput::try_new(value).expect("입력"), Currency::Krw).expect("가격")
    }
    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }
    fn ratio(value: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(value).expect("입력")).expect("비율")
    }
    fn master() -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            instrument_id: id("sample-a"),
            symbol: Symbol::try_new("005930").expect("코드"),
            name: DisplayInstrumentName::try_new("고정 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: day(60),
        })
        .expect("마스터")
    }
    fn local(time: Timestamp) -> Timestamp {
        Timestamp::from_parts(
            time.utc_seconds(),
            time.nanosecond(),
            OffsetSeconds::new(32400),
        )
        .expect("현지 시각")
    }
    fn local_midnight(value: i64) -> Timestamp {
        local(at(day(value)
            .utc_seconds()
            .as_i64()
            .checked_sub(32400)
            .expect("현지 자정")))
    }
    fn day(value: i64) -> Timestamp {
        at(value.checked_mul(86400).expect("일"))
    }
    fn eligible() -> CostEligibleTask {
        eligible_for("102")
    }
    fn eligible_for(high: &str) -> CostEligibleTask {
        eligible_for_master(high, &master())
    }
    fn eligible_for_master(high: &str, source: &MasterInstrument) -> CostEligibleTask {
        let candles: Vec<_> = (0_i64..61)
            .map(|index| {
                let fixture_price = price(if index % 2 == 0 { "100" } else { high });
                Candle::try_new(
                    source.row().instrument_id.clone(),
                    CandleTime::try_new(local_midnight(index), CandlePeriod::Daily)
                        .expect("일봉 시각"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(day(61))),
                    Ohlcv::try_new(
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        quantity("13"),
                    )
                    .expect("OHLCV"),
                    CandleObservation::new(day(61), CandleStatus::Finalized),
                )
                .expect("일봉")
            })
            .collect();
        let volatility = Volatility::calculate(&candles).expect("변동성");
        let task = EstimateTask::create(
            EstimateContext::new(
                source
                    .price_observation(price("100"), local(day(60)))
                    .expect("관측"),
                day(60),
            ),
            volatility,
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제");
        let zero = CostRate::try_new(ratio("0")).expect("비용");
        let period = EstimateCostPeriod::from_calendar_result(
            &task,
            at(day(65)
                .utc_seconds()
                .as_i64()
                .checked_add(3600)
                .expect("끝")),
        )
        .expect("만기");
        let cost = TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                zero,
                CommissionValidity::try_new(None, None).expect("유효"),
            )),
            Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
            period,
        )
        .expect("비용");
        CostEligibleTask::assess(task, &cost).expect("적격")
    }

    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("종목")
    }
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
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
    fn task_id(value: &str) -> CalibrationEstimateId {
        CalibrationEstimateId::try_new(value).expect("과제 ID")
    }
    fn ai_run(value: &str) -> AiRunId {
        AiRunId::try_new(value).expect("실행")
    }
    fn identity(value: &str) -> ModelIdentityPart {
        ModelIdentityPart::try_new(value).expect("메타데이터")
    }
    fn input(key: &str, probability: &str) -> EstimateSubmissionInput {
        input_for_task(key, probability, &eligible())
    }
    fn input_for_task(
        key: &str,
        probability: &str,
        task: &CostEligibleTask,
    ) -> EstimateSubmissionInput {
        EstimateSubmissionInput {
            task_id: task_id(key),
            submission: SignalSubmission::create(
                SignalTaskKey::from_task(task),
                CalibrationKey::new(
                    CalibrationModel::new(
                        identity("claude"),
                        identity("actual-model"),
                        identity("revision-3"),
                    ),
                    Horizon::new(HorizonSelection::FewDays),
                ),
                AiProbability::parse(&AiProbabilityInput::try_new(probability).expect("확률"))
                    .expect("확률"),
                None,
                day(61),
            )
            .expect("제출"),
            narrative: EstimateNarrative {
                rationale: UntrustedText::try_new("근거 하나").expect("근거"),
                counterargument: UntrustedText::try_new("반론 둘").expect("반론"),
            },
            metadata: EstimateMetadata {
                effort: ReasoningEffort::High,
                cli_version: identity("2.7.3"),
                prompt: EstimatePrompt {
                    id: domain::ScorecardPromptId::try_new("estimate-prompt").expect("프롬프트"),
                    version: identity("v4"),
                    hash: PromptHash::try_new(
                        "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
                    )
                    .expect("해시"),
                },
                evidence_bundle: EvidenceBundleId::try_new("bundle-nine").expect("묶음"),
            },
        }
    }
    #[derive(Debug)]
    struct RepoState {
        assignments: EstimateAssignments<AiRunId>,
        records: Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>,
    }
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum RepositoryCall {
        Assignments,
        Commit,
    }
    #[derive(Debug)]
    struct Repo {
        state: RwLock<RepoState>,
        calls: RwLock<Vec<RepositoryCall>>,
        fail: RwLock<Option<SystemPortError>>,
        read_error: RwLock<Option<SystemPortError>>,
        barrier: Option<Barrier>,
    }
    impl Repo {
        fn new() -> Self {
            Self::with_second_task(eligible())
        }
        fn with_second_task(second: CostEligibleTask) -> Self {
            Self {
                calls: RwLock::new(Vec::new()),
                state: RwLock::new(RepoState {
                    assignments: EstimateAssignments::default()
                        .issue(task_id("one"), ai_run("alpha"), eligible())
                        .expect("발급")
                        .issue(task_id("two"), ai_run("alpha"), second)
                        .expect("발급")
                        .issue(task_id("three"), ai_run("beta"), eligible())
                        .expect("발급"),
                    records: Vec::new(),
                }),
                fail: RwLock::new(None),
                read_error: RwLock::new(None),
                barrier: None,
            }
        }
    }
    impl EstimateSubmissionRepository for Repo {
        fn assignments(
            &self,
        ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> {
            self.calls
                .write()
                .expect("호출 기록")
                .push(RepositoryCall::Assignments);
            let scripted_error = self.read_error.write().expect("잠금").take();
            if let Some(error) = scripted_error {
                return std::future::ready(Err(error));
            }
            let result = self.state.write().expect("잠금").assignments.clone();
            if let Some(barrier) = &self.barrier {
                barrier.wait();
            }
            std::future::ready(Ok(result))
        }
        fn commit_submission(
            &self,
            previous: &EstimateAssignments<AiRunId>,
            next: &EstimateAssignments<AiRunId>,
            records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
        ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> {
            self.calls
                .write()
                .expect("호출 기록")
                .push(RepositoryCall::Commit);
            let scripted_error = self.fail.write().expect("잠금").take();
            if let Some(error) = scripted_error {
                return std::future::ready(Err(error));
            }
            let mut state = self.state.write().expect("잠금");
            if &state.assignments != previous {
                return std::future::ready(Ok(EstimateCommitOutcome::Conflict));
            }
            state.assignments = next.clone();
            state.records.extend_from_slice(records);
            std::future::ready(Ok(EstimateCommitOutcome::Committed))
        }
    }
    #[derive(Debug)]
    struct Listener {
        repo: Arc<Repo>,
        events: Arc<RwLock<Vec<DomainEvent>>>,
        result: Result<(), SubscriberFailure>,
    }
    impl Listener {
        fn verify_recorded(&self) {
            assert_ne!(self.repo.state.write().expect("잠금").records, Vec::new());
        }
    }
    impl EventSubscriber for Listener {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.verify_recorded();
            match self.events.write() {
                Ok(mut events) => events.push(event.clone()),
                Err(poison) => poison.into_inner().push(event.clone()),
            }
            self.result
        }
    }
    fn bus(
        repo: &Arc<Repo>,
        result: Result<(), SubscriberFailure>,
    ) -> (application::EventBus, Arc<RwLock<Vec<DomainEvent>>>) {
        let events = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            std::iter::once(DomainEventKind::EstimateSubmitted).collect(),
            Listener {
                repo: Arc::clone(repo),
                events: Arc::clone(&events),
                result,
            },
        );
        (builder.build(), events)
    }
    fn other_task() -> CostEligibleTask {
        let mut row = master().row().clone();
        row.instrument_id = id("sample-b");
        row.symbol = Symbol::try_new("000660").expect("다른 코드");
        eligible_for_master("103", &MasterInstrument::try_new(row).expect("다른 마스터"))
    }
    #[test]
    fn complete_records_are_saved_before_exactly_one_event_each() {
        let second_task = other_task();
        let repo = Arc::new(Repo::with_second_task(second_task.clone()));
        let (events, seen) = bus(&repo, Ok(()));
        let first = input("one", "17");
        let mut second = input_for_task("two", "83", &second_task);
        second.narrative.rationale = UntrustedText::try_new("다른 근거 셋").expect("근거");
        second.narrative.counterargument = UntrustedText::try_new("다른 반론 넷").expect("반론");
        second.metadata.effort = ReasoningEffort::Low;
        second.metadata.cli_version = identity("3.8.9");
        second.metadata.prompt.id =
            domain::ScorecardPromptId::try_new("another-prompt").expect("다른 프롬프트");
        second.metadata.prompt.version = identity("v7");
        second.metadata.evidence_bundle =
            EvidenceBundleId::try_new("bundle-twelve").expect("다른 묶음");
        let expected = vec![first.clone(), second.clone()];
        assert_eq!(
            run(SubmitEstimate::new(repo.as_ref(), &events)
                .execute(&ai_run("alpha"), vec![first, second]))
            .expect("수락")
            .len(),
            2
        );
        let state = repo.state.write().expect("잠금");
        assert_eq!(state.records.len(), 2);
        for (record, request) in state.records.iter().zip(expected) {
            assert_eq!(record.task_id().as_estimate_id(), &request.task_id);
            assert_eq!(record.run(), &ai_run("alpha"));
            assert_eq!(record.submission(), &request.submission);
            assert_eq!(record.narrative(), &request.narrative);
            assert_eq!(record.metadata(), &request.metadata);
            assert_eq!(record.source_status(), EstimateSourceStatus::Missing);
        }
        assert_eq!(
            *seen.write().expect("잠금"),
            vec![
                DomainEvent::EstimateSubmitted(id("sample-a")),
                DomainEvent::EstimateSubmitted(id("sample-b"))
            ]
        );
    }
    #[test]
    fn invalid_batches_leave_every_record_and_assignment_untouched() {
        for (requests, error) in [
            (
                vec![input("unknown", "21")],
                EstimateAssignmentError::UnknownTask,
            ),
            (
                vec![input("three", "22")],
                EstimateAssignmentError::WrongRun,
            ),
            (
                vec![input("one", "23"), input("two", "24"), input("one", "25")],
                EstimateAssignmentError::AssignmentExceeded,
            ),
            (
                vec![input("one", "26"), input("one", "27")],
                EstimateAssignmentError::AlreadyConsumed,
            ),
            (
                vec![input("one", "28"), input("unknown", "29")],
                EstimateAssignmentError::UnknownTask,
            ),
        ] {
            let repo = Arc::new(Repo::new());
            let before = repo.state.write().expect("잠금").assignments.clone();
            let (events, seen) = bus(&repo, Ok(()));
            assert_eq!(
                run(SubmitEstimate::new(repo.as_ref(), &events).execute(&ai_run("alpha"), requests)),
                Err(SubmitEstimateError::Assignment(error))
            );
            let state = repo.state.write().expect("잠금");
            assert_eq!(state.assignments, before);
            assert_eq!(state.records, Vec::new());
            assert_eq!(*seen.write().expect("잠금"), Vec::new());
        }
    }
    #[test]
    fn duplicate_after_success_never_records_or_publishes_again() {
        let repo = Arc::new(Repo::new());
        let (events, seen) = bus(&repo, Ok(()));
        let service = SubmitEstimate::new(repo.as_ref(), &events);
        run(service.execute(&ai_run("alpha"), vec![input("one", "31")])).expect("수락");
        assert_eq!(
            run(service.execute(&ai_run("alpha"), vec![input("one", "72")])),
            Err(SubmitEstimateError::Assignment(
                EstimateAssignmentError::AlreadyConsumed
            ))
        );
        assert_eq!(repo.state.write().expect("잠금").records.len(), 1);
        assert_eq!(seen.write().expect("잠금").len(), 1);
    }
    #[test]
    fn failed_commit_does_not_consume_or_publish_and_can_retry() {
        let repo = Arc::new(Repo::new());
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        *repo.fail.write().expect("잠금") = Some(error);
        let (events, seen) = bus(&repo, Ok(()));
        let service = SubmitEstimate::new(repo.as_ref(), &events);
        assert_eq!(
            run(service.execute(&ai_run("alpha"), vec![input("one", "41")])),
            Err(SubmitEstimateError::Storage(error))
        );
        assert_eq!(repo.state.write().expect("잠금").records, Vec::new());
        assert_eq!(*seen.write().expect("잠금"), Vec::new());
        run(service.execute(&ai_run("alpha"), vec![input("one", "42")])).expect("재시도");
        assert_eq!(seen.write().expect("잠금").len(), 1);
    }
    #[test]
    fn simultaneous_different_payloads_accept_only_one() {
        let mut repository = Repo::new();
        repository.barrier = Some(Barrier::new(2));
        let repo = Arc::new(repository);
        let (events, seen) = bus(&repo, Ok(()));
        let service = SubmitEstimate::new(repo.as_ref(), &events);
        let results = std::thread::scope(|scope| {
            let first_thread =
                scope.spawn(|| run(service.execute(&ai_run("alpha"), vec![input("one", "16")])));
            let second_thread =
                scope.spawn(|| run(service.execute(&ai_run("alpha"), vec![input("one", "87")])));
            [
                first_thread.join().expect("첫 스레드"),
                second_thread.join().expect("둘째 스레드"),
            ]
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == Err(SubmitEstimateError::ConcurrentUpdate))
                .count(),
            1
        );
        assert_eq!(repo.state.write().expect("잠금").records.len(), 1);
        assert_eq!(seen.write().expect("잠금").len(), 1);
    }
    #[test]
    fn delivery_failure_preserves_success_without_republishing() {
        let repo = Arc::new(Repo::new());
        let (events, seen) = bus(&repo, Err(SubscriberFailure::Unavailable));
        let service = SubmitEstimate::new(repo.as_ref(), &events);
        let reports =
            run(service.execute(&ai_run("alpha"), vec![input("one", "58")])).expect("기록 성공");
        assert_eq!(reports.first().expect("보고").failures.len(), 1);
        assert_eq!(repo.state.write().expect("잠금").records.len(), 1);
        assert_eq!(seen.write().expect("잠금").len(), 1);
    }
    #[test]
    fn empty_batch_has_no_effect() {
        let repo = Arc::new(Repo::new());
        let (events, seen) = bus(&repo, Ok(()));
        assert_eq!(
            run(SubmitEstimate::new(repo.as_ref(), &events).execute(&ai_run("alpha"), Vec::new())),
            Ok(Vec::new())
        );
        assert_eq!(*repo.calls.write().expect("호출 기록"), Vec::new());
        assert_eq!(repo.state.write().expect("잠금").records, Vec::new());
        assert_eq!(*seen.write().expect("잠금"), Vec::new());
    }
    #[test]
    fn invalid_record_rolls_back_entire_batch() {
        let repo = Arc::new(Repo::new());
        let (events, seen) = bus(&repo, Ok(()));
        let mut request = input("two", "64");
        let original = request.submission;
        request.submission = SignalSubmission::create(
            original.task_key().clone(),
            CalibrationKey::new(
                original.calibration_key().model().clone(),
                Horizon::new(HorizonSelection::WeeksToMonths),
            ),
            original.probability(),
            None,
            original.submitted_at(),
        )
        .expect("제출");
        assert_eq!(
            run(SubmitEstimate::new(repo.as_ref(), &events)
                .execute(&ai_run("alpha"), vec![input("one", "23"), request])),
            Err(SubmitEstimateError::Record(
                domain::EstimateRecordError::HorizonMismatch
            ))
        );
        assert_eq!(repo.state.write().expect("잠금").records, Vec::new());
        assert_eq!(*seen.write().expect("잠금"), Vec::new());
        run(SubmitEstimate::new(repo.as_ref(), &events).execute(
            &ai_run("alpha"),
            vec![input("one", "24"), input("two", "65")],
        ))
        .expect("둘 다 소비 가능");
    }
    #[test]
    fn read_failure_preserves_exact_cause_and_never_commits_or_publishes() {
        let repo = Arc::new(Repo::new());
        let (events, seen) = bus(&repo, Ok(()));
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        *repo.read_error.write().expect("잠금") = Some(error);
        assert_eq!(
            run(SubmitEstimate::new(repo.as_ref(), &events)
                .execute(&ai_run("alpha"), vec![input("one", "46")])),
            Err(SubmitEstimateError::Storage(error))
        );
        assert_eq!(repo.state.write().expect("잠금").records, Vec::new());
        assert_eq!(*seen.write().expect("잠금"), Vec::new());
    }
    #[test]
    fn task_mismatch_preserves_every_assignment_and_record() {
        let repo = Arc::new(Repo::new());
        let (events, seen) = bus(&repo, Ok(()));
        let before = repo.state.write().expect("잠금").assignments.clone();
        let invalid = input_for_task("two", "74", &other_task());
        assert_eq!(
            run(SubmitEstimate::new(repo.as_ref(), &events)
                .execute(&ai_run("alpha"), vec![input("one", "32"), invalid])),
            Err(SubmitEstimateError::Record(
                domain::EstimateRecordError::TaskMismatch
            ))
        );
        let state = repo.state.write().expect("잠금");
        assert_eq!(state.assignments, before);
        assert_eq!(state.records, Vec::new());
        assert_eq!(*seen.write().expect("잠금"), Vec::new());
        assert_eq!(
            *repo.calls.write().expect("호출 기록"),
            vec![RepositoryCall::Assignments]
        );
    }

    fn collide_distinct(
        repo: &Arc<Repo>,
        second_run: &AiRunId,
        second_id: &CalibrationEstimateId,
    ) -> (AiRunId, CalibrationEstimateId) {
        let (events, seen) = bus(repo, Ok(()));
        let service = SubmitEstimate::new(repo.as_ref(), &events);
        let results = std::thread::scope(|scope| {
            let first =
                scope.spawn(|| run(service.execute(&ai_run("alpha"), vec![input("one", "36")])));
            let second = scope
                .spawn(|| run(service.execute(second_run, vec![input(second_id.as_str(), "79")])));
            [
                first.join().expect("첫 스레드"),
                second.join().expect("둘째 스레드"),
            ]
        });
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| **result == Err(SubmitEstimateError::ConcurrentUpdate))
                .count(),
            1
        );
        assert_eq!(repo.state.write().expect("잠금").records.len(), 1);
        assert_eq!(seen.write().expect("잠금").len(), 1);
        let failed = results.iter().position(Result::is_err).expect("충돌 하나");
        if failed == 0 {
            (ai_run("alpha"), task_id("one"))
        } else {
            (second_run.clone(), second_id.clone())
        }
    }
    #[test]
    fn distinct_tasks_conflict_without_consumption_and_retry_from_latest_state() {
        for (second_run, second_id) in [
            (ai_run("alpha"), task_id("two")),
            (ai_run("beta"), task_id("three")),
        ] {
            let mut repository = Repo::new();
            repository.barrier = Some(Barrier::new(2));
            let mut repo = Arc::new(repository);
            let (retry_run, retry_id) = collide_distinct(&repo, &second_run, &second_id);
            assert_eq!(
                repo.state
                    .write()
                    .expect("잠금")
                    .assignments
                    .lookup(&retry_id)
                    .expect("과제")
                    .consumption(),
                domain::EstimateConsumption::Available
            );
            Arc::get_mut(&mut repo).expect("경쟁 종료").barrier = None;
            let (events, seen) = bus(&repo, Ok(()));
            run(SubmitEstimate::new(repo.as_ref(), &events)
                .execute(&retry_run, vec![input(retry_id.as_str(), "79")]))
            .expect("최신 원장 재시도");
            let stored = repo.state.write().expect("잠금");
            assert_eq!(stored.records.len(), 2);
            assert_eq!(
                stored
                    .assignments
                    .lookup(&retry_id)
                    .expect("과제")
                    .consumption(),
                domain::EstimateConsumption::Consumed
            );
            assert_eq!(seen.write().expect("잠금").len(), 1);
        }
    }
}
