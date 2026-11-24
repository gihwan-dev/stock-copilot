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
        Clock, CredentialSource, DataAvailability, FeedCursor, FeedSource, InformationError,
        NewsFeed, PublicationTime, ResetTime, ResponseMismatch, SecretValue,
    };
    use domain::{
        CalendarDate, CalendarYear, DayNumber, DisclosureSource, MonthNumber, Nanosecond,
        OffsetSeconds, Symbol, Timestamp, UnixSeconds,
    };
    use sources::{
        DartAmendmentKind, DartCorpClass, DartCorrectionStatus, DartDisclosureClient,
        DartDisclosureFeed, PressMacroSource, RssNewsFeed, SourceEndpoint, SourceHttpClient,
        SourceWait, group_dart_amendments,
    };
    use testkit::{ConnectionObservation, ConnectionProbe, serve_sequence};

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

    fn date(year: i32, month: u8, day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(year),
            MonthNumber::new(month),
            DayNumber::new(day),
        )
        .expect("date")
    }

    fn json_response(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json; charset=utf-8\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    #[test]
    fn missing_dart_key_returns_not_configured_with_zero_connections_while_rss_still_works() {
        let rt = runtime();
        let probe = ConnectionProbe::bind().expect("probe");
        let dart = DartDisclosureClient::new(make_client(probe.port().expect("port")), None);
        assert!(format!("{dart:?}").contains("configured: false"));
        assert_eq!(
            rt.block_on(dart.fetch_page(date(2026, 11, 11), date(2026, 11, 11), 1)),
            Ok(DataAvailability::NotConfigured)
        );
        let feed = DartDisclosureFeed::new(dart, date(2026, 11, 11));
        assert_eq!(
            feed.source(),
            FeedSource::Disclosure(DisclosureSource::Dart)
        );
        assert_eq!(
            rt.block_on(feed.new_items(FeedCursor::Start)),
            Ok(DataAvailability::NotConfigured)
        );
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );

        let rss_xml = r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><item><title>정상 RSS</title><link>https://www.mk.co.kr/news/stock/1</link></item></channel></rss>"#;
        let rss_resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/xml\r\nConnection: close\r\n\r\n{rss_xml}",
            rss_xml.len()
        )
        .into_bytes();
        let server = serve_sequence(vec![rss_resp]).expect("server");
        let rss = RssNewsFeed::new(make_client(server.port()), PressMacroSource::Maekyung);
        let DataAvailability::Available((items, _cursor)) =
            rt.block_on(rss.new_items(FeedCursor::Start)).expect("rss")
        else {
            panic!("expected rss items");
        };
        assert_eq!(items.len(), 1);
        let _captured = server.finish().expect("captured");
    }

    const DART_LIST_FIXTURE: &str = r#"{
        "status": "000",
        "message": "정상",
        "page_no": 1,
        "page_count": 100,
        "total_count": 3,
        "total_page": 1,
        "list": [
            {
                "corp_cls": "Y",
                "corp_name": "삼성전자",
                "corp_code": "00126380",
                "stock_code": "005930",
                "report_nm": "단일판매ㆍ공급계약체결",
                "rcept_no": "20261111800100",
                "flr_nm": "삼성전자",
                "rcept_dt": "20261111",
                "rm": "유정"
            },
            {
                "corp_cls": "Y",
                "corp_name": "삼성전자",
                "corp_code": "00126380",
                "stock_code": "005930",
                "report_nm": "[기재정정]단일판매ㆍ공급계약체결",
                "rcept_no": "20261111800200",
                "flr_nm": "삼성전자",
                "rcept_dt": "20261111",
                "rm": "유"
            },
            {
                "corp_cls": "K",
                "corp_name": "알파종목",
                "corp_code": "00999999",
                "stock_code": "0153R0",
                "report_nm": "[첨부정정]유상증자결정",
                "rcept_no": "20261111900300",
                "flr_nm": "알파종목",
                "rcept_dt": "20261111",
                "rm": "철"
            }
        ]
    }"#;

    #[test]
    fn dart_list_parses_amendments_alphanumeric_stock_codes_and_groups_corrections() {
        let rt = runtime();
        let server = serve_sequence(vec![json_response(DART_LIST_FIXTURE)]).expect("server");
        let secret = Arc::new(SecretValue::new(b"dart-test-key-0123456789".to_vec()));
        let dart = DartDisclosureClient::new(make_client(server.port()), Some(secret));
        assert!(!format!("{dart:?}").contains("dart-test-key"));

        let DataAvailability::Available(page) = rt
            .block_on(dart.fetch_page(date(2026, 11, 11), date(2026, 11, 11), 1))
            .expect("page")
        else {
            panic!("expected page");
        };
        assert_eq!(page.total_count(), 3);
        assert_eq!(page.entries().len(), 3);

        let original = page.entries().first().expect("orig");
        let amended = page.entries().get(1).expect("amended");
        let withdrawn = page.entries().get(2).expect("withdrawn");
        assert_eq!(original.corp_class(), &DartCorpClass::Kospi);
        assert_eq!(
            original.correction(),
            DartCorrectionStatus::SupersededByCorrection
        );
        assert_eq!(
            amended.correction(),
            DartCorrectionStatus::Amended(DartAmendmentKind::EntryCorrection)
        );
        assert!(amended.correction().is_amendment());
        assert_eq!(
            original.correction_group_key(),
            amended.correction_group_key()
        );
        assert_eq!(
            withdrawn.stock_code(),
            Some(&Symbol::try_new("0153R0").expect("sym"))
        );
        assert!(withdrawn.correction().is_withdrawn());

        let groups = group_dart_amendments(page.entries());
        assert_eq!(
            groups
                .get(&original.correction_group_key())
                .expect("group")
                .len(),
            2
        );
        let captured = server.finish().expect("captured");
        let raw_call = String::from_utf8_lossy(captured.first().expect("req"));
        assert!(raw_call.contains("crtfc_key=dart-test-key-0123456789"));
        assert!(raw_call.contains("bgn_de=20261111&end_de=20261111"));
    }

    #[test]
    fn dart_status_codes_and_feed_timestamp_contract_are_enforced() {
        let rt = runtime();
        let secret = Arc::new(SecretValue::new(b"dart-key-ok".to_vec()));
        for (body, expected) in [
            (
                r#"{"status":"013","message":"조회된 데이타가 없습니다."}"#,
                Ok(DataAvailability::NoData),
            ),
            (
                r#"{"status":"010","message":"등록되지 않은 키입니다."}"#,
                Err(InformationError::InvalidCredential(CredentialSource::Dart)),
            ),
            (
                r#"{"status":"020","message":"요청 제한 초과"}"#,
                Err(InformationError::Unavailable(ResetTime::NotProvided)),
            ),
            (
                "not-json",
                Err(InformationError::InvalidResponse(ResponseMismatch::Schema)),
            ),
        ] {
            let server = serve_sequence(vec![json_response(body)]).expect("server");
            let dart =
                DartDisclosureClient::new(make_client(server.port()), Some(Arc::clone(&secret)));
            assert_eq!(
                rt.block_on(dart.fetch_date_as_feed(date(2026, 11, 11), date(2026, 11, 11))),
                expected
            );
            let _captured = server.finish().expect("captured");
        }

        let single = r#"{"status":"000","list":[{"corp_cls":"Y","corp_name":"삼성전자","corp_code":"00126380","stock_code":"005930","report_nm":"사업보고서","rcept_no":"20261111000001","flr_nm":"삼성전자","rcept_dt":"20261111","rm":""}]}"#;
        let server = serve_sequence(vec![json_response(single)]).expect("server");
        let dart = DartDisclosureClient::new(make_client(server.port()), Some(secret));
        let DataAvailability::Available(obs) = rt
            .block_on(dart.fetch_date_as_feed(date(2026, 11, 11), date(2026, 11, 11)))
            .expect("feed")
        else {
            panic!("expected obs");
        };
        assert_eq!(
            obs.first().expect("obs").published_at(),
            PublicationTime::NotProvided
        );
        let _captured = server.finish().expect("captured");
    }
}
