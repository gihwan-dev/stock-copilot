#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use application::{
        AiRunId, EstimateRunContext, EstimateSubmissionRepository, EventBusBuilder,
        ReasoningEffort, ToolCallLedger, ToolCallOutcome, ToolCallRecord, ToolCallRecordInput,
        ToolName, ToolPayloadJson, ToolProtocolVersion, ToolRequestId,
    };
    use domain::{
        AdjustmentBasis, AiProbability, AiProbabilityInput, CalibrationEstimateId, CalibrationKey,
        CalibrationModel, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        CommissionRateFact, CommissionValidity, CommonShare, CostEligibleTask, CostRate, Currency,
        DisplayInstrumentName, EstimateAssignments, EstimateContext, EstimateCostPeriod,
        EstimateMetadata, EstimateOutcome, EstimatePrompt, EstimateTask, EvidenceBundleId, Horizon,
        HorizonSelection, HoursPermission, InstrumentId, KoreanMasterDetail, LiquidationTrading,
        ListingStatus, Market, MasterCode, MasterInstrument, MasterInstrumentRow,
        MasterSecurityType, ModelIdentityPart, Nanosecond, NumericInput, NxtTrading, OffsetSeconds,
        Ohlcv, OutcomeAdjustment, Price, PriceAdjustment, PromptHash, Quantity, Ratio,
        RiskAppetite, ScorecardObservation, ScorecardPromptId, Symbol, Timestamp, TradeCost,
        TradeTaxFact, TradingSuspension, UnixSeconds, Volatility,
    };
    use mcp_server::{
        EstimateToolPorts, ToolErrorCode, ToolInvocation, ToolRegistry, register_estimate_tools,
        validate_arguments_against_schema,
    };
    use serde_json::{Map, Value, json};
    use testkit_ports::SteppedClock;
    use testkit_ports::memory::MemorySignalEvaluationRepository;
    use testkit_ports::system_fakes::MemoryToolCallLedger;
    use testkit_ports::tool_read_fakes::{
        FixedEstimateAssignments, FixedEstimateRunContext, FixedProfileSource,
        FixedScorecardObservations,
    };

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("테스트 런타임")
    }

    fn price(value: &str) -> Price {
        Price::parse(&NumericInput::try_new(value).expect("입력"), Currency::Krw).expect("가격")
    }

    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }

    fn ratio(value: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(value).expect("입력")).expect("비율")
    }

    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("시각")
    }

    fn day(value: i64) -> Timestamp {
        at(value.checked_mul(86_400_i64).expect("일"))
    }

    fn local(time: Timestamp) -> Timestamp {
        Timestamp::from_parts(
            time.utc_seconds(),
            time.nanosecond(),
            OffsetSeconds::new(32_400_i32),
        )
        .expect("현지 시각")
    }

    fn local_midnight(value: i64) -> Timestamp {
        local(at(day(value)
            .utc_seconds()
            .as_i64()
            .checked_sub(32_400_i64)
            .expect("현지 자정")))
    }

    fn master() -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            instrument_id: InstrumentId::try_new("sample-a").expect("종목"),
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

    fn candles(source: &MasterInstrument) -> Vec<Candle> {
        (0_i64..61_i64)
            .map(|index| {
                let fixture = price(if index % 2 == 0 { "100" } else { "102" });
                Candle::try_new(
                    source.row().instrument_id.clone(),
                    CandleTime::try_new(local_midnight(index), CandlePeriod::Daily).expect("시각"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(day(61))),
                    Ohlcv::try_new(fixture, fixture, fixture, fixture, quantity("13"))
                        .expect("OHLCV"),
                    CandleObservation::new(day(61), CandleStatus::Finalized),
                )
                .expect("일봉")
            })
            .collect()
    }

    fn eligible() -> CostEligibleTask {
        let source = master();
        let task = EstimateTask::create(
            EstimateContext::new(
                source
                    .price_observation(price("100"), local(day(60)))
                    .expect("관측"),
                day(60),
            ),
            Volatility::calculate(&candles(&source)).expect("변동성"),
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
                .checked_add(3_600_i64)
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

    fn task_id(value: &str) -> CalibrationEstimateId {
        CalibrationEstimateId::try_new(value).expect("과제 ID")
    }

    fn run_id(value: &str) -> AiRunId {
        AiRunId::try_new(value).expect("실행")
    }

    fn identity(value: &str) -> ModelIdentityPart {
        ModelIdentityPart::try_new(value).expect("메타데이터")
    }

    fn assignments() -> EstimateAssignments<AiRunId> {
        EstimateAssignments::default()
            .issue(task_id("one"), run_id("alpha"), eligible())
            .expect("발급")
            .issue(task_id("other"), run_id("beta"), eligible())
            .expect("발급")
    }

    fn context(run: &str) -> EstimateRunContext {
        EstimateRunContext {
            run: run_id(run),
            model: CalibrationModel::new(identity("claude"), identity("model"), identity("v1")),
            horizon: Horizon::new(HorizonSelection::FewDays),
            metadata: EstimateMetadata {
                effort: ReasoningEffort::High,
                cli_version: identity("2.7.3"),
                prompt: EstimatePrompt {
                    id: ScorecardPromptId::try_new("estimate-prompt").expect("프롬프트"),
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

    fn observations(count: usize) -> Vec<ScorecardObservation> {
        let eligible = eligible();
        let task = eligible.task();
        let adjustment =
            OutcomeAdjustment::from_actions(task, AdjustmentBasis::new(local(day(70))), &[])
                .expect("수정");
        let outcome = EstimateOutcome::evaluate(task, &[], &adjustment, &[], local(day(70)));
        (0..count)
            .map(|index| {
                ScorecardObservation::try_new(
                    task_id(&format!("est-{index}")),
                    CalibrationKey::new(
                        CalibrationModel::new(
                            identity("claude"),
                            identity("model"),
                            identity("v1"),
                        ),
                        Horizon::new(HorizonSelection::FewDays),
                    ),
                    ScorecardPromptId::try_new("estimate-prompt").expect("프롬프트"),
                    AiProbability::parse(&AiProbabilityInput::try_new("60").expect("입력"))
                        .expect("확률"),
                    outcome.clone(),
                )
                .expect("관측")
            })
            .collect()
    }

    struct Fixture {
        registry: ToolRegistry,
        submissions: Arc<MemorySignalEvaluationRepository>,
    }

    async fn fixture(observed: usize) -> Fixture {
        let history = MemoryToolCallLedger::recording();
        let record = ToolCallRecord::new(ToolCallRecordInput {
            run_id: run_id("alpha"),
            request_id: ToolRequestId::try_new("q1").expect("요청"),
            tool_name: ToolName::try_new("get_quote").expect("도구"),
            arguments_json: ToolPayloadJson::try_new("{}").expect("인자"),
            result_json: ToolPayloadJson::try_new("{}").expect("결과"),
            outcome: ToolCallOutcome::Succeeded,
            started_at: day(61),
            finished_at: day(61),
            protocol_version: ToolProtocolVersion::try_new("2026-07-28").expect("버전"),
        });
        history.record_call(&record).await.expect("기록");
        let submissions = Arc::new(MemorySignalEvaluationRepository::new(assignments()));
        let ports = Arc::new(EstimateToolPorts {
            assignments: Arc::new(FixedEstimateAssignments::new(assignments())),
            submissions: Arc::clone(&submissions),
            history: Arc::new(history),
            context: Arc::new(FixedEstimateRunContext(Ok(context("alpha")))),
            settings: Arc::new(FixedProfileSource(Ok(None))),
            observations: Arc::new(FixedScorecardObservations(Ok(observations(observed)))),
            clock: Arc::new(SteppedClock::new(day(61))),
            events: Arc::new(EventBusBuilder::default().build()),
        });
        let mut registry = ToolRegistry::new();
        register_estimate_tools(&mut registry, &ports).expect("등록");
        Fixture {
            registry,
            submissions,
        }
    }

    async fn call(
        registry: &ToolRegistry,
        run: &str,
        name: &str,
        args: Value,
    ) -> Result<Value, ToolErrorCode> {
        let tool_name = ToolName::try_new(name).expect("이름");
        let (spec, tool) = registry.get(&tool_name).expect("등록된 도구");
        let arguments: Map<String, Value> = args.as_object().cloned().expect("객체 인자");
        validate_arguments_against_schema(&arguments, spec.input_schema())
            .map_err(|err| err.code())?;
        let run_identity = run_id(run);
        tool.invoke(ToolInvocation {
            run_id: &run_identity,
            arguments: &arguments,
        })
        .await
        .map_err(|err| err.code())
    }

    #[test]
    fn estimate_task_is_read_by_task_id_only_within_its_run() {
        runtime().block_on(async {
            let fixture = fixture(0_usize).await;
            let task = call(
                &fixture.registry,
                "alpha",
                "get_estimate_task",
                json!({ "task_id": "one" }),
            )
            .await
            .expect("과제");
            assert_eq!(task["task_id"], json!("one"));
            assert_eq!(task["symbol"], json!("005930"));
            assert_eq!(task["reference_price"]["amount"], json!("100"));
            assert_eq!(task["status"], json!("available"));
            assert!(
                task["base_probability"].is_string(),
                "코드가 계산한 기준 확률"
            );
            assert!(
                task["event"]["target_return"].is_string()
                    && task["event"]["stop_loss"].is_string()
            );
            for (run, args) in [
                ("alpha", json!({ "task_id": "other" })),
                ("alpha", json!({ "task_id": "missing" })),
                ("beta", json!({ "task_id": "one" })),
                ("alpha", json!({ "task_id": "one", "symbol": "000660" })),
                ("alpha", json!({ "symbol": "005930" })),
            ] {
                assert_eq!(
                    call(&fixture.registry, run, "get_estimate_task", args.clone()).await,
                    Err(ToolErrorCode::InvalidInput),
                    "{run} {args}"
                );
            }
        });
    }

    fn submission(task: &str, probability: u64, sources: &[&str]) -> Value {
        json!({
            "task_id": task,
            "p": probability,
            "rationale": "실적 개선",
            "counterargument": "환율 부담",
            "sources": sources
        })
    }

    #[test]
    fn submit_estimate_accepts_only_the_five_fields_and_ledger_sources() {
        runtime().block_on(async {
            let fixture = fixture(0_usize).await;
            let mut with_side = submission("one", 60_u64, &[]);
            with_side
                .as_object_mut()
                .expect("객체")
                .insert(String::from("side"), json!("buy"));
            for (args, label) in [
                (with_side, "방향 필드"),
                (submission("one", 100_u64, &[]), "p 상한"),
                (submission("one", 0_u64, &[]), "p 하한"),
                (
                    submission("one", 60_u64, &["tool:q404"]),
                    "원장에 없는 출처",
                ),
                (submission("other", 60_u64, &[]), "다른 실행 과제"),
            ] {
                assert_eq!(
                    call(&fixture.registry, "alpha", "submit_estimate", args).await,
                    Err(ToolErrorCode::InvalidInput),
                    "{label}"
                );
            }
            let accepted = call(
                &fixture.registry,
                "alpha",
                "submit_estimate",
                submission("one", 62_u64, &["tool:q1"]),
            )
            .await
            .expect("제출");
            assert_eq!(accepted["recorded"], json!("estimate_only_no_order"));
            let records = fixture
                .submissions
                .submission_records(&InstrumentId::try_new("sample-a").expect("종목"))
                .await
                .expect("조회");
            assert_eq!(records.len(), 1_usize, "추정 하나만 기록");
            assert_eq!(
                call(
                    &fixture.registry,
                    "alpha",
                    "submit_estimate",
                    submission("one", 62_u64, &[])
                )
                .await,
                Err(ToolErrorCode::InvalidInput),
                "같은 과제 재제출 거부"
            );
        });
    }

    #[test]
    fn profile_and_scorecard_are_code_computed_views() {
        runtime().block_on(async {
            let empty = fixture(0_usize).await;
            let profile = call(&empty.registry, "alpha", "get_profile", json!({}))
                .await
                .expect("성향");
            assert_eq!(profile["source"], json!("default"));
            assert_eq!(profile["risk_appetite"], json!("neutral"));
            let none = call(&empty.registry, "alpha", "get_scorecard", json!({}))
                .await
                .expect("성적표");
            assert_eq!(none["status"], json!("no_judged_estimates_yet"));
            let few = fixture(5_usize).await;
            let cards = call(&few.registry, "alpha", "get_scorecard", json!({}))
                .await
                .expect("성적표");
            assert_eq!(
                cards["cards"][0]["status"],
                json!("pending_under_30_judged"),
                "30건 미만은 판정 보류"
            );
            assert_eq!(cards["cards"][0]["key"]["horizon"], json!("few_days"));
            assert_eq!(
                call(
                    &few.registry,
                    "alpha",
                    "get_scorecard",
                    json!({ "model": "x" })
                )
                .await,
                Err(ToolErrorCode::InvalidInput)
            );
        });
    }
}
