#[cfg(test)]
mod tests {
    use application::{
        AiRunId, EstimateCommitOutcome, EstimateSubmissionInput, EstimateSubmissionRepository,
        EventBusBuilder, EventSubscriber, ReasoningEffort, SubmitEstimate, SubscriberFailure,
        SystemFailure, SystemPort, SystemPortError,
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
        DomainEvent, DomainEventKind, EstimateAssignments, EstimateMetadata, EstimateNarrative,
        EstimatePrompt, EstimateSubmissionRecord, EvidenceBundleId, ModelIdentityPart, PromptHash,
        SignalSubmission, SignalTaskKey, UntrustedText,
    };
    use std::{
        future::Future,
        sync::{Arc, Barrier, RwLock, mpsc},
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
    use application::resolve_estimates::{
        EstimateResolutionRepository, ResolutionCalibration, ResolutionCommitOutcome,
        ResolutionDataIssue, ResolutionInputs, ResolutionMarketData, ResolutionReport,
        ResolutionStatus, ResolveEstimates,
    };
    use domain::{
        Calibration, CalibrationLedgerHead, CalibrationPosition, CalibrationSnapshot,
        Candle as Bar, EstimateOutcome, MarketCalendar, MarketSession, OutcomeAdjustment,
        OutcomeFailure, OutcomeState, OutcomeUnavailable, SessionData, SessionInformation,
        TimeInterval,
    };

    fn row(day_number: i64) -> MarketSession {
        let start = local(day(day_number));
        let end = local(at(day(day_number)
            .utc_seconds()
            .as_i64()
            .checked_add(240)
            .expect("끝")));
        MarketSession::try_new(
            start.offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(TimeInterval::try_new(start, end).expect("구간"), None)
                        .expect("장"),
                ),
                after_market: None,
            },
        )
        .expect("장 일정")
    }
    fn calendars() -> Vec<MarketCalendar> {
        (60_i64..=65)
            .map(|index| {
                MarketCalendar::try_new(
                    row(index.checked_sub(1).expect("전일")),
                    row(index),
                    row(index.checked_add(1).expect("익일")),
                )
                .expect("달력")
            })
            .collect()
    }
    fn adjusted(task: &CostEligibleTask) -> OutcomeAdjustment {
        OutcomeAdjustment::from_actions(task.task(), AdjustmentBasis::new(local(day(70))), &[])
            .expect("수정")
    }
    fn bar(
        task: &CostEligibleTask,
        factor: &OutcomeAdjustment,
        date: i64,
        minute: i64,
        prices: (&str, &str),
    ) -> Bar {
        let (high, low) = prices;
        let period = if minute == 0 {
            CandlePeriod::Daily
        } else {
            CandlePeriod::Minute
        };
        let timestamp = if minute == 0 {
            local_midnight(date)
        } else {
            local(at(day(date)
                .utc_seconds()
                .as_i64()
                .checked_add(minute.checked_mul(60).expect("초"))
                .expect("분")))
        };
        Bar::try_new(
            task.task().volatility().instrument().clone(),
            CandleTime::try_new(timestamp, period).expect("시각"),
            PriceAdjustment::Adjusted(factor.basis()),
            Ohlcv::try_new(
                price(low),
                price(high),
                price(low),
                price(low),
                quantity("13"),
            )
            .expect("가격"),
            CandleObservation::new(day(70), CandleStatus::Finalized),
        )
        .expect("봉")
    }
    fn inputs(task: &CostEligibleTask) -> ResolutionInputs {
        let factor = adjusted(task);
        let mut candles: Vec<_> = (1..=4)
            .map(|minute| bar(task, &factor, 60, minute, ("100", "100")))
            .collect();
        candles.extend((61..=65).map(|date| bar(task, &factor, date, 0, ("100", "100"))));
        ResolutionInputs {
            calendars: calendars(),
            adjustment: factor,
            candles,
        }
    }
    fn issued_records(
        tasks: &[CostEligibleTask],
    ) -> (EstimateAssignments<AiRunId>, Vec<EstimateSubmissionInput>) {
        let mut assignments = EstimateAssignments::default();
        let mut requests = Vec::new();
        for (index, task) in tasks.iter().enumerate() {
            let key = format!("estimate-{index}");
            assignments = assignments
                .issue(task_id(&key), ai_run("alpha"), task.clone())
                .expect("발급");
            requests.push(input_for_task(
                &key,
                if index % 2 == 0 { "71" } else { "63" },
                task,
            ));
        }
        (assignments, requests)
    }
    #[derive(Debug)]
    struct State {
        assignments: EstimateAssignments<AiRunId>,
        submissions: Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>,
        outcomes: Vec<(CalibrationEstimateId, EstimateOutcome)>,
        tables: Vec<(CalibrationKey, ResolutionCalibration)>,
        unavailable: Vec<(CalibrationEstimateId, ResolutionDataIssue)>,
        fail: bool,
    }
    #[derive(Debug)]
    struct Repo {
        state: RwLock<State>,
        barrier: Option<Barrier>,
        forced_commit: Option<ResolutionCommitOutcome>,
    }
    impl EstimateSubmissionRepository for Repo {
        fn submission_records(
            &self,
            instrument: &InstrumentId,
        ) -> impl Future<
            Output = Result<
                Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>,
                SystemPortError,
            >,
        > {
            let records = self
                .state
                .read()
                .expect("읽기")
                .submissions
                .iter()
                .filter(|record| {
                    record
                        .submission()
                        .task_key()
                        .task()
                        .volatility()
                        .instrument()
                        == instrument
                })
                .cloned()
                .collect();
            std::future::ready(Ok(records))
        }
        fn assignments(
            &self,
        ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> {
            std::future::ready(Ok(self.state.read().expect("읽기").assignments.clone()))
        }
        fn commit_submission(
            &self,
            previous: &EstimateAssignments<AiRunId>,
            next: &EstimateAssignments<AiRunId>,
            records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
        ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> {
            let mut state = self.state.write().expect("쓰기");
            if &state.assignments != previous {
                return std::future::ready(Ok(EstimateCommitOutcome::Conflict));
            }
            state.assignments = next.clone();
            state.submissions.extend_from_slice(records);
            std::future::ready(Ok(EstimateCommitOutcome::Committed))
        }
    }
    fn storage_error() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
    }
    fn pending(state: &State) -> Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>> {
        state
            .submissions
            .iter()
            .filter(|record| {
                !state
                    .outcomes
                    .iter()
                    .any(|(id, _)| id == record.task_id().as_estimate_id())
            })
            .cloned()
            .collect()
    }
    impl EstimateResolutionRepository for Repo {
        fn pending_submissions(
            &self,
        ) -> impl Future<
            Output = Result<
                Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>,
                SystemPortError,
            >,
        > {
            let records = pending(&self.state.read().expect("읽기"));
            if let Some(barrier) = &self.barrier {
                barrier.wait();
            }
            std::future::ready(Ok(records))
        }
        fn calibration(
            &self,
            key: &CalibrationKey,
        ) -> impl Future<Output = Result<Option<ResolutionCalibration>, SystemPortError>> {
            let table = self
                .state
                .read()
                .expect("읽기")
                .tables
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, table)| table.clone());
            std::future::ready(Ok(table))
        }
        fn resolution(
            &self,
            id: &CalibrationEstimateId,
        ) -> impl Future<Output = Result<Option<EstimateOutcome>, SystemPortError>> {
            let state = self.state.read().expect("읽기");
            if state.fail {
                return std::future::ready(Err(storage_error()));
            }
            let outcome = state
                .outcomes
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map(|(_, value)| value.clone());
            std::future::ready(Ok(outcome))
        }
        fn record_unavailable(
            &self,
            record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
            reason: ResolutionDataIssue,
        ) -> impl Future<Output = Result<ResolutionCommitOutcome, SystemPortError>> {
            let mut state = self.state.write().expect("쓰기");
            if state.fail {
                return std::future::ready(Err(storage_error()));
            }
            let id = record.task_id().as_estimate_id();
            if state.outcomes.iter().any(|(candidate, _)| candidate == id) {
                return std::future::ready(Ok(ResolutionCommitOutcome::AlreadyResolved));
            }
            state.unavailable.retain(|(candidate, _)| candidate != id);
            state.unavailable.push((id.clone(), reason));
            std::future::ready(Ok(ResolutionCommitOutcome::Committed))
        }
        fn commit_resolution(
            &self,
            record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
            outcome: &EstimateOutcome,
            previous: &ResolutionCalibration,
            next: &Calibration,
        ) -> impl Future<Output = Result<ResolutionCommitOutcome, SystemPortError>> {
            if let Some(result) = self.forced_commit {
                return std::future::ready(Ok(result));
            }
            let mut state = self.state.write().expect("쓰기");
            if state.fail {
                return std::future::ready(Err(storage_error()));
            }
            let id = record.task_id().as_estimate_id();
            let key = record.submission().calibration_key();
            if state.outcomes.iter().any(|(candidate, _)| candidate == id) {
                return std::future::ready(Ok(ResolutionCommitOutcome::AlreadyResolved));
            }
            let current = state
                .tables
                .iter()
                .find(|(candidate, _)| candidate == key)
                .map(|(_, table)| table);
            if current != Some(previous) {
                return std::future::ready(Ok(ResolutionCommitOutcome::Conflict));
            }
            let Some(next_position) = previous.head.position().as_u64().checked_add(1) else {
                return std::future::ready(Err(storage_error()));
            };
            let head =
                CalibrationLedgerHead::new(key.clone(), CalibrationPosition::new(next_position));
            let snapshot = match CalibrationSnapshot::try_new(next.clone(), head.clone()) {
                Ok(value) => value,
                Err(_error) => return std::future::ready(Err(storage_error())),
            };
            state.outcomes.push((id.clone(), outcome.clone()));
            state.unavailable.retain(|(candidate, _)| candidate != id);
            state.tables.retain(|(candidate, _)| candidate != key);
            state
                .tables
                .push((key.clone(), ResolutionCalibration { snapshot, head }));
            std::future::ready(Ok(ResolutionCommitOutcome::Committed))
        }
    }
    #[derive(Debug)]
    struct Data(
        Vec<(
            CalibrationEstimateId,
            Result<ResolutionInputs, ResolutionDataIssue>,
        )>,
    );
    impl ResolutionMarketData for Data {
        fn inputs(
            &self,
            record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
            _now: Timestamp,
        ) -> impl Future<Output = Result<ResolutionInputs, ResolutionDataIssue>> {
            let value = self
                .0
                .iter()
                .find(|(id, _)| id == record.task_id().as_estimate_id())
                .expect("자료")
                .1
                .clone();
            std::future::ready(value)
        }
    }
    #[derive(Debug)]
    struct CapturingData {
        data: Data,
        calls: RwLock<Vec<(CalibrationEstimateId, Timestamp)>>,
    }
    impl ResolutionMarketData for CapturingData {
        fn inputs(
            &self,
            record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
            now: Timestamp,
        ) -> impl Future<Output = Result<ResolutionInputs, ResolutionDataIssue>> {
            self.calls
                .write()
                .expect("조회 기록")
                .push((record.task_id().as_estimate_id().clone(), now));
            self.data.inputs(record, now)
        }
    }
    #[derive(Debug, PartialEq, Eq)]
    enum DelaySignal {
        Entered,
        Finished,
    }
    #[derive(Debug)]
    struct DelayedData {
        data: Data,
        entered: mpsc::Sender<DelaySignal>,
        resume: Arc<Barrier>,
    }
    impl ResolutionMarketData for DelayedData {
        fn inputs(
            &self,
            record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
            now: Timestamp,
        ) -> impl Future<Output = Result<ResolutionInputs, ResolutionDataIssue>> {
            self.entered.send(DelaySignal::Entered).expect("조회 시작");
            self.resume.wait();
            self.data.inputs(record, now)
        }
    }
    fn delayed_reports(
        repo: &Repo,
        market: &DelayedData,
        bus: &application::EventBus,
        finished: &mpsc::Sender<DelaySignal>,
    ) -> Vec<ResolutionReport> {
        let reports = run(ResolveEstimates::new(repo, market, bus).execute(day(70)));
        finished.send(DelaySignal::Finished).expect("실행 종료");
        reports.expect("판정 실행")
    }
    #[derive(Clone)]
    struct Collector(Arc<RwLock<Vec<DomainEvent>>>);
    impl EventSubscriber for Collector {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            match self.0.write() {
                Ok(mut events) => events.push(event.clone()),
                Err(poison) => poison.into_inner().push(event.clone()),
            }
            Ok(())
        }
    }
    fn events() -> (application::EventBus, Arc<RwLock<Vec<DomainEvent>>>) {
        let collected = Arc::new(RwLock::new(Vec::new()));
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            std::iter::once(DomainEventKind::EstimateResolved).collect(),
            Collector(Arc::clone(&collected)),
        );
        (builder.build(), collected)
    }
    fn calibration_state(
        key: &CalibrationKey,
        table: Calibration,
        position: CalibrationPosition,
    ) -> ResolutionCalibration {
        let head = CalibrationLedgerHead::new(key.clone(), position);
        let snapshot = CalibrationSnapshot::try_new(table, head.clone()).expect("반영 위치");
        ResolutionCalibration { snapshot, head }
    }
    fn initialize_calibrations(state: &mut State) {
        let keys: Vec<_> = state
            .submissions
            .iter()
            .map(|record| record.submission().calibration_key().clone())
            .collect();
        for key in keys {
            if state.tables.iter().all(|(stored, _)| stored != &key) {
                let table = Calibration::try_new(key.clone(), &[]).expect("빈 표");
                let value = calibration_state(&key, table, CalibrationPosition::new(0));
                state.tables.push((key, value));
            }
        }
    }
    fn setup(tasks: &[CostEligibleTask]) -> Repo {
        let (assignments, requests) = issued_records(tasks);
        let repo = Repo {
            state: RwLock::new(State {
                assignments,
                submissions: Vec::new(),
                outcomes: Vec::new(),
                tables: Vec::new(),
                unavailable: Vec::new(),
                fail: false,
            }),
            barrier: None,
            forced_commit: None,
        };
        let (bus, _) = events();
        run(SubmitEstimate::new(&repo, &bus).execute(&ai_run("alpha"), requests)).expect("제출");
        initialize_calibrations(&mut repo.state.write().expect("쓰기"));
        repo
    }
    fn data(tasks: &[CostEligibleTask]) -> Data {
        Data(
            tasks
                .iter()
                .enumerate()
                .map(|(index, task)| (task_id(&format!("estimate-{index}")), Ok(inputs(task))))
                .collect(),
        )
    }
    fn resolved_state(status: &ResolutionStatus) -> Option<OutcomeState> {
        match status {
            ResolutionStatus::Resolved { outcome, .. } => Some(outcome.state()),
            ResolutionStatus::Ongoing
            | ResolutionStatus::Unavailable(_)
            | ResolutionStatus::Storage(_)
            | ResolutionStatus::Calibration(_)
            | ResolutionStatus::MissingFact
            | ResolutionStatus::AlreadyResolved
            | ResolutionStatus::Conflict => None,
        }
    }
    fn assert_first_run(repo: &Repo, collected: &RwLock<Vec<DomainEvent>>) {
        let state = repo.state.read().expect("읽기");
        assert_eq!(state.outcomes.len(), 3);
        assert!(
            state
                .outcomes
                .iter()
                .all(|(_, outcome)| outcome.state()
                    == OutcomeState::Failure(OutcomeFailure::Expired))
        );
        assert_eq!(state.tables.len(), 1);
        assert_eq!(
            state.tables[0].1.snapshot.through(),
            &state.tables[0].1.head
        );
        assert_eq!(*state.tables[0].1.head.position().as_u64(), 3);
        assert_eq!(state.tables[0].1.snapshot.calibration().facts().len(), 3);
        assert_eq!(
            state.tables[0]
                .1
                .snapshot
                .calibration()
                .for_combination(&state.tables[0].0)
                .expect("같은 조합")
                .judgment_count()
                .as_usize(),
            1
        );
        assert_eq!(collected.read().expect("이벤트").len(), 3);
    }
    #[test]
    fn every_submitted_estimate_is_enumerated_resolved_and_published_once() {
        let tasks = vec![eligible(), eligible(), eligible()];
        let repo = setup(&tasks);
        assert_eq!(run(repo.pending_submissions()).expect("열거").len(), 3);
        let (bus, collected) = events();
        let report =
            run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70))).expect("실행");
        assert_eq!(report.len(), 3);
        assert_eq!(
            report
                .iter()
                .filter_map(|entry| resolved_state(&entry.status))
                .count(),
            3
        );
        assert_first_run(&repo, &collected);
        assert_eq!(
            run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70)))
                .expect("재실행"),
            Vec::new()
        );
    }
    #[test]
    fn report_keeps_the_identity_of_each_submitted_estimate() {
        let tasks = vec![eligible(), eligible()];
        let repo = setup(&tasks);
        let (bus, _) = events();
        let reports =
            run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70))).expect("판정");
        assert_eq!(
            reports
                .into_iter()
                .map(|report| report.id)
                .collect::<Vec<_>>(),
            vec![task_id("estimate-0"), task_id("estimate-1")]
        );
    }
    #[test]
    fn commit_conflict_keeps_the_estimate_pending_without_an_event() {
        let tasks = vec![eligible()];
        let mut repo = setup(&tasks);
        let initial = repo.state.read().expect("읽기").tables.clone();
        repo.forced_commit = Some(ResolutionCommitOutcome::Conflict);
        let (bus, collected) = events();
        let reports =
            run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70))).expect("판정");
        assert_eq!(reports[0].status, ResolutionStatus::Conflict);
        let state = repo.state.read().expect("읽기");
        assert_eq!(pending(&state).len(), 1);
        assert_eq!(state.outcomes, Vec::new());
        assert_eq!(state.tables, initial);
        assert!(collected.read().expect("이벤트").is_empty());
    }
    #[test]
    fn missing_calibration_snapshot_keeps_the_estimate_pending() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        repo.state.write().expect("쓰기").tables.clear();
        let (bus, collected) = events();
        let reports =
            run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70))).expect("판정");
        assert_eq!(
            reports[0].status,
            ResolutionStatus::Calibration(domain::CalibrationError::MissingCalibration)
        );
        let state = repo.state.read().expect("읽기");
        assert_eq!(pending(&state).len(), 1);
        assert_eq!(state.outcomes, Vec::new());
        assert_eq!(state.tables, Vec::new());
        assert!(collected.read().expect("이벤트").is_empty());
    }
    #[test]
    fn stale_calibration_snapshot_never_commits_or_publishes() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let key = repo.state.read().expect("읽기").tables[0].0.clone();
        repo.state.write().expect("쓰기").tables[0].1.head =
            CalibrationLedgerHead::new(key, CalibrationPosition::new(1));
        let initial = repo.state.read().expect("읽기").tables.clone();
        let (bus, collected) = events();
        let reports =
            run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70))).expect("판정");
        assert_eq!(
            reports[0].status,
            ResolutionStatus::Calibration(domain::CalibrationError::StaleCalibration)
        );
        let state = repo.state.read().expect("읽기");
        assert_eq!(pending(&state).len(), 1);
        assert_eq!(state.outcomes, Vec::new());
        assert_eq!(state.tables, initial);
        assert!(collected.read().expect("이벤트").is_empty());
    }
    #[test]
    fn calibration_head_for_another_model_is_rejected() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let horizon = repo.state.read().expect("읽기").tables[0].0.horizon();
        let model = CalibrationModel::new(identity("other"), identity("model"), identity("v1"));
        let key = CalibrationKey::new(model, horizon);
        repo.state.write().expect("쓰기").tables[0].1.head =
            CalibrationLedgerHead::new(key, CalibrationPosition::new(0));
        let initial = repo.state.read().expect("읽기").tables.clone();
        let (bus, collected) = events();
        let reports =
            run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70))).expect("판정");
        assert_eq!(
            reports[0].status,
            ResolutionStatus::Calibration(domain::CalibrationError::MismatchedCombination)
        );
        let state = repo.state.read().expect("읽기");
        assert_eq!(state.outcomes, Vec::new());
        assert_eq!(state.tables, initial);
        assert!(collected.read().expect("이벤트").is_empty());
    }
    #[test]
    fn changed_ledger_head_rejects_stale_atomic_commit() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let market = data(&tasks);
        let (record, previous) = {
            let state = repo.state.read().expect("읽기");
            (state.submissions[0].clone(), state.tables[0].1.clone())
        };
        let input = market.0[0].1.as_ref().expect("입력");
        let outcome = EstimateOutcome::evaluate_with_complete_data(
            record.submission().task_key().task(),
            &input.calendars,
            &input.adjustment,
            &input.candles,
            day(70),
        );
        let key = record.submission().calibration_key();
        let fact = outcome
            .calibration_fact(
                record.task_id().as_estimate_id().clone(),
                key.model().clone(),
                record.submission().probability(),
            )
            .expect("사실");
        let next = previous
            .snapshot
            .calibration()
            .with_fact(fact)
            .expect("새 표");
        repo.state.write().expect("쓰기").tables[0].1.head =
            CalibrationLedgerHead::new(key.clone(), CalibrationPosition::new(1));
        assert_eq!(
            run(repo.commit_resolution(&record, &outcome, &previous, &next)),
            Ok(ResolutionCommitOutcome::Conflict)
        );
        let state = repo.state.read().expect("읽기");
        assert_eq!(state.outcomes, Vec::new());
        assert_eq!(state.tables[0].1.snapshot, previous.snapshot);
        assert_eq!(*state.tables[0].1.head.position().as_u64(), 1);
    }
    #[test]
    fn target_stop_and_same_minute_ambiguity_follow_domain() {
        let tasks = vec![eligible(), eligible(), eligible(), eligible()];
        let repo = setup(&tasks);
        let mut market = data(&tasks);
        for (index, (high, low)) in [("200", "100"), ("100", "1"), ("200", "1")]
            .into_iter()
            .enumerate()
        {
            let input = market.0[index].1.as_mut().expect("입력");
            input.candles[0] = bar(&tasks[index], &input.adjustment, 60, 1, (high, low));
        }
        let simultaneous_day = market.0[3].1.as_mut().expect("입력");
        simultaneous_day.candles[4] =
            bar(&tasks[3], &simultaneous_day.adjustment, 61, 0, ("200", "1"));
        simultaneous_day.candles.push(bar(
            &tasks[3],
            &simultaneous_day.adjustment,
            61,
            1,
            ("200", "1"),
        ));
        let (bus, _) = events();
        let report =
            run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("실행");
        let states: Vec<_> = report
            .iter()
            .map(|entry| resolved_state(&entry.status).expect("판정"))
            .collect();
        assert_eq!(
            states,
            vec![
                OutcomeState::Success,
                OutcomeState::Failure(OutcomeFailure::Stop),
                OutcomeState::Failure(OutcomeFailure::Ambiguous),
                OutcomeState::Failure(OutcomeFailure::Ambiguous)
            ]
        );
    }
    #[test]
    fn incomplete_minute_and_daily_data_remain_pending_and_retry() {
        let tasks = vec![eligible(), eligible()];
        let repo = setup(&tasks);
        let initial = repo.state.read().expect("읽기").tables.clone();
        let mut market = data(&tasks);
        market.0[0].1.as_mut().expect("자료").candles.retain(|bar| {
            bar.time().timestamp()
                != local(at(day(60)
                    .utc_seconds()
                    .as_i64()
                    .checked_add(120)
                    .expect("시각")))
        });
        market.0[1]
            .1
            .as_mut()
            .expect("자료")
            .candles
            .retain(|bar| bar.time().date() != local(day(62)).offset_calendar_date());
        let (bus, collected) = events();
        let report =
            run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("실행");
        assert!(report.iter().all(|entry| entry.status
            == ResolutionStatus::Unavailable(ResolutionDataIssue::Incomplete(
                OutcomeUnavailable::MissingCandles
            ))));
        assert_eq!(run(repo.pending_submissions()).expect("조회").len(), 2);
        assert_eq!(*collected.read().expect("이벤트"), Vec::new());
        assert_eq!(repo.state.read().expect("읽기").tables, initial);
        assert_eq!(
            repo.state.read().expect("읽기").unavailable,
            vec![
                (
                    task_id("estimate-0"),
                    ResolutionDataIssue::Incomplete(OutcomeUnavailable::MissingCandles)
                ),
                (
                    task_id("estimate-1"),
                    ResolutionDataIssue::Incomplete(OutcomeUnavailable::MissingCandles)
                )
            ]
        );
        assert_eq!(
            run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70)))
                .expect("재시도")
                .len(),
            2
        );
        assert_eq!(repo.state.read().expect("읽기").unavailable, Vec::new());
    }
    #[test]
    fn ongoing_and_storage_failure_do_not_commit_or_publish() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let initial = repo.state.read().expect("읽기").tables.clone();
        let (bus, collected) = events();
        let ongoing = run(
            ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(local(at(day(60)
                .utc_seconds()
                .as_i64()
                .checked_add(120)
                .expect("시각")))),
        )
        .expect("진행");
        assert_eq!(ongoing[0].status, ResolutionStatus::Ongoing);
        repo.state.write().expect("쓰기").fail = true;
        let failed =
            run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70))).expect("실행");
        assert_eq!(failed[0].status, ResolutionStatus::Storage(storage_error()));
        let state = repo.state.read().expect("읽기");
        assert_eq!(state.outcomes, Vec::new());
        assert_eq!(state.tables, initial);
        assert_eq!(*collected.read().expect("이벤트"), Vec::new());
    }
    #[test]
    fn distinct_instruments_update_the_same_model_without_losing_either_fact() {
        let mut second_row = master().row().clone();
        second_row.instrument_id = id("sample-b");
        second_row.symbol = Symbol::try_new("000660").expect("코드");
        let second = MasterInstrument::try_new(second_row).expect("마스터");
        let tasks = vec![eligible(), eligible_for_master("102", &second)];
        let repo = setup(&tasks);
        let mut market = data(&tasks);
        let input = market.0[1].1.as_mut().expect("입력");
        input.candles[0] = bar(&tasks[1], &input.adjustment, 60, 1, ("200", "100"));
        let (bus, collected) = events();
        let report =
            run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("실행");
        assert_eq!(
            report
                .iter()
                .filter_map(|entry| resolved_state(&entry.status))
                .count(),
            2
        );
        let state = repo.state.read().expect("읽기");
        assert_eq!(
            state.tables[0]
                .1
                .snapshot
                .calibration()
                .for_combination(&state.tables[0].0)
                .expect("같은 조합")
                .judgment_count()
                .as_usize(),
            2
        );
        let table = state.tables[0]
            .1
            .snapshot
            .calibration()
            .for_combination(&state.tables[0].0)
            .expect("같은 조합");
        let facts = table.selected_facts();
        assert_eq!(facts[0].estimate().id(), &task_id("estimate-0"));
        assert_eq!(facts[0].estimate().instrument(), &id("sample-a"));
        assert_eq!(
            facts[0].estimate().predicted_probability(),
            AiProbability::parse(&AiProbabilityInput::try_new("71").expect("입력")).expect("확률")
        );
        assert_eq!(facts[0].outcome(), domain::CalibrationOutcome::Miss);
        assert_eq!(facts[1].estimate().id(), &task_id("estimate-1"));
        assert_eq!(facts[1].estimate().instrument(), &id("sample-b"));
        assert_eq!(
            facts[1].estimate().predicted_probability(),
            AiProbability::parse(&AiProbabilityInput::try_new("63").expect("입력")).expect("확률")
        );
        assert_eq!(facts[1].outcome(), domain::CalibrationOutcome::Hit);
        assert_eq!(
            *collected.read().expect("이벤트"),
            vec![
                DomainEvent::EstimateResolved(id("sample-a")),
                DomainEvent::EstimateResolved(id("sample-b"))
            ]
        );
    }
    #[test]
    fn unavailable_instrument_does_not_prevent_other_instrument_resolving() {
        let mut row = master().row().clone();
        row.instrument_id = id("sample-b");
        row.symbol = Symbol::try_new("000660").expect("코드");
        let second = MasterInstrument::try_new(row).expect("마스터");
        let tasks = vec![eligible(), eligible_for_master("102", &second)];
        let repo = setup(&tasks);
        let mut market = data(&tasks);
        market.0[0].1 = Err(ResolutionDataIssue::Incomplete(
            OutcomeUnavailable::Calendar,
        ));
        let (bus, collected) = events();
        let reports =
            run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("실행");
        assert_eq!(
            reports[0].status,
            ResolutionStatus::Unavailable(ResolutionDataIssue::Incomplete(
                OutcomeUnavailable::Calendar
            ))
        );
        assert_eq!(
            resolved_state(&reports[1].status),
            Some(OutcomeState::Failure(OutcomeFailure::Expired))
        );
        assert_eq!(
            *collected.read().expect("이벤트"),
            vec![DomainEvent::EstimateResolved(id("sample-b"))]
        );
        assert_eq!(
            run(repo.pending_submissions())
                .expect("미판정")
                .iter()
                .map(|record| record.task_id().as_estimate_id().clone())
                .collect::<Vec<_>>(),
            vec![task_id("estimate-0")]
        );
        assert_eq!(
            repo.state.read().expect("읽기").unavailable,
            vec![(
                task_id("estimate-0"),
                ResolutionDataIssue::Incomplete(OutcomeUnavailable::Calendar)
            )]
        );
    }
    #[test]
    fn market_read_failure_keeps_endpoint_and_retry_reason_for_the_next_run() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let mut market = data(&tasks);
        let error = application::PortError::new(
            application::ReadEndpoint::Candles,
            application::HttpStatus::NotReceived,
            application::PortErrorKind::Unavailable {
                cause: application::UnavailableCause::Network,
                retry: application::RetryDelay::Unknown,
            },
        );
        market.0[0].1 = Err(ResolutionDataIssue::Read(error));
        let (bus, collected) = events();
        let reports =
            run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("실행");
        assert_eq!(
            reports[0].status,
            ResolutionStatus::Unavailable(ResolutionDataIssue::Read(error))
        );
        assert_eq!(
            repo.state.read().expect("읽기").unavailable,
            vec![(task_id("estimate-0"), ResolutionDataIssue::Read(error))]
        );
        assert_eq!(*collected.read().expect("이벤트"), Vec::new());
        assert_eq!(run(repo.pending_submissions()).expect("미판정").len(), 1);
    }
    #[test]
    fn full_calibration_table_leaves_resolution_pending_with_reason() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let market = data(&tasks);
        let record = repo.state.read().expect("읽기").submissions[0].clone();
        let input = market.0[0].1.as_ref().expect("입력");
        let outcome = EstimateOutcome::evaluate_with_complete_data(
            record.submission().task_key().task(),
            &input.calendars,
            &input.adjustment,
            &input.candles,
            day(70),
        );
        let key = record.submission().calibration_key().clone();
        let facts: Vec<_> = (0..65_536_u32)
            .map(|index| {
                outcome
                    .calibration_fact(
                        task_id(&format!("past-{index:05}")),
                        key.model().clone(),
                        record.submission().probability(),
                    )
                    .expect("사실")
            })
            .collect();
        let old = Calibration::try_new(key.clone(), &facts).expect("기존 표");
        repo.state.write().expect("쓰기").tables[0].1 =
            calibration_state(&key, old.clone(), CalibrationPosition::new(65_536));
        let (bus, collected) = events();
        let reports =
            run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("실행");
        assert_eq!(
            reports[0].status,
            ResolutionStatus::Unavailable(ResolutionDataIssue::Calibration(
                domain::CalibrationError::TooManyFacts
            ))
        );
        let state = repo.state.read().expect("읽기");
        assert_eq!(state.outcomes.len(), 0);
        assert_eq!(state.tables[0].1.snapshot.calibration(), &old);
        assert_eq!(
            state.unavailable,
            vec![(
                task_id("estimate-0"),
                ResolutionDataIssue::Calibration(domain::CalibrationError::TooManyFacts)
            )]
        );
        assert!(collected.read().expect("이벤트").is_empty());
        assert_eq!(pending(&state).len(), 1);
    }
    #[test]
    fn each_market_request_receives_the_submitted_id_and_execution_time() {
        let tasks = vec![eligible(), eligible()];
        let repo = setup(&tasks);
        let market = CapturingData {
            data: data(&tasks),
            calls: RwLock::new(Vec::new()),
        };
        let (bus, _) = events();
        run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("실행");
        assert_eq!(
            *market.calls.read().expect("조회"),
            vec![
                (task_id("estimate-0"), day(70)),
                (task_id("estimate-1"), day(70))
            ]
        );
    }
    #[test]
    fn concurrent_resolution_commits_only_once_and_rejects_replacement() {
        let tasks = vec![eligible()];
        let mut repo = setup(&tasks);
        repo.barrier = Some(Barrier::new(2));
        let market = data(&tasks);
        let (bus, collected) = events();
        let (first, second) = std::thread::scope(|scope| {
            let first_thread = scope.spawn(|| {
                run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("첫 실행")
            });
            let second_thread = scope.spawn(|| {
                run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70)))
                    .expect("둘째 실행")
            });
            (
                first_thread.join().expect("첫 완료"),
                second_thread.join().expect("둘째 완료"),
            )
        });
        let statuses = [&first[0].status, &second[0].status];
        assert_eq!(
            statuses
                .iter()
                .filter(|status| resolved_state(status).is_some())
                .count(),
            1
        );
        assert_eq!(
            statuses
                .iter()
                .filter(|status| **status == &ResolutionStatus::AlreadyResolved)
                .count(),
            1
        );
        let (original, record, calibration) = {
            let state = repo.state.read().expect("읽기");
            (
                state.outcomes[0].1.clone(),
                state.submissions[0].clone(),
                state.tables[0].1.clone(),
            )
        };
        let replacement = replacement_outcome(&tasks[0], &market, &record);
        assert_eq!(replacement.state(), OutcomeState::Success);
        assert_eq!(
            run(repo.commit_resolution(
                &record,
                &replacement,
                &calibration,
                calibration.snapshot.calibration()
            )),
            Ok(ResolutionCommitOutcome::AlreadyResolved)
        );
        let state = repo.state.read().expect("읽기");
        assert_eq!(state.outcomes[0].1, original);
        assert_eq!(state.tables[0].1, calibration);
        assert_eq!(collected.read().expect("이벤트").len(), 1);
    }
    #[test]
    fn stale_different_result_reports_already_resolved_after_other_commit() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let mut latest = data(&tasks);
        let input = latest.0[0].1.as_mut().expect("입력");
        input.candles[0] = bar(&tasks[0], &input.adjustment, 60, 1, ("200", "100"));
        let (entered_tx, entered_rx) = mpsc::channel();
        let resume = Arc::new(Barrier::new(2));
        let delayed = DelayedData {
            data: latest,
            entered: entered_tx.clone(),
            resume: Arc::clone(&resume),
        };
        let (bus, collected) = events();
        let delayed_report = std::thread::scope(|scope| {
            let repo_ref = &repo;
            let bus_ref = &bus;
            let runner =
                scope.spawn(move || delayed_reports(repo_ref, &delayed, bus_ref, &entered_tx));
            assert_eq!(entered_rx.recv().expect("시작 신호"), DelaySignal::Entered);
            let first_result =
                run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70)));
            resume.wait();
            let first = first_result.expect("먼저 확정");
            assert_eq!(
                resolved_state(&first[0].status),
                Some(OutcomeState::Failure(OutcomeFailure::Expired))
            );
            runner.join().expect("느린 실행 종료")
        });
        assert_eq!(delayed_report[0].status, ResolutionStatus::AlreadyResolved);
        let state = repo.state.read().expect("읽기");
        assert_eq!(
            state.outcomes[0].1.state(),
            OutcomeState::Failure(OutcomeFailure::Expired)
        );
        assert_eq!(state.tables[0].1.snapshot.calibration().facts().len(), 1);
        assert_eq!(collected.read().expect("이벤트").len(), 1);
    }
    #[test]
    fn stale_data_failure_cannot_overwrite_a_completed_resolution() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let mut missing = data(&tasks);
        missing.0[0].1 = Err(ResolutionDataIssue::Incomplete(
            OutcomeUnavailable::Calendar,
        ));
        let (entered_tx, entered_rx) = mpsc::channel();
        let resume = Arc::new(Barrier::new(2));
        let delayed = DelayedData {
            data: missing,
            entered: entered_tx.clone(),
            resume: Arc::clone(&resume),
        };
        let (bus, collected) = events();
        let reports = std::thread::scope(|scope| {
            let repo_ref = &repo;
            let bus_ref = &bus;
            let runner =
                scope.spawn(move || delayed_reports(repo_ref, &delayed, bus_ref, &entered_tx));
            assert_eq!(entered_rx.recv().expect("시작 신호"), DelaySignal::Entered);
            let first = run(ResolveEstimates::new(&repo, &data(&tasks), &bus).execute(day(70)));
            resume.wait();
            assert_eq!(
                resolved_state(&first.expect("먼저 확정")[0].status),
                Some(OutcomeState::Failure(OutcomeFailure::Expired))
            );
            runner.join().expect("늦은 실행 종료")
        });
        assert_eq!(reports[0].status, ResolutionStatus::AlreadyResolved);
        let state = repo.state.read().expect("읽기");
        assert_eq!(state.outcomes.len(), 1);
        assert_eq!(state.unavailable, Vec::new());
        assert_eq!(collected.read().expect("이벤트").len(), 1);
    }
    #[test]
    fn conflicting_stored_outcome_and_calibration_report_repository_integrity() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let market = data(&tasks);
        let record = repo.state.read().expect("읽기").submissions[0].clone();
        let input = market.0[0].1.as_ref().expect("입력");
        let expired = EstimateOutcome::evaluate_with_complete_data(
            record.submission().task_key().task(),
            &input.calendars,
            &input.adjustment,
            &input.candles,
            day(70),
        );
        let success = replacement_outcome(&tasks[0], &market, &record);
        assert_eq!(
            expired.state(),
            OutcomeState::Failure(OutcomeFailure::Expired)
        );
        assert_eq!(success.state(), OutcomeState::Success);
        let key = record.submission().calibration_key().clone();
        let success_fact = success
            .calibration_fact(
                record.task_id().as_estimate_id().clone(),
                key.model().clone(),
                record.submission().probability(),
            )
            .expect("사실");
        let inconsistent_table = Calibration::try_new(key.clone(), &[success_fact]).expect("표");
        let (entered_tx, entered_rx) = mpsc::channel();
        let resume = Arc::new(Barrier::new(2));
        let delayed = DelayedData {
            data: market,
            entered: entered_tx.clone(),
            resume: Arc::clone(&resume),
        };
        let (bus, collected) = events();
        let reports = std::thread::scope(|scope| {
            let repo_ref = &repo;
            let bus_ref = &bus;
            let runner =
                scope.spawn(move || delayed_reports(repo_ref, &delayed, bus_ref, &entered_tx));
            assert_eq!(entered_rx.recv().expect("시작 신호"), DelaySignal::Entered);
            insert_inconsistent_resolution(&repo, &expired, &key, &inconsistent_table);
            resume.wait();
            runner.join().expect("느린 실행 종료")
        });
        assert_eq!(
            reports[0].status,
            ResolutionStatus::Storage(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::RepositoryIntegrity
            ))
        );
        let state = repo.state.read().expect("읽기");
        assert_eq!(state.outcomes[0].1, expired);
        assert_eq!(
            state.tables[0].1.snapshot.calibration(),
            &inconsistent_table
        );
        assert!(collected.read().expect("이벤트").is_empty());
    }
    fn insert_inconsistent_resolution(
        repo: &Repo,
        outcome: &EstimateOutcome,
        key: &CalibrationKey,
        table: &Calibration,
    ) {
        let mut state = repo.state.write().expect("쓰기");
        state
            .outcomes
            .push((task_id("estimate-0"), outcome.clone()));
        state.tables[0].1 = calibration_state(key, table.clone(), CalibrationPosition::new(1));
    }
    #[test]
    fn conflicting_table_without_resolution_stays_an_error_and_read_failure_is_preserved() {
        let tasks = vec![eligible()];
        let repo = setup(&tasks);
        let mut market = data(&tasks);
        let record = repo.state.read().expect("읽기").submissions[0].clone();
        let original = market.0[0].1.as_ref().expect("입력");
        let expired = EstimateOutcome::evaluate_with_complete_data(
            record.submission().task_key().task(),
            &original.calendars,
            &original.adjustment,
            &original.candles,
            day(70),
        );
        let key = record.submission().calibration_key().clone();
        let fact = expired
            .calibration_fact(
                record.task_id().as_estimate_id().clone(),
                key.model().clone(),
                record.submission().probability(),
            )
            .expect("사실");
        let table = Calibration::try_new(key.clone(), &[fact]).expect("보정표");
        repo.state.write().expect("쓰기").tables[0].1 =
            calibration_state(&key, table.clone(), CalibrationPosition::new(1));
        let changed = market.0[0].1.as_mut().expect("입력");
        changed.candles[0] = bar(&tasks[0], &changed.adjustment, 60, 1, ("200", "100"));
        let (bus, collected) = events();
        let first =
            run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("실행");
        assert_eq!(
            first[0].status,
            ResolutionStatus::Calibration(domain::CalibrationError::ConflictingEstimate)
        );
        repo.state.write().expect("쓰기").fail = true;
        let second =
            run(ResolveEstimates::new(&repo, &market, &bus).execute(day(70))).expect("실행");
        assert_eq!(second[0].status, ResolutionStatus::Storage(storage_error()));
        let state = repo.state.read().expect("읽기");
        assert_eq!(state.outcomes, Vec::new());
        assert_eq!(state.tables[0].1.snapshot.calibration(), &table);
        assert_eq!(*collected.read().expect("이벤트"), Vec::new());
    }
    fn replacement_outcome(
        task: &CostEligibleTask,
        market: &Data,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
    ) -> EstimateOutcome {
        let input = market.0[0].1.as_ref().expect("입력");
        EstimateOutcome::evaluate_with_complete_data(
            record.submission().task_key().task(),
            &input.calendars,
            &input.adjustment,
            &[bar(task, &input.adjustment, 60, 1, ("200", "100"))],
            day(70),
        )
    }
}
