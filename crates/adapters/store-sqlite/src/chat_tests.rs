use crate::sql::{chat as chat_sql, chat_tests as sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{ChatStore, Database, DatabasePath};
use application::{
    AiRunId, ChatCard, ChatCardId, ChatCardKind, ChatFailureReason, ChatRepository, ChatRunLedger,
    ChatRunLedgerEntry, ChatRunModelSpec, ChatRunProfile, ChatRunTiming, ChatSequence,
    ChatSessionId, ChatSessionRecord, ChatSessionSummary, ChatSource, ChatSourceKind, ChatTurnId,
    ChatTurnIdentity, ChatTurnStatus, CredentialSource, IsolationFailure, ModelName,
    ModelSelection, ReasoningEffort, RecordOutcome, ResetTime, ResponseMismatch, RetryDelay,
    RunKind, RunPurpose, StoredAssistantAttachments, StoredAssistantReply, StoredChatContent,
    StoredChatEntry, StoredUserMessage, SystemFailure, TokenCount, TokenUsage, ToolCallLedger,
    ToolCallOutcome, ToolCallRecord, ToolCallRecordInput, ToolName, ToolPayloadJson,
    ToolProtocolVersion, ToolRequestId, UserMessageId,
};
use domain::{
    Nanosecond, NormalizedUrl, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds,
    UntrustedText,
};
use testkit_ports::chat_fakes::MemoryChatRepository;

type Observed = (
    Option<ChatSessionRecord>,
    Vec<ChatSessionSummary>,
    Option<StoredUserMessage>,
    Option<StoredUserMessage>,
);

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(7),
        OffsetSeconds::new(32_400),
    )
    .expect("time")
}

fn session() -> ChatSessionId {
    ChatSessionId::try_new("session-1").expect("session")
}

fn turn() -> ChatTurnId {
    ChatTurnId::try_new("turn-1").expect("turn")
}

fn run() -> AiRunId {
    AiRunId::try_new("run-1").expect("run")
}

fn text(value: &str) -> UntrustedText {
    UntrustedText::try_new(value).expect("text")
}

fn span() -> TimeSpan {
    TimeSpan::try_new(SpanSeconds::new(3), Nanosecond::new(5)).expect("span")
}

fn statuses() -> Vec<ChatTurnStatus> {
    vec![
        ChatTurnStatus::InProgress,
        ChatTurnStatus::Completed,
        ChatTurnStatus::Cancelled,
        ChatTurnStatus::Interrupted,
        ChatTurnStatus::RateLimited(ResetTime::Known(at(50))),
        ChatTurnStatus::RateLimited(ResetTime::NotProvided),
        ChatTurnStatus::Failed(ChatFailureReason::IsolationRejected(
            IsolationFailure::ApiKeySource,
        )),
        ChatTurnStatus::Failed(ChatFailureReason::IsolationNotVerified),
        ChatTurnStatus::Failed(ChatFailureReason::McpUnavailable),
        ChatTurnStatus::Failed(ChatFailureReason::ProviderUnavailable),
        ChatTurnStatus::Failed(ChatFailureReason::InvalidResponse(ResponseMismatch::Schema)),
        ChatTurnStatus::Failed(ChatFailureReason::InvalidCredential(CredentialSource::Fred)),
        ChatTurnStatus::Failed(ChatFailureReason::ForbiddenToolAttempt),
        ChatTurnStatus::Failed(ChatFailureReason::EmptyMessage),
        ChatTurnStatus::Failed(ChatFailureReason::PromptTooLong),
        ChatTurnStatus::Failed(ChatFailureReason::StorageError(SystemFailure::Unavailable(
            RetryDelay::Known(span()),
        ))),
        ChatTurnStatus::Failed(ChatFailureReason::StorageError(SystemFailure::Unavailable(
            RetryDelay::Unknown,
        ))),
        ChatTurnStatus::Failed(ChatFailureReason::StorageError(
            SystemFailure::SecretIntegrity,
        )),
        ChatTurnStatus::Failed(ChatFailureReason::QueueBusy),
    ]
}

fn user_entry(sequence: u64, id: &str) -> StoredChatEntry {
    StoredChatEntry::new(
        ChatSequence::new(sequence),
        at(100),
        turn(),
        StoredChatContent::User(StoredUserMessage::from_store_result(
            UserMessageId::try_new(id).expect("message"),
            text("삼성전자 어때? 1:2"),
        )),
    )
}

fn assistant_entry(sequence: u64, status: ChatTurnStatus) -> StoredChatEntry {
    StoredChatEntry::new(
        ChatSequence::new(sequence),
        at(200),
        turn(),
        StoredChatContent::Assistant(StoredAssistantReply::new(
            run(),
            text("답변 3:4"),
            status,
            StoredAssistantAttachments {
                actual_model: Some(ModelName::try_new("opus-5.5").expect("model")),
                sources: vec![
                    ChatSource::new(
                        ChatSourceKind::Web,
                        text("기사"),
                        Some(NormalizedUrl::try_new("https://example.com/a").expect("url")),
                    ),
                    ChatSource::new(ChatSourceKind::Tool, text("get_portfolio"), None),
                ],
                cards: vec![ChatCard::new(
                    ChatCardKind::Proposal,
                    ChatCardId::try_new("card-1").expect("card"),
                )],
            },
        )),
    )
}

