use super::{ArticleBodies, StoredArticles, StoredNews, StoredThesis};
use crate::ai_runtime::RunMetadataBook;
use application::{
    AiRunId, ArticleFetchFailure, ArticleToolPort, BodyReadingSetting, CollectedEntry,
    CollectedItemHandle, CollectedItemQuery, CollectedItemReads, CollectedKind, LookbackDays,
    NewsDataSettings, NewsDataSettingsChange, NewsDataSettingsRepository, NewsKeyword,
    NewsToolReads, ObservedCollectedItem, PublicationTime, SystemPortError, ThesisToolPort,
    Watchlist, WatchlistEntry, WatchlistRepository, WatchlistStoreOutcome, WriteCompleted,
};
use domain::{
    AiBodyReadToggle, DartReceiptNo, Disclosure, DisclosureSource, EvidenceRef, InstrumentId,
    NewsItem, NewsSource, NormalizedUrl, ReceiptNo, Symbol, Timestamp, UntrustedText,
};
use std::future::{Future, ready};
use std::sync::Arc;

const NOW: i64 = 1_790_000_000;
const DAY: i64 = 86_400;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        domain::UnixSeconds::new(seconds),
        domain::Nanosecond::new(0_u32),
        domain::OffsetSeconds::new(0_i32),
    )
    .expect("시각")
}

fn handle(raw: &str) -> CollectedItemHandle {
    CollectedItemHandle::try_new(raw).expect("핸들")
}

fn url(raw: &str) -> NormalizedUrl {
    NormalizedUrl::try_new(raw).expect("주소")
}

fn text(raw: &str) -> UntrustedText {
    UntrustedText::try_new(raw).expect("텍스트")
}

fn article_address(id: &str) -> String {
    let mut address = String::from("https://www.hankyung.com/article/");
    address.push_str(id);
    address
}

fn news_item(id: &str, title: &str, collected: i64, instrument: &str) -> ObservedCollectedItem {
    ObservedCollectedItem {
        handle: handle(id),
        entry: CollectedEntry::News(NewsItem::new(
            NewsSource::Hankyung,
            url(&article_address(id)),
            text(title),
            Some(text("본문 요약")),
        )),
        published_at: PublicationTime::NotProvided,
        collected_at: at(collected),
        instruments: vec![InstrumentId::try_new(instrument).expect("종목")],
    }
}

fn filing() -> ObservedCollectedItem {
    ObservedCollectedItem {
        handle: handle("d-9"),
        entry: CollectedEntry::Disclosure(Disclosure::new(
            ReceiptNo::Dart(DartReceiptNo::try_new("20261125000001").expect("접수")),
            url("https://dart.fss.or.kr/dsaf001/main.do?rcpNo=20261125000001"),
            text("주요사항보고서"),
            None,
        )),
        published_at: PublicationTime::Known(at(NOW - 10)),
        collected_at: at(NOW - 5),
        instruments: Vec::new(),
    }
}

struct Store {
    items: Vec<ObservedCollectedItem>,
    settings: Option<NewsDataSettings>,
    watchlist: Watchlist,
}

impl Store {
    fn new() -> Self {
        Self {
            items: vec![
                news_item("n-1", "반도체 수출 급증", NOW - 60, "005930"),
                news_item("n-2", "환율 하락", NOW - 3 * DAY, "000660"),
                filing(),
            ],
            settings: None,
            watchlist: Watchlist::try_from_entries(vec![WatchlistEntry::new(
                InstrumentId::try_new("005930").expect("종목"),
                at(NOW),
            )])
            .expect("관심 종목"),
        }
    }
}

const fn kind_of(item: &ObservedCollectedItem) -> CollectedKind {
    match item.entry {
        CollectedEntry::News(_) => CollectedKind::News,
        CollectedEntry::Disclosure(_) => CollectedKind::Disclosure,
    }
}

impl CollectedItemReads for Store {
    fn collected_since(
        &self,
        kind: CollectedKind,
        since: Timestamp,
    ) -> impl Future<Output = Result<Vec<ObservedCollectedItem>, SystemPortError>> + Send {
        ready(Ok(self
            .items
            .iter()
            .filter(|item| kind_of(item) == kind && item.collected_at >= since)
            .cloned()
            .collect()))
    }

