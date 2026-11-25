use super::AppWebResearch;
use crate::ai_runtime::{ChatTurnBook, ChatTurnRef};
use application::{
    AiEvent, AiRunId, AuditEntry, AuditIntegrity, AuditLog, CollectionQuery, CollectionTopic,
    InformationError, IsolationCheck, IsolationFailure, RecordOutcome, ResearchTopic, RunInput,
    RunKind, SystemPortError, Watchlist, WatchlistEntry, WatchlistRepository,
    WatchlistStoreOutcome, WebResearchToolFailure, WebResearchToolPort, WebResearchToolRequest,
};
use application::{
    ChatRepository, ChatRunLedger, ChatRunLedgerEntry, ChatSequence, ChatSessionId,
    ChatSessionRecord, ChatSessionSummary, ChatTurnId, StoredChatContent, StoredChatEntry,
    StoredUserMessage, UserMessageId, WriteCompleted,
};
use domain::{InstrumentId, Symbol, Timestamp, UntrustedText};
use std::future::{Future, ready};
use std::sync::{Arc, RwLock};
use testkit_ports::chat_fakes::{AiProviderCall, ScriptedAiEventStream, ScriptedAiProvider};
use testkit_ports::chat_fakes::{MemoryChatRepository, MemoryChatRunLedger};

fn at() -> Timestamp {
    Timestamp::from_parts(
        domain::UnixSeconds::new(1_790_000_000_i64),
        domain::Nanosecond::new(0_u32),
        domain::OffsetSeconds::new(0_i32),
    )
    .expect("시각")
}

#[derive(Default)]
struct Store {
    audits: RwLock<Vec<AuditEntry>>,
    chat: MemoryChatRepository,
    ledger: MemoryChatRunLedger,
}

impl ChatRepository for Store {
    fn ensure_session(
        &self,
        session: &ChatSessionId,
        now: Timestamp,
    ) -> impl Future<Output = Result<ChatSessionRecord, SystemPortError>> + Send {
        self.chat.ensure_session(session, now)
    }

    fn load_session(
        &self,
        session: &ChatSessionId,
    ) -> impl Future<Output = Result<Option<ChatSessionRecord>, SystemPortError>> + Send {
        self.chat.load_session(session)
    }

    fn list_sessions(
        &self,
    ) -> impl Future<Output = Result<Vec<ChatSessionSummary>, SystemPortError>> + Send {
        self.chat.list_sessions()
    }

    fn append_entry(
        &self,
        session: &ChatSessionId,
        entry: &StoredChatEntry,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        self.chat.append_entry(session, entry)
    }

    fn load_user_message(
        &self,
        session: &ChatSessionId,
        message_id: &UserMessageId,
    ) -> impl Future<Output = Result<Option<StoredUserMessage>, SystemPortError>> + Send {
        self.chat.load_user_message(session, message_id)
    }
}

impl ChatRunLedger for Store {
    fn record_chat_run(
        &self,
        entry: &ChatRunLedgerEntry,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        self.ledger.record_chat_run(entry)
    }
}

impl WatchlistRepository for Store {
    fn load(&self) -> impl Future<Output = Result<Watchlist, SystemPortError>> + Send {
        ready(Ok(Watchlist::try_from_entries(
            ["AAPL", "005930"]
                .into_iter()
                .map(|id| WatchlistEntry::new(InstrumentId::try_new(id).expect("종목"), at()))
                .collect(),
        )
        .expect("관심 종목")))
    }

    fn store(
        &self,
        _previous: &Watchlist,
        _next: &Watchlist,
    ) -> impl Future<Output = Result<WatchlistStoreOutcome, SystemPortError>> + Send {
        ready(Ok(WatchlistStoreOutcome::Stored))
    }
}

impl AuditLog for Store {
    fn append(
        &self,
        entry: &AuditEntry,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        if let Ok(mut audits) = self.audits.write() {
            audits.push(entry.clone());
        }
        ready(Ok(RecordOutcome::Recorded))
    }

