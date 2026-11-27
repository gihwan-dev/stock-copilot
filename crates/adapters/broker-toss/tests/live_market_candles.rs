mod support;
#[cfg(test)]
mod tests {
    use application::BrokerRead;
    use application::{
        Clock, EventBus, MarketInfo, PortErrorKind, ResponseMismatch, SecretKey, SecretStore,
        SecretValue, SystemPortError, WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
        TossTimestamp,
    };
    use domain::{
        AdjustmentBasis, CandlePeriod, CandleStatus, CommonShare, Currency, DisplayInstrumentName,
        IndicatorCandleStatus, IndicatorCode, InstrumentId, KoreanMasterDetail, LiquidationTrading,
        ListingStatus, MasterCode, MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow,
        MasterSecurityType, NxtTrading, PriceAdjustment, Symbol, TimeInterval, Timestamp,
        TradingSuspension,
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
    fn page(rows: &[Value]) -> Value {
        json!({ "result": { "candles": rows, "nextBefore": null } })
    }
    fn span(start: &str, end: &str) -> TimeInterval {
        TimeInterval::try_new(at(start), at(end)).expect("window")
    }
    fn adjusted(window: TimeInterval) -> PriceAdjustment {
        PriceAdjustment::Adjusted(AdjustmentBasis::new(window.end()))
    }

    #[test]
    fn adjusted_daily_candles_across_a_split_keep_adjusted_prices() {
        let server = serve(&[page(&[
            row("2026-03-26T14:00:00+09:00", "10300"),
            row("2026-03-25T14:00:00+09:00", "10200"),
            row("2026-03-24T14:00:00+09:00", "10000"),
        ])]);
        let reader = adapter_at(&server, "2026-03-27T10:00:00+09:00").with_live_market_sessions();
        let window = span("2026-03-24T00:00:00+09:00", "2026-03-27T10:00:00+09:00");
        let candles =
            run(reader.candles(&instrument(), window, CandlePeriod::Daily, adjusted(window)))
                .expect("adjusted candles");
        let closes: Vec<_> = candles
            .iter()
            .map(|candle| {
                (
                    candle.values().close().as_money(),
                    candle.adjustment(),
                    candle.status(),
                )
            })
            .collect();
        assert_eq!(
            closes,
            vec![
                (krw("10000"), adjusted(window), CandleStatus::Finalized),
                (krw("10200"), adjusted(window), CandleStatus::Finalized),
                (krw("10300"), adjusted(window), CandleStatus::Finalized),
            ]
        );
        let captured = String::from_utf8_lossy(&server.finish().expect("served")).into_owned();
        assert!(captured.contains("adjusted=true"));
        assert!(!captured.contains("market-calendar"));
    }

    #[test]
    fn unadjusted_split_jump_is_rejected_on_the_adjusted_path() {
        let server = serve(&[page(&[
            row("2026-03-26T14:00:00+09:00", "10300"),
            row("2026-03-25T14:00:00+09:00", "51000"),
            row("2026-03-24T14:00:00+09:00", "50000"),
        ])]);
        let reader = adapter_at(&server, "2026-03-27T10:00:00+09:00").with_live_market_sessions();
        let window = span("2026-03-24T00:00:00+09:00", "2026-03-27T10:00:00+09:00");
        let failure =
            run(reader.candles(&instrument(), window, CandlePeriod::Daily, adjusted(window)))
                .expect_err("raw split jump");
        assert_eq!(
            failure.kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        );
    }

    fn index_row(stamp: &str, close: &str) -> Value {
        json!({
            "timestamp": stamp,
            "openPrice": close,
            "highPrice": close,
            "lowPrice": close,
            "closePrice": close,
            "volume": "542000000"
        })
    }
    fn open_day(date: &str) -> Value {
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
    fn calendar(previous: Value, today: Value, next: Value) -> Value {
        let mut page = json!({ "result": {} });
        page["result"]["previousBusinessDay"] = previous;
        page["result"]["today"] = today;
        page["result"]["nextBusinessDay"] = next;
        page
    }
    fn holiday_calendar() -> Value {
        calendar(
            open_day("2026-06-09"),
            json!({ "date": "2026-06-10", "integrated": null }),
            open_day("2026-06-11"),
        )
    }
    fn monday_calendar() -> Value {
        calendar(
            open_day("2026-06-12"),
            open_day("2026-06-15"),
            open_day("2026-06-16"),
        )
    }
    fn index_page() -> Value {
        page(&[
            index_row("2026-06-15T09:00:00+09:00", "2830.5"),
            index_row("2026-06-12T09:00:00+09:00", "2820.1"),
            index_row("2026-06-11T09:00:00+09:00", "2812.45"),
            index_row("2026-06-09T09:00:00+09:00", "2798.1"),
        ])
    }
    fn index_statuses(
        reader: &TossAccountRead<Store, Time, Renewal>,
    ) -> Vec<(Timestamp, IndicatorCandleStatus)> {
        run(reader.indicator_candles(
            IndicatorCode::Kospi,
            span("2026-06-08T00:00:00+09:00", "2026-06-15T23:00:00+09:00"),
            CandlePeriod::Daily,
        ))
        .expect("index candles")
        .iter()
        .map(|candle| {
            (
                candle.observation().server_at().expect("time"),
                candle.status(),
            )
        })
        .collect()
    }

    #[test]
    fn live_index_candles_accept_a_weekday_holiday_and_mark_today_by_its_session() {
        let server = serve(&[index_page(), holiday_calendar(), monday_calendar()]);
        let reader = adapter_at(&server, "2026-06-15T10:00:00+09:00").with_live_market_sessions();
        assert_eq!(
            index_statuses(&reader),
            vec![
                (
                    at("2026-06-09T09:00:00+09:00"),
                    IndicatorCandleStatus::Finalized
                ),
                (
                    at("2026-06-11T09:00:00+09:00"),
                    IndicatorCandleStatus::Finalized
                ),
                (
                    at("2026-06-12T09:00:00+09:00"),
                    IndicatorCandleStatus::Finalized
                ),
                (
                    at("2026-06-15T09:00:00+09:00"),
                    IndicatorCandleStatus::InProgress
                ),
            ]
        );
        let captured = String::from_utf8_lossy(&server.finish().expect("served")).into_owned();
        assert_eq!(captured.matches("market-calendar").count(), 2);
    }

    #[test]
    fn live_index_candles_reuse_known_sessions_and_close_today_after_the_session() {
        let server = serve(&[
            index_page(),
            holiday_calendar(),
            monday_calendar(),
            index_page(),
        ]);
        let reader = adapter_at(&server, "2026-06-15T16:00:00+09:00").with_live_market_sessions();
        let first = index_statuses(&reader);
        let second = index_statuses(&reader);
        assert_eq!(first, second);
        assert_eq!(
            first.last(),
            Some(&(
                at("2026-06-15T09:00:00+09:00"),
                IndicatorCandleStatus::Finalized
            ))
        );
        let captured = String::from_utf8_lossy(&server.finish().expect("served")).into_owned();
        assert_eq!(captured.matches("market-calendar").count(), 2);
    }

    #[test]
    fn live_index_candles_reject_a_gap_on_an_open_weekday() {
        let server = serve(&[
            index_page(),
            calendar(
                open_day("2026-06-09"),
                open_day("2026-06-10"),
                open_day("2026-06-11"),
            ),
        ]);
        let reader = adapter_at(&server, "2026-06-15T10:00:00+09:00").with_live_market_sessions();
        let failure = run(reader.indicator_candles(
            IndicatorCode::Kospi,
            span("2026-06-08T00:00:00+09:00", "2026-06-15T23:00:00+09:00"),
            CandlePeriod::Daily,
        ))
        .expect_err("missing open day");
        assert_eq!(
            failure.kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        );
    }
}