    fn collected_item(
        &self,
        handle: &CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<ObservedCollectedItem>, SystemPortError>> + Send {
        ready(Ok(self
            .items
            .iter()
            .find(|item| &item.handle == handle)
            .cloned()))
    }
}

impl NewsDataSettingsRepository for Store {
    fn load(
        &self,
    ) -> impl Future<Output = Result<Option<NewsDataSettings>, SystemPortError>> + Send {
        ready(Ok(self.settings.clone()))
    }

    fn store(
        &self,
        _settings: &NewsDataSettings,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        ready(Ok(WriteCompleted))
    }
}

impl WatchlistRepository for Store {
    fn load(&self) -> impl Future<Output = Result<Watchlist, SystemPortError>> + Send {
        ready(Ok(self.watchlist.clone()))
    }

    fn store(
        &self,
        _previous: &Watchlist,
        _next: &Watchlist,
    ) -> impl Future<Output = Result<WatchlistStoreOutcome, SystemPortError>> + Send {
        ready(Ok(WatchlistStoreOutcome::Stored))
    }
}

struct EchoBodies;

impl ArticleBodies for EchoBodies {
    fn fetch(
        &self,
        source: &NewsSource,
        url: &NormalizedUrl,
        settings: &NewsDataSettings,
    ) -> impl Future<Output = Result<UntrustedText, ArticleFetchFailure>> + Send {
        let allowed =
            settings.body_read().policy_for(source) == domain::AiBodyReadPolicy::BodyAllowed;
        ready(if allowed && *source == NewsSource::Hankyung {
            Ok(text(url.as_str()))
        } else {
            Err(ArticleFetchFailure::NotFound)
        })
    }
}

fn query(days: u16, symbol: Option<&str>, keyword: Option<&str>) -> CollectedItemQuery {
    CollectedItemQuery {
        symbol: symbol.map(|raw| Symbol::try_new(raw).expect("종목")),
        keyword: keyword.map(|raw| NewsKeyword::try_new(raw).expect("검색어")),
        lookback: LookbackDays::try_new(days).expect("기간"),
    }
}

fn clock() -> Arc<testkit_ports::SteppedClock> {
    Arc::new(testkit_ports::SteppedClock::new(at(NOW)))
}

fn news_tools() -> StoredNews<Store, testkit_ports::SteppedClock> {
    StoredNews::new(
        Arc::new(Store::new()),
        Arc::new(testkit_ports::SteppedClock::new(at(NOW))),
    )
}

fn search(query: &CollectedItemQuery) -> Vec<application::NewsToolLine> {
    tauri::async_runtime::block_on(news_tools().search_news(query)).expect("조회")
}

fn handles(query: &CollectedItemQuery) -> Vec<String> {
    search(query)
        .into_iter()
        .map(|line| line.handle.as_str().to_owned())
        .collect()
}

#[test]
fn news_search_bounds_by_lookback_and_filters_symbol_and_keyword() {
    assert_eq!(handles(&query(1, None, None)), ["n-1"]);
    assert_eq!(handles(&query(7, None, None)), ["n-1", "n-2"]);
    assert_eq!(handles(&query(7, Some("000660"), None)), ["n-2"]);
    assert_eq!(handles(&query(7, None, Some("반도체"))), ["n-1"]);
    let line = search(&query(1, None, None))
        .into_iter()
        .next()
        .expect("뉴스");
    assert_eq!(line.collected_at, at(NOW - 60));
    assert_eq!(
        line.published_at,
        at(NOW - 60),
        "게시 시각이 없으면 수집 시각"
    );
    assert_eq!(line.symbols, vec![Symbol::try_new("005930").expect("종목")]);
    assert_eq!(line.summary, Some(text("본문 요약")));
}

#[test]
fn disclosure_tool_lists_filings_with_their_filed_time() {
    let lines = tauri::async_runtime::block_on(news_tools().disclosures(&query(1, None, None)))
        .expect("공시");
    let line = lines.first().expect("공시");
    assert_eq!(line.handle, handle("d-9"));
    assert_eq!(line.source, DisclosureSource::Dart);
    assert_eq!(line.filed_at, at(NOW - 10));
    assert_eq!(line.collected_at, at(NOW - 5));
}

#[test]
fn article_tool_locates_press_items_and_reads_only_when_body_reading_is_on() {
    let mut store = Store::new();
    let closed = StoredArticles::new(Arc::new(Store::new()), Arc::new(EchoBodies), clock());
    let located = tauri::async_runtime::block_on(closed.locate(&handle("n-1")))
        .expect("위치")
        .expect("기사");
    assert_eq!(located.url, url("https://www.hankyung.com/article/n-1"));
    assert_eq!(
        tauri::async_runtime::block_on(closed.locate(&handle("d-9"))),
        Ok(None),
        "공시는 기사 도구 대상 아님"
    );
    assert_eq!(
        tauri::async_runtime::block_on(closed.locate(&handle("n-7"))),
        Ok(None)
    );
    assert_eq!(
        tauri::async_runtime::block_on(closed.body_reading(&NewsSource::Hankyung)),
        Ok(BodyReadingSetting::Disabled),
        "설정이 없으면 꺼짐"
    );
    let mut settings = NewsDataSettings::initial();
    settings
        .apply(NewsDataSettingsChange::BodyRead(
            NewsSource::Hankyung,
            AiBodyReadToggle::Enabled,
        ))
        .expect("켜기");
    store.settings = Some(settings);
    let open = StoredArticles::new(Arc::new(store), Arc::new(EchoBodies), clock());
    assert_eq!(
        tauri::async_runtime::block_on(open.body_reading(&NewsSource::Hankyung)),
        Ok(BodyReadingSetting::Enabled)
    );
    let body = tauri::async_runtime::block_on(open.fetch_body(&located.url)).expect("본문");
    assert_eq!(body.fetched_at, at(NOW));
    assert_eq!(body.text, text("https://www.hankyung.com/article/n-1"));
    assert_eq!(
        tauri::async_runtime::block_on(
            open.fetch_body(&url("https://www.hankyung.com/article/unknown"))
        ),
        Err(ArticleFetchFailure::NotFound),
        "수집 목록에 없는 주소는 거부"
    );
}

#[test]
fn thesis_tool_resolves_watchlist_targets_collected_evidence_and_run_metadata() {
    let runs = RunMetadataBook::new();
    let thesis = StoredThesis::new(Arc::new(Store::new()), runs);
    let run = AiRunId::try_new("chat-run-1").expect("실행");
    let instrument = InstrumentId::try_new("005930").expect("종목");
    assert_eq!(
        tauri::async_runtime::block_on(
            thesis.resolve_target(&run, &Symbol::try_new("005930").expect("종목"))
        ),
        Ok(Some(instrument.clone()))
    );
    assert_eq!(
        tauri::async_runtime::block_on(
            thesis.resolve_target(&run, &Symbol::try_new("000660").expect("종목"))
        ),
        Ok(None),
        "관심·보유가 아닌 종목은 대상이 아님"
    );
    match tauri::async_runtime::block_on(thesis.collected_evidence(
        &run,
        &instrument,
        &handle("n-1"),
    )) {
        Ok(Some(EvidenceRef::News(NewsSource::Hankyung, _))) => {}
        other => panic!("뉴스 근거: {other:?}"),
    }
    match tauri::async_runtime::block_on(thesis.collected_evidence(
        &run,
        &instrument,
        &handle("d-9"),
    )) {
        Ok(Some(EvidenceRef::Disclosure(_, _))) => {}
        other => panic!("공시 근거: {other:?}"),
    }
    assert_eq!(
        tauri::async_runtime::block_on(thesis.collected_evidence(
            &run,
            &instrument,
            &handle("n-8")
        )),
        Ok(None)
    );
    assert!(
        tauri::async_runtime::block_on(thesis.run_metadata(&run)).is_err(),
        "등록되지 않은 실행은 메타데이터가 없다"
    );
}