    fn verify(&self) -> impl Future<Output = Result<AuditIntegrity, SystemPortError>> + Send {
        ready(Ok(AuditIntegrity::Verified))
    }
}

fn research_with(
    provider: &Arc<ScriptedAiProvider>,
    store: &Arc<Store>,
    turns: ChatTurnBook,
) -> AppWebResearch<Store, ScriptedAiProvider, testkit_ports::SteppedClock> {
    AppWebResearch::new(
        Arc::clone(store),
        Arc::clone(provider),
        Arc::new(testkit_ports::SteppedClock::new(at())),
        turns,
    )
}

fn research(
    provider: &Arc<ScriptedAiProvider>,
    store: &Arc<Store>,
) -> AppWebResearch<Store, ScriptedAiProvider, testkit_ports::SteppedClock> {
    research_with(provider, store, ChatTurnBook::new())
}

fn request(symbol: &str, topics: Vec<ResearchTopic>) -> WebResearchToolRequest {
    WebResearchToolRequest::CodeTopics {
        symbols: vec![Symbol::try_new(symbol).expect("종목")],
        topics,
    }
}

fn run_id() -> AiRunId {
    AiRunId::try_new("chat-run-1").expect("실행")
}

fn audit_count(store: &Store) -> usize {
    store.audits.read().expect("감사 기록").len()
}

#[test]
fn approved_code_topics_launch_one_collection_run_and_return_its_summary() {
    let provider = Arc::new(ScriptedAiProvider::default());
    provider
        .script_run()
        .push(Ok(ScriptedAiEventStream::from_events(vec![
            AiEvent::IsolationChecked(IsolationCheck::Passed),
            AiEvent::TextDelta(UntrustedText::try_new("실적 발표 예정").expect("요약")),
        ])));
    let store = Arc::new(Store::default());
    let port = research(&provider, &store);
    let outcome = tauri::async_runtime::block_on(port.request_research(
        &run_id(),
        &request(
            "AAPL",
            vec![ResearchTopic::Earnings, ResearchTopic::Litigation],
        ),
    ))
    .expect("조사");
    assert_eq!(outcome.summary.as_untrusted_str(), "실적 발표 예정");
    assert_eq!(outcome.collected_at, at());
    assert_eq!(audit_count(&store), 2_usize, "검색어마다 감사 기록");
    let calls = provider.calls();
    let [AiProviderCall::Run(spec)] = calls.as_slice() else {
        panic!("수집 실행 하나: {calls:?}");
    };
    assert_eq!(spec.kind(), RunKind::Collection);
    assert_eq!(
        spec.input(),
        &RunInput::Collection(CollectionQuery::CodeTopics {
            instruments: vec![Symbol::try_new("AAPL").expect("종목")],
            topics: vec![CollectionTopic::Earnings, CollectionTopic::Disclosures],
        })
    );
}

#[test]
fn rejected_queries_unknown_targets_and_user_mode_never_reach_the_collector() {
    let provider = Arc::new(ScriptedAiProvider::default());
    let store = Arc::new(Store::default());
    let port = research(&provider, &store);
    let ask = |request: WebResearchToolRequest| {
        tauri::async_runtime::block_on(port.request_research(&run_id(), &request)).err()
    };
    assert_eq!(
        ask(request("005930", vec![ResearchTopic::Earnings])),
        Some(WebResearchToolFailure::QueryRejected),
        "숫자 종목 코드가 든 검색어는 SEC-9 검사에서 거부"
    );
    assert_eq!(audit_count(&store), 1_usize, "거부도 감사에 남는다");
    assert_eq!(
        ask(request("MSFT", vec![ResearchTopic::Earnings])),
        Some(WebResearchToolFailure::QueryRejected),
        "앱이 모르는 종목"
    );
    assert_eq!(
        ask(request("AAPL", Vec::new())),
        Some(WebResearchToolFailure::QueryRejected)
    );
    assert_eq!(
        ask(WebResearchToolRequest::StoredUserMessage),
        Some(WebResearchToolFailure::UserMessageMissing),
        "채팅 턴에 묶이지 않은 실행"
    );
    assert_eq!(provider.calls().len(), 0_usize);
}

