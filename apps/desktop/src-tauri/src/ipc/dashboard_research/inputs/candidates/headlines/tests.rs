use super::{Archive, Company, TitleLinker};
use application::{
    CandidateNewsRepository, CollectedEntry, CollectedItemHandle, CollectedObservation,
    DataAvailability, FeedCursor, InformationError, NewsDataSettings, ObservedCollectedItem,
    PublicationTime, StockAlias, SystemPortError, TrackedInstrument,
};
use domain::{
    DisplayInstrumentName, InstrumentId, KnownMarket, Nanosecond, NewsItem, NewsSource,
    NormalizedUrl, OffsetSeconds, Symbol, Timestamp, UnixSeconds, UntrustedText,
};
use sources::TargetedHeadlines;
use testkit_ports::SteppedClock;
use tokio::sync::Mutex;

fn moment(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}
fn id(value: &str) -> InstrumentId {
    InstrumentId::try_new(value).expect("종목 ID")
}
fn article(path: &str, title: &str) -> CollectedEntry {
    CollectedEntry::News(NewsItem::new(
        NewsSource::YahooFinance,
        NormalizedUrl::try_new(&format!("https://finance.yahoo.com/news/{path}")).expect("주소"),
        UntrustedText::try_new(title).expect("제목"),
        None,
    ))
}
struct Matcher;
impl TitleLinker for Matcher {
    fn instruments_in(&self, _title: &UntrustedText, _aliases: &[StockAlias]) -> Vec<InstrumentId> {
        Vec::new()
    }
    fn instruments_named(
        &self,
        title: &UntrustedText,
        _names: &[crate::news_tracking::MatchName],
    ) -> Vec<InstrumentId> {
        if title.as_untrusted_str().contains("Example") {
            vec![id("NEW")]
        } else {
            Vec::new()
        }
    }
}
#[derive(Default)]
struct Store(Mutex<Vec<ObservedCollectedItem>>);
impl CandidateNewsRepository for Store {
    async fn retain_candidate_news(
        &self,
        item: &NewsItem,
        instrument: &InstrumentId,
        published_at: Timestamp,
        collected_at: Timestamp,
    ) -> Result<ObservedCollectedItem, SystemPortError> {
        let record = ObservedCollectedItem {
            handle: CollectedItemHandle::try_new("n-1").expect("핸들"),
            entry: CollectedEntry::News(item.clone()),
            published_at: PublicationTime::Known(published_at),
            collected_at,
            instruments: vec![instrument.clone()],
        };
        self.0.lock().await.push(record.clone());
        Ok(record)
    }
}
struct Feed(Result<Vec<CollectedObservation>, InformationError>);
type Reply =
    Option<Result<DataAvailability<(Vec<CollectedObservation>, FeedCursor)>, InformationError>>;
impl TargetedHeadlines for Feed {
    fn target_headlines(
        &self,
        source: NewsSource,
        target: &TrackedInstrument,
    ) -> impl Future<Output = Reply> + Send {
        assert_eq!(source, NewsSource::YahooFinance);
        assert_eq!(target.symbol.as_str(), "NEW");
        std::future::ready(Some(
            self.0
                .clone()
                .map(|items| DataAvailability::Available((items, FeedCursor::Start))),
        ))
    }
}
fn target() -> TrackedInstrument {
    TrackedInstrument {
        symbol: Symbol::try_new("NEW").expect("티커"),
        market: KnownMarket::Us,
        name: DisplayInstrumentName::try_new("Example Corp").expect("이름"),
        aliases: Vec::new(),
    }
}
fn archive(items: Vec<ObservedCollectedItem>) -> Archive {
    Archive {
        items,
        settings: Some(NewsDataSettings::initial()),
        failed: false,
    }
}
fn live_feed(published: PublicationTime, observed: Timestamp, day: i64) -> Feed {
    Feed(Ok(vec![
        CollectedObservation::new(article("peer", "Peer guidance"), published, observed),
        CollectedObservation::new(
            article("future", "Example forecast"),
            PublicationTime::Known(moment(day.saturating_add(1))),
            observed,
        ),
        CollectedObservation::new(
            article("undated", "Example report"),
            PublicationTime::NotProvided,
            observed,
        ),
        CollectedObservation::new(
            article("valid", "Example factory opens"),
            published,
            observed,
        ),
    ]))
}
#[test]
fn live_company_search_rejects_unrelated_future_or_undated_hits_and_registers_the_source() {
    let day = 1_800_000_000;
    let published = PublicationTime::Known(moment(day - 80));
    let observed = moment(day - 30);
    let feed = live_feed(published, observed, day);
    let empty = archive(Vec::new());
    let target = target();
    let canonical = id("US-master-117");
    let company = Company {
        target: &target,
        canonical: Some(&canonical),
        linker: Some(&Matcher),
        names: &[],
    };
    let store = Store::default();
    tauri::async_runtime::block_on(async {
        let (evidence_rows, failed) = empty
            .for_candidate(
                &company,
                Some(&store),
                Some(&feed),
                &SteppedClock::new(moment(day)),
            )
            .await;
        assert!(!failed);
        assert_eq!(evidence_rows.len(), 1);
        let source = evidence_rows.first().expect("실제 확인한 기사");
        assert_eq!(
            source.url.as_ref().expect("원문").as_str(),
            "https://finance.yahoo.com/news/valid"
        );
        assert_eq!(source.observed_at, observed);
        let persisted = store.0.lock().await.clone();
        let row = persisted.first().expect("저장된 기사");
        assert_eq!(row.published_at, published);
        assert_eq!(row.collected_at, observed);
        assert_eq!(row.instruments, std::slice::from_ref(&canonical));
        let offline = archive(persisted);
        let (cached, unavailable) = offline
            .for_candidate(
                &company,
                Some(&store),
                Some(&Feed(Err(InformationError::Unavailable(
                    application::ResetTime::NotProvided,
                )))),
                &SteppedClock::new(moment(day + 21_601)),
            )
            .await;
        assert!(!unavailable);
        assert_eq!(cached, evidence_rows);
    });
}

#[test]
fn exact_master_id_accepts_disjoint_ticker_and_rejects_unlinked_or_ambiguous_ticker() {
    let target = target();
    let canonical = id("US-master-117");
    let company = Company {
        target: &target,
        canonical: Some(&canonical),
        linker: Some(&Matcher),
        names: &[],
    };
    let item = |key: &str, title: &str| ObservedCollectedItem {
        handle: CollectedItemHandle::try_new("n-1").expect("핸들"),
        entry: article("issuer", title),
        published_at: PublicationTime::Known(moment(10)),
        collected_at: moment(20),
        instruments: vec![id(key)],
    };
    assert!(company.linked(&item("US-master-117", "quarterly update")));
    assert!(company.linked(&item("NEW", "Example report")));
    assert!(!company.linked(&item("NEW", "Peer announcement")));
    assert!(!company.linked(&item("different-id", "Example report")));
}