fn conversation() -> Vec<StoredChatEntry> {
    let mut entries = vec![user_entry(1, "message-1")];
    let mut sequence = 2_u64;
    for status in statuses() {
        entries.push(assistant_entry(sequence, status));
        sequence = sequence.checked_add(1).expect("sequence");
    }
    entries
}

async fn exercise<R: ChatRepository>(repository: &R) -> Observed {
    let fresh = repository
        .ensure_session(&session(), at(10))
        .await
        .expect("ensure");
    assert_eq!(fresh.entries().to_vec(), Vec::<StoredChatEntry>::new());
    for entry in conversation() {
        repository
            .append_entry(&session(), &entry)
            .await
            .expect("append");
    }
    let again = repository
        .ensure_session(&session(), at(999))
        .await
        .expect("ensure again");
    assert_eq!(again.summary().created_at(), at(10));
    (
        repository.load_session(&session()).await.expect("load"),
        repository.list_sessions().await.expect("list"),
        repository
            .load_user_message(
                &session(),
                &UserMessageId::try_new("message-1").expect("id"),
            )
            .await
            .expect("user"),
        repository
            .load_user_message(&session(), &UserMessageId::try_new("missing").expect("id"))
            .await
            .expect("missing"),
    )
}

#[test]
fn chat_sessions_round_trip_and_match_the_memory_contract() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let stored = rt.block_on(exercise(&ChatStore::new(&db)));
    let memory = rt.block_on(exercise(&MemoryChatRepository::default()));
    assert_eq!(stored, memory);
    let (loaded, _, found, missing) = stored;
    let record = loaded.expect("record");
    assert_eq!(record.entries(), conversation().as_slice());
    assert_eq!(record.summary().updated_at(), at(200));
    assert_eq!(
        *record.summary().entry_count().as_u64(),
        u64::try_from(conversation().len()).expect("len")
    );
    assert!(found.is_some());
    assert!(missing.is_none());
    rt.block_on(db.close()).expect("close");
}

#[test]
fn chat_sessions_survive_reopen_and_reject_unknown_sessions() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    {
        let db = Database::open(&path, &key(), &clock()).expect("open");
        let store = ChatStore::new(&db);
        assert_eq!(
            rt.block_on(store.append_entry(&session(), &user_entry(1, "m")))
                .expect_err("no session")
                .failure(),
            SystemFailure::RepositoryIntegrity
        );
        assert_eq!(
            rt.block_on(store.load_session(&session())).expect("load"),
            None
        );
        rt.block_on(store.ensure_session(&session(), at(10)))
            .expect("ensure");
        rt.block_on(store.append_entry(&session(), &user_entry(1, "m")))
            .expect("append");
        rt.block_on(db.close()).expect("close")
    };
    let db = Database::open(&path, &key(), &clock()).expect("reopen");
    let record = rt
        .block_on(ChatStore::new(&db).load_session(&session()))
        .expect("load")
        .expect("record");
    assert_eq!(record.entries(), [user_entry(1, "m")].as_slice());
    rt.block_on(db.close()).expect("close");
}

fn ledger_entry(status: ChatTurnStatus) -> ChatRunLedgerEntry {
    ChatRunLedgerEntry::new(
        ChatTurnIdentity {
            session: session(),
            turn: turn(),
            run: run(),
        },
        ChatRunProfile {
            kind: RunKind::Analysis,
            purpose: RunPurpose::Chat,
        },
        ChatRunModelSpec {
            requested_model: ModelSelection::Named(ModelName::try_new("opus").expect("model")),
            actual_model: None,
            effort: ReasoningEffort::XHigh,
            usage: Some(TokenUsage::new(
                TokenCount::new(1),
                TokenCount::new(2),
                TokenCount::new(3),
                TokenCount::new(4),
            )),
        },
        ChatRunTiming {
            started_at: at(1),
            finished_at: at(2),
            first_token_latency: Some(span()),
        },
        status,
    )
}

fn tool_call(result: &str) -> ToolCallRecord {
    ToolCallRecord::new(ToolCallRecordInput {
        run_id: run(),
        request_id: ToolRequestId::try_new("7").expect("request"),
        tool_name: ToolName::try_new("get_portfolio").expect("tool"),
        arguments_json: ToolPayloadJson::try_new("{}").expect("args"),
        result_json: ToolPayloadJson::try_new(result).expect("result"),
        outcome: ToolCallOutcome::ToolError,
        started_at: at(1),
        finished_at: at(2),
        protocol_version: ToolProtocolVersion::try_new("2025-06-18").expect("version"),
    })
}

