#[cfg(test)]
mod tests {
    use std::future::{Future, ready};
    use std::io::Write;
    use std::pin::Pin;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use std::time::Duration;

    use application::{
        Clock, CredentialSource, DataAvailability, InformationError, ResponseMismatch, SecretValue,
    };
    use domain::{
        CalendarDate, CalendarYear, CommonShare, Currency, DayNumber, DisplayInstrumentName,
        InstrumentId, KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode,
        MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType,
        MonthNumber, Nanosecond, NxtTrading, OffsetSeconds, Symbol, Timestamp, TradingSuspension,
        UnixSeconds, UntrustedText,
    };
    use flate2::Compression;
    use flate2::write::DeflateEncoder;
    use sources::{
        DartCorpCode, DartCorpMapClient, DartCorpRecord, DartMatchFailureReason, SourceEndpoint,
        SourceHttpClient, SourceWait, extract_corp_code_xml, parse_corp_code_xml,
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

    fn build_single_entry_zip(file_name: &str, data: &[u8], compression_code: u16) -> Vec<u8> {
        let compressed = if compression_code == 8_u16 {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(data).expect("write deflate");
            encoder.finish().expect("finish deflate")
        } else {
            data.to_vec()
        };
        let name_bytes = file_name.as_bytes();
        let name_len = u16::try_from(name_bytes.len()).expect("name len");
        let comp_len = u32::try_from(compressed.len()).expect("comp len");
        let uncomp_len = u32::try_from(data.len()).expect("uncomp len");

        let mut out = Vec::new();
        out.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&compression_code.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&comp_len.to_le_bytes());
        out.extend_from_slice(&uncomp_len.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(&compressed);

        let cd_offset = u32::try_from(out.len()).expect("cd offset");
        out.extend_from_slice(&[0x50, 0x4B, 0x01, 0x02]);
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&20_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&compression_code.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&comp_len.to_le_bytes());
        out.extend_from_slice(&uncomp_len.to_le_bytes());
        out.extend_from_slice(&name_len.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(name_bytes);
        let cd_size = u32::try_from(out.len())
            .expect("end")
            .saturating_sub(cd_offset);

        out.extend_from_slice(&[0x50, 0x4B, 0x05, 0x06]);
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out
    }

    fn binary_response(body: &[u8]) -> Vec<u8> {
        let mut out = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/zip\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        out.extend_from_slice(body);
        out
    }

    fn kr_master(id: &str, symbol: &str, name: &str) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("market"),
            symbol: Symbol::try_new(symbol).expect("sym"),
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
            observed_at: Timestamp::from_parts(
                UnixSeconds::new(1_800_000_000),
                Nanosecond::new(0),
                OffsetSeconds::new(32_400),
            )
            .expect("ts"),
        })
        .expect("master")
    }

    fn us_master(id: &str, symbol: &str, name: &str) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("market"),
            symbol: Symbol::try_new(symbol).expect("sym"),
            instrument_id: InstrumentId::try_new(id).expect("id"),
            name: DisplayInstrumentName::try_new(name).expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("FOREIGN_STOCK").expect("type"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: Timestamp::from_parts(
                UnixSeconds::new(1_800_000_000),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("ts"),
        })
        .expect("master")
    }

    const CORP_CODE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<result>
  <list>
    <corp_code>00126380</corp_code>
    <corp_name>삼성전자</corp_name>
    <corp_eng_name>Samsung Electronics</corp_eng_name>
    <stock_code>005930</stock_code>
    <modify_date>20261110</modify_date>
  </list>
  <list>
    <corp_code>00999901</corp_code>
    <corp_name>알파종목</corp_name>
    <corp_eng_name>Alpha Corp</corp_eng_name>
    <stock_code>0153R0</stock_code>
    <modify_date>20261110</modify_date>
  </list>
  <list>
    <corp_code>00888801</corp_code>
    <corp_name>중복갑</corp_name>
    <corp_eng_name></corp_eng_name>
    <stock_code>000660</stock_code>
    <modify_date>20261110</modify_date>
  </list>
  <list>
    <corp_code>00888802</corp_code>
    <corp_name>중복을</corp_name>
    <corp_eng_name></corp_eng_name>
    <stock_code>000660</stock_code>
    <modify_date>20261110</modify_date>
  </list>
  <list>
    <corp_code>00777701</corp_code>
    <corp_name>비상장법인</corp_name>
    <corp_eng_name></corp_eng_name>
    <stock_code> </stock_code>
    <modify_date>20261101</modify_date>
  </list>
