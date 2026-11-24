#[cfg(test)]
mod tests {
    use std::future::{Future, ready};
    use std::pin::Pin;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use std::time::Duration;

    use application::{
        Clock, CollectedEntry, DataAvailability, FeedCursor, FeedSource, NewsFeed, PublicationTime,
        UserAgentRequirement,
    };
    use domain::{
        DisclosureSource, Nanosecond, NewsSource, OffsetSeconds, ReceiptNo, SecEdgarReceiptNo,
        Symbol, Timestamp, UnixSeconds,
    };
    use sources::{
        BROWSER_USER_AGENT, ContactUserAgent, HttpDataSource, SecCik, SecEdgarClient,
        SecEdgarCurrentAtomFeed, SecEdgarSubmissionsFeed, SourceEndpoint, SourceHttpClient,
        SourceRegistry, SourceWait, YahooFinanceFeed,
    };
    use testkit::{ConnectionObservation, ConnectionProbe, serve_sequence};

    struct RecordingClock {
        nanos: AtomicU64,
        wait_count: AtomicU64,
        min_wait_nanos: AtomicU64,
    }

    impl RecordingClock {
        fn new() -> Self {
            Self {
                nanos: AtomicU64::new(0),
                wait_count: AtomicU64::new(0),
                min_wait_nanos: AtomicU64::new(u64::MAX),
            }
        }

        fn wait_count(&self) -> u64 {
            self.wait_count.load(Ordering::SeqCst)
        }

        fn min_wait_nanos(&self) -> u64 {
            self.min_wait_nanos.load(Ordering::SeqCst)
        }
    }

    impl Clock for RecordingClock {
        fn now(&self) -> Timestamp {
            let secs = 1_800_000_000_i64.saturating_add(
                i64::try_from(
                    self.nanos
                        .load(Ordering::SeqCst)
                        .checked_div(1_000_000_000)
                        .expect("div"),
                )
                .expect("sec"),
            );
            Timestamp::from_parts(
                UnixSeconds::new(secs),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("ts")
        }
    }

    impl SourceWait for RecordingClock {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            let delta = u64::try_from(duration.as_nanos()).expect("nanos");
            self.wait_count.fetch_add(1_u64, Ordering::SeqCst);
            self.min_wait_nanos.fetch_min(delta, Ordering::SeqCst);
            self.nanos.fetch_add(delta, Ordering::SeqCst);
            Box::pin(ready(()))
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    fn make_client(
        port: u16,
        contact: Option<ContactUserAgent>,
    ) -> (Arc<SourceHttpClient>, Arc<RecordingClock>) {
        let clock = Arc::new(RecordingClock::new());
        let clock_copy = Arc::clone(&clock);
        let wait_copy = Arc::clone(&clock);
        let client = Arc::new(
            SourceHttpClient::new(
                SourceEndpoint::local(port).expect("endpoint"),
                clock_copy,
                wait_copy,
                contact,
            )
            .expect("client"),
        );
        (client, clock)
    }

    fn ok_response(content_type: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    const YAHOO_RSS_FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>Yahoo Finance: AAPL, MSFT News</title>
<item>
  <title>Apple Unveils Next-Gen M6 Chips</title>
  <link>https://finance.yahoo.com/news/apple-m6-100000.html</link>
  <description>Apple announced new silicon across Mac lineup.</description>
  <pubDate>Wed, 11 Nov 2026 14:00:00 +0000</pubDate>
</item>
<item>
  <title>Microsoft Expands AI Cloud Regions</title>
  <link>https://finance.yahoo.com/news/msft-cloud-110000.html</link>
  <description>Cloud capacity rises in North America.</description>
  <pubDate>Wed, 11 Nov 2026 15:00:00 +0000</pubDate>
</item>
</channel></rss>"#;

    const SEC_TICKERS_FIXTURE: &str = r#"{
        "0": {"cik_str": 320193, "ticker": "AAPL", "title": "Apple Inc."},
        "1": {"cik_str": "0001067983", "ticker": "BRK-B", "title": "BERKSHIRE HATHAWAY INC"}
    }"#;

