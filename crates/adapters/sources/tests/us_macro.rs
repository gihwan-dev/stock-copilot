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
        Clock, DataAvailability, InformationError, MacroSeries, MacroSeriesId, MacroSource,
        MacroUnit, SecretValue,
    };
    use domain::{
        Amount, AmountInput, CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond,
        OffsetSeconds, Timestamp, UnixSeconds,
    };
    use sources::{
        FredSeries, SourceEndpoint, SourceHttpClient, SourceWait, TreasuryTenor, UsMacroClient,
        parse_treasury_yield_xml,
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
        let wait = Arc::clone(&clock);
        Arc::new(
            SourceHttpClient::new(
                SourceEndpoint::local(port).expect("endpoint"),
                clock,
                wait,
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

    fn response(content_type: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn amount(raw: &str) -> Amount {
        Amount::parse(&AmountInput::new(raw)).expect("amount")
    }

    const TREASURY_FIXTURE: &str = r#"<?xml version="1.0" encoding="utf-8" standalone="yes"?>
<feed xml:base="https://home.treasury.gov/resource-center/data-chart-center/interest-rates/" xmlns="http://www.w3.org/2005/Atom" xmlns:d="http://schemas.microsoft.com/ado/2007/08/dataservices" xmlns:m="http://schemas.microsoft.com/ado/2007/08/dataservices/metadata">
  <title type="text">DailyTreasuryYieldCurveRateData</title>
  <entry>
    <content type="application/xml">
      <m:properties>
        <d:Id m:type="Edm.Int32">9001</d:Id>
        <d:NEW_DATE m:type="Edm.DateTime">2026-11-02T00:00:00</d:NEW_DATE>
        <d:BC_1MONTH m:type="Edm.Double">4.05</d:BC_1MONTH>
        <d:BC_10YEAR m:type="Edm.Double">4.21</d:BC_10YEAR>
      </m:properties>
    </content>
  </entry>
  <entry>
    <content type="application/xml">
      <m:properties>
        <d:Id m:type="Edm.Int32">9002</d:Id>
        <d:NEW_DATE m:type="Edm.DateTime">2026-11-03T00:00:00</d:NEW_DATE>
        <d:BC_1MONTH m:type="Edm.Double">4.04</d:BC_1MONTH>
        <d:BC_10YEAR m:type="Edm.Double">4.18</d:BC_10YEAR>
      </m:properties>
    </content>
  </entry>
  <entry>
    <content type="application/xml">
      <m:properties>
        <d:Id m:type="Edm.Int32">9003</d:Id>
        <d:NEW_DATE m:type="Edm.DateTime">2026-11-04T00:00:00</d:NEW_DATE>
        <d:BC_1MONTH m:type="Edm.Double">4.03</d:BC_1MONTH>
        <d:BC_10YEAR m:null="true" />
      </m:properties>
    </content>
  </entry>
</feed>"#;

    const FRED_FIXTURE: &str = r#"{"realtime_start":"2026-11-20","realtime_end":"2026-11-20","observation_start":"2026-11-02","observation_end":"2026-11-04","units":"lin","output_type":1,"file_type":"json","order_by":"observation_date","sort_order":"asc","count":3,"offset":0,"limit":100000,"observations":[{"realtime_start":"2026-11-20","realtime_end":"2026-11-20","date":"2026-11-02","value":"6012.34"},{"realtime_start":"2026-11-20","realtime_end":"2026-11-20","date":"2026-11-03","value":"."},{"realtime_start":"2026-11-20","realtime_end":"2026-11-20","date":"2026-11-04","value":"6050.10"}]}"#;

    #[test]
    fn treasury_xml_yields_percent_values_within_the_requested_range() {
        let server =
            serve_sequence(vec![response("application/xml", TREASURY_FIXTURE)]).expect("server");
        let client = UsMacroClient::new(make_client(server.port()), None);
        let series = MacroSeriesId::try_new(TreasuryTenor::TenYear.series_id()).expect("id");
        let DataAvailability::Available(values) = runtime()
            .block_on(client.daily_values(&series, date(2026, 11, 3), date(2026, 11, 30)))
            .expect("treasury")
        else {
            panic!("expected values");
        };
        assert_eq!(values.len(), 1_usize);
        assert_eq!(values[0].series(), &series);
        assert_eq!(values[0].fact().as_of(), date(2026, 11, 3));
        assert_eq!(values[0].fact().value().amount(), &amount("4.18"));
        assert_eq!(values[0].fact().unit(), MacroUnit::Percent);
        assert_eq!(values[0].fact().source(), &MacroSource::UsTreasury);
        let calls = server.finish().expect("finish");
        assert_eq!(calls.len(), 1_usize);
        let wire = std::str::from_utf8(&calls[0]).expect("utf8");
        assert!(
            wire.contains("data=daily_treasury_yield_curve&field_tdr_date_value=2026"),
            "{wire}"
        );
        assert!(!wire.contains("api_key"), "{wire}");
    }

    #[test]
    fn treasury_rejects_broken_documents_and_skips_null_cells() {
        let start = date(2026, 1, 1);
        let end = date(2026, 12, 31);
        let one_month = parse_treasury_yield_xml(
            TREASURY_FIXTURE.as_bytes(),
            TreasuryTenor::OneMonth,
            start,
            end,
        )
        .expect("parse");
        assert_eq!(one_month.len(), 3_usize);
        let ten_year = parse_treasury_yield_xml(
            TREASURY_FIXTURE.as_bytes(),
            TreasuryTenor::TenYear,
            start,
            end,
        )
        .expect("parse");
        assert_eq!(ten_year.len(), 2_usize);
        for broken in [
            "<html><body>blocked</body></html>",
            "not xml",
            "<feed><entry><m:properties><d:NEW_DATE>2026-11-02</d:NEW_DATE>",
            "<feed><m:properties><d:NEW_DATE>bad</d:NEW_DATE><d:BC_10YEAR>1</d:BC_10YEAR></m:properties></feed>",
            "<feed><m:properties><d:NEW_DATE>2026-11-02</d:NEW_DATE><d:BC_10YEAR>x1</d:BC_10YEAR></m:properties></feed>",
        ] {
            assert_eq!(
                parse_treasury_yield_xml(broken.as_bytes(), TreasuryTenor::TenYear, start, end),
                Err(InformationError::InvalidResponse(
                    application::ResponseMismatch::Schema
                )),
                "{broken}"
            );
        }
    }

    #[test]
    fn fred_without_key_makes_no_connection_and_reports_not_configured() {
        let probe = ConnectionProbe::bind().expect("probe");
        let client = UsMacroClient::new(make_client(probe.port().expect("port")), None);
        assert!(format!("{client:?}").contains("fred_configured: false"));
        let rt = runtime();
        let sp500 = MacroSeriesId::try_new(FredSeries::Sp500.series_id()).expect("id");
        assert_eq!(
            rt.block_on(client.daily_values(&sp500, date(2026, 11, 2), date(2026, 11, 4))),
            Ok(DataAvailability::NotConfigured)
        );
        let unknown = MacroSeriesId::try_new("GOLDAMGBD228NLBM").expect("id");
        assert_eq!(
            rt.block_on(client.daily_values(&unknown, date(2026, 11, 2), date(2026, 11, 4))),
            Ok(DataAvailability::NoData)
        );
        let tenor = MacroSeriesId::try_new("UST_10Y").expect("id");
        assert_eq!(
            rt.block_on(client.daily_values(&tenor, date(2026, 11, 4), date(2026, 11, 2))),
            Ok(DataAvailability::NoData)
        );
        assert_eq!(
            probe.observe().expect("probe"),
            ConnectionObservation::NoConnection
        );
    }

    #[test]
    fn fred_with_key_sends_the_key_and_skips_missing_dots() {
        let server =
            serve_sequence(vec![response("application/json", FRED_FIXTURE)]).expect("server");
        let client = UsMacroClient::new(
            make_client(server.port()),
            Some(Arc::new(SecretValue::new(b"fredkey0123".to_vec()))),
        );
        assert!(!format!("{client:?}").contains("fredkey0123"));
        let sp500 = MacroSeriesId::try_new("SP500").expect("id");
        let DataAvailability::Available(values) = runtime()
            .block_on(client.daily_values(&sp500, date(2026, 11, 2), date(2026, 11, 4)))
            .expect("fred")
        else {
            panic!("expected fred values");
        };
        assert_eq!(values.len(), 2_usize);
        assert_eq!(values[0].fact().value().amount(), &amount("6012.34"));
        assert_eq!(values[1].fact().as_of(), date(2026, 11, 4));
        assert_eq!(values[1].fact().unit(), MacroUnit::IndexPoints);
        assert_eq!(values[1].fact().source(), &MacroSource::Fred);
        let calls = server.finish().expect("finish");
        let wire = std::str::from_utf8(&calls[0]).expect("utf8");
        assert!(wire.contains("series_id=SP500"), "{wire}");
        assert!(
            wire.contains("observation_start=2026-11-02&observation_end=2026-11-04"),
            "{wire}"
        );
        assert!(wire.contains("api_key=fredkey0123"), "{wire}");
    }

    #[test]
    fn fred_server_error_and_malformed_json_are_rejected() {
        let failure =
            b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                .to_vec();
        let server = serve_sequence(vec![
            failure,
            response(
                "application/json",
                r#"{"observations":[{"date":"2026-11-02","value":"abc"}]}"#,
            ),
        ])
        .expect("server");
        let client = UsMacroClient::new(
            make_client(server.port()),
            Some(Arc::new(SecretValue::new(b"fredkey0123".to_vec()))),
        );
        let rt = runtime();
        let vix = MacroSeriesId::try_new("VIXCLS").expect("id");
        assert_eq!(
            rt.block_on(client.daily_values(&vix, date(2026, 11, 2), date(2026, 11, 4))),
            Err(InformationError::Unavailable(
                application::ResetTime::NotProvided
            ))
        );
        assert_eq!(
            rt.block_on(client.daily_values(&vix, date(2026, 11, 2), date(2026, 11, 4))),
            Err(InformationError::InvalidResponse(
                application::ResponseMismatch::Schema
            ))
        );
        let _calls = server.finish().expect("finish");
    }
}
