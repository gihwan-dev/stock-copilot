#[cfg(test)]
mod tests {
    use application::{
        AiRunId, EstimateCommitOutcome, EstimateRunContext, EstimateSubmissionRepository,
        EstimateToolArguments, EventBusBuilder, ReasoningEffort, SubmitEstimateError,
        SubmitEstimateTool, SubmitEstimateToolError, SystemPortError, ToolCallLedger,
        ToolCallOutcome, ToolCallRecord, ToolCallRecordInput, ToolName, ToolPayloadJson,
        ToolProtocolVersion, ToolRequestId,
    };
    use domain::{
        AdjustmentBasis, AiProbabilityError, AiProbabilityInput, CalibrationEstimateId,
        CalibrationModel, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        CommissionRateFact, CommissionValidity, CommonShare, CostEligibleTask, CostRate, Currency,
        DisplayInstrumentName, EstimateAssignmentError, EstimateAssignments, EstimateContext,
        EstimateCostPeriod, EstimateMetadata, EstimatePrompt, EstimateSourceStatus,
        EstimateSubmissionRecord, EstimateTask, EvidenceBundleId, EvidenceRef, Horizon,
        HorizonSelection, HoursPermission, InstrumentId, KoreanMasterDetail, LiquidationTrading,
        ListingStatus, Market, MasterCode, MasterInstrument, MasterInstrumentRow,
        MasterSecurityType, ModelIdentityPart, Nanosecond, NumericInput, NxtTrading, OffsetSeconds,
        Ohlcv, Price, PriceAdjustment, PromptHash, Quantity, Ratio, RiskAppetite, Symbol,
        ThesisError, Timestamp, ToolReference, TradeCost, TradeTaxFact, TradingSuspension,
        UnixSeconds, UntrustedText, Volatility,
    };
    use std::{
        future::Future,
        sync::RwLock,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock, memory::MemorySignalEvaluationRepository, system_fakes::MemoryToolCallLedger,
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
    fn day(value: i64) -> Timestamp {
        at(value.checked_mul(86400).expect("일"))
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
    fn eligible() -> CostEligibleTask {
        let source = master();
        let candles: Vec<_> = (0_i64..61)
            .map(|index| {
                let fixture_price = price(if index % 2 == 0 { "100" } else { "102" });
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
    fn assignments() -> EstimateAssignments<AiRunId> {
        EstimateAssignments::default()
            .issue(task_id("one"), ai_run("alpha"), eligible())
            .expect("발급")
            .issue(task_id("other"), ai_run("beta"), eligible())
            .expect("발급")
    }
    fn context(run_id: &str) -> EstimateRunContext {
        EstimateRunContext {
            run: ai_run(run_id),
            model: CalibrationModel::new(
                identity("claude"),
                identity("actual-model"),
                identity("revision-3"),
            ),
            horizon: Horizon::new(HorizonSelection::FewDays),
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
    fn arguments(task: &str, probability: &str, sources: &[&str]) -> EstimateToolArguments {
        EstimateToolArguments {
            task_id: task_id(task),
            probability: AiProbabilityInput::try_new(probability).expect("확률 입력"),
            rationale: UntrustedText::try_new("실적 개선").expect("근거"),
            counterargument: UntrustedText::try_new("환율 부담").expect("반론"),
            sources: sources
                .iter()
                .map(|source| ToolReference::try_new(source).expect("참조"))
                .collect(),
        }
    }
    fn call(run_id: &str, request: &str, outcome: ToolCallOutcome) -> ToolCallRecord {
        ToolCallRecord::new(ToolCallRecordInput {
            run_id: ai_run(run_id),
            request_id: ToolRequestId::try_new(request).expect("요청"),
            tool_name: ToolName::try_new("get_quote").expect("도구"),
            arguments_json: ToolPayloadJson::try_new("{}").expect("인자"),
            result_json: ToolPayloadJson::try_new("{\"price\":\"100\"}").expect("결과"),
            outcome,
            started_at: day(61),
            finished_at: day(61),
            protocol_version: ToolProtocolVersion::try_new("2026-07-28").expect("버전"),
        })
    }
    fn ledger() -> MemoryToolCallLedger {
        let ledger = MemoryToolCallLedger::recording();
        for record in [
            call("alpha", "q1", ToolCallOutcome::Succeeded),
            call("alpha", "q2", ToolCallOutcome::Succeeded),
            call("alpha", "bad", ToolCallOutcome::ToolError),
            call("beta", "q9", ToolCallOutcome::Succeeded),
        ] {
            run(ledger.record_call(&record)).expect("기록");
        }
        ledger
    }
    fn records(
        repository: &MemorySignalEvaluationRepository,
    ) -> Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>> {
        run(repository.submission_records(&id("sample-a"))).expect("조회")
    }
    fn submit(
        repository: &MemorySignalEvaluationRepository,
        context_run: &str,
        args: EstimateToolArguments,
    ) -> Result<application::DeliveryReport, SubmitEstimateToolError> {
        let history = ledger();
        let clock = SteppedClock::new(day(61));
        let events = EventBusBuilder::default().build();
        run(
            SubmitEstimateTool::new(repository, &history, &clock, &events)
                .execute(context(context_run), args),
        )
    }

    #[test]
    fn tool_sources_from_the_same_run_are_recorded() {
        let repository = MemorySignalEvaluationRepository::new(assignments());
        submit(
            &repository,
            "alpha",
            arguments("one", "62", &["tool:q1", "tool:q2"]),
        )
        .expect("제출");
        let saved = records(&repository);
        assert_eq!(saved.len(), 1);
        let record = saved.first().expect("기록");
        assert_eq!(record.run(), &ai_run("alpha"));
        assert_eq!(
            record.submission().probability(),
            domain::AiProbability::parse(&AiProbabilityInput::try_new("62").expect("입력"))
                .expect("확률")
        );
        let items: Vec<_> = record
            .submission()
            .evidence()
            .expect("출처")
            .as_items()
            .to_vec();
        assert_eq!(
            items,
            vec![
                EvidenceRef::Tool(ToolReference::try_new("tool:q1").expect("참조")),
                EvidenceRef::Tool(ToolReference::try_new("tool:q2").expect("참조")),
            ]
        );
        assert_eq!(
            record.narrative().rationale,
            UntrustedText::try_new("실적 개선").expect("근거")
        );
        assert_eq!(
            record.narrative().counterargument,
            UntrustedText::try_new("환율 부담").expect("반론")
        );
        assert_eq!(record.submission().submitted_at(), day(61));
        assert_eq!(
            submit(&repository, "alpha", arguments("one", "62", &[])),
            Err(SubmitEstimateToolError::Submit(
                SubmitEstimateError::Assignment(EstimateAssignmentError::AlreadyConsumed)
            ))
        );
    }

    #[test]
    fn empty_sources_are_recorded_as_missing() {
        let repository = MemorySignalEvaluationRepository::new(assignments());
        submit(&repository, "alpha", arguments("one", "1", &[])).expect("제출");
        let saved = records(&repository);
        let record = saved.first().expect("기록");
        assert_eq!(record.submission().evidence(), None);
        assert_eq!(record.source_status(), EstimateSourceStatus::Missing);
    }

    #[test]
    fn sources_missing_from_the_run_ledger_are_rejected() {
        for sources in [
            vec!["tool:unknown"],
            vec!["tool:q1", "tool:q9"],
            vec!["tool:bad"],
        ] {
            let repository = MemorySignalEvaluationRepository::new(assignments());
            assert_eq!(
                submit(&repository, "alpha", arguments("one", "50", &sources)),
                Err(SubmitEstimateToolError::UnknownSource)
            );
            assert_eq!(records(&repository), Vec::new());
            assert_eq!(run(repository.assignments()).expect("배정"), assignments());
        }
    }

    #[test]
    fn duplicate_sources_are_rejected() {
        let repository = MemorySignalEvaluationRepository::new(assignments());
        assert_eq!(
            submit(
                &repository,
                "alpha",
                arguments("one", "50", &["tool:q1", "tool:q1"])
            ),
            Err(SubmitEstimateToolError::Sources(
                ThesisError::DuplicateSource
            ))
        );
        assert_eq!(records(&repository), Vec::new());
    }

    #[test]
    fn probability_outside_whole_percent_range_is_rejected() {
        for (value, error) in [
            ("0", AiProbabilityError::InvalidPercent),
            ("07", AiProbabilityError::InvalidPercent),
            ("-1", AiProbabilityError::InvalidPercent),
            ("5.", AiProbabilityError::InvalidPercent),
        ] {
            let repository = MemorySignalEvaluationRepository::new(assignments());
            assert_eq!(
                submit(&repository, "alpha", arguments("one", value, &[])),
                Err(SubmitEstimateToolError::Probability(error))
            );
            assert_eq!(records(&repository), Vec::new());
        }
        assert_eq!(
            AiProbabilityInput::try_new("100").map(|_input| ()),
            Err(AiProbabilityError::InvalidLength)
        );
    }

    #[test]
    fn tasks_of_another_run_or_unknown_tasks_are_rejected() {
        let repository = MemorySignalEvaluationRepository::new(assignments());
        assert_eq!(
            submit(&repository, "alpha", arguments("other", "50", &[])),
            Err(SubmitEstimateToolError::Assignment(
                EstimateAssignmentError::WrongRun
            ))
        );
        assert_eq!(
            submit(&repository, "alpha", arguments("missing", "50", &[])),
            Err(SubmitEstimateToolError::Assignment(
                EstimateAssignmentError::UnknownTask
            ))
        );
        assert_eq!(records(&repository), Vec::new());
    }

    #[derive(Debug)]
    struct ConflictOnce {
        inner: MemorySignalEvaluationRepository,
        conflicts: RwLock<usize>,
        commits: RwLock<Vec<()>>,
    }
    impl EstimateSubmissionRepository for ConflictOnce {
        fn submission_records(
            &self,
            instrument: &InstrumentId,
        ) -> impl Future<
            Output = Result<
                Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>,
                SystemPortError,
            >,
        > + Send {
            self.inner.submission_records(instrument)
        }
        fn assignments(
            &self,
        ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send
        {
            self.inner.assignments()
        }
        async fn commit_submission(
            &self,
            previous: &EstimateAssignments<AiRunId>,
            next: &EstimateAssignments<AiRunId>,
            records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
        ) -> Result<EstimateCommitOutcome, SystemPortError> {
            self.commits.write().expect("잠금").push(());
            let remaining = *self.conflicts.read().expect("잠금");
            if remaining > 0 {
                *self.conflicts.write().expect("잠금") = remaining.saturating_sub(1);
                return Ok(EstimateCommitOutcome::Conflict);
            }
            self.inner.commit_submission(previous, next, records).await
        }
    }

    #[test]
    fn concurrent_update_is_retried_with_the_original_submission() {
        let repository = ConflictOnce {
            inner: MemorySignalEvaluationRepository::new(assignments()),
            conflicts: RwLock::new(2),
            commits: RwLock::new(Vec::new()),
        };
        let history = ledger();
        let clock = SteppedClock::new(day(61));
        let events = EventBusBuilder::default().build();
        run(
            SubmitEstimateTool::new(&repository, &history, &clock, &events)
                .execute(context("alpha"), arguments("one", "70", &["tool:q1"])),
        )
        .expect("제출");
        assert_eq!(repository.commits.read().expect("잠금").len(), 3);
        assert_eq!(clock.calls().len(), 1);
        let saved = records(&repository.inner);
        assert_eq!(saved.len(), 1);
        assert_eq!(
            saved.first().expect("기록").submission().submitted_at(),
            day(61)
        );
    }
}
