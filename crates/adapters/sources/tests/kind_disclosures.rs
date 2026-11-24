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
        SecretValue,
    };
    use domain::{
        CalendarDate, CalendarYear, CommonShare, Currency, DartReceiptNo, DayNumber,
        DisclosureSource, DisplayInstrumentName, InstrumentId, KindReceiptNo, KoreanMasterDetail,
        LiquidationTrading, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentFacts,
        MasterInstrumentRow, MasterSecurityType, MonthNumber, Nanosecond, NxtTrading,
        OffsetSeconds, ReceiptNo, Symbol, Timestamp, TradingSuspension, UnixSeconds,
    };
    use sources::{
        CorpCodeResolution, CorpNameCodeResolver, DartCorpMapClient, DartDisclosureClient,
        DartFallbackCoverage, DartRssClient, DomesticDisclosureCollector, DomesticDisclosureRoute,
        KindCorpMatchFailureReason, KindDisclosureClient, KindDisclosureFeed, KindFetchWindow,
        KindMarket, ReceiptCorrespondence, SourceEndpoint, SourceHttpClient, SourceWait,
        kind_dart_receipt_correspondence, parse_corp_code_xml,
    };
    use testkit::serve_sequence;

    struct AutoClock(AtomicU64);

    impl Clock for AutoClock {
        fn now(&self) -> Timestamp {
            let secs = 1_800_000_000_i64.saturating_add(
                i64::try_from(
                    self.0
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

    impl SourceWait for AutoClock {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            let delta = u64::try_from(duration.as_nanos()).expect("nanos");
            self.0.fetch_add(delta, Ordering::SeqCst);
            Box::pin(ready(()))
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    fn make_client(port: u16) -> Arc<SourceHttpClient> {
        let clock = Arc::new(AutoClock(AtomicU64::new(0)));
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

    fn ok_response(content_type: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn status_response(status_line: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn kr_instrument(id: &str, symbol: &str, name: &str, listing_market: &str) -> MasterInstrument {
        let now = Timestamp::from_parts(
            UnixSeconds::new(1_800_000_000),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("ts");
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new(listing_market).expect("market"),
            symbol: Symbol::try_new(symbol).expect("symbol"),
            instrument_id: InstrumentId::try_new(id).expect("id"),
            name: DisplayInstrumentName::try_new(name).expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("type"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: now,
        })
        .expect("master")
    }

    const KIND_RSS_SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0"><channel><title>KIND 오늘의 공시</title>
<item>
  <title><![CDATA[[유]삼성전자 영업(잠정)실적(공정공시)]]></title>
  <link>https://kind.krx.co.kr/common/disclsviewer.do?method=search&amp;acptno=20261111000749</link>
  <pubDate>Wed, 11 Nov 2026 16:40:00 +0900</pubDate>
  <author>삼성전자</author>
  <category>공정공시</category>
</item>
<item>
  <title><![CDATA[[코]알파코드 단일판매·공급계약체결]]></title>
  <link>https://kind.krx.co.kr/common/disclsviewer.do?method=search&amp;acptno=20261111000812</link>
  <pubDate>Wed, 11 Nov 2026 16:45:00 +0900</pubDate>
  <author>(주)알파코드</author>
  <category>수시공시</category>
</item>
<item>
  <title><![CDATA[[유]미등록회사 기타경영사항]]></title>
  <link>https://kind.krx.co.kr/common/disclsviewer.do?method=search&amp;acptno=20261111000900</link>
  <pubDate>Wed, 11 Nov 2026 16:50:00 +0900</pubDate>
  <author>미등록회사</author>
  <category>수시공시</category>
</item>
</channel></rss>"#;

    const DART_TODAY_RSS_SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:dc="http://purl.org/dc/elements/1.1/">
<channel><title>DART 최근공시</title>
<item>
  <title>(유가)삼성전자 - 영업(잠정)실적(공정공시)</title>
  <link>https://dart.fss.or.kr/dsaf001/main.do?rcpNo=20261111800749</link>
  <dc:date>2026-11-11T07:40:00Z</dc:date>
</item>
</channel></rss>"#;

    const DART_API_LIST_SAMPLE: &str = r#"{
        "status": "000",
        "message": "정상",
        "page_no": 1,
        "total_page": 1,
        "total_count": 2,
        "list": [
            {
                "corp_cls": "Y",
                "corp_name": "삼성전자",
                "corp_code": "00126380",
                "stock_code": "005930",
                "report_nm": "영업(잠정)실적(공정공시)",
                "rcept_no": "20261111800749",
                "flr_nm": "삼성전자",
                "rcept_dt": "20261111",
                "rm": ""
            },
            {
                "corp_cls": "K",
                "corp_name": "알파코드",
                "corp_code": "00999999",
                "stock_code": "0153R0",
                "report_nm": "임원ㆍ주요주주특정증권등소유상황보고서",
                "rcept_no": "20261111000555",
                "flr_nm": "홍길동",
                "rcept_dt": "20261111",
                "rm": ""
            }
        ]
    }"#;

    #[test]
    fn startup_catchup_requests_500_items_and_parses_kind_rss_fields() {
        let server = serve_sequence(vec![ok_response(
            "text/xml; charset=utf-8",
            KIND_RSS_SAMPLE,
        )])
        .expect("server");
        let kind_client = KindDisclosureClient::new(make_client(server.port()));
        let entries = runtime()
            .block_on(kind_client.fetch_entries(KindFetchWindow::StartupCatchUp))
            .expect("entries");
        let wire_calls = server.finish().expect("finish");
        let first_wire = std::str::from_utf8(&wire_calls[0]).expect("utf8");
        assert!(first_wire.contains("currentPageSize=500"), "{first_wire}");
        assert_eq!(entries.len(), 3_usize);
        assert_eq!(entries[0].market(), KindMarket::Kospi);
        assert_eq!(entries[1].market(), KindMarket::Kosdaq);
        assert_eq!(entries[0].receipt_no().as_str(), "20261111000749");
        assert_eq!(
            entries[0]
                .category()
                .map(domain::UntrustedText::as_untrusted_str),
            Some("공정공시")
        );
        match entries[0].published_at() {
            PublicationTime::Known(_) => {}
            PublicationTime::NotProvided => panic!("expected minute-level KST pubDate"),
        }
    }

    #[test]
    fn corp_name_resolver_matches_alphanumeric_codes_and_lists_failures() {
        let entries = sources::parse_kind_rss(KIND_RSS_SAMPLE.as_bytes()).expect("entries");
        let master = MasterInstrumentFacts::try_new(vec![
            kr_instrument("inst-samsung", "005930", "삼성전자", "KOSPI"),
            kr_instrument("inst-alpha", "0153R0", "알파코드", "KOSDAQ"),
            kr_instrument("inst-dup-1", "000111", "동명이사", "KOSPI"),
            kr_instrument("inst-dup-2", "000222", "동명이사", "KOSPI"),
        ])
        .expect("master");
        let dart_xml = r#"<?xml version="1.0" encoding="UTF-8"?><result>
<list><corp_code>00126380</corp_code><corp_name>삼성전자주식회사</corp_name><stock_code>005930</stock_code><modify_date>20261110</modify_date></list>
</result>"#;
        let dart_map = parse_corp_code_xml(dart_xml.as_bytes()).expect("dart map");
        let _unused_client = DartCorpMapClient::new(make_client(9_u16), None);
        let resolver = CorpNameCodeResolver::from_master(&master, Some(&dart_map));

        let report = resolver.resolve_entries(&entries);
        assert_eq!(report.matched.len(), 2_usize);
        assert_eq!(report.matched[0].symbol.as_str(), "005930");
        assert_eq!(report.matched[1].symbol.as_str(), "0153R0");
        assert_eq!(report.unmatched.len(), 1_usize);
        assert_eq!(
            report.unmatched[0].reason,
            KindCorpMatchFailureReason::Unmatched
        );
        assert_eq!(
            report.unmatched[0].corp_name.as_untrusted_str(),
            "미등록회사"
        );
        match resolver.resolve("동명이사", KindMarket::Kospi) {
            CorpCodeResolution::Ambiguous(candidates) => assert_eq!(candidates.len(), 2_usize),
            other @ (CorpCodeResolution::Matched(_) | CorpCodeResolution::Unmatched) => {
                panic!("expected ambiguous, got {other:?}")
            }
        }
    }

    #[test]
    fn kind_feed_deduplicates_across_polls_using_cursor_and_regular_size_100() {
        let server = serve_sequence(vec![
            ok_response("text/xml; charset=utf-8", KIND_RSS_SAMPLE),
            ok_response("text/xml; charset=utf-8", KIND_RSS_SAMPLE),
        ])
        .expect("server");
        let feed = KindDisclosureFeed::new(
            KindDisclosureClient::new(make_client(server.port())),
            KindFetchWindow::Regular,
        );
        assert_eq!(
            feed.source(),
            FeedSource::Disclosure(DisclosureSource::Kind)
        );

        let first = runtime()
            .block_on(feed.new_items(FeedCursor::Start))
            .expect("first");
        let (items, cursor) = match first {
            DataAvailability::Available(pair) => pair,
            other @ (DataAvailability::NotConfigured | DataAvailability::NoData) => {
                panic!("expected available, got {other:?}")
            }
        };
        assert_eq!(items.len(), 3_usize);

        let second = runtime().block_on(feed.new_items(cursor)).expect("second");
        assert_eq!(second, DataAvailability::NoData);

        let wire_calls = server.finish().expect("finish");
        let first_wire = std::str::from_utf8(&wire_calls[0]).expect("utf8");
        assert!(first_wire.contains("currentPageSize=100"), "{first_wire}");
    }

    #[test]
    fn kind_dart_receipt_correspondence_distinguishes_exchange_and_direct_filings() {
        let kind_rcp = KindReceiptNo::try_new("20261111000749").expect("kind");
        let dart_kospi = DartReceiptNo::try_new("20261111800749").expect("kospi");
        let dart_kosdaq = DartReceiptNo::try_new("20261111900749").expect("kosdaq");
        let dart_direct = DartReceiptNo::try_new("20261111000749").expect("direct");
        let dart_other = DartReceiptNo::try_new("20261111800750").expect("other");

        assert_eq!(
            kind_dart_receipt_correspondence(&kind_rcp, &dart_kospi, KindMarket::Kospi),
            ReceiptCorrespondence::MatchedExchangeFiling
        );
        assert_eq!(
            kind_dart_receipt_correspondence(&kind_rcp, &dart_kosdaq, KindMarket::Kosdaq),
            ReceiptCorrespondence::MatchedExchangeFiling
        );
        assert_eq!(
            kind_dart_receipt_correspondence(&kind_rcp, &dart_kosdaq, KindMarket::Kospi),
            ReceiptCorrespondence::Mismatch
        );
        assert_eq!(
            kind_dart_receipt_correspondence(&kind_rcp, &dart_direct, KindMarket::Kospi),
            ReceiptCorrespondence::DirectDartFilingDifferent
        );
        assert_eq!(
            kind_dart_receipt_correspondence(&kind_rcp, &dart_other, KindMarket::Kospi),
            ReceiptCorrespondence::Mismatch
        );
    }

    #[test]
    fn blocked_kind_switches_to_dart_rss_and_dart_api_list_when_key_present() {
        let server = serve_sequence(vec![
            status_response("403 Forbidden", "<html>Access Denied - Akamai</html>"),
            ok_response("text/xml; charset=utf-8", DART_TODAY_RSS_SAMPLE),
            ok_response("application/json", DART_API_LIST_SAMPLE),
        ])
        .expect("server");
        let client = make_client(server.port());
        let dart_key = Arc::new(SecretValue::new(b"dart-fallback-key".to_vec()));
        let collector = DomesticDisclosureCollector::new(
            KindDisclosureClient::new(Arc::clone(&client)),
            DartRssClient::new(Arc::clone(&client)),
            Some(DartDisclosureClient::new(client, Some(dart_key))),
        );
        let date = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(11),
        )
        .expect("date");

        let batch = runtime()
            .block_on(collector.collect(KindFetchWindow::Regular, date))
            .expect("fallback batch");
        assert_eq!(
            batch.route,
            DomesticDisclosureRoute::DartFallback(DartFallbackCoverage::RssAndApi)
        );
        assert!(batch.kind_error.is_some());
        assert_eq!(batch.observations.len(), 2_usize);

        let rss_merged = batch
            .observations
            .iter()
            .find(|obs| match obs.entry() {
                CollectedEntry::Disclosure(disc) => {
                    disc.key().receipt_no()
                        == &ReceiptNo::Dart(DartReceiptNo::try_new("20261111800749").expect("r"))
                }
                CollectedEntry::News(_) => false,
            })
            .expect("merged item");
        match rss_merged.published_at() {
            PublicationTime::Known(_) => {}
            PublicationTime::NotProvided => panic!("expected DART RSS timestamp preserved"),
        }
        let wire_calls = server.finish().expect("finish");
        assert_eq!(wire_calls.len(), 3_usize);
    }

    #[test]
    fn akamai_html_challenge_on_kind_falls_back_to_keyless_dart_rss() {
        let server = serve_sequence(vec![
            ok_response(
                "text/html",
                "<!DOCTYPE html><html><body>Bot verification challenge</body></html>",
            ),
            ok_response("text/xml; charset=utf-8", DART_TODAY_RSS_SAMPLE),
        ])
        .expect("server");
        let client = make_client(server.port());
        let collector = DomesticDisclosureCollector::new(
            KindDisclosureClient::new(Arc::clone(&client)),
            DartRssClient::new(Arc::clone(&client)),
            Some(DartDisclosureClient::new(client, None)),
        );
        let date = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(11),
        )
        .expect("date");

        let batch = runtime()
            .block_on(collector.collect(KindFetchWindow::StartupCatchUp, date))
            .expect("fallback rss only");
        assert_eq!(
            batch.route,
            DomesticDisclosureRoute::DartFallback(DartFallbackCoverage::RssOnly)
        );
        assert!(batch.kind_error.is_some());
        assert_eq!(batch.observations.len(), 1_usize);
        let wire_calls = server.finish().expect("finish");
        assert_eq!(wire_calls.len(), 2_usize);
    }
}
