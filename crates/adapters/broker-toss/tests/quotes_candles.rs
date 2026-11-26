mod support;
#[cfg(test)]
mod support_tests;
#[cfg(test)]
mod tests {
    use application::{
        BrokerRead, Clock, EventBus, SecretKey, SecretStore, SecretValue, SystemPortError,
        WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
        TossTimestamp,
    };
    use domain::{
        AdjustmentBasis, CandlePeriod, CandleStatus, CommonShare, Currency, DisplayInstrumentName,
        InstrumentId, KoreanMasterDetail, LiquidationTrading, ListingStatus, MarketSession,
        MasterCode, MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow,
        MasterSecurityType, NumericInput, NxtTrading, Price, PriceAdjustment, SessionData,
        SessionInformation, Symbol, TimeInterval, Timestamp, TradingSuspension,
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
    fn master(now: Timestamp) -> MasterInstrumentFacts {
        MasterInstrumentFacts::try_new(
            [
                ("005930", "KOSPI", Currency::Krw),
                ("AAPL", "NASDAQ", Currency::Usd),
            ]
            .map(|(symbol, market, currency)| {
                MasterInstrument::try_new(MasterInstrumentRow {
                    listing_market: MasterCode::try_new(market).expect("market"),
                    symbol: Symbol::try_new(symbol).expect("symbol"),
                    instrument_id: InstrumentId::try_new(&format!("synthetic-{symbol}"))
                        .expect("id"),
                    name: DisplayInstrumentName::try_new("합성 종목").expect("name"),
                    security_type: MasterSecurityType::from_code(
                        MasterCode::try_new("STOCK").expect("type"),
                    ),
                    currency,
                    listing_status: ListingStatus::Active,
                    common_share: CommonShare::Common,
                    korean_detail: (currency == Currency::Krw).then_some(KoreanMasterDetail {
                        liquidation: LiquidationTrading::Regular,
                        krx: TradingSuspension::Clear,
                        nxt: NxtTrading::Unsupported,
                    }),
                    observed_at: now,
                })
                .expect("master")
            })
            .to_vec(),
        )
        .expect("facts")
    }
    fn adapter(server: &ScriptedHttpServer) -> TossAccountRead<Store, Time, Renewal> {
        adapter_with_master(server, master(crate::support::time().now()))
    }
    fn adapter_with_master(
        server: &ScriptedHttpServer,
        facts: MasterInstrumentFacts,
    ) -> TossAccountRead<Store, Time, Renewal> {
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
            Time(Arc::<crate::support::FakeTime>::clone(&clock)),
            facts,
        )
    }
    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }
    fn at(text: &str) -> Timestamp {
        TossTimestamp::try_from(text)
            .expect("timestamp")
            .into_domain()
    }
    fn instrument() -> domain::QuoteInstrument {
        master(at("2026-03-25T12:00:00+09:00")).instruments()[0]
            .quote_instrument()
            .expect("quote")
    }
    fn window() -> TimeInterval {
        TimeInterval::try_new(
            at("2026-03-24T00:00:00+09:00"),
            at("2026-03-26T00:00:00+09:00"),
        )
        .expect("window")
    }
    fn sessions() -> Vec<MarketSession> {
        ["2026-03-24", "2026-03-25"]
            .map(|date| {
                let open = at(&format!("{date}T09:00:00+09:00"));
                let close = at(&format!("{date}T15:30:00+09:00"));
                MarketSession::try_new(
                    open.offset_calendar_date(),
                    SessionInformation::Kr {
                        pre_market: None,
                        regular_market: Some(
                            SessionData::try_new(
                                TimeInterval::try_new(open, close).expect("session"),
                                None,
                            )
                            .expect("data"),
                        ),
                        after_market: None,
                    },
                )
                .expect("calendar")
            })
            .to_vec()
    }

    fn fixture(name: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(format!("tests/market_recordings/t07-{name}.json"))
                .expect("fixture"),
        )
        .expect("json")
    }
    fn serve(values: &[Value]) -> ScriptedHttpServer {
        ScriptedHttpServer::start(
            std::iter::once(issued())
                .chain(values.iter().map(|value| response("200 OK", value)))
                .collect(),
        )
        .expect("server")
    }
    #[test]
    fn t20_market_recording_replays_and_shared_catalog_remains_available() {
        assert_ne!(crate::support_tests::recordings().len(), 0);
        let recording = fixture("quote");
        let server = crate::support_tests::serve_recordings(&recording);
        let clock = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&clock),
            clock,
        )
        .expect("client");
        let read = broker_toss::ReadRequest::new(crate::support_tests::route(&recording));
        let response =
            run(client.get(&read, &SecretValue::new(b"fake-market-token".to_vec()))).expect("get");
        assert_eq!(response.status(), 200);
        assert_eq!(
            serde_json::from_slice::<Value>(response.body().as_exposed_secret()).expect("body"),
            recording["body"]
        );
        server.finish().expect("read");
    }
    #[test]
    fn market_recordings_match_spec_and_generated_models() {
        for name in ["quote", "daily", "minute", "last"] {
            let value = fixture(name);
            crate::support_tests::validation::validate_recording(
                &crate::support_tests::spec(),
                &value,
            )
            .expect("schema");
            if name == "quote" {
                let _decoded: Vec<broker_toss_schema::types::PriceResponse> =
                    serde_json::from_value(value["body"]["result"].clone())
                        .expect("generated quote");
            } else {
                let _decoded: broker_toss_schema::types::CandlePageResponse =
                    serde_json::from_value(value["body"]["result"].clone())
                        .expect("generated candles");
            }
        }
    }
    #[test]
    fn quote_preserves_instrument_currency_price_and_both_times() {
        let body = fixture("quote")["body"].clone();
        let server = serve(&[body]);
        let reader = adapter(&server);
        let requested = instrument();
        let result = run(reader.quotes(&[requested.clone(), requested.clone()])).expect("batch");
        assert_eq!(result.len(), 2);
        for row in result {
            assert_eq!(row.instrument, requested);
            let quote = row.result.as_ref().expect("quote");
            assert_eq!(quote.symbol(), requested.symbol());
            assert_eq!(quote.currency(), Currency::Krw);
            assert_eq!(quote.timestamp(), Some(at("2026-03-25T09:30:00.123+09:00")));
            assert_eq!(
                quote.last_price(),
                Some(
                    Price::parse(
                        &NumericInput::try_new("72000").expect("input"),
                        Currency::Krw
                    )
                    .expect("price")
                )
            );
            assert!(quote.collected_at() > quote.timestamp().expect("time"));
        }
        let requests = server.finish().expect("requests");
        assert!(
            String::from_utf8_lossy(&requests)
                .contains("GET /api/v1/prices?symbols=005930 HTTP/1.1")
        );
    }
    #[test]
    fn malformed_quote_is_an_individual_error_and_batch_limits_are_local() {
        for field in ["lastPrice", "currency", "timestamp"] {
            let mut body = fixture("quote")["body"].clone();
            body["result"][0][field] = json!("invalid");
            let server = serve(&[body]);
            let reader = adapter(&server);
            run(reader.quotes(&[instrument()])).expect("batch")[0]
                .result
                .as_ref()
                .expect_err("rejected");
            let _captured = server.finish().expect("requests");
        }
        let server = ScriptedHttpServer::start(Vec::new()).expect("server");
        let reader = adapter(&server);
        run(reader.quotes(&[])).expect_err("rejected");
        run(reader.quotes(&vec![instrument(); 201])).expect_err("rejected");
        assert_eq!(server.finish().expect("no requests").len(), 0);
    }
    #[test]
    fn daily_candles_preserve_each_field_order_and_explicit_adjustment() {
        for adjustment in [
            PriceAdjustment::Raw,
            PriceAdjustment::Adjusted(AdjustmentBasis::new(at("2026-03-26T00:00:00+09:00"))),
        ] {
            let server = serve(&[fixture("daily")["body"].clone()]);
            let reader = adapter(&server).with_candle_sessions(sessions());
            let candles =
                run(reader.candles(&instrument(), window(), CandlePeriod::Daily, adjustment))
                    .expect("candles");
            assert_eq!(candles.len(), 2);
            assert_eq!(candles[0].time().timestamp(), window().start());
            assert_eq!(
                candles[1].time().timestamp(),
                at("2026-03-25T00:00:00+09:00")
            );
            assert_eq!(candles[1].instrument(), instrument().instrument_id());
            assert_eq!(candles[1].adjustment(), adjustment);
            assert_eq!(candles[1].status(), CandleStatus::Finalized);
            let values = candles[1].values();
            for (price, text) in [
                (values.open(), "71600"),
                (values.high(), "72300"),
                (values.low(), "71500"),
                (values.close(), "72000"),
            ] {
                assert_eq!(
                    price,
                    Price::parse(&NumericInput::try_new(text).expect("input"), Currency::Krw)
                        .expect("price")
                );
            }
            assert_eq!(
                values.volume(),
                domain::Quantity::parse(&NumericInput::try_new("3521000").expect("input"))
                    .expect("quantity")
            );
            let requests = server.finish().expect("requests");
            let read = String::from_utf8_lossy(&requests);
            assert!(read.contains("count=200"));
            assert!(read.contains("interval=1d"));
            assert!(read.contains(match adjustment {
                PriceAdjustment::Raw => "adjusted=false",
                PriceAdjustment::Adjusted(_) => "adjusted=true",
            }));
            assert!(read.contains("before="));
        }
    }
    #[test]
    fn minute_candles_validate_order_duplicates_future_alignment_and_numeric_fields() {
        let base = fixture("minute")["body"].clone();
        let window = TimeInterval::try_new(
            at("2026-03-25T09:31:00+09:00"),
            at("2026-03-25T09:33:00+09:00"),
        )
        .expect("window");
        {
            let server = serve(std::slice::from_ref(&base));
            let reader = adapter(&server);
            let candles = run(reader.candles(
                &instrument(),
                window,
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            ))
            .expect("minute");
            assert_eq!(candles.len(), 2);
            assert_eq!(candles[0].time().period(), CandlePeriod::Minute);
            assert_eq!(
                candles[1].time().timestamp(),
                at("2026-03-25T09:32:00+09:00")
            );
            let _captured = server.finish().expect("requests");
        };
        for (field, replacement) in [
            ("timestamp", "2099-01-01T09:32:00+09:00"),
            ("timestamp", "2026-03-25T09:32:01+09:00"),
            ("timestamp", "2026-03-25T09:34:00+09:00"),
            ("currency", "USD"),
            ("openPrice", "0"),
            ("volume", "oops"),
        ] {
            let mut body = base.clone();
            body["result"]["candles"][0][field] = json!(replacement);
            let server = serve(&[body]);
            let reader = adapter(&server);
            run(reader.candles(
                &instrument(),
                window,
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            ))
            .expect_err("rejected");
            let _captured = server.finish().expect("requests");
        }
    }

    #[test]
    fn minute_reversed_and_duplicate_rows_fail() {
        for duplicate in [false, true] {
            let mut body = fixture("minute")["body"].clone();
            body["result"]["nextBefore"] = Value::Null;
            if duplicate {
                body["result"]["candles"][1] = body["result"]["candles"][0].clone();
            } else {
                body["result"]["candles"]
                    .as_array_mut()
                    .expect("rows")
                    .reverse();
            }
            let server = serve(&[body]);
            let reader = adapter(&server);
            run(reader.candles(
                &instrument(),
                minute_window(),
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            ))
            .expect_err("order");
            let _captured = server.finish().expect("requests");
        }
    }
    fn minute(index: usize) -> Value {
        let mut row = fixture("minute")["body"]["result"]["candles"][0].clone();
        row["timestamp"] = json!(format!(
            "2026-03-25T{:02}:{:02}:00+09:00",
            9_usize
                .checked_add(index.checked_div(60).expect("divisor"))
                .expect("hour"),
            index.rem_euclid(60)
        ));
        row
    }
    fn page(indices: impl IntoIterator<Item = usize>, next: Option<usize>) -> Value {
        json!({"result":{"candles":indices.into_iter().map(minute).collect::<Vec<_>>(),"nextBefore":next.map(|index|minute(index)["timestamp"].as_str().expect("time").to_owned())}})
    }
    fn minute_window() -> TimeInterval {
        TimeInterval::try_new(
            at("2026-03-25T09:00:00+09:00"),
            at("2026-03-25T12:21:00+09:00"),
        )
        .expect("window")
    }
    #[test]
    fn two_hundred_one_candles_page_backwards_and_merge_only_equal_overlap() {
        for conflict in [false, true] {
            let first = page((1..=200).rev(), Some(1));
            let mut second = page([1, 0], None);
            if conflict {
                second["result"]["candles"][0]["volume"] = json!("19");
            }
            let server = serve(&[first, second]);
            let reader = adapter(&server);
            let result = run(reader.candles(
                &instrument(),
                minute_window(),
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            ));
            if conflict {
                result.expect_err("rejected");
            } else {
                let rows = result.expect("pages");
                assert_eq!(rows.len(), 201);
                assert_eq!(rows[0].time().timestamp(), minute_window().start());
                assert_eq!(
                    rows[200].time().timestamp(),
                    at("2026-03-25T12:20:00+09:00")
                );
            }
            let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
            assert_eq!(requests.matches("GET /api/v1/candles?").count(), 2);
            assert!(requests.contains("before=2026-03-25T09%3A01%3A00%2B09%3A00"));
        }
    }
    #[test]
    fn pagination_has_a_finite_page_bound() {
        let pages = (1..=64)
            .rev()
            .map(|index| page([index], Some(index)))
            .collect::<Vec<_>>();
        let server = serve(&pages);
        let reader = adapter(&server);
        run(reader.candles(
            &instrument(),
            minute_window(),
            CandlePeriod::Minute,
            PriceAdjustment::Raw,
        ))
        .expect_err("rejected");
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("GET /api/v1/candles?").count(), 64);
    }
    #[test]
    fn missing_daily_calendar_and_minute_gap_fail_closed() {
        {
            let server = serve(&[fixture("daily")["body"].clone()]);
            let reader = adapter(&server);
            run(reader.candles(
                &instrument(),
                window(),
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            ))
            .expect_err("rejected");
            let _captured = server.finish().expect("requests");
        };
        {
            let server = serve(&[page([2, 0], None)]);
            let reader = adapter(&server);
            run(reader.candles(
                &instrument(),
                minute_window(),
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            ))
            .expect_err("rejected");
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn revoked_token_and_rate_limit_use_existing_manager_and_limiter() {
        for route in ["quote", "minute"] {
            let revoked = json!({"error":{"code":"token-revoked","requestId":"synthetic-market-401","message":"revoked"}});
            let mut body = fixture(route)["body"].clone();
            if route == "minute" {
                body["result"]["nextBefore"] = Value::Null;
            }
            let server = ScriptedHttpServer::start(vec![
                issued(),
                response("401 Unauthorized", &revoked),
                issued(),
                response("200 OK", &body),
            ])
            .expect("server");
            let reader = adapter(&server);
            if route == "quote" {
                run(reader.quotes(&[instrument()])).expect("batch")[0]
                    .result
                    .as_ref()
                    .expect("success");
            } else {
                run(reader.candles(
                    &instrument(),
                    minute_window(),
                    CandlePeriod::Minute,
                    PriceAdjustment::Raw,
                ))
                .expect("success");
            }
            let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
            assert_eq!(requests.matches("/oauth2/token ").count(), 2);
        }
    }
    #[test]
    fn exhausted_rate_limit_uses_existing_limiter() {
        for route in ["quote", "minute"] {
            let limited = json!({"error":{"code":"rate-limit-exceeded","requestId":"synthetic-market-429","message":"limited"}});
            let server = ScriptedHttpServer::start(
                std::iter::once(issued())
                    .chain((0_usize..4).map(|_| response("429 Too Many Requests", &limited)))
                    .collect(),
            )
            .expect("server");
            let reader = adapter(&server);
            let error = if route == "quote" {
                run(reader.quotes(&[instrument()])).expect_err("limit")
            } else {
                run(reader.candles(
                    &instrument(),
                    minute_window(),
                    CandlePeriod::Minute,
                    PriceAdjustment::Raw,
                ))
                .expect_err("limit")
            };
            assert_eq!(
                error.kind(),
                application::PortErrorKind::Unavailable {
                    cause: application::UnavailableCause::RateLimited,
                    retry: application::RetryDelay::Known(
                        domain::TimeSpan::try_new(
                            domain::SpanSeconds::new(2),
                            domain::Nanosecond::new(0)
                        )
                        .expect("retry")
                    )
                }
            );
            let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
            assert_eq!(requests.matches("GET /api/v1/").count(), 4);
        }
    }
    #[test]
    fn quote_missing_extra_duplicate_null_time_and_currency_are_distinguished() {
        for body in [
            json!({"result":[]}),
            {
                let mut value = fixture("quote")["body"].clone();
                value["result"][0]["timestamp"] = Value::Null;
                value
            },
            {
                let mut value = fixture("quote")["body"].clone();
                value["result"][0]["currency"] = json!("USD");
                value
            },
            {
                let mut value = fixture("quote")["body"].clone();
                value["result"][0]["timestamp"] = json!("2099-01-01T00:00:00Z");
                value
            },
        ] {
            let server = serve(std::slice::from_ref(&body));
            let reader = adapter(&server);
            let rows = run(reader.quotes(&[instrument()])).expect("batch");
            if body["result"][0]["timestamp"].is_null()
                && !body["result"].as_array().expect("rows").is_empty()
            {
                assert_eq!(rows[0].result.as_ref().expect("quote").timestamp(), None);
            } else {
                rows[0].result.as_ref().expect_err("rejected");
            }
            let _captured = server.finish().expect("requests");
        }
        for body in [
            {
                let mut value = fixture("quote")["body"].clone();
                value["result"][0]["symbol"] = json!("AAPL");
                value
            },
            {
                let mut value = fixture("quote")["body"].clone();
                let extra = value["result"][0].clone();
                value["result"].as_array_mut().expect("rows").push(extra);
                value
            },
            json!({"result":{}}),
        ] {
            let server = serve(&[body]);
            let reader = adapter(&server);
            run(reader.quotes(&[instrument()])).expect_err("rejected");
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn candle_page_size_cursor_and_empty_page_rules_fail_closed() {
        for body in [
            page((0..201).rev(), None),
            page([1], Some(201)),
            {
                let mut value = page([1], Some(1));
                value["result"]["nextBefore"] = json!("invalid");
                value
            },
            page([], Some(0)),
            json!({"result":{"candles":null}}),
        ] {
            let server = serve(&[body]);
            let reader = adapter(&server);
            run(reader.candles(
                &instrument(),
                minute_window(),
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            ))
            .expect_err("rejected");
            let _captured = server.finish().expect("requests");
        }
        let server = serve(&[page([], None)]);
        let reader = adapter(&server);
        assert_eq!(
            run(reader.candles(
                &instrument(),
                minute_window(),
                CandlePeriod::Minute,
                PriceAdjustment::Raw
            ))
            .expect("empty")
            .len(),
            0
        );
        let _captured = server.finish().expect("requests");
    }
    #[test]
    fn daily_calendar_rejects_duplicate_closed_and_missing_trading_dates() {
        for context in [
            {
                let mut rows = sessions();
                rows.push(rows[0].clone());
                rows
            },
            vec![
                MarketSession::try_new(
                    at("2026-03-24T00:00:00+09:00").offset_calendar_date(),
                    SessionInformation::Kr {
                        pre_market: None,
                        regular_market: None,
                        after_market: None,
                    },
                )
                .expect("closed"),
            ],
        ] {
            let server = serve(&[fixture("daily")["body"].clone()]);
            let mut full_context = context;
            if full_context.len() == 1 {
                full_context.push(sessions()[1].clone());
            }
            let reader = adapter(&server).with_candle_sessions(full_context);
            run(reader.candles(
                &instrument(),
                window(),
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            ))
            .expect_err("rejected");
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn daily_gap_requires_explicit_nontrading_dates() {
        for calendar in [Vec::new(), sessions()] {
            let mut body = fixture("daily")["body"].clone();
            body["result"]["candles"][0]["timestamp"] = json!("2026-03-26T00:00:00+09:00");
            body["result"]["nextBefore"] = Value::Null;
            let mut context = calendar;
            let start = at("2026-03-26T09:00:00+09:00");
            let end = at("2026-03-26T15:30:00+09:00");
            context.push(
                MarketSession::try_new(
                    start.offset_calendar_date(),
                    SessionInformation::Kr {
                        pre_market: None,
                        regular_market: Some(
                            SessionData::try_new(
                                TimeInterval::try_new(start, end).expect("range"),
                                None,
                            )
                            .expect("data"),
                        ),
                        after_market: None,
                    },
                )
                .expect("day"),
            );
            if context.len() == 1 {
                context.push(sessions()[0].clone());
            }
            let server = serve(&[body]);
            let reader = adapter(&server).with_candle_sessions(context);
            let range = TimeInterval::try_new(window().start(), at("2026-03-27T00:00:00+09:00"))
                .expect("range");
            run(reader.candles(
                &instrument(),
                range,
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            ))
            .expect_err("rejected");
            let _captured = server.finish().expect("requests");
        }
    }
    fn invalid_response<T: std::fmt::Debug>(result: Result<T, application::PortError>) {
        let error = result.expect_err("invalid response");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
    }
    #[test]
    fn batch_limit_exactly_two_hundred_is_supported() {
        let server = serve(&[fixture("quote")["body"].clone()]);
        let reader = adapter(&server);
        let rows = run(reader.quotes(&vec![instrument(); 200])).expect("two hundred");
        assert_eq!(rows.len(), 200);
        for row in rows {
            row.result.expect("quote");
        }
        let _captured = server.finish().expect("requests");
    }
    #[test]
    fn local_batch_rejection_remains_typed() {
        let server = ScriptedHttpServer::start(Vec::new()).expect("server");
        let reader = adapter(&server);
        for inputs in [Vec::new(), vec![instrument(); 201]] {
            let error = run(reader.quotes(&inputs)).expect_err("batch size");
            assert_eq!(
                error.kind(),
                application::PortErrorKind::InvalidRequest(
                    application::RequestRejection::BatchLimitExceeded
                )
            );
            assert_eq!(error.status(), application::HttpStatus::NotReceived);
        }
        assert_eq!(server.finish().expect("requests").len(), 0);
    }
    #[test]
    fn candles_exclude_the_right_endpoint_and_keep_the_left() {
        let server = serve(&[page([2, 1, 0], None)]);
        let reader = adapter(&server);
        let range = TimeInterval::try_new(minute_window().start(), at("2026-03-25T09:02:00+09:00"))
            .expect("range");
        let rows = run(reader.candles(
            &instrument(),
            range,
            CandlePeriod::Minute,
            PriceAdjustment::Raw,
        ))
        .expect("range");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].time().timestamp(), range.start());
        assert_eq!(rows[1].time().timestamp(), at("2026-03-25T09:01:00+09:00"));
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert!(requests.contains("symbol=005930"));
    }
    #[test]
    fn invalid_cursors_preserve_validation_error_without_a_second_request() {
        for next in [Some(2), Some(201)] {
            let server = serve(&[page([1], next)]);
            let reader = adapter(&server);
            invalid_response(run(reader.candles(
                &instrument(),
                minute_window(),
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            )));
            let _captured = server.finish().expect("requests");
        }
    }
    fn daily_history() -> (Vec<Value>, Vec<MarketSession>, Timestamp) {
        let mut stamp = at("2026-03-25T00:00:00+09:00");
        let mut rows = Vec::new();
        let mut calendar = Vec::new();
        for _ in 0_usize..201 {
            let date = stamp.offset_calendar_date();
            let text = format!(
                "{:04}-{:02}-{:02}",
                date.year().as_i32(),
                date.month().as_u8(),
                date.day().as_u8()
            );
            let mut row = fixture("daily")["body"]["result"]["candles"][0].clone();
            row["timestamp"] = json!(format!("{text}T00:00:00+09:00"));
            rows.push(row);
            let open = at(&format!("{text}T09:00:00+09:00"));
            let close = at(&format!("{text}T15:30:00+09:00"));
            calendar.push(
                MarketSession::try_new(
                    date,
                    SessionInformation::Kr {
                        pre_market: None,
                        regular_market: Some(
                            SessionData::try_new(
                                TimeInterval::try_new(open, close).expect("range"),
                                None,
                            )
                            .expect("data"),
                        ),
                        after_market: None,
                    },
                )
                .expect("session"),
            );
            stamp = stamp.checked_add(domain::TimeSpan::DAY).expect("next day");
        }
        (rows, calendar, stamp)
    }
    #[test]
    fn two_hundred_one_daily_bars_use_two_pages_with_the_same_adjustment_basis() {
        let (rows, calendar, stamp) = daily_history();
        let first = json!({"result":{"candles":rows.iter().skip(1).rev().cloned().collect::<Vec<_>>(),"nextBefore":rows[1]["timestamp"]}});
        let second = json!({"result":{"candles":[rows[1],rows[0]],"nextBefore":null}});
        let server = serve(&[first, second]);
        let reader = adapter(&server).with_candle_sessions(calendar);
        let adjustment = PriceAdjustment::Adjusted(AdjustmentBasis::new(stamp));
        let range = TimeInterval::try_new(at("2026-03-25T00:00:00+09:00"), stamp).expect("range");
        let bars = run(reader.candles(&instrument(), range, CandlePeriod::Daily, adjustment))
            .expect("daily pages");
        assert_eq!(bars.len(), 201);
        for bar in bars {
            assert_eq!(bar.adjustment(), adjustment);
            assert_eq!(bar.status(), CandleStatus::Finalized);
        }
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("adjusted=true").count(), 2);
    }
    #[test]
    fn review1_adjusted_half_price_is_rejected() {
        let mut body = fixture("daily")["body"].clone();
        body["result"]["nextBefore"] = Value::Null;
        for (index, value) in [(0, "36000"), (1, "72000")] {
            for field in ["openPrice", "highPrice", "lowPrice", "closePrice"] {
                body["result"]["candles"][index][field] = json!(value);
            }
        }
        let server = serve(&[body]);
        let result = run(adapter(&server).with_candle_sessions(sessions()).candles(
            &instrument(),
            window(),
            CandlePeriod::Daily,
            PriceAdjustment::Adjusted(AdjustmentBasis::new(window().end())),
        ));
        invalid_response(result);
        let _captured = server.finish().expect("requests");
    }
    #[test]
    fn review1_dst_fall_keeps_calendar_dates() {
        for offset in ["-04:00", "-05:00"] {
            let mut body = fixture("daily")["body"].clone();
            body["result"]["nextBefore"] = Value::Null;
            body["result"]["candles"][0]["timestamp"] =
                json!(format!("2026-11-02T00:00:00{offset}"));
            body["result"]["candles"][1]["timestamp"] = json!("2026-10-30T00:00:00-04:00");
            for row in body["result"]["candles"].as_array_mut().expect("rows") {
                row["currency"] = json!("USD");
            }
            let calendar = us_sessions([
                ("2026-10-30", "-04:00"),
                ("2026-10-31", offset),
                ("2026-11-01", offset),
                ("2026-11-02", offset),
            ]);
            let server = serve(&[body]);
            let inst = master(at("2026-10-30T00:00:00-04:00")).instruments()[1]
                .quote_instrument()
                .expect("us");
            let range = TimeInterval::try_new(
                at("2026-10-30T00:00:00-04:00"),
                at("2026-11-03T00:00:00-05:00"),
            )
            .expect("window");
            let result = run(adapter(&server).with_candle_sessions(calendar).candles(
                &inst,
                range,
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            ));
            assert_eq!(result.expect("local calendar dates").len(), 2);
            let _captured = server.finish().expect("requests");
        }
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
        let clock = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&clock),
            clock,
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
            master(now),
        )
    }
    #[test]
    fn review1_daily_status_boundary() {
        for (now, status) in [
            ("2026-03-25T15:29:59+09:00", CandleStatus::InProgress),
            ("2026-03-25T15:30:00+09:00", CandleStatus::Finalized),
        ] {
            let server = serve(&[fixture("daily")["body"].clone()]);
            let result = run(adapter_at(&server, at(now))
                .with_candle_sessions(sessions())
                .candles(
                    &instrument(),
                    window(),
                    CandlePeriod::Daily,
                    PriceAdjustment::Raw,
                ))
            .expect("daily");
            assert_eq!(result[1].status(), status);
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn review1_zero_current_price_is_closed_until_policy_is_confirmed() {
        for value in ["0", "-0.00", "-1"] {
            let mut body = fixture("quote")["body"].clone();
            body["result"][0]["lastPrice"] = json!(value);
            let server = serve(&[body]);
            let reader = adapter(&server);
            let rows = run(reader.quotes(&[instrument()])).expect("batch");
            assert_eq!(
                rows[0].result.as_ref().expect_err("invalid price").kind(),
                application::PortErrorKind::InvalidResponse(
                    application::ResponseMismatch::Validation
                )
            );
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn review2_local_session_absence_is_an_unsupported_diagnostic() {
        let server = serve(&[fixture("daily")["body"].clone()]);
        let reader = adapter(&server);
        let error = run(reader.candles(
            &instrument(),
            window(),
            CandlePeriod::Daily,
            PriceAdjustment::Raw,
        ))
        .expect_err("missing session");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::Unsupported(application::BrokerCapability::DailyCandles)
        );
        let _captured = server.finish().expect("requests");
        let capabilities = TossAccountRead::<Store, Time, Renewal>::capabilities();
        for capability in [
            application::BrokerCapability::DailyCandles,
            application::BrokerCapability::MinuteCandles,
            application::BrokerCapability::AdjustedHistory,
            application::BrokerCapability::RawHistory,
        ] {
            assert_eq!(
                capabilities.support_for(capability),
                application::CapabilitySupport::Supported
            );
        }
    }
    #[test]
    fn review2_relative_move_boundaries_include_exact_endpoints() {
        for (value, expected) in [
            ("50400", 2_usize),
            ("93600", 2),
            ("50399", 0),
            ("93601", 0),
            ("36000", 0),
        ] {
            let mut body = fixture("daily")["body"].clone();
            for (field, prior) in [
                ("openPrice", "70000"),
                ("highPrice", "75000"),
                ("lowPrice", "65000"),
                ("closePrice", "72000"),
            ] {
                body["result"]["candles"][1][field] = json!(prior);
                body["result"]["candles"][0][field] = json!(value);
            }
            let server = serve(&[body]);
            let reader = adapter(&server).with_candle_sessions(sessions());
            let result = run(reader.candles(
                &instrument(),
                window(),
                CandlePeriod::Daily,
                PriceAdjustment::Adjusted(AdjustmentBasis::new(window().end())),
            ));
            if expected == 0 {
                invalid_response(result);
            } else {
                assert_eq!(result.expect("within guard").len(), expected);
            }
            let _captured = server.finish().expect("requests");
        }
    }

    fn us_sessions(dates: [(&str, &str); 4]) -> Vec<MarketSession> {
        dates
            .into_iter()
            .enumerate()
            .map(|(index, (date, offset))| {
                let open = at(&format!("{date}T09:30:00{offset}"));
                let close = at(&format!("{date}T16:00:00{offset}"));
                let regular = (index == 0 || index == 3).then(|| {
                    SessionData::try_new(TimeInterval::try_new(open, close).expect("range"), None)
                        .expect("data")
                });
                MarketSession::try_new(
                    open.offset_calendar_date(),
                    SessionInformation::Us {
                        day_market: None,
                        pre_market: None,
                        regular_market: regular,
                        after_market: None,
                    },
                )
                .expect("session")
            })
            .collect()
    }
    fn us_daily_body(left: &str, right: &str) -> Value {
        let mut body = fixture("daily")["body"].clone();
        body["result"]["nextBefore"] = Value::Null;
        body["result"]["candles"][0]["timestamp"] = json!(right);
        body["result"]["candles"][1]["timestamp"] = json!(left);
        for row in body["result"]["candles"].as_array_mut().expect("rows") {
            row["currency"] = json!("USD");
        }
        body
    }
    #[test]
    fn review1_dst_spring_keeps_calendar_dates() {
        let body = us_daily_body("2026-03-06T00:00:00-05:00", "2026-03-09T00:00:00-04:00");
        let calendar = us_sessions([
            ("2026-03-06", "-05:00"),
            ("2026-03-07", "-05:00"),
            ("2026-03-08", "-04:00"),
            ("2026-03-09", "-04:00"),
        ]);
        let server = serve(&[body]);
        let reader = adapter(&server).with_candle_sessions(calendar);
        let requested = master(window().start()).instruments()[1]
            .quote_instrument()
            .expect("us");
        let range = TimeInterval::try_new(
            at("2026-03-06T00:00:00-05:00"),
            at("2026-03-10T00:00:00-04:00"),
        )
        .expect("window");
        let rows =
            run(reader.candles(&requested, range, CandlePeriod::Daily, PriceAdjustment::Raw))
                .expect("us candles");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            *rows[0].time().timestamp().offset_seconds().as_i32(),
            -18_000_i32
        );
        assert_eq!(
            *rows[1].time().timestamp().offset_seconds().as_i32(),
            -14_400_i32
        );
        let _captured = server.finish().expect("requests");
    }
    #[test]
    fn review2_us_adjusted_daily_moves_keep_unlimited_policy() {
        let mut body = us_daily_body("2026-10-30T00:00:00-04:00", "2026-11-02T00:00:00-05:00");
        for key in ["openPrice", "highPrice", "lowPrice", "closePrice"] {
            body["result"]["candles"][0][key] = json!("36000");
        }
        let calendar = us_sessions([
            ("2026-10-30", "-04:00"),
            ("2026-10-31", "-04:00"),
            ("2026-11-01", "-05:00"),
            ("2026-11-02", "-05:00"),
        ]);
        let requested = master(window().start()).instruments()[1]
            .quote_instrument()
            .expect("us");
        let range = TimeInterval::try_new(
            at("2026-10-30T00:00:00-04:00"),
            at("2026-11-03T00:00:00-05:00"),
        )
        .expect("window");
        let basis = AdjustmentBasis::new(range.end());
        let server = serve(&[body]);
        let reader = adapter(&server).with_candle_sessions(calendar);
        let rows = run(reader.candles(
            &requested,
            range,
            CandlePeriod::Daily,
            PriceAdjustment::Adjusted(basis),
        ))
        .expect("us candles");
        assert_eq!(rows.len(), 2);
        for row in rows {
            assert_eq!(row.values().currency(), Currency::Usd);
            assert_eq!(row.instrument(), requested.instrument_id());
            assert_eq!(row.adjustment(), PriceAdjustment::Adjusted(basis));
        }
        let _captured = server.finish().expect("requests");
    }
    #[test]
    fn review2_previous_close_controls_the_day_band() {
        let mut body = fixture("daily")["body"].clone();
        for (index, value) in [(1, "100"), (0, "120")] {
            for field in ["openPrice", "highPrice", "lowPrice", "closePrice"] {
                body["result"]["candles"][index][field] = json!(value);
            }
        }
        let server = serve(&[body]);
        let reader = adapter(&server).with_candle_sessions(sessions());
        let rows = run(reader.candles(
            &instrument(),
            window(),
            CandlePeriod::Daily,
            PriceAdjustment::Adjusted(AdjustmentBasis::new(window().end())),
        ))
        .expect("relative move");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].values().close(),
            Price::parse(&NumericInput::try_new("100").expect("input"), Currency::Krw)
                .expect("price")
        );
        assert_eq!(
            rows[1].values().close(),
            Price::parse(&NumericInput::try_new("120").expect("input"), Currency::Krw)
                .expect("price")
        );
        let _captured = server.finish().expect("requests");
    }
    #[test]
    fn review2_relative_guard_checks_low_and_high() {
        for (field, value) in [("lowPrice", "50399"), ("highPrice", "93601")] {
            let mut body = fixture("daily")["body"].clone();
            for key in ["openPrice", "highPrice", "lowPrice", "closePrice"] {
                body["result"]["candles"][1][key] = json!("72000");
            }
            body["result"]["candles"][0][field] = json!(value);
            let server = serve(&[body]);
            let reader = adapter(&server).with_candle_sessions(sessions());
            invalid_response(run(reader.candles(
                &instrument(),
                window(),
                CandlePeriod::Daily,
                PriceAdjustment::Adjusted(AdjustmentBasis::new(window().end())),
            )));
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn review2_outside_daily_rows_do_not_require_extra_sessions() {
        for boundary in ["2026-03-23T00:00:00+09:00", "2026-03-26T00:00:00+09:00"] {
            let mut body = fixture("daily")["body"].clone();
            body["result"]["nextBefore"] = Value::Null;
            let mut extra = body["result"]["candles"][0].clone();
            extra["timestamp"] = json!(boundary);
            let rows = body["result"]["candles"].as_array_mut().expect("rows");
            rows.push(extra);
            rows.sort_by(|left, right| {
                right["timestamp"]
                    .as_str()
                    .expect("time")
                    .cmp(left["timestamp"].as_str().expect("time"))
            });
            let server = serve(&[body]);
            let reader = adapter(&server).with_candle_sessions(sessions());
            let result = run(reader.candles(
                &instrument(),
                window(),
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            ))
            .expect("only in-window sessions needed");
            assert_eq!(result.len(), 2);
            assert_eq!(result[0].time().timestamp(), window().start());
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn review2_adjusted_daily_uses_existing_master_without_historical_limits() {
        let server = serve(&[fixture("daily")["body"].clone()]);
        let reader =
            adapter_at(&server, crate::support::time().now()).with_candle_sessions(sessions());
        let rows = run(reader.candles(
            &instrument(),
            window(),
            CandlePeriod::Daily,
            PriceAdjustment::Adjusted(AdjustmentBasis::new(window().end())),
        ))
        .expect("supplied master and sessions are sufficient");
        assert_eq!(rows.len(), 2);
        let _captured = server.finish().expect("requests");
    }
    #[test]
    fn review2_outside_rows_keep_numeric_currency_time_and_order_validation() {
        for (field, value) in [
            ("openPrice", "0"),
            ("highPrice", "1"),
            ("volume", "-1"),
            ("currency", "USD"),
            ("timestamp", "2026-03-23T00:00:01+09:00"),
            ("timestamp", "2026-03-24T00:00:00+09:00"),
        ] {
            let mut body = fixture("daily")["body"].clone();
            body["result"]["nextBefore"] = Value::Null;
            let mut extra = body["result"]["candles"][0].clone();
            extra["timestamp"] = json!("2026-03-23T00:00:00+09:00");
            extra[field] = json!(value);
            body["result"]["candles"]
                .as_array_mut()
                .expect("rows")
                .push(extra);
            let server = serve(&[body]);
            let reader = adapter(&server).with_candle_sessions(sessions());
            invalid_response(run(reader.candles(
                &instrument(),
                window(),
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            )));
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn review2_adjusted_guard_requires_matching_supported_master() {
        let mut unsupported = master(window().start()).instruments()[0].row().clone();
        unsupported.listing_status = ListingStatus::Delisted;
        for facts in [
            MasterInstrumentFacts::try_new(Vec::new()).expect("empty facts"),
            MasterInstrumentFacts::try_new(vec![
                MasterInstrument::try_new(unsupported).expect("master"),
            ])
            .expect("facts"),
        ] {
            let server = serve(&[fixture("daily")["body"].clone()]);
            let reader = adapter_with_master(&server, facts).with_candle_sessions(sessions());
            let error = run(reader.candles(
                &instrument(),
                window(),
                CandlePeriod::Daily,
                PriceAdjustment::Adjusted(AdjustmentBasis::new(window().end())),
            ))
            .expect_err("missing policy");
            assert_eq!(
                error.kind(),
                application::PortErrorKind::Unsupported(
                    application::BrokerCapability::AdjustedHistory,
                )
            );
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn history_candles_accept_past_rows_without_sessions_in_either_order() {
        let mut body = fixture("daily")["body"].clone();
        let rows = body["result"]["candles"].as_array().expect("rows").clone();
        body["result"]["candles"] = json!(rows.into_iter().rev().collect::<Vec<_>>());
        let server = serve(&[body]);
        let candles =
            run(adapter(&server).history_daily_candles(&instrument(), window())).expect("history");
        assert_eq!(candles.len(), 2);
        assert_eq!(candles[0].time().timestamp(), window().start());
        assert!(
            candles
                .iter()
                .all(|candle| candle.status() == CandleStatus::Finalized
                    && candle.adjustment() == PriceAdjustment::Raw)
        );
        let requests = server.finish().expect("requests");
        assert!(String::from_utf8_lossy(&requests).contains("adjusted=false"));
    }
    #[test]
    fn history_candles_skip_only_the_broken_and_future_rows() {
        let mut body = fixture("daily")["body"].clone();
        let mut broken = body["result"]["candles"][0].clone();
        broken["timestamp"] = json!("2026-03-25T06:00:00+09:00");
        broken["highPrice"] = json!("1");
        let mut future = body["result"]["candles"][0].clone();
        future["timestamp"] = json!("2030-01-01T00:00:00+09:00");
        let rows: Vec<Value> = [future, broken]
            .into_iter()
            .chain(body["result"]["candles"].as_array().expect("rows").clone())
            .collect();
        body["result"]["candles"] = json!(rows);
        let server = serve(&[body]);
        let wide = TimeInterval::try_new(
            at("2026-03-24T00:00:00+09:00"),
            at("2031-01-01T00:00:00+09:00"),
        )
        .expect("window");
        let candles =
            run(adapter(&server).history_daily_candles(&instrument(), wide)).expect("history");
        let times: Vec<_> = candles
            .iter()
            .map(|candle| candle.time().timestamp())
            .collect();
        assert_eq!(
            times,
            vec![
                at("2026-03-24T00:00:00+09:00"),
                at("2026-03-25T00:00:00+09:00")
            ]
        );
    }
    #[test]
    fn history_candles_normalize_session_stamped_rows_to_their_local_day() {
        let mut body = fixture("daily")["body"].clone();
        body["result"]["candles"][0]["timestamp"] = json!("2026-03-25T09:00:00+09:00");
        body["result"]["candles"][1]["timestamp"] = json!("2026-03-24T15:30:00+09:00");
        let server = serve(&[body]);
        let candles =
            run(adapter(&server).history_daily_candles(&instrument(), window())).expect("history");
        let times: Vec<_> = candles
            .iter()
            .map(|candle| candle.time().timestamp())
            .collect();
        assert_eq!(
            times,
            vec![
                at("2026-03-24T00:00:00+09:00"),
                at("2026-03-25T00:00:00+09:00")
            ]
        );
    }
}
