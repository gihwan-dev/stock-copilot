use super::{
    TaskBundle, at_us, build_task_bundle, calibration_key, master_classifications, money,
    shifted_us, us_calendars, us_price, usd_krw_rate, zero_cost_facts,
};
use crate::tests::{Fixture, clock, key, runtime};
use crate::{Database, DatabasePath, SharedDatabase, SqliteRepositories};
use crate::{EstimateStore, SignalStore};
use application::{AiRunId, EstimateRepository, EstimateTaskRepository, ReasoningEffort};
use application::{
    CollectedEntry, CollectedItemHandle, CollectedTimes, EstimateSource, EstimateToolArguments,
    EvaluateSignalInputs, EvaluateSignalOutcome, EvaluateSignals, EventBus, NewsRepository,
    PublicationTime, SignalEvaluationData, SignalRepository, SubmitEstimateTool, ToolCallLedger,
    ToolCallOutcome, ToolCallRecord, ToolCallRecordInput, ToolName, ToolPayloadJson,
    ToolProtocolVersion, ToolRequestId,
};
use domain::{
    AiProbabilityInput, BuySignalOccurrenceId, BuyingPower, CalibrationEstimateId, Currency,
    EstimateSubmissionRecord, InvestorProfile, NormalizedUrl, OrderSide, Policy, Portfolio,
    Timestamp, UntrustedText,
};
use domain::{NewsItem, NewsSource};
use testkit_ports::{SteppedClock, estimate_fixtures::sample_estimate_run_context};

struct Inputs(EvaluateSignalInputs);

impl SignalEvaluationData for Inputs {
    fn inputs(
        &self,
        _record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        _now: Timestamp,
    ) -> impl Future<Output = Result<EvaluateSignalInputs, application::SystemPortError>> + Send
    {
        std::future::ready(Ok(self.0.clone()))
    }
}

fn observation(bundle: &TaskBundle) -> Inputs {
    let portfolio = Portfolio::new(
        Vec::new(),
        BuyingPower::new(money("10000000", Currency::Krw), shifted_us(61, 21_603)),
        BuyingPower::new(money("0", Currency::Usd), shifted_us(61, 21_603)),
    )
    .expect("잔고");
    let rate = usd_krw_rate();
    let (assignments, security) = master_classifications(&bundle.master);
    let (commission, tax) = zero_cost_facts();
    Inputs(EvaluateSignalInputs {
        calibration: Some(bundle.calibration.clone()),
        calibration_head: Some(bundle.calibration.through().clone()),
        valuation: Some(portfolio.evaluate(Some(rate))),
        policy: Policy::new(InvestorProfile::default()),
        assignments,
        security,
        target_price: Some(
            bundle
                .master
                .price_observation(us_price("100"), shifted_us(61, 21_603))
                .expect("시세"),
        ),
        calendars: us_calendars(),
        quantity_costs: None,
        restoration: Some(application::SignalRestorationBasis {
            master: bundle.master.clone(),
            holding_masters: Vec::new(),
            estimate: bundle.stored_estimate.clone(),
            commission,
            tax,
            maturity: at_us(70),
            portfolio: Some(portfolio),
            rate: Some(rate),
        }),
    })
}

async fn record_lookup(store: &SqliteRepositories, run: &AiRunId) -> Timestamp {
    let item = NewsItem::new(
        NewsSource::Yonhap,
        NormalizedUrl::try_new("https://example.com/report").expect("URL"),
        UntrustedText::try_new("발표 자료").expect("제목"),
        None,
    );
    store.store(&item).await.expect("수집");
    let entry = CollectedEntry::News(item);
    let submitted = shifted_us(61, 21_602);
    store
        .record_times(&entry, PublicationTime::Known(submitted), submitted)
        .await
        .expect("시각");
    let result = r#"{"items":[{"handle":"n-1","citation_digest":"693c97c1fee1bbd60c5f879d74f6f995b3b00e42cb06b9b9b7896bfaa72b4f55"}]}"#;
    store
        .record_call(&ToolCallRecord::new(ToolCallRecordInput {
            run_id: run.clone(),
            request_id: ToolRequestId::try_new("lookup").expect("요청"),
            tool_name: ToolName::try_new("search_news").expect("도구"),
            arguments_json: ToolPayloadJson::try_new("{}").expect("인자"),
            result_json: ToolPayloadJson::try_new(result).expect("결과"),
            outcome: ToolCallOutcome::Succeeded,
            started_at: submitted,
            finished_at: submitted,
            protocol_version: ToolProtocolVersion::try_new("2025-06-18").expect("버전"),
        }))
        .await
        .expect("원장");
    submitted
}

fn arguments(id: CalibrationEstimateId) -> EstimateToolArguments<EstimateSource> {
    EstimateToolArguments {
        task_id: id,
        probability: AiProbabilityInput::try_new("90").expect("확률"),
        rationale: UntrustedText::try_new("실적 상승").expect("근거"),
        counterargument: UntrustedText::try_new("수요 둔화").expect("반론"),
        sources: vec![EstimateSource::Collected(
            CollectedItemHandle::try_new("n-1").expect("핸들"),
        )],
    }
}

#[test]
fn successful_news_submission_raises_and_reloads_a_buy_occurrence_from_the_same_database() {
    let dir = Fixture::new();
    let database = Database::open(&DatabasePath::new(dir.path()), &key(), &clock()).expect("DB");
    runtime().block_on(async {
        let bundle = build_task_bundle("signal-e2e", "AAPL", OrderSide::Buy);
        let shared = SharedDatabase::new();
        SignalStore::new(&database)
            .save_master(&bundle.master)
            .await
            .expect("마스터 저장");
        let id = CalibrationEstimateId::try_new("estimate-e2e").expect("과제");
        let mut context = sample_estimate_run_context("run-signal").expect("실행");
        let key = calibration_key();
        context.model = key.model().clone();
        context.horizon = key.horizon();
        EstimateStore::new(&database)
            .issue_task(&id, &context.run, &bundle.eligible)
            .await
            .expect("발행");
        assert!(shared.install(database).await.is_none());
        let store = SqliteRepositories::new(shared);
        store
            .append_calibration(&key, bundle.calibration.calibration())
            .await
            .expect("빈 보정 표");
        let submitted = record_lookup(&store, &context.run).await;
        let events = EventBus::default();
        SubmitEstimateTool::new(&store, &store, &SteppedClock::new(submitted), &events)
            .execute(context, arguments(id))
            .await
            .expect("제출");
        let instrument = &bundle.master.row().instrument_id;
        let outcome = EvaluateSignals::new(&store, &observation(&bundle), &events)
            .execute(
                instrument,
                shifted_us(61, 21_604),
                BuySignalOccurrenceId::try_new("occ-news").expect("카드"),
            )
            .await
            .expect("평가");
        let EvaluateSignalOutcome::Applied { reason, .. } = outcome else {
            panic!("평가 결과 {outcome:?}");
        };
        assert_eq!(reason, None, "조건을 통과한 신호");
        let card = store
            .snapshot(instrument)
            .await
            .expect("조회")
            .current
            .expect("저장된 신호");
        assert!(card.occurrence.is_some(), "발생 기록");
    });
}