    const SEC_SUBMISSIONS_FIXTURE: &str = r#"{
        "cik": "320193",
        "name": "Apple Inc.",
        "filings": {
            "recent": {
                "accessionNumber": ["0000320193-26-000105", "0000320193-26-000099"],
                "form": ["8-K", "10-Q"],
                "filingDate": ["2026-11-11", "2026-10-30"],
                "reportDate": ["2026-11-10", "2026-09-26"],
                "acceptanceDateTime": ["2026-11-11T16:30:05.000Z", "2026-10-30T21:05:00.000Z"],
                "primaryDocument": ["aapl-20261110.htm", "aapl-20260926.htm"],
                "primaryDocDescription": ["8-K", "10-Q"]
            }
        }
    }"#;

    const SEC_ATOM_FIXTURE: &str = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Latest Filings - Wed, 11 Nov 2026</title>
  <entry>
    <title>8-K - Apple Inc. (0000320193) (Filer)</title>
    <link rel="alternate" type="text/html" href="https://www.sec.gov/Archives/edgar/data/320193/000032019326000105/0000320193-26-000105-index.htm"/>
    <summary type="html">Filed: 2026-11-11 AccNo: 0000320193-26-000105</summary>
    <updated>2026-11-11T16:30:05-05:00</updated>
    <category scheme="https://www.sec.gov/" label="form type" term="8-K"/>
    <id>urn:tag:sec.gov,2008:accession-number=0000320193-26-000105</id>
  </entry>
</feed>"#;

    #[test]
    fn yahoo_finance_sends_browser_user_agent_and_deduplicates_by_cursor() {
        let server = serve_sequence(vec![
            ok_response("application/xml", YAHOO_RSS_FIXTURE),
            ok_response("application/xml", YAHOO_RSS_FIXTURE),
        ])
        .expect("server");
        let (client, _clock) = make_client(server.port(), None);
        let feed = YahooFinanceFeed::new(
            client,
            vec![
                Symbol::try_new("AAPL").expect("sym"),
                Symbol::try_new("MSFT").expect("sym"),
            ],
        );

        assert_eq!(feed.source(), FeedSource::News(NewsSource::YahooFinance));
        assert_eq!(
            feed.metadata().requires_user_agent(),
            UserAgentRequirement::Browser
        );

        let rt = runtime();
        let DataAvailability::Available((items, cursor)) = rt
            .block_on(feed.new_items(FeedCursor::Start))
            .expect("first")
        else {
            panic!("expected yahoo items");
        };
        assert_eq!(items.len(), 2_usize);
        assert_eq!(
            rt.block_on(feed.new_items(cursor)).expect("second"),
            DataAvailability::NoData
        );

        let calls = server.finish().expect("finish");
        let wire_head = std::str::from_utf8(&calls[0]).expect("utf8");
        assert!(
            wire_head.contains("s=AAPL%2CMSFT&region=US&lang=en-US"),
            "{wire_head}"
        );
        let expected_ua = format!("user-agent: {BROWSER_USER_AGENT}");
        assert!(
            wire_head
                .to_ascii_lowercase()
                .contains(&expected_ua.to_ascii_lowercase()),
            "{wire_head}"
        );
    }

    #[test]
    fn sec_edgar_without_contact_makes_zero_connections_across_all_endpoints() {
        let probe = ConnectionProbe::bind().expect("probe");
        let (client, _clock) = make_client(probe.port().expect("port"), None);
        let sec = SecEdgarClient::new(client);
        let apple_cik = SecCik::try_new("320193").expect("cik");
        let rt = runtime();

        assert_eq!(
            rt.block_on(sec.fetch_ticker_cik_map()),
            Ok(DataAvailability::NotConfigured)
        );
        assert_eq!(
            rt.block_on(sec.fetch_company_submissions(&apple_cik)),
            Ok(DataAvailability::NotConfigured)
        );
        assert_eq!(
            rt.block_on(sec.fetch_current_atom(Some(&apple_cik), Some("8-K"))),
            Ok(DataAvailability::NotConfigured)
        );

        let sub_feed = SecEdgarSubmissionsFeed::new(sec.clone(), apple_cik.clone());
        assert_eq!(
            sub_feed.source(),
            FeedSource::Disclosure(DisclosureSource::SecEdgar)
        );
        assert_eq!(
            sub_feed.metadata().requires_user_agent(),
            UserAgentRequirement::Contact
        );
        assert_eq!(
            rt.block_on(sub_feed.new_items(FeedCursor::Start)),
            Ok(DataAvailability::NotConfigured)
        );

        let atom_feed = SecEdgarCurrentAtomFeed::new(sec, Some(apple_cik), Some("8-K".to_owned()));
        assert_eq!(
            rt.block_on(atom_feed.new_items(FeedCursor::Start)),
            Ok(DataAvailability::NotConfigured)
        );

        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );
    }

