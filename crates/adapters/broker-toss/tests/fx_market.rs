mod support;
#[cfg(test)]
mod support_tests;
#[cfg(test)]
mod tests {
    use application::{
        Clock, EventBus, MarketInfo, SecretKey, SecretStore, SecretValue, SystemPortError,
        WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
        TossTimestamp,
    };
    use domain::{
        Currency, FxPair, FxRateKind, MasterInstrumentFacts, NumericInput, Price, Timestamp,
    };
    use domain::{IndicatorCode, IndicatorDisplay, KnownMarket, Market, MarketState};
    use serde_json::{Value, json};
    use std::{future::Future, sync::Arc};
    use testkit::ScriptedHttpServer;

    struct Store;
    impl SecretStore for Store {
        fn load(
            &self,
            key: SecretKey,
        ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
            std::future::ready({
                Ok(Some(SecretValue::new(match key {
                    SecretKey::TossClientId => b"fake-id-19".to_vec(),
                    SecretKey::TossClientSecret => b"fake-key-73".to_vec(),
                    SecretKey::TossAccessToken
                    | SecretKey::DatabaseEncryptionKey
                    | SecretKey::AuditHmacKey
                    | SecretKey::DartApiKey
                    | SecretKey::FredApiKey => panic!("unexpected key"),
                })))
            })
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
    fn response(status: &str, body: &Value) -> Vec<u8> {
        let text = body.to_string();
        format!("HTTP/1.1 {status}\r\nRetry-After: 2\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len()).into_bytes()
    }
    fn issued() -> Vec<u8> {
        response(
            "200 OK",
            &json!({"access_token":"fake-access-91","token_type":"Bearer","expires_in":3600}),
        )
    }
    fn body() -> Value {
        serde_json::from_str::<Value>(
            &std::fs::read_to_string("tests/recordings/exchange-rate.json").expect("file"),
        )
        .expect("recording")["body"]
            .clone()
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
            MasterInstrumentFacts::try_new(vec![]).expect("empty"),
        )
    }
    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }
    fn assert_rate_limit(error: application::PortError) {
        assert_eq!(
            error.kind(),
            application::PortErrorKind::Unavailable {
                cause: application::UnavailableCause::RateLimited,
                retry: application::RetryDelay::Known(
                    domain::TimeSpan::try_new(
                        domain::SpanSeconds::new(2),
                        domain::Nanosecond::new(0)
                    )
                    .expect("delay")
                ),
            }
        );
    }
    fn at(text: &str) -> Timestamp {
        TossTimestamp::try_from(text).expect("time").into_domain()
    }
    #[test]
    fn exchange_rate_preserves_kind_value_pair_validity_and_query() {
        for (kind, expected) in [(FxRateKind::Buy, "1381.7"), (FxRateKind::Mid, "1374.2")] {
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &body())])
                .expect("server");
            let reader = adapter(&server);
            let result =
                run(reader.fx_rate(FxPair::UsdKrw, kind, Some(at("2026-03-25T09:30:30+09:00"))))
                    .expect("rate");
            assert_eq!(result.kind(), kind);
            assert_eq!(result.pair(), FxPair::UsdKrw);
            assert_eq!(
                result.quote_per_base(),
                Price::parse(
                    &NumericInput::try_new(expected).expect("input"),
                    Currency::Krw
                )
                .expect("price")
            );
            assert_eq!(result.as_of(), at("2026-03-25T09:30:00+09:00"));
            assert_eq!(result.valid_until(), at("2026-03-25T09:31:00+09:00"));
            let captured = server.finish().expect("requests");
            let requests = String::from_utf8_lossy(&captured);
            assert!(requests.contains("GET /api/v1/exchange-rate?"));
            assert!(requests.contains("baseCurrency=USD"));
            assert!(requests.contains("quoteCurrency=KRW"));
            assert!(requests.contains("dateTime=2026-03-25T00%3A30%3A30"));
        }
    }
    #[test]
    fn exchange_rate_rejects_wrong_pair_bad_values_and_outside_interval() {
        for (field, value) in [
            ("baseCurrency", json!("KRW")),
            ("quoteCurrency", json!("USD")),
            ("rate", json!("bad")),
            ("midRate", json!("0")),
            ("validFrom", json!("bad")),
            ("validUntil", json!("2026-03-25T09:29:00+09:00")),
        ] {
            let mut input = body();
            input["result"][field] = value;
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)])
                .expect("server");
            let reader = adapter(&server);
            run(reader.fx_rate(
                FxPair::UsdKrw,
                FxRateKind::Buy,
                Some(at("2026-03-25T09:30:30+09:00")),
            ))
            .expect_err("rejected");
            server.finish().expect("requests");
        }
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body())]).expect("server");
        run(adapter(&server).fx_rate(
            FxPair::UsdKrw,
            FxRateKind::Mid,
            Some(at("2026-03-25T09:31:00+09:00")),
        ))
        .expect_err("exclusive end");
        server.finish().expect("requests");
    }
    #[test]
    fn exchange_rate_reuses_token_refresh_and_rate_limit_errors() {
        let revoked =
            json!({"error":{"code":"token-revoked","message":"synthetic","requestId":"fake"}});
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("401 Unauthorized", &revoked),
            issued(),
            response("200 OK", &body()),
        ])
        .expect("server");
        run(adapter(&server).fx_rate(
            FxPair::UsdKrw,
            FxRateKind::Buy,
            Some(at("2026-03-25T09:30:30+09:00")),
        ))
        .expect("refreshed");
        let requests = server.finish().expect("requests");
        assert_eq!(
            String::from_utf8_lossy(&requests)
                .matches("GET /api/v1/exchange-rate")
                .count(),
            2
        );
        let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
        let limited_server = ScriptedHttpServer::start(vec![
            issued(),
            response("429 Too Many Requests", &limited),
            response("429 Too Many Requests", &limited),
            response("429 Too Many Requests", &limited),
            response("429 Too Many Requests", &limited),
        ])
        .expect("server");
        let error = run(adapter(&limited_server).fx_rate(FxPair::UsdKrw, FxRateKind::Buy, None))
            .expect_err("rate limit");
        assert_rate_limit(error);
        limited_server.finish().expect("requests");
    }
    fn recording(name: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(format!("tests/market_recordings/t08-{name}.json"))
                .expect("fixture"),
        )
        .expect("json")
    }
    #[test]
    fn calendars_preserve_market_sessions_and_holidays() {
        for (name, market, expected, instant, state) in [
            (
                "kr",
                KnownMarket::Kr,
                Market::Kr,
                "2026-03-25T09:30:00+09:00",
                MarketState::RegularMarket,
            ),
            (
                "us",
                KnownMarket::Us,
                Market::Us,
                "2026-03-25T23:00:00+09:00",
                MarketState::RegularMarket,
            ),
            (
                "kr-holiday",
                KnownMarket::Kr,
                Market::Kr,
                "2026-05-05T09:30:00+09:00",
                MarketState::Closed,
            ),
            (
                "us-holiday",
                KnownMarket::Us,
                Market::Us,
                "2026-07-03T23:00:00+09:00",
                MarketState::Closed,
            ),
        ] {
            let fixture = recording(name);
            crate::support_tests::validation::validate_recording(
                &crate::support_tests::spec(),
                &fixture,
            )
            .expect("schema");
            let server =
                ScriptedHttpServer::start(vec![issued(), response("200 OK", &fixture["body"])])
                    .expect("server");
            let calendar = run(adapter(&server).calendar(market)).expect("calendar");
            assert_eq!(calendar.today().market(), expected);
            assert_eq!(calendar.state_at(at(instant)), state);
            assert_eq!(
                calendar.today().sessions().is_empty(),
                name.contains("holiday")
            );
            let captured = server.finish().expect("requests");
            assert!(String::from_utf8_lossy(&captured).contains(match market {
                KnownMarket::Kr => "/api/v1/market-calendar/KR",
                KnownMarket::Us => "/api/v1/market-calendar/US",
            }));
        }
    }
    #[test]
    fn calendars_reject_invalid_or_missing_time_and_date_order() {
        for (name, path, value) in [
            (
                "kr",
                vec!["today", "integrated", "regularMarket", "endTime"],
                json!("invalid"),
            ),
            ("kr", vec!["today", "date"], json!("2026-03-24")),
            (
                "us",
                vec!["today", "preMarket", "startTime"],
                json!("invalid"),
            ),
            ("us", vec!["today", "regularMarket", "endTime"], Value::Null),
        ] {
            let mut input = recording(name)["body"].clone();
            let mut row = &mut input["result"];
            for key in path {
                row = &mut row[key];
            }
            *row = value;
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)])
                .expect("server");
            run(adapter(&server).calendar(if name == "kr" {
                KnownMarket::Kr
            } else {
                KnownMarket::Us
            }))
            .expect_err("rejected");
            server.finish().expect("requests");
        }
    }
    #[test]
    fn indicator_quotes_keep_requested_codes_and_distinct_points() {
        let fixture = recording("indices");
        crate::support_tests::validation::validate_recording(
            &crate::support_tests::spec(),
            &fixture,
        )
        .expect("schema");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &fixture["body"])])
                .expect("server");
        let values =
            run(adapter(&server).indicator_quotes(&[IndicatorCode::Kosdaq, IndicatorCode::Kospi]))
                .expect("indicators");
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].code(), IndicatorCode::Kosdaq);
        assert_eq!(values[1].code(), IndicatorCode::Kospi);
        let IndicatorDisplay::Points(first) = values[0].value().display() else {
            panic!("points")
        };
        let IndicatorDisplay::Points(second) = values[1].value().display() else {
            panic!("points")
        };
        assert_eq!(first.as_str(), "845.32");
        assert_eq!(second.as_str(), "2812.45");
        assert_eq!(
            values[0].observation().server_at(),
            Some(at("2026-06-11T15:29:00+09:00"))
        );
        assert_eq!(
            values[1].observation().server_at(),
            Some(at("2026-06-11T15:30:00+09:00"))
        );
        let captured = server.finish().expect("requests");
        assert!(
            String::from_utf8_lossy(&captured)
                .contains("/api/v1/market-indicators/prices?symbols=")
        );
    }
    #[test]
    fn indicator_quotes_reject_duplicates_unknowns_missing_and_invalid_values() {
        let original = recording("indices")["body"].clone();
        for input in [
            json!({"result":[original["result"][0].clone(), original["result"][0].clone()]}),
            json!({"result":[original["result"][0].clone()]}),
            json!({"result":[{"symbol":"KOSPI","lastPrice":"-1"}, original["result"][1].clone()]}),
            json!({"result":[{"symbol":"UNKNOWN","lastPrice":"3"}, original["result"][1].clone()]}),
        ] {
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)])
                .expect("server");
            run(adapter(&server).indicator_quotes(&[IndicatorCode::Kospi, IndicatorCode::Kosdaq]))
                .expect_err("rejected");
            server.finish().expect("requests");
        }
        let server = ScriptedHttpServer::start(vec![]).expect("server");
        run(adapter(&server).indicator_quotes(&[IndicatorCode::Unknown]))
            .expect_err("unknown code");
        run(adapter(&server).indicator_quotes(&[])).expect_err("empty");
        assert_eq!(server.finish().expect("no requests").len(), 0);
    }

    #[test]
    fn t08_recording_replays_on_existing_transport() {
        assert_ne!(crate::support_tests::recordings().len(), 0);
        let fixture = recording("indices");
        let server = crate::support_tests::serve_recordings(&fixture);
        let clock = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&clock),
            clock,
        )
        .expect("client");
        let request = broker_toss::ReadRequest::new(crate::support_tests::route(&fixture));
        let response =
            run(client.get(&request, &SecretValue::new(b"fake-token".to_vec()))).expect("response");
        assert_eq!(response.status(), 200);
        assert_eq!(
            serde_json::from_slice::<Value>(response.body().as_exposed_secret()).expect("body"),
            fixture["body"]
        );
        server.finish().expect("request");
    }

    #[test]
    fn each_calendar_and_indicator_route_uses_refresh_and_rate_limit() {
        for route in 0_usize..3 {
            let revoked =
                json!({"error":{"code":"token-revoked","message":"synthetic","requestId":"fake"}});
            let fixture = recording(match route {
                0 => "kr",
                1 => "us",
                _ => "indices",
            });
            let server = ScriptedHttpServer::start(vec![
                issued(),
                response("401 Unauthorized", &revoked),
                issued(),
                response("200 OK", &fixture["body"]),
            ])
            .expect("server");
            let reader = adapter(&server);
            match route {
                0 => {
                    run(reader.calendar(KnownMarket::Kr)).expect("retry");
                }
                1 => {
                    run(reader.calendar(KnownMarket::Us)).expect("retry");
                }
                _ => {
                    run(reader.indicator_quotes(&[IndicatorCode::Kospi, IndicatorCode::Kosdaq]))
                        .expect("retry");
                }
            }
            let captured = server.finish().expect("requests");
            let path = match route {
                0 => "/api/v1/market-calendar/KR",
                1 => "/api/v1/market-calendar/US",
                _ => "/api/v1/market-indicators/prices",
            };
            assert_eq!(String::from_utf8_lossy(&captured).matches(path).count(), 2);
            let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
            let mut responses = vec![issued()];
            responses.extend((0_usize..4).map(|_| response("429 Too Many Requests", &limited)));
            let limited_server = ScriptedHttpServer::start(responses).expect("server");
            let limited_reader = adapter(&limited_server);
            let error = match route {
                0 => run(limited_reader.calendar(KnownMarket::Kr)).expect_err("429"),
                1 => run(limited_reader.calendar(KnownMarket::Us)).expect_err("429"),
                _ => {
                    run(limited_reader.indicator_quotes(&[IndicatorCode::Kospi])).expect_err("429")
                }
            };
            assert_rate_limit(error);
            limited_server.finish().expect("requests");
        }
    }
    #[test]
    fn current_exchange_rate_must_cover_clock_instant() {
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body())]).expect("server");
        run(adapter(&server).fx_rate(FxPair::UsdKrw, FxRateKind::Buy, None))
            .expect_err("stale response");
        server.finish().expect("requests");
    }
    #[test]
    fn calendar_keeps_regular_close_separate_from_extended_sessions() {
        let fixture = recording("kr");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &fixture["body"])])
                .expect("server");
        let calendar = run(adapter(&server).calendar(KnownMarket::Kr)).expect("calendar");
        let sessions = calendar.today().sessions();
        assert_eq!(sessions.len(), 3);
        assert_eq!(sessions[0].state(), MarketState::PreMarket);
        assert_eq!(sessions[1].state(), MarketState::RegularMarket);
        assert_eq!(
            sessions[1].session().interval().start(),
            at("2026-03-25T09:00:00+09:00")
        );
        assert_eq!(
            sessions[1].session().interval().end(),
            at("2026-03-25T15:30:00+09:00")
        );
        assert_eq!(sessions[2].state(), MarketState::AfterMarket);
        assert_eq!(
            sessions[2].session().interval().start(),
            at("2026-03-25T15:30:00+09:00")
        );
        assert_eq!(
            sessions[2].session().interval().end(),
            at("2026-03-25T20:00:00+09:00")
        );
        assert_eq!(
            calendar.state_at(at("2026-03-25T16:00:00+09:00")),
            MarketState::AfterMarket
        );
        server.finish().expect("requests");
    }
    #[test]
    fn us_calendar_keeps_day_pre_regular_and_after_market_distinct() {
        let fixture = recording("us");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &fixture["body"])])
                .expect("server");
        let calendar = run(adapter(&server).calendar(KnownMarket::Us)).expect("calendar");
        let sessions = calendar.today().sessions();
        assert_eq!(sessions.len(), 4);
        assert_eq!(sessions[0].state(), MarketState::DayMarket);
        assert_eq!(sessions[1].state(), MarketState::PreMarket);
        assert_eq!(sessions[2].state(), MarketState::RegularMarket);
        assert_eq!(
            sessions[2].session().interval().end(),
            at("2026-03-26T05:00:00+09:00")
        );
        assert_eq!(sessions[3].state(), MarketState::AfterMarket);
        assert_eq!(
            sessions[3].session().interval().start(),
            at("2026-03-26T05:00:00+09:00")
        );
        assert_eq!(
            sessions[3].session().interval().end(),
            at("2026-03-26T07:00:00+09:00")
        );
        server.finish().expect("requests");
    }
    #[test]
    fn calendar_rejects_unknown_session_kinds() {
        let mut integrated = recording("kr")["body"].clone();
        integrated["result"]["today"]["integrated"]["unknownMarket"] =
            json!({"startTime":"2026-03-25T20:00:00+09:00", "endTime":"2026-03-25T21:00:00+09:00"});
        let mut segment = recording("kr")["body"].clone();
        segment["result"]["today"]["integrated"]["regularMarket"]["unknownSession"] =
            json!({"startTime":"2026-03-25T09:00:00+09:00"});
        let mut us = recording("us")["body"].clone();
        us["result"]["today"]["unknownMarket"] =
            json!({"startTime":"2026-03-25T07:00:00+09:00", "endTime":"2026-03-25T08:00:00+09:00"});
        let mut top = recording("kr")["body"].clone();
        top["result"]["unknownMarket"] = json!({"startTime":"2026-03-25T20:00:00+09:00"});
        for (market, body) in [
            (KnownMarket::Kr, integrated),
            (KnownMarket::Kr, segment),
            (KnownMarket::Us, us),
            (KnownMarket::Kr, top),
        ] {
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)])
                .expect("server");
            let error = run(adapter(&server).calendar(market)).expect_err("unknown session");
            assert_eq!(
                error.kind(),
                application::PortErrorKind::InvalidResponse(
                    application::ResponseMismatch::Validation
                )
            );
            server.finish().expect("requests");
        }
    }

    #[test]
    fn reverse_currency_pair_preserves_pair_and_distinct_rates() {
        let mut input = body();
        input["result"]["baseCurrency"] = json!("KRW");
        input["result"]["quoteCurrency"] = json!("USD");
        input["result"]["rate"] = json!("0.00073");
        input["result"]["midRate"] = json!("0.00072");
        for (kind, expected) in [(FxRateKind::Buy, "0.00073"), (FxRateKind::Mid, "0.00072")] {
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)])
                .expect("server");
            let result = run(adapter(&server).fx_rate(
                FxPair::KrwUsd,
                kind,
                Some(at("2026-03-25T09:30:30+09:00")),
            ))
            .expect("rate");
            assert_eq!(result.pair(), FxPair::KrwUsd);
            assert_eq!(result.kind(), kind);
            assert_eq!(
                result.quote_per_base(),
                Price::parse(
                    &NumericInput::try_new(expected).expect("input"),
                    Currency::Usd
                )
                .expect("price")
            );
            let captured = server.finish().expect("requests");
            let captured_text = String::from_utf8_lossy(&captured);
            assert!(captured_text.contains("baseCurrency=KRW"));
            assert!(captured_text.contains("quoteCurrency=USD"));
        }
    }

    #[test]
    fn us_index_proxy_etf_reuses_existing_quote_endpoint() {
        use application::BrokerRead;
        use domain::{
            CommonShare, DisplayInstrumentName, InstrumentId, ListingStatus, MasterCode,
            MasterInstrument, MasterInstrumentRow, MasterSecurityType, Symbol,
        };
        let instrument = MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("NYSE").expect("market"),
            symbol: Symbol::try_new("SPY").expect("symbol"),
            instrument_id: InstrumentId::try_new("synthetic-proxy-etf").expect("id"),
            name: DisplayInstrumentName::try_new("합성 지수 대용 ETF").expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("FOREIGN_ETF").expect("type"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: at("2026-03-25T09:00:00+09:00"),
        })
        .expect("master")
        .quote_instrument()
        .expect("quote instrument");
        let body = json!({"result":[{"symbol":"SPY", "timestamp":"2026-03-25T15:30:00Z", "lastPrice":"531.27", "currency":"USD"}]});
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)]).expect("server");
        let result =
            run(adapter(&server).quotes(std::slice::from_ref(&instrument))).expect("quotes");
        assert_eq!(result[0].instrument, instrument);
        let quote = result[0].result.as_ref().expect("price");
        assert_eq!(quote.currency(), Currency::Usd);
        assert_eq!(
            quote.last_price(),
            Some(
                Price::parse(
                    &NumericInput::try_new("531.27").expect("input"),
                    Currency::Usd
                )
                .expect("price")
            )
        );
        let captured = server.finish().expect("requests");
        assert!(String::from_utf8_lossy(&captured).contains("GET /api/v1/prices?symbols=SPY"));
    }
    #[test]
    fn capabilities_match_implemented_market_endpoints() {
        use application::{BrokerCapability as Cap, CapabilitySupport};
        let capabilities = TossAccountRead::<Store, Time, Renewal>::capabilities();
        for capability in [
            Cap::IndicatorQuotes,
            Cap::IndicatorCandles,
            Cap::Calendar,
            Cap::Sectors,
            Cap::Sector,
            Cap::SectorComposition,
            Cap::SectorEtfs,
            Cap::Rankings,
            Cap::InvestorTradingRankings,
            Cap::SectorRankings,
            Cap::StockInvestorFlow,
            Cap::IndicatorInvestorFlow,
            Cap::ProgramTrades,
            Cap::ShortSelling,
            Cap::CreditTrades,
            Cap::SecuritiesLending,
        ] {
            assert_eq!(
                capabilities.support_for(capability),
                CapabilitySupport::Supported,
                "{capability:?}"
            );
        }
        for capability in [
            Cap::Instruments,
            Cap::Orderbook,
            Cap::PriceLimits,
            Cap::SellableQuantity,
        ] {
            assert_eq!(
                capabilities.support_for(capability),
                CapabilitySupport::Unsupported,
                "{capability:?}"
            );
        }
    }
    fn indicator_window(start: &str, end: &str) -> domain::TimeInterval {
        domain::TimeInterval::try_new(at(start), at(end)).expect("window")
    }
    #[test]
    fn indicator_daily_candles_preserve_points_time_and_unknown_finality() {
        let fixture = recording("indicator-daily");
        crate::support_tests::validation::validate_recording(
            &crate::support_tests::spec(),
            &fixture,
        )
        .expect("schema");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &fixture["body"])])
                .expect("server");
        let rows = run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            indicator_window("2026-06-10T00:00:00+09:00", "2026-06-12T00:00:00+09:00"),
            domain::CandlePeriod::Daily,
        ))
        .expect("candles");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].observation().server_at(),
            Some(at("2026-06-10T09:00:00+09:00"))
        );
        assert_eq!(
            rows[1].observation().server_at(),
            Some(at("2026-06-11T09:00:00+09:00"))
        );
        assert_eq!(rows[1].code(), IndicatorCode::Kospi);
        assert_eq!(rows[1].status(), domain::IndicatorCandleStatus::Unknown);
        let IndicatorDisplay::Points(open) = rows[1].open().display() else {
            panic!("open points")
        };
        let IndicatorDisplay::Points(high) = rows[1].high().display() else {
            panic!("high points")
        };
        let IndicatorDisplay::Points(low) = rows[1].low().display() else {
            panic!("low points")
        };
        let IndicatorDisplay::Points(close) = rows[1].close().display() else {
            panic!("close points")
        };
        assert_eq!(
            (open.as_str(), high.as_str(), low.as_str(), close.as_str()),
            ("2798.32", "2820.15", "2790.1", "2812.45")
        );
        let captured = server.finish().expect("requests");
        let request = String::from_utf8_lossy(&captured);
        assert!(request.contains("/api/v1/market-indicators/KOSPI/candles?"));
        assert!(request.contains("interval=1d"));
        assert!(request.contains("count=200"));
        assert!(request.contains("before="));
    }
    #[test]
    fn indicator_minute_candles_preserve_finality_boundary() {
        let fixture = recording("indicator-minute");
        crate::support_tests::validation::validate_recording(
            &crate::support_tests::spec(),
            &fixture,
        )
        .expect("schema");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &fixture["body"])])
                .expect("server");
        let rows = run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            indicator_window("2027-01-15T07:57:00Z", "2027-01-15T08:01:00Z"),
            domain::CandlePeriod::Minute,
        ))
        .expect("candles");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].status(), domain::IndicatorCandleStatus::Finalized);
        assert_eq!(rows[1].status(), domain::IndicatorCandleStatus::InProgress);
        let captured = server.finish().expect("requests");
        assert!(String::from_utf8_lossy(&captured).contains("interval=1m"));
    }
    fn daily_window() -> domain::TimeInterval {
        indicator_window("2026-06-10T00:00:00+09:00", "2026-06-12T00:00:00+09:00")
    }
    #[test]
    fn indicator_candles_reject_malformed_rows_and_unusable_cursors() {
        let original = recording("indicator-daily")["body"].clone();
        let mut bad_number = original.clone();
        bad_number["result"]["candles"][0]["highPrice"] = json!("bad");
        let mut duplicate = original.clone();
        duplicate["result"]["candles"][1]["timestamp"] =
            duplicate["result"]["candles"][0]["timestamp"].clone();
        let mut future_cursor = original.clone();
        future_cursor["result"]["nextBefore"] = json!("2026-06-12T00:00:00+09:00");
        let mut oversized = original.clone();
        oversized["result"]["candles"] = json!(vec![original["result"]["candles"][0].clone(); 201]);
        for body in [bad_number, duplicate, future_cursor, oversized] {
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)])
                .expect("server");
            let error = run(adapter(&server).indicator_candles(
                IndicatorCode::Kospi,
                daily_window(),
                domain::CandlePeriod::Daily,
            ))
            .expect_err("invalid");
            assert_eq!(
                error.kind(),
                application::PortErrorKind::InvalidResponse(
                    application::ResponseMismatch::Validation
                )
            );
            server.finish().expect("requests");
        }
    }
    #[test]
    fn indicator_candle_errors_refresh_and_rate_limit() {
        let fixture = recording("indicator-daily");
        let revoked =
            json!({"error":{"code":"token-revoked","message":"synthetic","requestId":"fake"}});
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("401 Unauthorized", &revoked),
            issued(),
            response("200 OK", &fixture["body"]),
        ])
        .expect("server");
        run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            daily_window(),
            domain::CandlePeriod::Daily,
        ))
        .expect("refresh");
        server.finish().expect("requests");
        let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
        let mut responses = vec![issued()];
        responses.extend((0_usize..4).map(|_| response("429 Too Many Requests", &limited)));
        let limited_server = ScriptedHttpServer::start(responses).expect("server");
        assert_rate_limit(
            run(adapter(&limited_server).indicator_candles(
                IndicatorCode::Kospi,
                daily_window(),
                domain::CandlePeriod::Daily,
            ))
            .expect_err("limited"),
        );
        limited_server.finish().expect("requests");
    }
    #[test]
    fn indicator_candles_reject_unsupported_inputs_before_network() {
        let server = ScriptedHttpServer::start(vec![]).expect("server");
        let reader = adapter(&server);
        for code in [IndicatorCode::Unknown, IndicatorCode::KrBond10Y] {
            let error =
                run(reader.indicator_candles(code, daily_window(), domain::CandlePeriod::Minute))
                    .expect_err("unsupported");
            assert_eq!(
                error.kind(),
                application::PortErrorKind::InvalidRequest(
                    application::RequestRejection::UnsupportedQuery
                )
            );
        }
        assert_eq!(server.finish().expect("no requests").len(), 0);
    }
    #[test]
    fn indicator_candles_use_inclusive_cursor_and_deduplicate_equal_overlap() {
        let original = recording("indicator-daily")["body"].clone();
        let mut first = original.clone();
        first["result"]["candles"] = json!([original["result"]["candles"][0].clone()]);
        first["result"]["nextBefore"] = json!("2026-06-11T09:00:00+09:00");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &first),
            response("200 OK", &original),
        ])
        .expect("server");
        let rows = run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            daily_window(),
            domain::CandlePeriod::Daily,
        ))
        .expect("pages");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].observation().server_at(),
            Some(at("2026-06-10T09:00:00+09:00"))
        );
        assert_eq!(
            rows[1].observation().server_at(),
            Some(at("2026-06-11T09:00:00+09:00"))
        );
        let captured = server.finish().expect("requests");
        assert!(String::from_utf8_lossy(&captured).contains("2026-06-11T09%3A00%3A00%2B09%3A00"));
    }
    #[test]
    fn indicator_candles_reject_different_values_for_the_same_time() {
        let original = recording("indicator-daily")["body"].clone();
        let mut first = original.clone();
        let mut second = original;
        first["result"]["candles"] = json!([first["result"]["candles"][0].clone()]);
        first["result"]["nextBefore"] = json!("2026-06-11T09:00:00+09:00");
        second["result"]["candles"][0]["closePrice"] = json!("2811.45");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &first),
            response("200 OK", &second),
        ])
        .expect("server");
        run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            daily_window(),
            domain::CandlePeriod::Daily,
        ))
        .expect_err("conflict");
        server.finish().expect("requests");
    }
    fn holiday(date: &str) -> domain::MarketSession {
        domain::MarketSession::try_new(
            broker_toss::TossDate::try_from(date)
                .expect("date")
                .into_domain(),
            domain::SessionInformation::Kr {
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
        )
        .expect("holiday")
    }
    #[test]
    fn indicator_daily_gaps_require_closed_calendar_evidence() {
        let mut body = recording("indicator-daily")["body"].clone();
        body["result"]["candles"][0]["timestamp"] = json!("2026-06-15T09:00:00+09:00");
        body["result"]["candles"][1]["timestamp"] = json!("2026-06-12T09:00:00+09:00");
        let window = indicator_window("2026-06-12T00:00:00+09:00", "2026-06-16T00:00:00+09:00");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)]).expect("server");
        let error = run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            window,
            domain::CandlePeriod::Daily,
        ))
        .expect_err("missing calendar");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::Unsupported(
                application::BrokerCapability::IndicatorCandles
            )
        );
        server.finish().expect("requests");
        let holiday_server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)]).expect("server");
        let reader = adapter(&holiday_server)
            .with_candle_sessions(vec![holiday("2026-06-13"), holiday("2026-06-14")]);
        assert_eq!(
            run(reader.indicator_candles(
                IndicatorCode::Kospi,
                window,
                domain::CandlePeriod::Daily
            ))
            .expect("holidays")
            .len(),
            2
        );
        holiday_server.finish().expect("requests");
    }
    #[derive(Clone)]
    struct FixedTime(Timestamp);
    impl Clock for FixedTime {
        fn now(&self) -> Timestamp {
            self.0
        }
    }
    fn adapter_at(
        server: &ScriptedHttpServer,
        now: Timestamp,
    ) -> TossAccountRead<Store, FixedTime, Renewal> {
        let transport_time = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&transport_time),
            transport_time,
        )
        .expect("client");
        TossAccountRead::new(
            TokenManager::new(
                client,
                Store,
                FixedTime(now),
                Renewal,
                Arc::new(EventBus::default()),
            ),
            FixedTime(now),
            MasterInstrumentFacts::try_new(vec![]).expect("empty"),
        )
    }
    #[test]
    fn index_day_finalizes_after_regular_close_during_after_market() {
        let calendar_fixture = recording("kr");
        let mut candles = recording("indicator-daily")["body"].clone();
        candles["result"]["candles"] = json!([candles["result"]["candles"][0].clone()]);
        candles["result"]["candles"][0]["timestamp"] = json!("2026-03-25T09:00:00+09:00");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &calendar_fixture["body"]),
            response("200 OK", &candles),
        ])
        .expect("server");
        let now = at("2026-03-25T16:00:00+09:00");
        let reader = adapter_at(&server, now);
        let calendar = run(reader.calendar(KnownMarket::Kr)).expect("calendar");
        let rows = run(reader.indicator_candles(
            IndicatorCode::Kospi,
            indicator_window("2026-03-25T00:00:00+09:00", "2026-03-26T00:00:00+09:00"),
            domain::CandlePeriod::Daily,
        ))
        .expect("candle");
        assert_eq!(calendar.state_at(now), MarketState::AfterMarket);
        assert_eq!(rows[0].status(), domain::IndicatorCandleStatus::Unknown);
        assert_eq!(
            domain::finalize_index_day(rows[0].clone(), &calendar)
                .expect("regular close")
                .status(),
            domain::IndicatorCandleStatus::Finalized
        );
        server.finish().expect("requests");
    }
    #[test]
    fn indicator_candles_reject_overlap_with_different_reported_offset() {
        let original = recording("indicator-daily")["body"].clone();
        let mut first = original.clone();
        let mut second = original;
        first["result"]["candles"] = json!([first["result"]["candles"][0].clone()]);
        first["result"]["nextBefore"] = json!("2026-06-11T09:00:00+09:00");
        second["result"]["candles"][0]["timestamp"] = json!("2026-06-11T00:00:00Z");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &first),
            response("200 OK", &second),
        ])
        .expect("server");
        let error = run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            daily_window(),
            domain::CandlePeriod::Daily,
        ))
        .expect_err("offset mismatch");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        server.finish().expect("requests");
    }
    #[test]
    fn bond_daily_candle_keeps_yield_unit_and_unknown_finality() {
        let mut body = recording("indicator-daily")["body"].clone();
        body["result"]["candles"] = json!([body["result"]["candles"][0].clone()]);
        body["result"]["candles"][0]["openPrice"] = json!("3.10");
        body["result"]["candles"][0]["highPrice"] = json!("3.22");
        body["result"]["candles"][0]["lowPrice"] = json!("3.05");
        body["result"]["candles"][0]["closePrice"] = json!("3.17");
        body["result"]["candles"][0]["volume"] = json!("0");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)]).expect("server");
        let rows = run(adapter(&server).indicator_candles(
            IndicatorCode::KrBond10Y,
            daily_window(),
            domain::CandlePeriod::Daily,
        ))
        .expect("bond");
        assert_eq!(rows[0].code(), IndicatorCode::KrBond10Y);
        assert_eq!(rows[0].status(), domain::IndicatorCandleStatus::Unknown);
        let IndicatorDisplay::YieldPercent(close) = rows[0].close().display() else {
            panic!("yield")
        };
        assert_eq!(close.as_str(), "3.17");
        let captured = server.finish().expect("requests");
        assert!(
            String::from_utf8_lossy(&captured)
                .contains("/api/v1/market-indicators/KR_BOND_10Y/candles?")
        );
    }
    fn minute_stamp(index: i64) -> String {
        let seconds = 1_800_000_000_i64
            .checked_sub(index.checked_mul(60).expect("minutes"))
            .expect("time");
        let stamp = time::OffsetDateTime::from_unix_timestamp(seconds).expect("timestamp");
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:00Z",
            stamp.year(),
            u8::from(stamp.month()),
            stamp.day(),
            stamp.hour(),
            stamp.minute()
        )
    }
    fn minute_window() -> domain::TimeInterval {
        indicator_window("2027-01-15T04:00:00Z", "2027-01-15T08:01:00Z")
    }
    #[test]
    fn indicator_candle_page_accepts_200_rows() {
        let mut body = recording("indicator-minute")["body"].clone();
        let template = body["result"]["candles"][0].clone();
        let mut candles = Vec::new();
        for index in 0_i64..200 {
            let mut row = template.clone();
            row["timestamp"] = json!(minute_stamp(index));
            candles.push(row);
        }
        body["result"]["candles"] = json!(candles);
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)]).expect("server");
        let rows = run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            minute_window(),
            domain::CandlePeriod::Minute,
        ))
        .expect("200");
        assert_eq!(rows.len(), 200);
        assert_eq!(
            rows[0].observation().server_at(),
            Some(at(&minute_stamp(199)))
        );
        server.finish().expect("requests");
    }
    fn minute_pages(last_cursor: Option<&str>) -> Vec<Vec<u8>> {
        let template = recording("indicator-minute")["body"].clone();
        let mut responses = vec![issued()];
        for index in 0_i64..64 {
            let mut page = template.clone();
            let mut row = page["result"]["candles"][0].clone();
            row["timestamp"] = json!(minute_stamp(index));
            page["result"]["candles"] = json!([row]);
            page["result"]["nextBefore"] = if index == 63 {
                json!(last_cursor)
            } else {
                json!(minute_stamp(index.checked_add(1).expect("next")))
            };
            responses.push(response("200 OK", &page));
        }
        responses
    }
    #[test]
    fn indicator_candles_accept_complete_64_page_window() {
        let server = ScriptedHttpServer::start(minute_pages(None)).expect("server");
        let rows = run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            minute_window(),
            domain::CandlePeriod::Minute,
        ))
        .expect("64 pages");
        assert_eq!(rows.len(), 64);
        assert_eq!(
            rows[0].observation().server_at(),
            Some(at(&minute_stamp(63)))
        );
        server.finish().expect("requests");
    }
    #[test]
    fn indicator_candles_reject_incomplete_window_after_64_pages() {
        let server =
            ScriptedHttpServer::start(minute_pages(Some(&minute_stamp(64)))).expect("server");
        let error = run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            minute_window(),
            domain::CandlePeriod::Minute,
        ))
        .expect_err("page cap");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        server.finish().expect("requests");
    }
    #[test]
    fn indicator_candles_reject_missing_next_before_instead_of_assuming_last_page() {
        let mut body = recording("indicator-daily")["body"].clone();
        body["result"]
            .as_object_mut()
            .expect("result")
            .remove("nextBefore");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)]).expect("server");
        let error = run(adapter(&server).indicator_candles(
            IndicatorCode::Kospi,
            daily_window(),
            domain::CandlePeriod::Daily,
        ))
        .expect_err("missing cursor");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        server.finish().expect("requests");
    }
}
