#[cfg(test)]
mod tests {
    use std::future::{Future, ready};
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    use application::{
        Clock, CollectedEntry, CollectedObservation, DataAvailability, FeedCursor, FeedSource,
        NewsFeed, SecretValue, TrackedInstrument,
    };
    use domain::{
        DisclosureSource, DisplayInstrumentName, InstrumentId, KnownMarket, Nanosecond,
        NormalizedUrl, OffsetSeconds, Symbol, Timestamp, UnixSeconds,
    };
    use sources::{
        ContactFuture, DartKeySource, DartLatestFeed, NewsLinkHints, SecClientParts,
        SecContactSource, SecTrackedFeed, SecretFuture, SourceEndpoint, SourceHttpClient,
        SourceWait, TrackedTargets, sec_contact_header,
    };
    use testkit::serve_sequence;

    const BASE_SECONDS: i64 = 1_794_441_600;

    struct AutoClock(AtomicU64);

    impl Clock for AutoClock {
        fn now(&self) -> Timestamp {
            let elapsed = self
                .0
                .load(Ordering::SeqCst)
                .checked_div(1_000_000_000)
                .expect("초");
            Timestamp::from_parts(
                UnixSeconds::new(BASE_SECONDS.saturating_add(i64::try_from(elapsed).expect("초"))),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각")
        }
    }

    impl SourceWait for AutoClock {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            let delta = u64::try_from(duration.as_nanos()).expect("나노초");
            self.0.fetch_add(delta, Ordering::SeqCst);
            Box::pin(ready(()))
        }
    }

    fn parts(port: u16) -> SecClientParts {
        let clock = Arc::new(AutoClock(AtomicU64::new(0)));
        let shared: Arc<dyn Clock> = Arc::<AutoClock>::clone(&clock);
        SecClientParts {
            endpoint: SourceEndpoint::local(port).expect("주소"),
            clock: shared,
            wait: clock,
            diagnostics: None,
        }
    }

    fn client(port: u16) -> Arc<SourceHttpClient> {
        let made = parts(port);
        Arc::new(
            SourceHttpClient::new(made.endpoint, made.clock, made.wait, None).expect("클라이언트"),
        )
    }

    struct Inputs {
        key: Option<&'static str>,
        contact: Option<&'static str>,
        tracked: Vec<TrackedInstrument>,
    }

