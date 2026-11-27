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
    news: Vec<application::ObservedCollectedItem>,
}

impl application::CollectedItemReads for Store {
    fn collected_since(
        &self,
        _kind: application::CollectedKind,
        _since: Timestamp,
    ) -> impl Future<Output = Result<Vec<application::ObservedCollectedItem>, SystemPortError>> + Send
    {
        ready(Ok(self.news.clone()))
    }
    fn collected_item(
        &self,
        _handle: &application::CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<application::ObservedCollectedItem>, SystemPortError>> + Send
    {
        ready(Ok(None))
    }
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

struct HeldOnly(Vec<&'static str>);

type ToolRead<T> = Result<Option<T>, SystemPortError>;

impl crate::mcp_portfolio::PortfolioToolSource for HeldOnly {
    fn portfolio(
        &self,
    ) -> crate::ipc::state::BoxFuture<'_, ToolRead<application::PortfolioToolView>> {
        Box::pin(ready(Ok(None)))
    }

    fn allocation(
        &self,
    ) -> crate::ipc::state::BoxFuture<'_, ToolRead<application::AllocationToolView>> {
        Box::pin(ready(Ok(None)))
    }

    fn transactions(
        &self,
    ) -> crate::ipc::state::BoxFuture<'_, ToolRead<application::TransactionsToolView>> {
        Box::pin(ready(Ok(None)))
    }

    fn performance(
        &self,
        _period: application::PerformancePeriod,
    ) -> crate::ipc::state::BoxFuture<'_, ToolRead<application::PerformanceToolView>> {
        Box::pin(ready(Ok(None)))
    }

    fn held(&self) -> crate::ipc::state::BoxFuture<'_, Vec<Symbol>> {
        Box::pin(ready(
            self.0
                .iter()
                .map(|symbol| Symbol::try_new(symbol).expect("종목"))
                .collect(),
        ))
    }
}

fn samsung_master() -> domain::MasterInstrument {
    domain::MasterInstrument::try_new(domain::MasterInstrumentRow {
        listing_market: domain::MasterCode::try_new("KOSPI").expect("시장"),
        symbol: Symbol::try_new("005930").expect("심볼"),
        instrument_id: InstrumentId::try_new("inst-1").expect("ID"),
        name: domain::DisplayInstrumentName::try_new("삼성전자").expect("이름"),
        security_type: domain::MasterSecurityType::from_code(
            domain::MasterCode::try_new("STOCK").expect("유형"),
        ),
        currency: domain::Currency::Krw,
        listing_status: domain::ListingStatus::Active,
        common_share: domain::CommonShare::Common,
        korean_detail: Some(domain::KoreanMasterDetail {
            liquidation: domain::LiquidationTrading::Regular,
            krx: domain::TradingSuspension::Clear,
            nxt: domain::NxtTrading::Unsupported,
        }),
        observed_at: at(),
    })
    .expect("마스터")
}

#[test]
fn held_instruments_are_research_targets_and_domestic_codes_search_by_master_name() {
    let provider = Arc::new(ScriptedAiProvider::default());
    for _ in 0_u8..2_u8 {
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(UntrustedText::try_new("요약").expect("요약")),
            ])));
    }
    let store = Arc::new(Store::default());
    let directory = crate::holdings_link::InstrumentDirectory::default();
    directory.replace(&[samsung_master()]);
    let port = research(&provider, &store).with_holdings(
        crate::mcp_portfolio::CatalogPortfolio::connected(
            Arc::new(HeldOnly(vec!["005930", "NVDA"])),
            directory,
        ),
    );
    let known = tauri::async_runtime::block_on(port.known_targets(&run_id())).expect("대상");
    assert!(known.contains(&Symbol::try_new("005930").expect("종목")));
    assert!(known.contains(&Symbol::try_new("NVDA").expect("종목")));
    let samsung = tauri::async_runtime::block_on(
        port.request_research(&run_id(), &request("005930", vec![ResearchTopic::Earnings])),
    )
    .expect("종목명으로 검색해 SEC-9 통과");
    assert_eq!(samsung.summary.as_untrusted_str(), "요약");
    let nvidia = tauri::async_runtime::block_on(
        port.request_research(&run_id(), &request("NVDA", vec![ResearchTopic::Earnings])),
    )
    .expect("보유 종목은 관심 목록에 없어도 대상");
    assert_eq!(nvidia.summary.as_untrusted_str(), "요약");
    assert_eq!(provider.calls().len(), 2_usize);
    assert_eq!(
        tauri::async_runtime::block_on(
            port.request_research(&run_id(), &request("MSFT", vec![ResearchTopic::Earnings])),
        )
        .err(),
        Some(WebResearchToolFailure::QueryRejected),
        "보유도 관심도 아닌 종목은 거부"
    );
}

