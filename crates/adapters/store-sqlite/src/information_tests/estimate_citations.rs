use super::{news, text};
use crate::tests::{Fixture, clock, key, runtime};
use crate::{ChatStore, Database, DatabasePath, EstimateStore, InformationStore, SignalStore};
use application::{
    AiRunId, CollectedEntry, CollectedItemHandle, CollectedItemReads, CollectedTimes,
    EstimateSource, EstimateSubmissionRepository, EstimateTaskRepository, EstimateToolArguments,
    EventBus, NewsRepository, PublicationTime, SignalRepository, SubmitEstimateTool,
    SubmitEstimateToolError, ToolCallLedger, ToolCallOutcome, ToolCallRecord, ToolCallRecordInput,
    ToolName, ToolPayloadJson, ToolProtocolVersion, ToolRequestId,
};
use domain::{
    AiProbabilityInput, CalibrationEstimateId, EstimateAssignmentError, EvidenceRef, NewsSource,
};
use testkit_ports::SteppedClock;
use testkit_ports::estimate_fixtures::{sample_cost_eligible_task, sample_estimate_run_context};

fn call(
    run: &AiRunId,
    id: &str,
    name: &str,
    result: &str,
    outcome: ToolCallOutcome,
) -> ToolCallRecord {
    ToolCallRecord::new(ToolCallRecordInput {
        run_id: run.clone(),
        request_id: ToolRequestId::try_new(id).expect("요청"),
        tool_name: ToolName::try_new(name).expect("도구"),
        arguments_json: ToolPayloadJson::try_new(r#"{"keyword":"n-1"}"#).expect("인자"),
        result_json: ToolPayloadJson::try_new(result).expect("결과"),
        outcome,
        started_at: crate::tests::timestamp(),
        finished_at: crate::tests::timestamp(),
        protocol_version: ToolProtocolVersion::try_new("2025-06-18").expect("버전"),
    })
}

const ORIGINAL_RESULT: &str = r#"{"items":[{"handle":"n-1","symbols":["005930"],"citation_digest":"d05d94f04082c3cc48a13944d98c3db12c83e66392d9dbfd1e4007f15adcd7cc"}]}"#;

async fn record_unrelated_calls(chat: &ChatStore<'_>, run: &AiRunId) {
    let other = AiRunId::try_new("run-b").expect("다른 실행");
    for record in [
        call(
            run,
            "wrong-tool",
            "get_profile",
            ORIGINAL_RESULT,
            ToolCallOutcome::Succeeded,
        ),
        call(
            &other,
            "foreign",
            "search_news",
            ORIGINAL_RESULT,
            ToolCallOutcome::Succeeded,
        ),
        call(
            run,
            "failed",
            "search_news",
            ORIGINAL_RESULT,
            ToolCallOutcome::ToolError,
        ),
        call(
            run,
            "arguments",
            "search_news",
            r#"{"items":[]}"#,
            ToolCallOutcome::Succeeded,
        ),
        call(
            run,
            "nested",
            "search_news",
            r#"{"items":[{"title":{"untrusted_text":"n-1"}}]}"#,
            ToolCallOutcome::Succeeded,
        ),
    ] {
        chat.record_call(&record).await.expect("호출 기록");
    }
}

async fn verify_saved_and_replay(
    db: &Database,
    task: &domain::CostEligibleTask,
    context: application::EstimateRunContext,
    arguments: EstimateToolArguments<EstimateSource>,
    item: &domain::NewsItem,
) {
    let estimates = EstimateStore::new(db);
    let chat = ChatStore::new(db);
    let events = EventBus::default();
    let now = SteppedClock::new(task.task().created_at());
    let submit = SubmitEstimateTool::new(&estimates, &chat, &now, &events);
    let records = estimates
        .submission_records(task.task().volatility().instrument())
        .await
        .expect("제출 조회");
    let [record] = records.as_slice() else {
        panic!("제출 1건");
    };
    let facts = SignalStore::new(db)
        .sources(record.task_id())
        .await
        .expect("독립 원장");
    assert_eq!(facts.items, vec![item.collected().clone()]);
    let [EvidenceRef::News(NewsSource::Yonhap, url)] =
        record.submission().evidence().expect("출처").as_items()
    else {
        panic!("기사 출처");
    };
    assert_eq!(url.as_url(), item.collected().url());
    let replay = submit
        .execute(
            context,
            EstimateToolArguments {
                sources: Vec::<EstimateSource>::new(),
                ..arguments
            },
        )
        .await;
    assert_eq!(
        replay,
        Err(SubmitEstimateToolError::Submit(
            application::SubmitEstimateError::Assignment(EstimateAssignmentError::AlreadyConsumed)
        ))
    );
    assert_eq!(
        SignalStore::new(db)
            .sources(record.task_id())
            .await
            .expect("저장된 출처"),
        facts
    );
}

async fn issue_with_article(
    news_store: &InformationStore<'_>,
    estimates: &EstimateStore<'_>,
    run: &AiRunId,
    id: &CalibrationEstimateId,
    task: &domain::CostEligibleTask,
) -> (domain::NewsItem, CollectedItemHandle) {
    let at = task.task().created_at();
    let item = news(
        NewsSource::Yonhap,
        "https://example.com/original",
        "실적 발표",
        None,
    );
    news_store.store(&item).await.expect("기사 저장");
    news_store
        .record_times(
            &CollectedEntry::News(item.clone()),
            PublicationTime::Known(at),
            at,
        )
        .await
        .expect("수집 시각");
    estimates.issue_task(id, run, task).await.expect("발급");
    let handle = CollectedItemHandle::try_new("n-1").expect("기사");
    (item, handle)
}

async fn link(info: &InformationStore<'_>, item: &domain::NewsItem, symbol: &str) {
    let key = application::InformationKey::News(NewsSource::Yonhap, item.collected().url().clone());
    application::InformationLinks::set_instruments(
        info,
        &key,
        &[domain::InstrumentId::try_new(symbol).expect("종목")],
    )
    .await
    .expect("연결");
}

#[test]
fn verified_news_and_its_estimate_are_committed_together_and_a_retry_cannot_replace_sources() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("저장소");
    runtime().block_on(async {
        let news_store = InformationStore::new(&db);
        let estimates = EstimateStore::new(&db);
        let chat = ChatStore::new(&db);
        let task = sample_cost_eligible_task().expect("과제");
        let context = sample_estimate_run_context("run-a").expect("실행");
        let at = task.task().created_at();
        let id = CalibrationEstimateId::try_new("overnight-task").expect("아이디");
        let (item, handle) =
            issue_with_article(&news_store, &estimates, &context.run, &id, &task).await;
        record_unrelated_calls(&chat, &context.run).await;
        let events = EventBus::default();
        let now = SteppedClock::new(at);
        let submit = SubmitEstimateTool::new(&estimates, &chat, &now, &events);
        let arguments = EstimateToolArguments {
            task_id: id.clone(),
            probability: AiProbabilityInput::try_new("90").expect("확률"),
            rationale: text("공시 확인"),
            counterargument: text("불확실"),
            sources: vec![EstimateSource::Collected(handle)],
        };
        assert_eq!(
            submit.execute(context.clone(), arguments.clone()).await,
            Err(SubmitEstimateToolError::UnknownSource)
        );
        assert_eq!(
            estimates
                .submission_records(task.task().volatility().instrument())
                .await
                .expect("제출 조회")
                .len(),
            0
        );
        chat.record_call(&call(
            &context.run,
            "seen",
            "search_news",
            ORIGINAL_RESULT,
            ToolCallOutcome::Succeeded,
        ))
        .await
        .expect("목록 반환");
        link(&news_store, &item, "other-symbol").await;
        assert_eq!(
            submit.execute(context.clone(), arguments.clone()).await,
            Err(SubmitEstimateToolError::UnknownSource)
        );
        link(&news_store, &item, "005930").await;
        submit
            .execute(context.clone(), arguments.clone())
            .await
            .expect("제출");
        verify_saved_and_replay(&db, &task, context, arguments, &item).await;
    });
}