#[test]
fn isolation_rejection_or_empty_collection_fails_the_research() {
    let provider = Arc::new(ScriptedAiProvider::default());
    provider
        .script_run()
        .push(Ok(ScriptedAiEventStream::from_events(vec![
            AiEvent::IsolationChecked(IsolationCheck::Rejected(IsolationFailure::Tools)),
            AiEvent::TextDelta(UntrustedText::try_new("x").expect("요약")),
        ])));
    provider
        .script_run()
        .push(Ok(ScriptedAiEventStream::from_events(vec![
            AiEvent::IsolationChecked(IsolationCheck::Passed),
        ])));
    provider
        .script_run()
        .push(Err(InformationError::Unavailable(
            application::ResetTime::NotProvided,
        )));
    let store = Arc::new(Store::default());
    let port = research(&provider, &store);
    for _attempt in 0_u8..3_u8 {
        assert_eq!(
            tauri::async_runtime::block_on(
                port.request_research(&run_id(), &request("AAPL", vec![ResearchTopic::Guidance]))
            )
            .err(),
            Some(WebResearchToolFailure::CollectionFailed)
        );
    }
}

fn session() -> ChatSessionId {
    ChatSessionId::try_new("session-1").expect("세션")
}

fn turn() -> ChatTurnId {
    ChatTurnId::try_new("turn-1").expect("턴")
}

fn stored_message() -> StoredUserMessage {
    StoredUserMessage::from_store_result(
        UserMessageId::try_new("msg-1").expect("메시지"),
        UntrustedText::try_new("엔비디아 실적 반응 찾아봐").expect("문장"),
    )
}

fn seeded_store() -> Arc<Store> {
    let store = Arc::new(Store::default());
    tauri::async_runtime::block_on(async {
        let _session = store.ensure_session(&session(), at()).await.expect("세션");
        let WriteCompleted = store
            .append_entry(
                &session(),
                &StoredChatEntry::new(
                    ChatSequence::new(1),
                    at(),
                    turn(),
                    StoredChatContent::User(stored_message()),
                ),
            )
            .await
            .expect("저장");
    });
    store
}

#[test]
fn user_message_mode_researches_the_stored_message_of_the_running_chat_turn() {
    let provider = Arc::new(ScriptedAiProvider::default());
    provider
        .script_run()
        .push(Ok(ScriptedAiEventStream::from_events(vec![
            AiEvent::IsolationChecked(IsolationCheck::Passed),
            AiEvent::TextDelta(UntrustedText::try_new("발표 반응 요약").expect("요약")),
        ])));
    let store = seeded_store();
    let turns = ChatTurnBook::new();
    let port = research_with(&provider, &store, turns.clone());
    let unbound = tauri::async_runtime::block_on(
        port.request_research(&run_id(), &WebResearchToolRequest::StoredUserMessage),
    );
    assert_eq!(
        unbound.err(),
        Some(WebResearchToolFailure::UserMessageMissing)
    );
    let _turn = turns.open(
        &run_id(),
        ChatTurnRef {
            session: session(),
            turn: turn(),
        },
    );
    let outcome = tauri::async_runtime::block_on(
        port.request_research(&run_id(), &WebResearchToolRequest::StoredUserMessage),
    )
    .expect("조사");
    assert_eq!(outcome.summary.as_untrusted_str(), "발표 반응 요약");
    let calls = provider.calls();
    let [AiProviderCall::Run(spec)] = calls.as_slice() else {
        panic!("수집 실행 하나: {calls:?}");
    };
    assert_eq!(
        spec.input(),
        &RunInput::Collection(CollectionQuery::UserMessage(stored_message())),
        "저장된 사용자 문장 원문을 그대로 넘긴다"
    );
    assert_eq!(
        audit_count(&store),
        1_usize,
        "사용자 문장 조사도 감사에 남긴다"
    );
}
