mod support;
#[cfg(test)]
mod tests {
    use application::{
        BrokerCapability, BrokerRead, Clock, EventBus, PortErrorKind, SecretKey, SecretStore,
        SecretValue, SystemPortError, WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
        TossTimestamp,
    };
    use domain::{
        AdjustmentBasis, CandlePeriod, CandleStatus, CommonShare, Currency, DisplayInstrumentName,
        InstrumentId, KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode,
        MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType,
        NxtTrading, PriceAdjustment, Symbol, TimeInterval, Timestamp, TradingSuspension,
    };
    use serde_json::{Value, json};
    use std::{future::Future, sync::Arc};
    use testkit::ScriptedHttpServer;

    struct Store;
    impl SecretStore for Store {
        fn load(
            &self,
            key: SecretKey,
        ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
            std::future::ready(Ok(Some(SecretValue::new(match key {
                SecretKey::TossClientId => b"fake-id-23".to_vec(),
                SecretKey::TossClientSecret => b"fake-key-29".to_vec(),
                SecretKey::TossAccessToken
                | SecretKey::DatabaseEncryptionKey
                | SecretKey::AuditHmacKey
                | SecretKey::DartApiKey
                | SecretKey::FredApiKey => panic!("unexpected key"),
            }))))
        }
        fn store(
            &self,
            _: SecretKey,
            _: SecretValue,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            std::future::ready(Err(SystemPortError::new(
                application::SystemPort::SecretStore,
                application::SystemFailure::InvalidData,
            )))
        }
        fn remove(
            &self,
            _: SecretKey,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            std::future::ready(Err(SystemPortError::new(
                application::SystemPort::SecretStore,
                application::SystemFailure::InvalidData,
            )))
        }
    }
    struct Renewal;
    impl TokenRenewal for Renewal {
        fn close(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready(Ok(()))
        }
        fn reconnect(
            &self,
            _: &SecretValue,
        ) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready(Ok(()))
        }
        fn resubscribe(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready(Ok(()))
        }
        fn reconcile_orders(
            &self,
            _: &SecretValue,
        ) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready(Ok(()))
        }
    }
    #[derive(Clone)]
    struct Time(Timestamp);
    impl Clock for Time {
        fn now(&self) -> Timestamp {
            self.0
        }
    }
    fn response(body: &Value) -> Vec<u8> {
        let text = body.to_string();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
            text.len()
        )
        .into_bytes()
    }
    fn serve(bodies: &[Value]) -> ScriptedHttpServer {
        let issued =
            json!({"access_token":"fake-access-37","token_type":"Bearer","expires_in":3_600_i32});
        ScriptedHttpServer::start(
            std::iter::once(response(&issued))
                .chain(bodies.iter().map(response))
                .collect(),
        )
        .expect("server")
    }
    fn at(text: &str) -> Timestamp {
        TossTimestamp::try_from(text)
            .expect("timestamp")
            .into_domain()
    }
    fn master() -> MasterInstrumentFacts {
        MasterInstrumentFacts::try_new(vec![
            MasterInstrument::try_new(MasterInstrumentRow {
                listing_market: MasterCode::try_new("KOSPI").expect("market"),
                symbol: Symbol::try_new("000660").expect("symbol"),
                instrument_id: InstrumentId::try_new("synthetic-000660").expect("id"),
                name: DisplayInstrumentName::try_new("합성 종목").expect("name"),
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
                observed_at: at("2026-03-25T12:00:00+09:00"),
            })
            .expect("master"),
        ])
        .expect("facts")
    }
    fn adapter_at(server: &ScriptedHttpServer, now: &str) -> TossAccountRead<Store, Time, Renewal> {
        let fake = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&fake),
            fake,
        )
        .expect("client");
        TossAccountRead::new(
            TokenManager::new(
                client,
                Store,
                Time(at(now)),
                Renewal,
                Arc::new(EventBus::default()),
            ),
            Time(at(now)),
            master(),
        )
    }
    fn adapter(server: &ScriptedHttpServer) -> TossAccountRead<Store, Time, Renewal> {
        adapter_at(server, "2027-01-15T17:00:00+09:00")
    }
    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }
    fn instrument() -> domain::QuoteInstrument {
        master().instruments()[0].quote_instrument().expect("quote")
    }
    fn window() -> TimeInterval {
        TimeInterval::try_new(
            at("2026-03-20T00:00:00+09:00"),
            at("2026-03-27T00:00:00+09:00"),
        )
        .expect("window")
    }
    fn krw(text: &str) -> domain::Money {
        domain::Money::parse(
            &domain::NumericInput::try_new(text).expect("n"),
            Currency::Krw,
        )
        .expect("krw")
    }
    fn row(stamp: &str, close: &str) -> Value {
        json!({
            "timestamp": stamp,
            "openPrice": close,
            "highPrice": close,
            "lowPrice": close,
            "closePrice": close,
            "volume": "1200",
            "currency": "KRW"
        })
    }
    fn session_stamped_page() -> Value {
        json!({
            "result": {
                "candles": [
                    row("2026-03-26T14:00:00+09:00", "201000"),
                    row("2026-03-25T14:00:00+09:00", "199500"),
                    row("2026-03-24T14:00:00+09:00", "198000"),
                ],
                "nextBefore": null
            }
        })
    }

    #[test]
    fn strict_daily_candles_without_a_session_supply_report_the_missing_capability() {
        let server = serve(&[session_stamped_page()]);
        let failure = run(adapter(&server).candles(
            &instrument(),
            window(),
            CandlePeriod::Daily,
            PriceAdjustment::Adjusted(AdjustmentBasis::new(window().end())),
        ))
        .expect_err("no sessions");
        assert_eq!(
            failure.kind(),
            PortErrorKind::Unsupported(BrokerCapability::DailyCandles)
        );
    }

    #[test]
    fn history_daily_candles_read_session_stamped_rows_without_sessions() {
        let server = serve(&[session_stamped_page()]);
        let candles =
            run(adapter(&server).history_daily_candles(&instrument(), window())).expect("history");
        let days: Vec<_> = candles
            .iter()
            .map(|candle| {
                (
                    candle.time().timestamp(),
                    candle.values().close().as_money(),
                    candle.status(),
                )
            })
            .collect();
        assert_eq!(
            days,
            vec![
                (
                    at("2026-03-24T00:00:00+09:00"),
                    krw("198000"),
                    CandleStatus::Finalized
                ),
                (
                    at("2026-03-25T00:00:00+09:00"),
                    krw("199500"),
                    CandleStatus::Finalized
                ),
                (
                    at("2026-03-26T00:00:00+09:00"),
                    krw("201000"),
                    CandleStatus::Finalized
                ),
            ]
        );
    }

    fn kr_day(date: &str) -> Value {
        json!({
                "date": date,
                "integrated": {
                    "preMarket": null,
                    "regularMarket": {
                        "startTime": format!("{date}T09:00:00+09:00"),
                        "endTime": format!("{date}T15:30:00+09:00")
                    },
                    "afterMarket": null
                }
        })
    }
    fn monday_calendar() -> Value {
        json!({
            "result": {
                "previousBusinessDay": kr_day("2026-03-20"),
                "today": kr_day("2026-03-23"),
                "nextBusinessDay": kr_day("2026-03-24")
            }
        })
    }
    fn page(rows: &[Value]) -> Value {
        json!({ "result": { "candles": rows, "nextBefore": null } })
    }
    fn span(start: &str, end: &str) -> TimeInterval {
        TimeInterval::try_new(at(start), at(end)).expect("window")
    }
    fn statuses(candles: &[domain::Candle]) -> Vec<(Timestamp, CandleStatus)> {
        candles
            .iter()
            .map(|candle| (candle.time().timestamp(), candle.status()))
            .collect()
    }

    #[test]
    fn live_chart_candles_include_todays_open_candle_during_the_session() {
        let server = serve(&[
            page(&[
                row("2026-03-23T14:00:00+09:00", "201000"),
                row("2026-03-20T14:00:00+09:00", "199500"),
                row("2026-03-19T14:00:00+09:00", "198000"),
            ]),
            monday_calendar(),
        ]);
        let reader = adapter_at(&server, "2026-03-23T10:00:00+09:00").with_live_market_sessions();
        let candles = run(reader.candles(
            &instrument(),
            span("2026-03-18T00:00:00+09:00", "2026-03-23T10:00:00+09:00"),
            CandlePeriod::Daily,
            PriceAdjustment::Raw,
        ))
        .expect("chart candles");
        assert_eq!(
            statuses(&candles),
            vec![
                (at("2026-03-19T00:00:00+09:00"), CandleStatus::Finalized),
                (at("2026-03-20T00:00:00+09:00"), CandleStatus::Finalized),
                (at("2026-03-23T00:00:00+09:00"), CandleStatus::InProgress),
            ]
        );
        server.finish().expect("served");
    }

    #[test]
    fn live_scoring_reads_todays_closing_candle_after_the_close() {
        let today = || page(&[row("2026-03-23T14:00:00+09:00", "201000")]);
        let server = serve(&[today(), monday_calendar(), today()]);
        let reader = adapter_at(&server, "2026-03-23T16:00:00+09:00").with_live_market_sessions();
        let window = span("2026-03-22T00:00:00+09:00", "2026-03-23T16:00:00+09:00");
        let adjusted = run(reader.candles(
            &instrument(),
            window,
            CandlePeriod::Daily,
            PriceAdjustment::Adjusted(AdjustmentBasis::new(window.end())),
        ))
        .expect("adjusted");
        let raw = run(reader.candles(
            &instrument(),
            window,
            CandlePeriod::Daily,
            PriceAdjustment::Raw,
        ))
        .expect("raw");
        let closing = vec![(at("2026-03-23T00:00:00+09:00"), CandleStatus::Finalized)];
        assert_eq!(statuses(&adjusted), closing);
        assert_eq!(statuses(&raw), closing);
        assert_eq!(raw[0].values().close().as_money(), krw("201000"));
        server.finish().expect("served");
    }

    #[test]
    fn live_daily_candles_still_reject_a_future_day() {
        let server = serve(&[page(&[row("2026-03-24T14:00:00+09:00", "201000")])]);
        let reader = adapter_at(&server, "2026-03-23T10:00:00+09:00").with_live_market_sessions();
        let failure = run(reader.candles(
            &instrument(),
            span("2026-03-18T00:00:00+09:00", "2026-03-23T10:00:00+09:00"),
            CandlePeriod::Daily,
            PriceAdjustment::Raw,
        ))
        .expect_err("future day");
        assert_eq!(
            failure.kind(),
            PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
    }
}
