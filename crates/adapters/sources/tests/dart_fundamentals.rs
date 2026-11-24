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
        Clock, CredentialSource, DataAvailability, FeedCursor, FinancialSource, FinancialUnit,
        Fundamentals, InformationError, NewsFeed, SecretValue,
    };
    use domain::{
        Amount, AmountInput, CalendarDate, CalendarYear, Currency, DayNumber, MonthNumber,
        Nanosecond, OffsetSeconds, Symbol, Timestamp, UnixSeconds, UntrustedText,
    };
    use sources::{
        DartCorpCode, DartCorpCodeMap, DartCorpRecord, DartFundamentalsClient, PressMacroSource,
        RssNewsFeed, SourceEndpoint, SourceHttpClient, SourceWait,
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
        let wait_copy = Arc::clone(&clock);
        Arc::new(
            SourceHttpClient::new(
                SourceEndpoint::local(port).expect("endpoint"),
                clock_copy,
                wait_copy,
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

    fn corp_map_with_samsung() -> Arc<DartCorpCodeMap> {
        let rec = DartCorpRecord {
            corp_code: DartCorpCode::try_new("00126380").expect("corp"),
            corp_name: UntrustedText::try_new("삼성전자").expect("name"),
            corp_eng_name: Some(UntrustedText::try_new("SAMSUNG ELECTRONICS").expect("eng")),
            stock_code: Some(Symbol::try_new("005930").expect("sym")),
            modify_date: date(2026, 11, 1),
        };
        Arc::new(DartCorpCodeMap::from_records(vec![rec], 0))
    }

    fn json_response(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json; charset=utf-8\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    const CFS_FIXTURE: &str = r#"{
        "status": "000",
        "message": "정상",
        "list": [
            {
                "sj_div": "BS",
                "account_nm": "자산총계",
                "thstrm_amount": "455,905,980,000,000",
                "currency": "KRW"
            },
            {
                "sj_div": "IS",
                "account_nm": "매출액",
                "thstrm_amount": "300,870,903,000,000",
                "currency": "KRW"
            },
            {
                "sj_div": "IS",
                "account_nm": "영업손익",
                "thstrm_amount": "-1,250,500,000",
                "currency": "KRW"
            },
            {
                "sj_div": "IS",
                "account_nm": "빈줄",
                "thstrm_amount": "-",
                "currency": "KRW"
            }
        ]
    }"#;

    const NO_DATA_FIXTURE: &str = r#"{"status": "013", "message": "조회된 데이타가 없습니다."}"#;

    const OFS_FIXTURE: &str = r#"{
        "status": "000",
        "message": "정상",
        "list": [
            {
                "sj_div": "IS",
                "account_nm": "매출액",
                "thstrm_amount": "210,000,000,000,000",
                "currency": "KRW"
            }
        ]
    }"#;

    #[test]
    fn missing_dart_key_disables_only_fundamentals_with_zero_connections() {
        let rt = runtime();
        let probe = ConnectionProbe::bind().expect("probe");
        let fundamentals = DartFundamentalsClient::new(
            make_client(probe.port().expect("port")),
            None,
            corp_map_with_samsung(),
        );
        assert!(format!("{fundamentals:?}").contains("configured: false"));
        let symbol = Symbol::try_new("005930").expect("sym");
        assert_eq!(
            rt.block_on(fundamentals.statements(&symbol, date(2025, 12, 31))),
            Ok(DataAvailability::NotConfigured)
        );
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );

        let rss_xml = r#"<?xml version="1.0" encoding="UTF-8"?><rss version="2.0"><channel><item><title>정상 뉴스</title><link>https://www.hankyung.com/article/1</link></item></channel></rss>"#;
        let rss_resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/xml\r\nConnection: close\r\n\r\n{rss_xml}",
            rss_xml.len()
        )
        .into_bytes();
        let server = serve_sequence(vec![rss_resp]).expect("server");
        let feed = RssNewsFeed::new(make_client(server.port()), PressMacroSource::Hankyung);
        let DataAvailability::Available((items, _cursor)) =
            rt.block_on(feed.new_items(FeedCursor::Start)).expect("rss")
        else {
            panic!("expected rss items");
        };
        assert_eq!(items.len(), 1_usize);
        let _calls = server.finish().expect("finish");
    }

    fn assert_annual_statements(annual: &[application::FinancialStatement]) {
        assert_eq!(annual.len(), 2_usize);
        assert_eq!(annual[0].lines().len(), 1_usize);
        assert_eq!(
            annual[0].lines()[0].label().as_untrusted_str(),
            "BS: 자산총계"
        );
        assert_eq!(
            annual[0].lines()[0].fact().value().amount(),
            &Amount::parse(&AmountInput::new("455905980000000")).expect("amt")
        );
        assert_eq!(
            annual[0].lines()[0].fact().unit(),
            FinancialUnit::Currency(Currency::Krw)
        );
        assert_eq!(annual[0].lines()[0].fact().source(), FinancialSource::Dart);
        assert_eq!(annual[1].lines().len(), 2_usize);
        assert_eq!(
            annual[1].lines()[1].fact().value().amount(),
            &Amount::parse(&AmountInput::new("-1250500000")).expect("neg")
        );
    }

    #[test]
    fn dart_fundamentals_parses_exact_decimals_and_falls_back_from_cfs_to_ofs() {
        let server = serve_sequence(vec![
            json_response(CFS_FIXTURE),
            json_response(NO_DATA_FIXTURE),
            json_response(OFS_FIXTURE),
        ])
        .expect("server");
        let client = DartFundamentalsClient::new(
            make_client(server.port()),
            Some(Arc::new(SecretValue::new(b"test-dart-key-123".to_vec()))),
            corp_map_with_samsung(),
        );
        let symbol = Symbol::try_new("005930").expect("sym");
        let rt = runtime();

        let DataAvailability::Available(annual) = rt
            .block_on(client.statements(&symbol, date(2025, 12, 31)))
            .expect("annual")
        else {
            panic!("expected annual statements");
        };
        assert_annual_statements(&annual);

        let DataAvailability::Available(q3_fallback) = rt
            .block_on(client.statements(&symbol, date(2026, 9, 30)))
            .expect("q3")
        else {
            panic!("expected ofs fallback");
        };
        assert_eq!(q3_fallback.len(), 1_usize);
        assert_eq!(
            q3_fallback[0].lines()[0].fact().value().amount(),
            &Amount::parse(&AmountInput::new("210000000000000")).expect("ofs amt")
        );

        let calls = server.finish().expect("finish");
        assert_eq!(calls.len(), 3_usize);
        let first_wire = std::str::from_utf8(&calls[0]).expect("utf8");
        assert!(
            first_wire.contains("reprt_code=11011&fs_div=CFS"),
            "{first_wire}"
        );
        assert!(
            first_wire.contains("crtfc_key=test-dart-key-123"),
            "{first_wire}"
        );
        let second_wire = std::str::from_utf8(&calls[1]).expect("utf8");
        assert!(
            second_wire.contains("reprt_code=11014&fs_div=CFS"),
            "{second_wire}"
        );
        let third_wire = std::str::from_utf8(&calls[2]).expect("utf8");
        assert!(
            third_wire.contains("reprt_code=11014&fs_div=OFS"),
            "{third_wire}"
        );
    }

    #[test]
    fn unmapped_symbol_non_quarter_month_and_invalid_key_behave_deterministically() {
        let rt = runtime();
        let probe = ConnectionProbe::bind().expect("probe");
        let probe_client = DartFundamentalsClient::new(
            make_client(probe.port().expect("port")),
            Some(Arc::new(SecretValue::new(b"valid-key".to_vec()))),
            corp_map_with_samsung(),
        );
        let unknown = Symbol::try_new("999999").expect("sym");
        assert_eq!(
            rt.block_on(probe_client.statements(&unknown, date(2025, 12, 31))),
            Ok(DataAvailability::NoData)
        );
        let samsung = Symbol::try_new("005930").expect("sym");
        assert_eq!(
            rt.block_on(probe_client.statements(&samsung, date(2026, 5, 31))),
            Ok(DataAvailability::NoData)
        );
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );

        let bad_key = r#"{"status": "010", "message": "등록되지 않은 키입니다."}"#;
        let server = serve_sequence(vec![json_response(bad_key)]).expect("server");
        let bad_client = DartFundamentalsClient::new(
            make_client(server.port()),
            Some(Arc::new(SecretValue::new(b"wrong-key".to_vec()))),
            corp_map_with_samsung(),
        );
        assert_eq!(
            rt.block_on(bad_client.statements(&samsung, date(2025, 12, 31))),
            Err(InformationError::InvalidCredential(CredentialSource::Dart))
        );
        let _calls = server.finish().expect("finish");
    }
}
