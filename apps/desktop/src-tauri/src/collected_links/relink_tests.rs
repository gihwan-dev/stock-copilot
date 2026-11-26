use super::{TitleLinker, relink_recent};
use crate::news_tracking::{MatchName, MatchNameClarity, MatchNameRole};
use application::{
    CollectedEntry, CollectedItemHandle, CollectedItemReads, CollectedKind, InformationKey,
    InformationLinks, ObservedCollectedItem, PublicationTime, StockAlias, SystemPortError,
    WriteCompleted,
};
use domain::{
    InstrumentId, Nanosecond, NewsItem, NewsSource, NormalizedUrl, OffsetSeconds, Timestamp,
    UnixSeconds, UntrustedText,
};
use std::future::{Future, ready};
use std::sync::RwLock;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn id(symbol: &str) -> InstrumentId {
    InstrumentId::try_new(symbol).expect("종목")
}

fn item(title: &str, url: &str, linked: &[&str]) -> ObservedCollectedItem {
    ObservedCollectedItem {
        handle: CollectedItemHandle::try_new("n-1").expect("핸들"),
        entry: CollectedEntry::News(NewsItem::new(
            NewsSource::Yonhap,
            NormalizedUrl::try_new(url).expect("주소"),
            UntrustedText::try_new(title).expect("제목"),
            None,
        )),
        published_at: PublicationTime::NotProvided,
        collected_at: at(100),
        instruments: linked.iter().map(|symbol| id(symbol)).collect(),
    }
}

#[derive(Default)]
struct Store {
    links: RwLock<Vec<(InformationKey, Vec<InstrumentId>)>>,
}

impl CollectedItemReads for Store {
    fn collected_since(
        &self,
        kind: CollectedKind,
        _since: Timestamp,
    ) -> impl Future<Output = Result<Vec<ObservedCollectedItem>, SystemPortError>> + Send {
        let items = match kind {
            CollectedKind::News => vec![
                item(
                    "삼성전자 신공장",
                    "https://www.yna.co.kr/view/1",
                    &["000660"],
                ),
                item("환율 하락", "https://www.yna.co.kr/view/2", &[]),
            ],
            CollectedKind::Disclosure => Vec::new(),
        };
        ready(Ok(items))
    }

    fn collected_item(
        &self,
        _handle: &CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<ObservedCollectedItem>, SystemPortError>> + Send {
        ready(Ok(None))
    }
}

impl InformationLinks for Store {
    fn set_instruments(
        &self,
        item: &InformationKey,
        instruments: &[InstrumentId],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        if let Ok(mut links) = self.links.write() {
            links.push((item.clone(), instruments.to_vec()));
        }
        ready(Ok(WriteCompleted))
    }
}

struct Contains;

impl TitleLinker for Contains {
    fn instruments_in(&self, _title: &UntrustedText, _aliases: &[StockAlias]) -> Vec<InstrumentId> {
        Vec::new()
    }

    fn instruments_named(&self, title: &UntrustedText, names: &[MatchName]) -> Vec<InstrumentId> {
        names
            .iter()
            .filter(|name| title.as_untrusted_str().contains(name.name.as_str()))
            .map(|name| name.instrument.clone())
            .collect()
    }
}

#[test]
fn a_newly_tracked_name_is_added_to_recent_articles_without_dropping_old_links() {
    let store = Store::default();
    let names = vec![MatchName {
        instrument: id("005930"),
        name: String::from("삼성전자"),
        role: MatchNameRole::Official,
        clarity: MatchNameClarity::Distinct,
    }];
    let changed = tauri::async_runtime::block_on(relink_recent(&store, &Contains, &names, at(0)))
        .expect("다시 잇기");
    assert_eq!(changed, 1);
    let links = store.links.read().expect("연결").clone();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].1, vec![id("000660"), id("005930")]);
    assert_eq!(
        tauri::async_runtime::block_on(relink_recent(&store, &Contains, &[], at(0))),
        Ok(0)
    );
}