    fn assert_contact_headers(calls: Vec<Vec<u8>>, expected_count: usize) {
        assert_eq!(calls.len(), expected_count);
        for raw in calls {
            let text = std::str::from_utf8(&raw).expect("utf8");
            assert!(
                text.to_ascii_lowercase()
                    .contains("user-agent: stockcopilot research user@example.com"),
                "{text}"
            );
        }
    }

    #[test]
    fn sec_edgar_with_contact_sends_contact_header_and_enforces_rate_cap() {
        let spec = SourceRegistry::spec(HttpDataSource::SecEdgar);
        assert_eq!(spec.max_calls_per_second(), Some(10_u32));
        assert!(spec.min_interval() >= Duration::from_secs(5));

        let server = serve_sequence(vec![
            ok_response("application/json", SEC_TICKERS_FIXTURE),
            ok_response("application/json", SEC_SUBMISSIONS_FIXTURE),
            ok_response("application/atom+xml", SEC_ATOM_FIXTURE),
        ])
        .expect("server");
        let contact =
            ContactUserAgent::try_new("StockCopilot Research user@example.com").expect("contact");
        let (client, clock) = make_client(server.port(), Some(contact));
        let edgar = SecEdgarClient::new(client);
        let rt = runtime();

        let DataAvailability::Available(map) =
            rt.block_on(edgar.fetch_ticker_cik_map()).expect("tickers")
        else {
            panic!("expected ticker map");
        };
        let aapl_cik = map
            .cik_for_symbol(&Symbol::try_new("AAPL").expect("sym"))
            .expect("AAPL")
            .clone();
        assert_eq!(aapl_cik.as_str(), "0000320193");
        assert_eq!(
            map.cik_for_symbol(&Symbol::try_new("BRK.B").expect("sym"))
                .map(SecCik::as_str),
            Some("0001067983")
        );

        let sub_feed = SecEdgarSubmissionsFeed::new(edgar.clone(), aapl_cik.clone());
        let DataAvailability::Available((sub_items, _cursor)) = rt
            .block_on(sub_feed.new_items(FeedCursor::Start))
            .expect("submissions")
        else {
            panic!("expected submissions");
        };
        assert_eq!(sub_items.len(), 2_usize);
        let CollectedEntry::Disclosure(first_disc) = sub_items[0].entry() else {
            panic!("expected disclosure");
        };
        assert_eq!(
            first_disc.key().receipt_no(),
            &ReceiptNo::SecEdgar(SecEdgarReceiptNo::try_new("0000320193-26-000105").expect("acc"))
        );
        assert!(matches_known(sub_items[0].published_at()));

        let atom_feed = SecEdgarCurrentAtomFeed::new(edgar, Some(aapl_cik), Some("8-K".to_owned()));
        let DataAvailability::Available((atom_items, _atom_cursor)) = rt
            .block_on(atom_feed.new_items(FeedCursor::Start))
            .expect("atom")
        else {
            panic!("expected atom items");
        };
        assert_eq!(atom_items.len(), 1_usize);

        assert_eq!(clock.wait_count(), 2_u64);
        assert!(clock.min_wait_nanos() >= 5_000_000_000_u64);
        assert_contact_headers(server.finish().expect("finish"), 3_usize);
    }

    const fn matches_known(value: PublicationTime) -> bool {
        match value {
            PublicationTime::Known(_) => true,
            PublicationTime::NotProvided => false,
        }
    }
}