fn count(db: &Database, statement: &'static str) -> i64 {
    runtime()
        .block_on(db.read(move |connection| {
            connection.query_row(statement, [], |row| row.get::<_, i64>(0))
        }))
        .expect("count")
}

#[test]
fn run_and_tool_call_ledgers_are_append_only() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = ChatStore::new(&db);
    for status in statuses() {
        assert_eq!(
            rt.block_on(store.record_chat_run(&ledger_entry(status)))
                .expect("record"),
            RecordOutcome::Recorded
        );
    }
    assert_eq!(
        rt.block_on(store.record_chat_run(&ledger_entry(ChatTurnStatus::Completed)))
            .expect("again"),
        RecordOutcome::AlreadyRecorded
    );
    for result in ["{\"a\":1}", "{\"a\":1}"] {
        assert_eq!(
            rt.block_on(store.record_call(&tool_call(result)))
                .expect("tool"),
            RecordOutcome::Recorded
        );
    }
    assert_eq!(
        count(&db, sql::RUN_COUNT),
        i64::try_from(statuses().len()).expect("statuses")
    );
    assert_eq!(count(&db, sql::TOOL_COUNT), 2);
    let row = rt
        .block_on(db.read(|connection| {
            connection.query_row(sql::TOOL_ROW, [], |row| {
                Ok([
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ])
            })
        }))
        .expect("row");
    assert_eq!(
        row,
        [
            "run-1",
            "7",
            "get_portfolio",
            "{}",
            "{\"a\":1}",
            "tool_error",
            "2025-06-18"
        ]
        .map(String::from)
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn chat_tables_reject_rewrites_deletes_and_gaps() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = ChatStore::new(&db);
    rt.block_on(store.record_chat_run(&ledger_entry(ChatTurnStatus::Completed)))
        .expect("run");
    rt.block_on(store.record_call(&tool_call("{}")))
        .expect("tool");
    rt.block_on(store.ensure_session(&session(), at(1)))
        .expect("session");
    rt.block_on(store.append_entry(&session(), &user_entry(1, "m")))
        .expect("entry");
    for statement in [
        sql::UPDATE_ENTRY,
        sql::DELETE_ENTRY,
        sql::DELETE_SESSION,
        sql::RENAME_SESSION,
        sql::UPDATE_RUN,
        sql::DELETE_RUN,
        sql::UPDATE_TOOL,
        sql::DELETE_TOOL,
        sql::ENTRY_GAP,
    ] {
        assert!(
            rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
                .is_err(),
            "{statement}"
        );
    }
    rt.block_on(db.close()).expect("close");
}

#[test]
fn chat_sql_constants_match_the_migration_and_prepare() {
    assert_eq!(
        std::fs::read_to_string("migrations/0013_chat_and_run_ledgers.sql").expect("file"),
        chat_sql::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [
        chat_sql::SESSION,
        chat_sql::INSERT_SESSION,
        chat_sql::SESSIONS,
        chat_sql::TOUCH_SESSION,
        chat_sql::ENTRIES,
        chat_sql::INSERT_ENTRY,
        chat_sql::USER_MESSAGE,
        chat_sql::FIND_RUN,
        chat_sql::INSERT_RUN,
        chat_sql::INSERT_TOOL_CALL,
        sql::RUN_COUNT,
        sql::TOOL_COUNT,
        sql::TOOL_ROW,
    ] {
        connection.prepare(statement).expect(statement);
    }
}

#[test]
fn reopened_session_list_keeps_empty_sessions_and_titles_from_each_first_user_entry() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let main = ChatSessionId::try_new("main-chat").expect("기존 기본 대화");
    let empty = ChatSessionId::try_new("session-new").expect("빈 대화");
    let rt = runtime();
    rt.block_on(async {
        let db = Database::open(&path, &key(), &clock()).expect("open");
        let store = ChatStore::new(&db);
        store
            .ensure_session(&main, at(10))
            .await
            .expect("기본 대화");
        store.ensure_session(&empty, at(20)).await.expect("빈 대화");
        store
            .append_entry(&main, &assistant_entry(1, ChatTurnStatus::Completed))
            .await
            .expect("안내");
        store
            .append_entry(&main, &user_entry(2, "first"))
            .await
            .expect("첫 질문");
        store
            .append_entry(&main, &user_entry(3, "second"))
            .await
            .expect("다음 질문");
        db.close().await.expect("닫기");
        let reopened = Database::open(&path, &key(), &clock()).expect("재시작");
        let listed = ChatStore::new(&reopened)
            .list_sessions()
            .await
            .expect("목록");
        let [legacy, new] = listed.as_slice() else {
            panic!("두 대화")
        };
        assert_eq!(legacy.id(), &main);
        assert_eq!(
            legacy.title().expect("제목").as_untrusted_str(),
            "삼성전자 어때? 1:2"
        );
        assert_eq!(new.id(), &empty);
        assert_eq!(new.title(), None);
        assert_eq!(new.entry_count(), ChatSequence::new(0));
        reopened.close().await.expect("닫기");
    });
}
