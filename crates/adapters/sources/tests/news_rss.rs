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
        AiReadPolicy, Clock, CollectedEntry, ConditionalGet, DataAvailability, FeedCursor,
        FeedSource, InformationError, NewsFeed, PublicationTime, ResetTime, ResponseMismatch,
    };
    use domain::{Nanosecond, NewsSource, OffsetSeconds, Timestamp, UnixSeconds};
    use sources::{PressMacroSource, RssNewsFeed, SourceEndpoint, SourceHttpClient, SourceWait};
    use testkit::serve_sequence;

    struct AutoClock {
        nanos: AtomicU64,
    }

    impl AutoClock {
        fn new() -> Self {
            Self {
                nanos: AtomicU64::new(0),
            }
        }
    }

    impl Clock for AutoClock {
        fn now(&self) -> Timestamp {
            let nanos = self.nanos.load(Ordering::SeqCst);
            let seconds = 1_800_000_000_i64.saturating_add(
                i64::try_from(nanos.checked_div(1_000_000_000).expect("div")).expect("sec"),
            );
            Timestamp::from_parts(
                UnixSeconds::new(seconds),
                Nanosecond::new(u32::try_from(nanos % 1_000_000_000).expect("ns")),
                OffsetSeconds::new(0),
            )
            .expect("timestamp")
        }
    }

    impl SourceWait for AutoClock {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            let delta = u64::try_from(duration.as_nanos()).expect("nanos");
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

    fn xml_response(body: &str, headers: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/xml; charset=utf-8\r\n{headers}Connection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn raw_bytes_response(body: &[u8]) -> Vec<u8> {
        let mut head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/xml\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        head.extend_from_slice(body);
        head
    }

    fn make_client(port: u16) -> Arc<SourceHttpClient> {
        let clock = Arc::new(AutoClock::new());
        let clock_copy = Arc::clone(&clock);
        Arc::new(
            SourceHttpClient::new(
                SourceEndpoint::local(port).expect("endpoint"),
                clock_copy,
                clock,
                None,
            )
            .expect("client"),
        )
    }

    fn assert_known_pub_time(time: PublicationTime) {
        match time {
            PublicationTime::Known(_) => {}
            PublicationTime::NotProvided => panic!("expected known publication time"),
        }
    }

    const HANKYUNG_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>한국경제 증권</title>
<item>
  <title><![CDATA[삼성전자, 반도체 수주 확대 &amp; 실적 전망 상향]]></title>
  <link>https://www.hankyung.com/article/202611110253i?utm_source=rss</link>
  <pubDate>Wed, 11 Nov 2026 16:25:00 +0900</pubDate>
  <author>증권부</author>
</item>
</channel></rss>"#;

    const MAEKYUNG_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>매일경제 증권</title>
<item>
  <title><![CDATA[SK하이닉스 HBM 출하 증가]]></title>
  <link>https://www.mk.co.kr/news/stock/12205485</link>
  <description><![CDATA[메모리 업황 개선으로 수익성 회복세가 이어지고 있다.]]></description>
  <pubDate>Wed, 11 Nov 2026 16:37:00 +0900</pubDate>
</item>
</channel></rss>"#;

    const YONHAP_MARKET_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:dc="http://purl.org/dc/elements/1.1/"><channel><title>연합뉴스 마켓+</title>
<item>
  <title>코스피 외국인 매수에 상승 마감</title>
  <link>https://www.yna.co.kr/view/AKR20261111157200030</link>
  <description>코스피가 대형주 강세에 힘입어 상승 마감했다.</description>
  <pubDate>Wed, 11 Nov 2026 17:13:00 +0900</pubDate>
  <dc:creator>연합뉴스</dc:creator>
</item>
</channel></rss>"#;

    const YONHAP_ECONOMY_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>연합뉴스 경제</title>
<item>
  <title>코스피 외국인 매수에 상승 마감</title>
  <link>https://www.yna.co.kr/view/AKR20261111157200030#dup</link>
  <description>중복 기사</description>
  <pubDate>Wed, 11 Nov 2026 17:13:00 +0900</pubDate>
</item>
<item>
  <title>수출 물가 3개월 연속 상승</title>
  <link>https://www.yna.co.kr/view/AKR20261111157200099</link>
  <description>반도체 단가 회복 영향.</description>
  <pubDate>Wed, 11 Nov 2026 17:10:00 +0900</pubDate>
</item>
</channel></rss>"#;

    const EDAILY_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>이데일리 증권</title>
<item>
  <title>현대차 북미 판매 호조</title>
  <link>http://www.edaily.co.kr/news/newspath.asp?newsid=05362806645644736</link>
  <description><![CDATA[북미 하이브리드 판매가 전년 대비 증가했다.]]></description>
  <pubDate>Wed, 11 Nov 2026 17:14:00 +0900</pubDate>
</item>
</channel></rss>"#;

    const BOK_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>한국은행 보도자료</title>
<item>
  <title>2026년 11월 통화정책방향</title>
  <link>https://www.bok.or.kr/portal/bbs/B0000552/view.do?nttId=1001</link>
  <description><![CDATA[<p>금융통화위원회는 기준금리를 <strong>동결</strong>하기로&nbsp;결정하였다.</p>]]></description>
  <pubDate>2026-11-11 10:00:00</pubDate>
</item>
</channel></rss>"#;

    const FED_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>Federal Reserve Press Releases</title>
<item>
  <title>Federal Reserve issues FOMC statement</title>
  <link>https://www.federalreserve.gov/newsevents/pressreleases/monetary20261105a.htm</link>
  <description>Recent indicators suggest that economic activity has continued to expand.</description>
  <pubDate>Thu, 05 Nov 2026 14:00:00 EST</pubDate>
</item>
</channel></rss>"#;

    #[test]
    fn hankyung_parses_feed_and_uses_conditional_get() {
        let rt = runtime();
        let server = serve_sequence(vec![
            xml_response(
                HANKYUNG_XML,
                "Last-Modified: Wed, 11 Nov 2026 07:25:00 GMT\r\n",
            ),
            b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".to_vec(),
        ])
        .expect("server");
        let feed = RssNewsFeed::new(make_client(server.port()), PressMacroSource::Hankyung);
        assert_eq!(feed.source(), FeedSource::News(NewsSource::Hankyung));
        assert_eq!(
            feed.metadata().supports_conditional_get(),
            ConditionalGet::Supported
        );
        assert_eq!(
            feed.metadata().allow_ai_read(),
            AiReadPolicy::TitleAndSummary
        );
        let DataAvailability::Available((items, cursor)) =
            rt.block_on(feed.new_items(FeedCursor::Start)).expect("hk1")
        else {
            panic!("expected hankyung items");
        };
        assert_eq!(items.len(), 1);
        let first = items.first().expect("item");
        let CollectedEntry::News(news) = first.entry() else {
            panic!("expected news");
        };
        assert_eq!(
            news.collected().title().as_untrusted_str(),
            "삼성전자, 반도체 수주 확대 & 실적 전망 상향"
        );
        assert_eq!(
            news.collected().url().as_str(),
            "https://www.hankyung.com/article/202611110253i"
        );
        assert_eq!(news.collected().body(), None);
        assert_known_pub_time(first.published_at());
        assert_eq!(
            rt.block_on(feed.new_items(cursor)).expect("hk2"),
            DataAvailability::NoData
        );
        let captured = server.finish().expect("captured");
        let second_call = String::from_utf8_lossy(captured.get(1).expect("second"));
        assert!(
            second_call
                .to_ascii_lowercase()
                .contains("if-modified-since: wed, 11 nov 2026 07:25:00 gmt")
        );
    }

    #[test]
    fn maekyung_and_yonhap_parse_and_deduplicate_by_url() {
        let rt = runtime();
        let mk_server = serve_sequence(vec![
            xml_response(MAEKYUNG_XML, ""),
            xml_response(MAEKYUNG_XML, ""),
        ])
        .expect("server");
        let mk_feed = RssNewsFeed::new(make_client(mk_server.port()), PressMacroSource::Maekyung);
        assert_eq!(
            mk_feed.metadata().supports_conditional_get(),
            ConditionalGet::Unsupported
        );
        let DataAvailability::Available((mk_items, mk_cursor)) = rt
            .block_on(mk_feed.new_items(FeedCursor::Start))
            .expect("mk1")
        else {
            panic!("expected mk items");
        };
        assert_eq!(mk_items.len(), 1);
        let CollectedEntry::News(mk_news) = mk_items.first().expect("mk").entry() else {
            panic!("news");
        };
        assert_eq!(
            mk_news
                .collected()
                .body()
                .expect("summary")
                .as_untrusted_str(),
            "메모리 업황 개선으로 수익성 회복세가 이어지고 있다."
        );
        assert_eq!(
            rt.block_on(mk_feed.new_items(mk_cursor)).expect("mk2"),
            DataAvailability::NoData
        );
        let _mk_captured = mk_server.finish().expect("captured");

        let yna_server = serve_sequence(vec![
            xml_response(YONHAP_MARKET_XML, ""),
            xml_response(YONHAP_ECONOMY_XML, ""),
        ])
        .expect("server");
        let yna_feed = RssNewsFeed::new(make_client(yna_server.port()), PressMacroSource::Yonhap);
        assert_eq!(yna_feed.feed_urls().len(), 2);
        let DataAvailability::Available((yna_items, _cursor)) = rt
            .block_on(yna_feed.new_items(FeedCursor::Start))
            .expect("yna")
        else {
            panic!("expected yna items");
        };
        assert_eq!(yna_items.len(), 2);
        let _yna_captured = yna_server.finish().expect("captured");
    }

    #[test]
    fn edaily_bok_and_fed_parse_recorded_feeds() {
        let rt = runtime();
        let ed_server = serve_sequence(vec![
            xml_response(EDAILY_XML, "ETag: \"ed-1\"\r\n"),
            b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".to_vec(),
        ])
        .expect("server");
        let ed_feed = RssNewsFeed::new(make_client(ed_server.port()), PressMacroSource::Edaily);
        let DataAvailability::Available((ed_items, ed_cursor)) = rt
            .block_on(ed_feed.new_items(FeedCursor::Start))
            .expect("ed1")
        else {
            panic!("expected edaily");
        };
        assert_eq!(ed_items.len(), 1);
        assert_eq!(
            rt.block_on(ed_feed.new_items(ed_cursor)).expect("ed2"),
            DataAvailability::NoData
        );
        let ed_captured = ed_server.finish().expect("captured");
        let second_call = String::from_utf8_lossy(ed_captured.get(1).expect("second"));
        assert!(
            second_call
                .to_ascii_lowercase()
                .contains("if-none-match: \"ed-1\"")
        );

        let bok_server = serve_sequence(vec![xml_response(BOK_XML, "")]).expect("server");
        let bok_feed = RssNewsFeed::new(
            make_client(bok_server.port()),
            PressMacroSource::BankOfKorea,
        );
        let DataAvailability::Available((bok_items, _cursor)) = rt
            .block_on(bok_feed.new_items(FeedCursor::Start))
            .expect("bok")
        else {
            panic!("expected bok");
        };
        let first_bok = bok_items.first().expect("bok");
        let CollectedEntry::News(bok_news) = first_bok.entry() else {
            panic!("news");
        };
        assert_eq!(
            bok_news
                .collected()
                .body()
                .expect("summary")
                .as_untrusted_str(),
            "금융통화위원회는 기준금리를 동결하기로 결정하였다."
        );
        assert_known_pub_time(first_bok.published_at());
        let _bok_captured = bok_server.finish().expect("captured");
    }

    #[test]
    fn fed_parses_recorded_feed_and_uses_conditional_get() {
        let rt = runtime();
        let fed_server = serve_sequence(vec![
            xml_response(FED_XML, "ETag: \"fed-1\"\r\n"),
            b"HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".to_vec(),
        ])
        .expect("server");
        let fed_feed = RssNewsFeed::new(
            make_client(fed_server.port()),
            PressMacroSource::FederalReserve,
        );
        let DataAvailability::Available((fed_items, fed_cursor)) = rt
            .block_on(fed_feed.new_items(FeedCursor::Start))
            .expect("fed1")
        else {
            panic!("expected fed");
        };
        assert_eq!(fed_items.len(), 1);
        assert_known_pub_time(fed_items.first().expect("fed").published_at());
        assert_eq!(
            rt.block_on(fed_feed.new_items(fed_cursor)).expect("fed2"),
            DataAvailability::NoData
        );
        let _fed_captured = fed_server.finish().expect("captured");
    }

    #[test]
    fn broken_feeds_are_rejected_cleanly() {
        let rt = runtime();
        let broken_payloads: Vec<Vec<u8>> = vec![
            xml_response("", ""),
            xml_response("<!DOCTYPE html><html><body>Access Denied</body></html>", ""),
            xml_response("<rss><channel><item><title>unclosed", ""),
            xml_response(
                "<rss><channel><item><title>a</wrong></item></channel></rss>",
                "",
            ),
            raw_bytes_response(&[0xFF, 0xFE, 0x00, 0x01]),
            xml_response(
                "<rss version=\"2.0\"><channel><item><title>나쁜 링크</title><link>javascript:alert(1)</link></item></channel></rss>",
                "",
            ),
        ];

        for payload in broken_payloads {
            let server = serve_sequence(vec![payload]).expect("server");
            let feed = RssNewsFeed::new(make_client(server.port()), PressMacroSource::Maekyung);
            assert_eq!(
                rt.block_on(feed.new_items(FeedCursor::Start)),
                Err(InformationError::InvalidResponse(ResponseMismatch::Schema))
            );
            let _captured = server.finish().expect("captured");
        }
    }

    #[test]
    fn partial_broken_item_skips_bad_entry_and_http_503_maps_unavailable() {
        let rt = runtime();
        let partial_xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel>
<item><title>깨진 링크</title><link>not-a-url</link></item>
<item><title>정상 기사</title><link>https://www.mk.co.kr/news/stock/99999</link><description>요약</description></item>
</channel></rss>"#;
        let partial_server = serve_sequence(vec![xml_response(partial_xml, "")]).expect("server");
        let partial_feed = RssNewsFeed::new(
            make_client(partial_server.port()),
            PressMacroSource::Maekyung,
        );
        let DataAvailability::Available((items, _cursor)) = rt
            .block_on(partial_feed.new_items(FeedCursor::Start))
            .expect("partial")
        else {
            panic!("expected valid item");
        };
        assert_eq!(items.len(), 1);
        let _partial_captured = partial_server.finish().expect("captured");

        let err_server = serve_sequence(vec![
            b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_vec(),
        ])
        .expect("server");
        let err_feed = RssNewsFeed::new(make_client(err_server.port()), PressMacroSource::Hankyung);
        assert_eq!(
            rt.block_on(err_feed.new_items(FeedCursor::Start)),
            Err(InformationError::Unavailable(ResetTime::NotProvided))
        );
        let _err_captured = err_server.finish().expect("captured");
    }
}
