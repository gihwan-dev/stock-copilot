#[cfg(test)]
mod tests {
    use std::future::{Future, ready};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use application::{
        Clock, CollectedEntry, CollectedObservation, DataAvailability, FeedCursor, FeedSource,
        NewsFeed, PublicationTime, TrackedInstrument,
    };
    use domain::{
        DisplayInstrumentName, InstrumentId, KnownMarket, Nanosecond, NewsSource, NormalizedUrl,
        OffsetSeconds, Symbol, Timestamp, UnixSeconds,
    };
    use sources::{
        NewsLinkHints, SourceEndpoint, SourceHttpClient, SourceWait, TrackedNewsFeed,
        TrackedSearch, TrackedTargets, is_recent,
    };
    use testkit::serve_sequence;

    const BASE_SECONDS: i64 = 1_800_000_000;
    const FRESH: &str = "Fri, 15 Jan 2027 07:00:00 GMT";
    const STALE: &str = "Mon, 04 Jan 2027 07:00:00 GMT";

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn tracked(symbol: &str, market: KnownMarket, name: &str) -> TrackedInstrument {
        TrackedInstrument {
            symbol: Symbol::try_new(symbol).expect("종목"),
            market,
            name: DisplayInstrumentName::try_new(name).expect("이름"),
        }
    }

    fn holdings() -> Vec<TrackedInstrument> {
        vec![
            tracked("005930", KnownMarket::Kr, "삼성전자"),
            tracked("000660", KnownMarket::Kr, "SK하이닉스"),
            tracked("AAPL", KnownMarket::Us, "애플"),
        ]
    }

    struct AutoClock(AtomicU64);

    impl Clock for AutoClock {
        fn now(&self) -> Timestamp {
            let elapsed = self
                .0
                .load(Ordering::SeqCst)
                .checked_div(1_000_000_000)
                .expect("초");
            at(BASE_SECONDS.saturating_add(i64::try_from(elapsed).expect("초")))
        }
    }

