use super::{Archive, Company, TitleLinker};
use application::{
    CollectedEntry, CollectedItemHandle, CollectedObservation, CollectedPresence,
    CollectedPresenceCheck, CollectedTimes, DataAvailability, FeedCursor, InformationError,
    NewsDataSettings, NewsRepository, ObservedCollectedItem, PublicationTime, StockAlias,
    SystemPortError, TrackedInstrument, WriteCompleted,
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
struct Store {
    links: Mutex<Vec<NormalizedUrl>>,
    observations: Mutex<Vec<(PublicationTime, Timestamp)>>,
}
impl NewsRepository for Store {
    async fn store(&self, item: &NewsItem) -> Result<WriteCompleted, SystemPortError> {
        self.links.lock().await.push(item.collected().url().clone());
        Ok(WriteCompleted)
    }
    fn for_instrument(
        &self,
        _instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<NewsItem>, SystemPortError>> + Send {
        std::future::ready(Ok(Vec::new()))
    }
}
impl CollectedPresenceCheck for Store {
    async fn presence(&self, url: &NormalizedUrl) -> Result<CollectedPresence, SystemPortError> {
        Ok(if self.links.lock().await.contains(url) {
            CollectedPresence::Stored
        } else {
            CollectedPresence::New
        })
    }
}
impl CollectedTimes for Store {
    async fn record_times(
        &self,
        _entry: &CollectedEntry,
        published_at: PublicationTime,
        collected_at: Timestamp,
    ) -> Result<WriteCompleted, SystemPortError> {
        self.observations
            .lock()
            .await
            .push((published_at, collected_at));
        Ok(WriteCompleted)
    }
}
struct Feed(Vec<CollectedObservation>);
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
        std::future::ready(Some(Ok(DataAvailability::Available((
            self.0.clone(),
            FeedCursor::Start,
        )))))
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
#[test]
fn live_company_search_rejects_unrelated_future_or_undated_hits_and_registers_the_source() {
    let day = 1_800_000_000;
    let published = PublicationTime::Known(moment(day - 80));
    let observed = moment(day - 30);
    let feed = Feed(vec![
        CollectedObservation::new(article("peer", "Peer guidance"), published, observed),
        CollectedObservation::new(
            article("future", "Example forecast"),
            PublicationTime::Known(moment(day + 1)),
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
    ]);
    let archive = Archive {
        items: Vec::new(),
        settings: Some(NewsDataSettings::initial()),
        failed: false,
    };
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
        let (evidence_rows, failed) = archive
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
        assert_eq!(
            store.observations.lock().await.as_slice(),
            &[(published, observed)]
        );
        assert_eq!(store.links.lock().await.len(), 1);
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
