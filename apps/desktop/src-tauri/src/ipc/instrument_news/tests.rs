use super::{RECENT_NEWS_LIMIT, recent_news};
use crate::ipc::event::UiNewsSource;
use application::{
    CollectedEntry, CollectedItemHandle, CollectedItemReads, CollectedKind, ObservedCollectedItem,
    PublicationTime, SystemPortError,
};
use domain::{
    InstrumentId, Nanosecond, NewsItem, NewsSource, NormalizedUrl, OffsetSeconds, Symbol,
    Timestamp, UnixSeconds, UntrustedText,
};
use std::future::{Future, ready};

const NOW: i64 = 1_800_000_000;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn item(
    title: &str,
    source: NewsSource,
    url: &str,
    published: i64,
    instruments: &[&str],
) -> ObservedCollectedItem {
    ObservedCollectedItem {
        handle: CollectedItemHandle::try_new("n-1").expect("핸들"),
        entry: CollectedEntry::News(NewsItem::new(
            source,
            NormalizedUrl::try_new(url).expect("주소"),
            UntrustedText::try_new(title).expect("제목"),
            None,
        )),
        published_at: PublicationTime::Known(at(published)),
        collected_at: at(published),
        instruments: instruments
            .iter()
            .map(|symbol| InstrumentId::try_new(symbol).expect("종목"))
            .collect(),
    }
}

struct Stored(Vec<ObservedCollectedItem>);

impl CollectedItemReads for Stored {
    fn collected_since(
        &self,
        kind: CollectedKind,
        since: Timestamp,
    ) -> impl Future<Output = Result<Vec<ObservedCollectedItem>, SystemPortError>> + Send {
        assert_eq!(kind, CollectedKind::News, "뉴스만 읽는다");
        assert_eq!(since, at(NOW - 7 * 86_400), "7일 창");
        ready(Ok(self.0.clone()))
    }

    fn collected_item(
        &self,
        _handle: &CollectedItemHandle,
    ) -> impl Future<Output = Result<Option<ObservedCollectedItem>, SystemPortError>> + Send {
        ready(Ok(None))
    }
}

fn run(store: &Stored, symbol: &str) -> Vec<super::UiInstrumentNews> {
    tauri::async_runtime::block_on(recent_news(
        store,
        &Symbol::try_new(symbol).expect("종목"),
        at(NOW),
    ))
}

#[test]
fn only_linked_headlines_come_back_newest_first_with_same_story_relays_folded() {
    let store = Stored(vec![
        item(
            "삼성전자 3분기 영업이익 10조 돌파",
            NewsSource::Yonhap,
            "https://www.yna.co.kr/view/A1",
            NOW - 7_200,
            &["005930"],
        ),
        item(
            "삼성전자 3분기 영업이익 10조 돌파 - 매일경제",
            NewsSource::GoogleNews,
            "https://news.google.com/rss/articles/B1",
            NOW - 3_600,
            &["005930"],
        ),
        item(
            "환율 1,400원 돌파",
            NewsSource::Hankyung,
            "https://www.hankyung.com/a/1",
            NOW - 60,
            &[],
        ),
        item(
            "SK하이닉스 HBM 공급",
            NewsSource::Edaily,
            "https://www.edaily.co.kr/a/1",
            NOW - 120,
            &["000660"],
        ),
        item(
            "삼성전자 노조 협상 타결",
            NewsSource::Maekyung,
            "https://www.mk.co.kr/a/1",
            NOW - 10_800,
            &["005930", "000660"],
        ),
    ]);
    let lines = run(&store, "005930");
    assert_eq!(lines.len(), 2);
    assert_eq!(
        lines[0].title,
        "삼성전자 3분기 영업이익 10조 돌파 - 매일경제"
    );
    assert_eq!(lines[0].source, UiNewsSource::GoogleNews);
    assert_eq!(lines[1].title, "삼성전자 노조 협상 타결");
    assert_eq!(lines[1].url, "https://www.mk.co.kr/a/1");
    assert!(
        lines[1].published_at.starts_with("2027-01-15T"),
        "{}",
        lines[1].published_at
    );
}

#[test]
fn the_list_is_capped_so_the_card_stays_short() {
    let store = Stored(
        (0_i64..20)
            .map(|index| {
                item(
                    &format!(
                        "기사 {index} 번째 서로 다른 제목 {}",
                        index.saturating_mul(7919)
                    ),
                    NewsSource::Yonhap,
                    &format!("https://www.yna.co.kr/view/{index}"),
                    NOW.saturating_sub(index.saturating_mul(60)),
                    &["005930"],
                )
            })
            .collect(),
    );
    assert_eq!(run(&store, "005930").len(), RECENT_NEWS_LIMIT);
    assert_eq!(run(&store, "AAPL"), Vec::new(), "다른 종목 기사는 없다");
}