</result>"#;

    #[test]
    fn deflated_corp_code_zip_parses_and_lists_all_unmatched_master_instruments() {
        let rt = runtime();
        let zip = build_single_entry_zip("CORPCODE.xml", CORP_CODE_XML.as_bytes(), 8_u16);
        let server = serve_sequence(vec![binary_response(&zip)]).expect("server");
        let secret = Arc::new(SecretValue::new(b"dart-corp-key".to_vec()));
        let client = DartCorpMapClient::new(make_client(server.port()), Some(secret));

        let DataAvailability::Available(map) =
            rt.block_on(client.fetch_corp_code_map()).expect("fetch")
        else {
            panic!("expected corp code map");
        };
        assert_eq!(map.len(), 5);
        assert_eq!(
            map.corp_code_for_symbol(&Symbol::try_new("005930").expect("sym")),
            Some(&DartCorpCode::try_new("00126380").expect("code"))
        );
        assert_eq!(
            map.corp_code_for_symbol(&Symbol::try_new("0153R0").expect("sym")),
            Some(&DartCorpCode::try_new("00999901").expect("code"))
        );
        assert_eq!(
            map.corp_code_for_symbol(&Symbol::try_new("000660").expect("sym")),
            None
        );

        let master = MasterInstrumentFacts::try_new(vec![
            kr_master("kr-sec", "005930", "삼성전자"),
            kr_master("kr-alpha", "0153R0", "알파종목"),
            kr_master("kr-dup", "000660", "SK하이닉스"),
            kr_master("kr-miss", "035420", "NAVER"),
            us_master("us-aapl", "AAPL", "Apple"),
        ])
        .expect("master");
        let report = map.reconcile_with_master(&master);
        assert_eq!(report.matched.len(), 2);
        assert_eq!(report.unmatched.len(), 2);
        assert_eq!(
            report.unmatched.first().expect("first").reason,
            DartMatchFailureReason::AmbiguousStockCode
        );
        assert_eq!(
            report.unmatched.get(1).expect("second").reason,
            DartMatchFailureReason::MissingInDart
        );
        let _captured = server.finish().expect("captured");
    }

    #[test]
    fn incremental_merge_respects_modify_date() {
        let mut map = parse_corp_code_xml(CORP_CODE_XML.as_bytes()).expect("parse");
        let newer = DartCorpRecord {
            corp_code: DartCorpCode::try_new("00126380").expect("code"),
            corp_name: UntrustedText::try_new("삼성전자주식회사").expect("name"),
            corp_eng_name: None,
            stock_code: Some(Symbol::try_new("005930").expect("sym")),
            modify_date: CalendarDate::from_parts(
                CalendarYear::new(2026),
                MonthNumber::new(11),
                DayNumber::new(12),
            )
            .expect("date"),
        };
        map.merge_updates(vec![newer]);
        assert_eq!(
            map.record_for_corp_code(&DartCorpCode::try_new("00126380").expect("code"))
                .expect("rec")
                .corp_name
                .as_untrusted_str(),
            "삼성전자주식회사"
        );
    }

    #[test]
    fn missing_key_corrupt_zip_and_dart_xml_error_are_handled_safely() {
        let rt = runtime();
        let probe = ConnectionProbe::bind().expect("probe");
        let unconfigured = DartCorpMapClient::new(make_client(probe.port().expect("port")), None);
        assert_eq!(
            rt.block_on(unconfigured.fetch_corp_code_map()),
            Ok(DataAvailability::NotConfigured)
        );
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );

        let stored_zip = build_single_entry_zip("CORPCODE.xml", CORP_CODE_XML.as_bytes(), 0_u16);
        let extracted = extract_corp_code_xml(&stored_zip).expect("stored zip");
        assert_eq!(parse_corp_code_xml(&extracted).expect("parse").len(), 5);

        let bad_zip = b"PK\x03\x04truncated-corrupt-zip";
        assert_eq!(
            extract_corp_code_xml(bad_zip),
            Err(InformationError::InvalidResponse(ResponseMismatch::Schema))
        );

        let err_xml = b"<?xml version=\"1.0\"?><result><status>010</status><message>bad key</message></result>";
        assert_eq!(
            parse_corp_code_xml(err_xml),
            Err(InformationError::InvalidCredential(CredentialSource::Dart))
        );
    }
}