fn public_text(value: &str) -> UntrustedText {
    UntrustedText::try_new(value).expect("합성 자료")
}

fn observation(query: &str, year: i32) -> application::SearchObservation {
    application::SearchObservation {
        query: public_text(query),

        url: domain::NormalizedUrl::try_new("https://public.example/report").expect("URL"),
        source_date: Some((
            application::SourceDateKind::Observed,
            domain::CalendarDate::from_parts(
                domain::CalendarYear::new(year),
                domain::MonthNumber::new(9),
                domain::DayNumber::new(20),
            )
            .expect("관측일"),
        )),
        title: public_text("대표 지수 종가 상승"),
    }
}

fn search_event(query: &str, action: &str) -> AiEvent {
    AiEvent::WebSearch {
        query: public_text(query),
        action: public_text(action),
    }
}

const US_FORTNIGHT: &str = "US stock market SPY index performance past fortnight and outlook next fortnight dated economic releases";
const KR_MONTH: &str = "Korean stock market KOSPI index performance past month and outlook next month dated economic releases";

#[test]
fn market_outlook_rejects_unverified_out_of_window_or_unrelated_evidence() {
    let provider = Arc::new(ScriptedAiProvider::default());
    let port = research(&provider, &seeded_store());
    let request = WebResearchToolRequest::MarketOutlook {
        market: application::OutlookMarket::Us,
        horizon: application::OutlookHorizon::TwoWeeks,
    };
    let passed = AiEvent::IsolationChecked(IsolationCheck::Passed);
    let completed = search_event(US_FORTNIGHT, "completed");
    let fabricated = AiEvent::TextDelta(public_text("도구 없이 만든 요약"));
    let dated = observation(US_FORTNIGHT, 2026);
    for (events, observations) in [
        (vec![passed.clone(), fabricated.clone()], vec![]),
        (vec![passed.clone(), fabricated], vec![dated.clone()]),
        (
            vec![passed.clone(), completed.clone()],
            vec![observation(US_FORTNIGHT, 2025)],
        ),
        (
            vec![passed.clone(), completed.clone()],
            vec![observation(US_FORTNIGHT, 2027)],
        ),
        (vec![passed.clone(), completed.clone()], vec![]),
        (
            vec![passed.clone(), search_event(US_FORTNIGHT, "failed")],
            vec![dated.clone()],
        ),
        (vec![completed.clone()], vec![dated]),
        (vec![passed, completed], vec![observation(KR_MONTH, 2026)]),
    ] {
        provider.script_run().push(Ok(
            ScriptedAiEventStream::from_events(events).with_observations(observations)
        ));
        assert_eq!(
            tauri::async_runtime::block_on(port.request_research(&run_id(), &request)),
            Err(WebResearchToolFailure::CollectionFailed)
        );
    }
}

