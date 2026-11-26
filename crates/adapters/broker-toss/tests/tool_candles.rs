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
    struct Time(Arc<crate::support::FakeTime>);
    impl Clock for Time {
        fn now(&self) -> Timestamp {
            self.0.now()
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
    fn adapter(server: &ScriptedHttpServer) -> TossAccountRead<Store, Time, Renewal> {
        let clock = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&clock),
            Arc::<crate::support::FakeTime>::clone(&clock),
        )
        .expect("client");
        TossAccountRead::new(
            TokenManager::new(
                client,
                Store,
                Time(Arc::<crate::support::FakeTime>::clone(&clock)),
                Renewal,
                Arc::new(EventBus::default()),
            ),
            Time(clock),
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
    fn strict_daily_candles_need_market_sessions_the_live_reader_never_has() {
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
}
