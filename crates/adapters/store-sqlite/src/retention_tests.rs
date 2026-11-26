use crate::{
    Database, DatabasePath, InformationStore,
    tests::{Fixture, clock, key, runtime},
};
use application::{
    CollectedEntry, CollectedItemReads, CollectedKind, CollectedRetention, CollectedTimes,
    DisclosureRepository, InformationKey, InformationLinks, NewsRepository, PublicationTime,
    news_retention_cutoffs,
};
use domain::{
    Disclosure, InstrumentId, KindReceiptNo, Nanosecond, NewsItem, NewsSource, NormalizedUrl,
    OffsetSeconds, ReceiptNo, Timestamp, UnixSeconds, UntrustedText,
};

const DAY: i64 = 86_400;
const NOW: i64 = 1_800_000_000;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn news(source: NewsSource, url: &str) -> NewsItem {
    NewsItem::new(
        source,
        NormalizedUrl::try_new(url).expect("주소"),
        UntrustedText::try_new("삼성전자 실적").expect("제목"),
        None,
    )
}

fn news_source(item: &NewsItem) -> NewsSource {
    match item.collected().source() {
        domain::InformationSource::News(source) => source.clone(),
        domain::InformationSource::Web | domain::InformationSource::Disclosure(_) => {
            NewsSource::Yonhap
        }
    }
}

#[test]
fn old_news_is_removed_per_source_and_disclosures_are_kept() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    let items = [
        (
            news(NewsSource::GoogleNews, "https://news.google.com/a/old"),
            NOW - 8 * DAY,
        ),
        (
            news(NewsSource::GoogleNews, "https://news.google.com/a/new"),
            NOW - 6 * DAY,
        ),
        (
            news(NewsSource::Yonhap, "https://www.yna.co.kr/view/mid"),
            NOW - 30 * DAY,
        ),
        (
            news(NewsSource::Yonhap, "https://www.yna.co.kr/view/old"),
            NOW - 91 * DAY,
        ),
    ];
    for (source, item, collected) in items
        .iter()
        .map(|(item, collected)| (news_source(item), item, collected))
    {
        rt.block_on(store.store(item)).expect("저장");
        let entry = CollectedEntry::News(item.clone());
        rt.block_on(store.record_times(&entry, PublicationTime::NotProvided, at(*collected)))
            .expect("시각");
        let key = InformationKey::News(source, item.collected().url().clone());
        rt.block_on(store.set_instruments(&key, &[InstrumentId::try_new("005930").expect("종목")]))
            .expect("연결");
    }
    store_old_filing(&rt, &store);
    assert_pruned(&rt, &store);
}

fn store_old_filing(rt: &tokio::runtime::Runtime, store: &InformationStore<'_>) {
    let filing = Disclosure::new(
        ReceiptNo::Kind(KindReceiptNo::try_new("20250101000001").expect("번호")),
        NormalizedUrl::try_new("https://kind.krx.co.kr/a").expect("주소"),
        UntrustedText::try_new("오래된 공시").expect("제목"),
        None,
    );
    rt.block_on(store.store_disclosure(&filing)).expect("공시");
    rt.block_on(store.record_times(
        &CollectedEntry::Disclosure(filing),
        PublicationTime::NotProvided,
        at(NOW - 400 * DAY),
    ))
    .expect("공시 시각");
}

fn assert_pruned(rt: &tokio::runtime::Runtime, store: &InformationStore<'_>) {
    let removed = rt
        .block_on(store.prune_news(&news_retention_cutoffs(at(NOW))))
        .expect("정리");
    assert_eq!(*removed.as_u32(), 2);
    let left: Vec<String> = rt
        .block_on(store.collected_since(CollectedKind::News, at(0)))
        .expect("읽기")
        .into_iter()
        .filter_map(|item| match item.entry {
            CollectedEntry::News(news) => Some(news.collected().url().as_str().to_owned()),
            CollectedEntry::Disclosure(_) => None,
        })
        .collect();
    assert_eq!(left.len(), 2, "{left:?}");
    assert!(left.iter().all(|url| !url.ends_with("/old")), "{left:?}");
    assert_eq!(
        rt.block_on(store.collected_since(CollectedKind::Disclosure, at(0)))
            .expect("공시 읽기")
            .len(),
        1,
        "공시는 지우지 않는다"
    );
    assert_eq!(
        *rt.block_on(store.prune_news(&news_retention_cutoffs(at(NOW))))
            .expect("다시")
            .as_u32(),
        0
    );
}
