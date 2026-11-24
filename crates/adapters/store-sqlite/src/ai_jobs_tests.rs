use crate::sql::{ai_jobs as jobs_sql, ai_jobs_tests as sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{AiJobStore, Database, DatabasePath, PrinciplesStore};
use application::{
    AiJobFinishRecord, AiJobOutcome, AiJobRecordStore, AiJobRecordWrite, AiJobRun, AiJobTiming,
    PrinciplesRepository, RunKind, RunPurpose, SystemFailure, TokenCount, TokenUsage,
};
use domain::{
    AiJobId, CashFloor, ConcentrationFloor, InvestmentPrinciples, Nanosecond, OffsetSeconds,
    PolicyText, PrincipleNote, Timestamp, UnixSeconds,
};
use testkit_ports::MemoryAiJobRecords;
use testkit_ports::principles_fakes::MemoryPrinciplesRepository;

fn at(seconds: i64, nanos: u32) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn record(
    job: &str,
    purpose: RunPurpose,
    outcome: AiJobOutcome,
    finished: Timestamp,
) -> AiJobFinishRecord {
    AiJobFinishRecord::new(
        AiJobRun::new(
            AiJobId::try_new(job).expect("작업"),
            RunKind::Analysis,
            purpose,
        ),
        outcome,
        AiJobTiming::try_new(at(100, 0), finished).expect("시간"),
        TokenUsage::new(
            TokenCount::new(1_200),
            TokenCount::new(340),
            TokenCount::new(56),
            TokenCount::new(7),
        ),
    )
}

fn records() -> Vec<AiJobFinishRecord> {
    vec![
        record(
            "job-1",
            RunPurpose::MorningBriefing,
            AiJobOutcome::Completed,
            at(200, 0),
        ),
        record(
            "job-2",
            RunPurpose::PairedComparison,
            AiJobOutcome::IsolationRejected,
            at(300, 5),
        ),
        record("job-3", RunPurpose::Chat, AiJobOutcome::Waiting, at(150, 0)),
        record("job-1", RunPurpose::Chat, AiJobOutcome::Failed, at(400, 0)),
    ]
}

async fn exercise<S: AiJobRecordStore>(
    store: &S,
) -> (
    Vec<AiJobRecordWrite>,
    Vec<AiJobFinishRecord>,
    Option<AiJobFinishRecord>,
) {
    let mut writes = Vec::new();
    for item in records() {
        writes.push(store.store_finished(&item).await.expect("store"));
    }
    let between = store
        .finished_between(at(150, 0), at(300, 5))
        .await
        .expect("between");
    let found = store
        .finished(&AiJobId::try_new("job-1").expect("작업"))
        .await
        .expect("find");
    (writes, between, found)
}

#[test]
fn ai_job_records_append_once_and_match_the_memory_contract() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let stored = rt.block_on(exercise(&AiJobStore::new(&db)));
    let memory = rt.block_on(exercise(&MemoryAiJobRecords::default()));
    assert_eq!(stored, memory);
    assert_eq!(
        stored.0,
        vec![
            AiJobRecordWrite::Stored,
            AiJobRecordWrite::Stored,
            AiJobRecordWrite::Stored,
            AiJobRecordWrite::AlreadyStored,
        ]
    );
    assert_eq!(stored.1.len(), 3);
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    let store = AiJobStore::new(&reopened);
    assert_eq!(
        rt.block_on(store.finished(&AiJobId::try_new("job-2").expect("작업")))
            .expect("find"),
        records().into_iter().nth(1)
    );
    assert_eq!(
        rt.block_on(store.finished(&AiJobId::try_new("job-9").expect("작업")))
            .expect("none"),
        None
    );
    rt.block_on(reopened.close()).expect("close");
}

fn principles() -> InvestmentPrinciples {
    let text = PolicyText::try_new("  배당이 꾸준한 종목을 우선한다  ").expect("문구");
    InvestmentPrinciples::new(
        Some(PrincipleNote::try_new(&text).expect("원칙")),
        CashFloor::TwentyPercent,
        ConcentrationFloor::Eight,
    )
}

#[test]
fn principles_replace_and_match_the_memory_contract() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = PrinciplesStore::new(&db);
    let fake = MemoryPrinciplesRepository::default();
    assert_eq!(rt.block_on(store.principles()).expect("empty"), None);
    for value in [principles(), InvestmentPrinciples::default(), principles()] {
        rt.block_on(store.store_principles(&value)).expect("store");
        rt.block_on(fake.store_principles(&value)).expect("fake");
        assert_eq!(
            rt.block_on(store.principles()).expect("load"),
            rt.block_on(fake.principles()).expect("fake load")
        );
    }
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    assert_eq!(
        rt.block_on(PrinciplesStore::new(&reopened).principles())
            .expect("load"),
        Some(principles())
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn ai_job_and_principle_tables_reject_rewrites_and_corruption() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    rt.block_on(AiJobStore::new(&db).store_finished(&records()[0]))
        .expect("job");
    rt.block_on(PrinciplesStore::new(&db).store_principles(&principles()))
        .expect("principles");
    for statement in [sql::UPDATE_JOB, sql::DELETE_JOB, sql::DELETE_PRINCIPLES] {
        assert!(
            rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
                .is_err(),
            "{statement}"
        );
    }
    for statement in [sql::CORRUPT_JOB, sql::FUTURE_PRINCIPLES] {
        rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
            .expect(statement);
    }
    assert_eq!(
        rt.block_on(AiJobStore::new(&db).finished_between(at(0, 0), at(1_000, 0)))
            .expect_err("corrupt")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    assert_eq!(
        rt.block_on(PrinciplesStore::new(&db).principles())
            .expect_err("future")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn ai_job_sql_constants_match_the_migration_and_prepare() {
    assert_eq!(
        std::fs::read_to_string("migrations/0020_ai_jobs_and_principles.sql").expect("file"),
        jobs_sql::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [
        jobs_sql::INSERT_JOB,
        jobs_sql::FIND_JOB,
        jobs_sql::JOBS_BETWEEN,
        jobs_sql::FIND_PRINCIPLES,
        jobs_sql::UPSERT_PRINCIPLES,
    ] {
        connection.prepare(statement).expect(statement);
    }
}

#[test]
fn prompt_reference_round_trips_and_hash_comes_from_the_text() {
    use application::{LoadedPrompt, PromptHasher, PromptSource, PromptTask, PromptVersion};
    let hasher = crate::Sha256PromptHasher;
    assert_eq!(
        hasher.digest(&PromptSource::new("abc")).as_str(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let source = PromptSource::new(
        &std::fs::read_to_string("../../../prompts/estimate/v1.md").expect("추정 프롬프트"),
    );
    let loaded = LoadedPrompt::load(
        &PromptTask::new("estimate"),
        &PromptVersion::new("v1"),
        &source,
        &hasher,
    )
    .expect("로드");
    let changed = PromptSource::new(&format!("{}\n", source.as_str()));
    assert_ne!(
        hasher.digest(&changed),
        loaded.prompt_ref().digest().clone()
    );
    let with_prompt = record(
        "job-p",
        RunPurpose::Estimate,
        AiJobOutcome::Completed,
        at(500, 0),
    )
    .with_prompt(loaded.prompt_ref());
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = AiJobStore::new(&db);
    rt.block_on(store.store_finished(&with_prompt))
        .expect("store");
    let found = rt
        .block_on(store.finished(with_prompt.job()))
        .expect("find")
        .expect("record");
    assert_eq!(found.prompt(), Some(&loaded.prompt_ref()));
    assert_eq!(found, with_prompt);
    rt.block_on(db.close()).expect("close");
}