async fn record_initial_article(
    db: &Database,
    run: &AiRunId,
    handle: &CollectedItemHandle,
    at: domain::Timestamp,
) {
    let info = InformationStore::new(db);
    let first = news(
        NewsSource::Yonhap,
        "https://example.com/a",
        "동일 제목",
        Some("첫 본문"),
    );
    let entry = CollectedEntry::News(first.clone());
    info.store(&first).await.expect("저장");
    info.record_times(&entry, PublicationTime::Known(at), at)
        .await
        .expect("시각");
    let result = r#"{"items":[{"handle":"n-1","citation_digest":"321d8a004b3e98522df72198abdb0f008db98e758908fff5700e11535c486caa"}]}"#;
    ChatStore::new(db)
        .record_call(&call(
            run,
            "record",
            "search_news",
            result,
            ToolCallOutcome::Succeeded,
        ))
        .await
        .expect("원장");
    assert!(
        info.citation_in_run(run, handle)
            .await
            .expect("확인")
            .is_some()
    );
}

#[test]
fn reused_news_row_and_changed_body_cannot_inherit_a_prior_lookup() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("저장소");
    runtime().block_on(async {
        use application::CollectedRetention;
        let info = InformationStore::new(&db);
        let run = AiRunId::try_new("audit-run").expect("실행");
        let handle = CollectedItemHandle::try_new("n-1").expect("핸들");
        let at = crate::tests::timestamp();
        record_initial_article(&db, &run, &handle, at).await;
        let revised = news(
            NewsSource::Yonhap,
            "https://example.com/a",
            "동일 제목",
            Some("변경 본문"),
        );
        info.store(&revised).await.expect("본문 수정");
        assert!(
            info.citation_in_run(&run, &handle)
                .await
                .expect("본문 구분")
                .is_none()
        );
        info.prune_news(&[application::RetentionCutoff {
            source: NewsSource::Yonhap,
            before: at.checked_add(domain::TimeSpan::NANOSECOND).expect("만료"),
        }])
        .await
        .expect("삭제");
        let replacement = news(
            NewsSource::Yonhap,
            "https://example.com/b",
            "동일 제목",
            Some("첫 본문"),
        );
        info.store(&replacement).await.expect("행 재사용");
        info.record_times(
            &CollectedEntry::News(replacement),
            PublicationTime::Known(at),
            at,
        )
        .await
        .expect("시각");
        assert!(
            info.collected_item(&handle)
                .await
                .expect("재사용 확인")
                .is_some()
        );
        assert!(
            info.citation_in_run(&run, &handle)
                .await
                .expect("URL 구분")
                .is_none()
        );
    });
}