    impl SourceWait for AutoClock {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            let delta = u64::try_from(duration.as_nanos()).expect("나노초");
            self.0.fetch_add(delta, Ordering::SeqCst);
            Box::pin(ready(()))
        }
    }

    fn client(port: u16) -> Arc<SourceHttpClient> {
        let clock = Arc::new(AutoClock(AtomicU64::new(0)));
        let shared: Arc<dyn Clock> = Arc::<AutoClock>::clone(&clock);
        Arc::new(
            SourceHttpClient::new(
                SourceEndpoint::local(port).expect("주소"),
                shared,
                clock,
                None,
            )
            .expect("클라이언트"),
        )
    }

    struct FixedTargets(Vec<TrackedInstrument>);

    impl TrackedTargets for FixedTargets {
        fn tracked(&self) -> Pin<Box<dyn Future<Output = Vec<TrackedInstrument>> + Send + '_>> {
            Box::pin(ready(self.0.clone()))
        }
    }

    fn targets(list: Vec<TrackedInstrument>) -> Arc<dyn TrackedTargets> {
        Arc::new(FixedTargets(list))
    }

    fn xml(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/xml; charset=utf-8\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn failure() -> Vec<u8> {
        b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .to_vec()
    }

    fn rss(items: &[(&str, &str, &str)]) -> String {
        let mut body = String::new();
        for (title, link, date) in items {
            body.push_str("<item><title>");
            body.push_str(title);
            body.push_str("</title><link>");
            body.push_str(link);
            body.push_str("</link><pubDate>");
            body.push_str(date);
            body.push_str("</pubDate></item>");
        }
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><rss version=\"2.0\"><channel><title>t</title>{body}</channel></rss>"
        )
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("런타임")
    }

    fn titles(items: &[CollectedObservation]) -> Vec<String> {
        items
            .iter()
            .filter_map(|item| match item.entry() {
                CollectedEntry::News(news) => {
                    Some(news.collected().title().as_untrusted_str().to_owned())
                }
                CollectedEntry::Disclosure(_) => None,
            })
            .collect()
    }

    #[test]
    fn recency_keeps_three_days_and_undated_or_future_items() {
        let now = at(BASE_SECONDS);
        assert!(is_recent(
            PublicationTime::Known(at(BASE_SECONDS - 3_600)),
            now
        ));
        assert!(is_recent(
            PublicationTime::Known(at(BASE_SECONDS - 3 * 86_400)),
            now
        ));
        assert!(!is_recent(
            PublicationTime::Known(at(BASE_SECONDS - 3 * 86_400 - 1)),
            now
        ));
        assert!(is_recent(PublicationTime::NotProvided, now));
        assert!(is_recent(
            PublicationTime::Known(at(BASE_SECONDS + 60)),
            now
        ));
    }

    #[test]
    fn google_search_runs_once_per_tracked_name_drops_stale_items_and_survives_one_failure() {
        let first = rss(&[
            (
                "삼성전자 HBM 공급 - 한국경제",
                "https://news.google.com/rss/articles/A1",
                FRESH,
            ),
            (
                "삼성전자 옛 기사 - 한국경제",
                "https://news.google.com/rss/articles/A0",
                STALE,
            ),
        ]);
        let third = rss(&[(
            "애플 주가 급등 - 연합뉴스",
            "https://news.google.com/rss/articles/C1",
            FRESH,
        )]);
        let server = serve_sequence(vec![xml(&first), failure(), xml(&third)]).expect("서버");
        let feed = TrackedNewsFeed::new(
            client(server.port()),
            TrackedSearch::GoogleNews,
            targets(holdings()),
            NewsLinkHints::default(),
        );
        assert_eq!(feed.source(), FeedSource::News(NewsSource::GoogleNews));
        let found = runtime()
            .block_on(feed.new_items(FeedCursor::Start))
            .expect("수집");
        let DataAvailability::Available((items, _cursor)) = found else {
            panic!("기사가 있어야 한다: {found:?}");
        };
        assert_eq!(
            titles(&items),
            vec![
                String::from("삼성전자 HBM 공급 - 한국경제"),
                String::from("애플 주가 급등 - 연합뉴스"),
            ]
        );
        let requests = server.finish().expect("요청");
        assert_eq!(requests.len(), 3);
        let head = String::from_utf8_lossy(&requests[0]).into_owned();
        assert!(head.contains("/rss/search?q=%EC%82%BC%EC%84%B1"), "{head}");
    }

    #[test]
    fn when_every_tracked_search_fails_the_round_reports_the_failure() {
        let server = serve_sequence(vec![failure(), failure(), failure()]).expect("서버");
        let feed = TrackedNewsFeed::new(
            client(server.port()),
            TrackedSearch::GoogleNews,
            targets(holdings()),
            NewsLinkHints::default(),
        );
        let outcome = runtime().block_on(feed.new_items(FeedCursor::Start));
        let Err(_failure) = outcome else {
            panic!("모두 실패하면 실패로 남아야 한다: {outcome:?}");
        };
        assert_eq!(server.finish().expect("요청").len(), 3);
    }

    #[test]
    fn yahoo_asks_only_for_us_holdings_and_leaves_url_hints_for_linking() {
        let body = rss(&[(
            "iPhone demand cools in China",
            "https://finance.yahoo.com/news/iphone-demand-1.html",
            FRESH,
        )]);
        let server = serve_sequence(vec![xml(&body), xml(&body)]).expect("서버");
        let hints = NewsLinkHints::default();
        let feed = TrackedNewsFeed::new(
            client(server.port()),
            TrackedSearch::YahooFinance,
            targets(holdings()),
            hints.clone(),
        );
        let found = runtime()
            .block_on(feed.new_items(FeedCursor::Start))
            .expect("수집");
        let DataAvailability::Available((items, _cursor)) = found else {
            panic!("기사가 있어야 한다: {found:?}");
        };
        assert_eq!(items.len(), 1);
        let url = NormalizedUrl::try_new("https://finance.yahoo.com/news/iphone-demand-1.html")
            .expect("주소");
        assert_eq!(
            hints.take(&url).into_iter().collect::<Vec<_>>(),
            vec![InstrumentId::try_new("AAPL").expect("종목")]
        );
        assert_eq!(hints.take(&url).len(), 0, "힌트는 한 번 쓰고 지운다");
        let again = runtime()
            .block_on(feed.new_items(FeedCursor::Start))
            .expect("두 번째");
        assert_eq!(
            again,
            DataAvailability::NoData,
            "이미 본 주소는 다시 내지 않는다"
        );
        let requests = server.finish().expect("요청");
        assert_eq!(requests.len(), 2, "미국 종목 하나만 묻는다");
        assert!(String::from_utf8_lossy(&requests[0]).contains("s=AAPL"));
    }

    #[test]
    fn no_tracked_instruments_means_no_request() {
        let feed = TrackedNewsFeed::new(
            client(9),
            TrackedSearch::GoogleNews,
            targets(Vec::new()),
            NewsLinkHints::default(),
        );
        assert_eq!(
            runtime()
                .block_on(feed.new_items(FeedCursor::Start))
                .expect("수집"),
            DataAvailability::NoData
        );
    }
}