#[test]
fn market_outlook_uses_only_public_query_and_dated_search_observations() {
    use application::{OutlookHorizon, OutlookMarket};
    let store = seeded_store();
    tauri::async_runtime::block_on(store.append_entry(
        &session(),
        &StoredChatEntry::new(
            ChatSequence::new(2),
            at(),
            turn(),
            StoredChatContent::User(StoredUserMessage::from_store_result(
                UserMessageId::try_new("private").expect("id"),
                public_text("가상 계좌번호 123456789 보유 수량 37주인데 시장은?"),
            )),
        ),
    ))
    .expect("저장");
    for (market, horizon, query) in [
        (OutlookMarket::Us, OutlookHorizon::TwoWeeks, US_FORTNIGHT),
        (OutlookMarket::Kr, OutlookHorizon::Month, KR_MONTH),
    ] {
        let provider = Arc::new(ScriptedAiProvider::default());
        let turns = ChatTurnBook::new();
        let _turn = turns.open(
            &run_id(),
            ChatTurnRef {
                session: session(),
                turn: turn(),
            },
        );
        let port = research_with(&provider, &store, turns);
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                search_event(query, "completed"),
                AiEvent::TextDelta(public_text("근거 없는 요약")),
            ])
            .with_observations(vec![observation(query, 2026)])));
        let result = tauri::async_runtime::block_on(port.request_research(
            &run_id(),
            &WebResearchToolRequest::MarketOutlook { market, horizon },
        ))
        .expect("조사");
        assert_eq!(
            result.summary.as_untrusted_str(),
            "public.example | 관측일 2026-09-20 | 대표 지수 종가 상승"
        );
        assert_eq!(result.collected_at, at());
        let calls = provider.calls();
        let [AiProviderCall::Run(spec)] = calls.as_slice() else {
            panic!("수집 실행 하나")
        };
        let RunInput::Collection(CollectionQuery::MarketOutlook(approved)) = spec.input() else {
            panic!("제한된 질의")
        };
        assert_eq!(approved.text().as_str(), query);
    }
}

fn stored_article(
    url: &str,
    published: application::PublicationTime,
) -> application::ObservedCollectedItem {
    use application::{CollectedEntry, CollectedItemHandle, ObservedCollectedItem};
    ObservedCollectedItem {
        handle: CollectedItemHandle::try_new("n-1").expect("기사"),
        entry: CollectedEntry::News(domain::NewsItem::new(
            domain::NewsSource::Hankyung,
            domain::NormalizedUrl::try_new(url).expect("주소"),
            public_text("저장된 뉴스 제목"),
            None,
        )),
        published_at: published,
        collected_at: at(),
        instruments: Vec::new(),
    }
}

#[test]
fn undated_market_link_uses_only_the_matching_stored_article_publication_time() {
    use application::{OutlookHorizon, OutlookMarket, PublicationTime};
    let undated = application::SearchObservation {
        source_date: None,
        ..observation(US_FORTNIGHT, 2026)
    };
    for (url, published, expected) in [
        (
            "https://public.example/report",
            PublicationTime::Known(at()),
            Some("public.example | 보도일 2026-09-21 | 저장된 뉴스 제목"),
        ),
        (
            "https://public.example/report",
            PublicationTime::NotProvided,
            None,
        ),
        (
            "https://public.example/other",
            PublicationTime::Known(at()),
            None,
        ),
    ] {
        let row = stored_article(url, published);
        let provider = Arc::new(ScriptedAiProvider::default());
        let store = Arc::new(Store {
            news: vec![row],
            ..Store::default()
        });
        let port = research(&provider, &store);
        provider
            .script_run()
            .push(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                search_event(US_FORTNIGHT, "completed"),
            ])
            .with_observations(vec![undated.clone()])));
        let result = tauri::async_runtime::block_on(port.request_research(
            &run_id(),
            &WebResearchToolRequest::MarketOutlook {
                market: OutlookMarket::Us,
                horizon: OutlookHorizon::TwoWeeks,
            },
        ));
        match expected {
            Some(summary) => assert_eq!(
                result.expect("발행 시각 확인").summary.as_untrusted_str(),
                summary
            ),
            None => assert_eq!(
                result.err(),
                Some(WebResearchToolFailure::SourceDateUnverified)
            ),
        }
    }
}