    impl DartKeySource for Inputs {
        fn dart_key(&self) -> SecretFuture<'_> {
            Box::pin(ready(
                self.key
                    .map(|key| SecretValue::new(key.as_bytes().to_vec())),
            ))
        }
    }

    impl SecContactSource for Inputs {
        fn sec_contact(&self) -> ContactFuture<'_> {
            Box::pin(ready(self.contact.map(str::to_owned)))
        }
    }

    impl TrackedTargets for Inputs {
        fn tracked(&self) -> Pin<Box<dyn Future<Output = Vec<TrackedInstrument>> + Send + '_>> {
            Box::pin(ready(self.tracked.clone()))
        }
    }

    fn apple() -> TrackedInstrument {
        TrackedInstrument {
            symbol: Symbol::try_new("AAPL").expect("종목"),
            market: KnownMarket::Us,
            name: DisplayInstrumentName::try_new("애플").expect("이름"),
        }
    }

    fn json(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json; charset=utf-8\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("런타임")
    }

    fn urls(items: &[CollectedObservation]) -> Vec<String> {
        items
            .iter()
            .filter_map(|item| match item.entry() {
                CollectedEntry::Disclosure(filing) => {
                    Some(filing.collected().url().as_str().to_owned())
                }
                CollectedEntry::News(_) => None,
            })
            .collect()
    }

    const DART_LIST: &str = r#"{"status":"000","page_no":1,"total_page":1,"total_count":2,"list":[{"corp_cls":"Y","corp_name":"삼성전자","corp_code":"00126380","stock_code":"005930","report_nm":"주요사항보고서","rcept_no":"20261112000001","flr_nm":"삼성전자","rcept_dt":"20261112","rm":""},{"corp_cls":"E","corp_name":"비상장회사","corp_code":"00999999","stock_code":"","report_nm":"감사보고서","rcept_no":"20261112000002","flr_nm":"비상장회사","rcept_dt":"20261112","rm":""}]}"#;

    #[test]
    fn dart_without_a_key_is_not_configured_and_sends_nothing() {
        let feed = DartLatestFeed::new(
            client(9),
            Arc::new(Inputs {
                key: None,
                contact: None,
                tracked: Vec::new(),
            }),
            NewsLinkHints::default(),
        );
        assert_eq!(
            feed.source(),
            FeedSource::Disclosure(DisclosureSource::Dart)
        );
        assert_eq!(
            runtime().block_on(feed.new_items(FeedCursor::Start)),
            Ok(DataAvailability::NotConfigured)
        );
    }

    #[test]
    fn dart_with_a_key_reads_today_links_listed_codes_and_skips_seen_receipts() {
        let server = serve_sequence(vec![json(DART_LIST), json(DART_LIST)]).expect("서버");
        let hints = NewsLinkHints::default();
        let feed = DartLatestFeed::new(
            client(server.port()),
            Arc::new(Inputs {
                key: Some("0123456789abcdef0123456789abcdef01234567"),
                contact: None,
                tracked: Vec::new(),
            }),
            hints.clone(),
        );
        let found = runtime()
            .block_on(feed.new_items(FeedCursor::Start))
            .expect("수집");
        let DataAvailability::Available((items, _cursor)) = found else {
            panic!("공시가 있어야 한다: {found:?}");
        };
        assert_eq!(items.len(), 2);
        let listed =
            NormalizedUrl::try_new("https://dart.fss.or.kr/dsaf001/main.do?rcpNo=20261112000001")
                .expect("주소");
        assert_eq!(
            hints.take(&listed).into_iter().collect::<Vec<_>>(),
            vec![InstrumentId::try_new("005930").expect("종목")]
        );
        assert_eq!(
            runtime().block_on(feed.new_items(FeedCursor::Start)),
            Ok(DataAvailability::NoData),
            "이미 받은 접수 번호는 다시 내지 않는다"
        );
        let requests = server.finish().expect("요청");
        let head = String::from_utf8_lossy(&requests[0]).into_owned();
        assert!(head.contains("bgn_de=20261112&end_de=20261112"), "{head}");
    }

    const SEC_TICKERS: &str =
        r#"{"0": {"cik_str": 320193, "ticker": "AAPL", "title": "Apple Inc."}}"#;

    const SEC_SUBMISSIONS: &str = r#"{
        "cik": "320193",
        "name": "Apple Inc.",
        "filings": {
            "recent": {
                "accessionNumber": ["0000320193-26-000105", "0000320193-26-000077"],
                "form": ["8-K", "10-K"],
                "filingDate": ["2026-11-11", "2026-08-01"],
                "reportDate": ["2026-11-10", "2026-06-28"],
                "acceptanceDateTime": ["2026-11-11T16:30:05.000Z", "2026-08-01T21:05:00.000Z"],
                "primaryDocument": ["aapl-20261110.htm", "aapl-20260628.htm"],
                "primaryDocDescription": ["8-K", "10-K"]
            }
        }
    }"#;

    #[test]
    fn sec_without_a_contact_is_not_configured() {
        let feed = SecTrackedFeed::new(
            parts(9),
            Arc::new(Inputs {
                key: None,
                contact: None,
                tracked: vec![apple()],
            }),
            Arc::new(Inputs {
                key: None,
                contact: None,
                tracked: Vec::new(),
            }),
            NewsLinkHints::default(),
        );
        assert_eq!(
            runtime().block_on(feed.new_items(FeedCursor::Start)),
            Ok(DataAvailability::NotConfigured)
        );
    }

    #[test]
    fn sec_reads_recent_filings_of_us_holdings_with_the_contact_header() {
        let server = serve_sequence(vec![json(SEC_TICKERS), json(SEC_SUBMISSIONS)]).expect("서버");
        let hints = NewsLinkHints::default();
        let inputs = Arc::new(Inputs {
            key: None,
            contact: Some("tester@example.com"),
            tracked: vec![apple()],
        });
        let feed = SecTrackedFeed::new(
            parts(server.port()),
            Arc::<Inputs>::clone(&inputs),
            inputs,
            hints.clone(),
        );
        let found = runtime()
            .block_on(feed.new_items(FeedCursor::Start))
            .expect("수집");
        let DataAvailability::Available((items, _cursor)) = found else {
            panic!("공시가 있어야 한다: {found:?}");
        };
        let found_urls = urls(&items);
        assert_eq!(
            found_urls.len(),
            1,
            "7일보다 오래된 10-K는 뺀다: {found_urls:?}"
        );
        let url = NormalizedUrl::try_new(&found_urls[0]).expect("주소");
        assert_eq!(
            hints.take(&url).into_iter().collect::<Vec<_>>(),
            vec![InstrumentId::try_new("AAPL").expect("종목")]
        );
        let requests = server.finish().expect("요청");
        let head = String::from_utf8_lossy(&requests[0]).to_lowercase();
        assert!(
            head.contains("user-agent: stockcopilot tester@example.com"),
            "{head}"
        );
    }

    #[test]
    fn kind_rejections_name_the_stage_and_shape_without_the_body() {
        let html = sources::explain_kind_rss(b"<!DOCTYPE html><html><body>blocked</body></html>");
        assert_eq!(html.rejected, sources::KindRejection::Rejected);
        assert!(
            html.detail.starts_with("stage=feed shape=html"),
            "{}",
            html.detail
        );
        assert!(!html.detail.contains("blocked"), "{}", html.detail);
        let odd = sources::explain_kind_rss(
            r#"<?xml version="1.0"?><rss><channel><item><title>[유]삼성전자 공시</title><link>https://kind.krx.co.kr/x?no=1</link></item></channel></rss>"#
                .as_bytes(),
        );
        assert_eq!(odd.rejected, sources::KindRejection::Rejected);
        assert!(odd.detail.contains("skipped=no-acptno:1"), "{}", odd.detail);
        assert!(!odd.detail.contains("삼성전자"), "{}", odd.detail);
    }

    #[test]
    fn a_bare_email_gets_an_app_name_so_sec_accepts_the_header() {
        assert!(sec_contact_header("tester@example.com").is_some());
        assert!(sec_contact_header("Kim tester@example.com").is_some());
        assert!(sec_contact_header("no-email").is_none());
    }
}
