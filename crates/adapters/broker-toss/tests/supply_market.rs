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
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossDate, TossEndpoint,
        TossHttpClient,
    };
    use domain::QuoteInstrument;
    use domain::{MasterInstrumentFacts, Timestamp};
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
    fn instrument() -> QuoteInstrument {
        use domain::{
            CommonShare, Currency, DisplayInstrumentName, InstrumentId, KoreanMasterDetail,
            LiquidationTrading, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
            MasterSecurityType, NxtTrading, Symbol, TradingSuspension,
        };
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("market"),
            symbol: Symbol::try_new("005930").expect("symbol"),
            instrument_id: InstrumentId::try_new("synthetic-id").expect("id"),
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
            observed_at: crate::support::time().now(),
        })
        .expect("master")
        .quote_instrument()
        .expect("quote")
    }
    fn date(text: &str) -> domain::CalendarDate {
        TossDate::try_from(text).expect("date").into_domain()
    }
    fn recording(name: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(format!("tests/market_recordings/t10-{name}.json"))
                .expect("file"),
        )
        .expect("json")
    }
    #[test]
    fn supply_recordings_preserve_date_symbol_and_distinct_units() {
        for name in ["program", "short", "credit", "lending"] {
            crate::support_tests::validation::validate_recording(
                &crate::support_tests::spec(),
                &recording(name),
            )
            .expect("schema");
        }
        let program_server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("program")["body"]),
        ])
        .expect("server");
        let program = run(adapter(&program_server).program_trades(
            &instrument(),
            date("2026-07-16"),
            date("2026-07-17"),
        ))
        .expect("program");
        assert_eq!(program.len(), 2);
        assert_eq!(program[1].context().market(), domain::KnownMarket::Kr);
        assert_eq!(program[0].context().date(), date("2026-07-16"));
        assert_eq!(program[1].context().symbol().as_str(), "005930");
        assert_eq!(program[1].arbitrage().buy().to_string(), "152300");
        assert_eq!(program[1].non_arbitrage().net().to_string(), "105300");
        let captured = program_server.finish().expect("requests");
        assert!(
            String::from_utf8_lossy(&captured).contains("/api/v1/stocks/005930/program-trades?")
        );
        let short_server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("short")["body"]),
        ])
        .expect("server");
        let short = run(adapter(&short_server).short_selling(
            &instrument(),
            date("2026-07-15"),
            date("2026-07-16"),
        ))
        .expect("short");
        assert_eq!(short.len(), 2);
        assert_eq!(short[1].context().market(), domain::KnownMarket::Kr);
        assert_eq!(short[1].volume().to_string(), "512300");
        assert_eq!(short[1].amount().to_string(), "41250000000");
        assert_eq!(
            short[1].context().observation().server_at(),
            Some(
                broker_toss::TossTimestamp::try_from("2026-07-16T17:25:43+09:00")
                    .expect("time")
                    .into_domain()
            )
        );
        short_server.finish().expect("requests");
    }
    #[test]
    fn credit_and_lending_preserve_distinct_units() {
        let credit_server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("credit")["body"]),
        ])
        .expect("server");
        let credit = run(adapter(&credit_server).credit_trades(
            &instrument(),
            date("2026-07-15"),
            date("2026-07-16"),
        ))
        .expect("credit");
        let domain::CreditSide::Reported(margin) = credit[1].margin_loan() else {
            panic!("margin")
        };
        assert_eq!(margin.new_quantity().to_string(), "125300");
        assert_eq!(margin.trading_rate().to_string(), "0.09");
        credit_server.finish().expect("requests");
        let lending_server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("lending")["body"]),
        ])
        .expect("server");
        let lending = run(adapter(&lending_server).securities_lending(
            &instrument(),
            date("2026-07-16"),
            date("2026-07-17"),
        ))
        .expect("lending");
        assert_eq!(lending.len(), 2);
        assert_eq!(lending[1].execution_quantity().to_string(), "210500");
        assert_eq!(lending[1].balance_amount().to_string(), "1218720000000");
        lending_server.finish().expect("requests");
    }
    fn call(route: usize, server: &ScriptedHttpServer) -> Result<(), application::PortError> {
        let reader = adapter(server);
        let instrument = instrument();
        let from = date(if route == 1 || route == 2 {
            "2026-07-15"
        } else {
            "2026-07-16"
        });
        let through = date(if route == 1 || route == 2 {
            "2026-07-16"
        } else {
            "2026-07-17"
        });
        match route {
            0 => run(reader.program_trades(&instrument, from, through)).map(|_| ()),
            1 => run(reader.short_selling(&instrument, from, through)).map(|_| ()),
            2 => run(reader.credit_trades(&instrument, from, through)).map(|_| ()),
            _ => run(reader.securities_lending(&instrument, from, through)).map(|_| ()),
        }
    }
    #[test]
    fn supply_errors_reject_bad_rows_and_handle_401_and_429() {
        for (route, name) in ["program", "short", "credit", "lending"]
            .into_iter()
            .enumerate()
        {
            let mut input = recording(name)["body"].clone();
            match route {
                0 => input["result"]["records"][0]["arbitrage"]["netBuyVolume"] = json!("1"),
                1 => input["result"]["records"][0]["shortSellingAmount"] = json!("bad"),
                2 => input["result"]["records"][0]["marginLoan"]["newQuantity"] = json!("-1"),
                _ => input["result"]["records"][0]["balanceAmount"] = json!("-1"),
            }
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)])
                .expect("server");
            call(route, &server).expect_err("invalid");
            server.finish().expect("requests");
            let revoked =
                json!({"error":{"code":"token-revoked","message":"synthetic","requestId":"fake"}});
            let retry_server = ScriptedHttpServer::start(vec![
                issued(),
                response("401 Unauthorized", &revoked),
                issued(),
                response("200 OK", &recording(name)["body"]),
            ])
            .expect("server");
            call(route, &retry_server).expect("refresh");
            retry_server.finish().expect("requests");
            let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
            let mut responses = vec![issued()];
            responses.extend((0_usize..4).map(|_| response("429 Too Many Requests", &limited)));
            let limited_server = ScriptedHttpServer::start(responses).expect("server");
            let error = call(route, &limited_server).expect_err("limited");
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
                    )
                }
            );
            limited_server.finish().expect("requests");
        }
    }
    #[test]
    fn supply_pages_are_ordered_bounded_and_cursor_is_used() {
        let fixture = recording("program")["body"].clone();
        let mut first = fixture.clone();
        let mut second = fixture;
        first["result"]["records"] = json!([first["result"]["records"][0].clone()]);
        first["result"]["nextUntil"] = json!("2026-07-16");
        second["result"]["records"] = json!([second["result"]["records"][1].clone()]);
        second["result"]["nextUntil"] = Value::Null;
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &first),
            response("200 OK", &second),
        ])
        .expect("server");
        let values = run(adapter(&server).program_trades(
            &instrument(),
            date("2026-07-16"),
            date("2026-07-17"),
        ))
        .expect("pages");
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].context().date(), date("2026-07-16"));
        assert_eq!(values[1].context().date(), date("2026-07-17"));
        let captured = server.finish().expect("requests");
        let captured_text = String::from_utf8_lossy(&captured);
        assert!(captured_text.contains("until=2026-07-17"));
        assert!(captured_text.contains("until=2026-07-16"));
        assert!(captured_text.contains("count=100"));
    }
    #[test]
    fn supply_recording_replays_on_transport() {
        assert_ne!(crate::support_tests::recordings().len(), 0);
        let fixture = recording("program");
        let server = crate::support_tests::serve_recordings(&fixture);
        let clock = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&clock),
            clock,
        )
        .expect("client");
        let response = run(client.get(
            &broker_toss::ReadRequest::new(crate::support_tests::route(&fixture)),
            &SecretValue::new(b"fake-token".to_vec()),
        ))
        .expect("response");
        assert_eq!(response.status(), 200);
        server.finish().expect("request");
    }

    #[test]
    fn investor_flows_preserve_units_dates_and_investor_groups() {
        let stock = recording("stock-flow");
        crate::support_tests::validation::validate_recording(&crate::support_tests::spec(), &stock)
            .expect("schema");
        let stock_server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &stock["body"])])
                .expect("server");
        let rows = run(adapter(&stock_server).stock_investor_flow(
            &instrument(),
            date("2026-07-16"),
            date("2026-07-17"),
        ))
        .expect("stock flow");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].date(), date("2026-07-16"));
        assert_eq!(rows[1].date(), date("2026-07-17"));
        assert_eq!(rows[1].market(), domain::KnownMarket::Kr);
        assert_eq!(
            rows[1].top_level()[0].investor(),
            domain::InvestorClass::Foreigner
        );
        assert_eq!(rows[1].top_level()[0].net().to_string(), "119900");
        assert_eq!(
            rows[1].foreign_scope(),
            domain::ForeignInvestorScope::RegisteredOnly
        );
        stock_server.finish().expect("requests");
    }
    #[test]
    fn indicator_flow_preserves_distinct_groups_and_amounts() {
        let indicator = recording("indicator-flow");
        crate::support_tests::validation::validate_recording(
            &crate::support_tests::spec(),
            &indicator,
        )
        .expect("schema");
        let index_server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &indicator["body"])])
                .expect("server");
        let rows = run(adapter(&index_server).indicator_investor_flow(
            domain::IndicatorCode::Kospi,
            date("2026-06-10"),
            date("2026-06-11"),
            domain::FlowInterval::Day,
        ))
        .expect("index flow");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].date(), date("2026-06-11"));
        assert_eq!(rows[1].market(), domain::KnownMarket::Kr);
        assert_eq!(rows[1].interval(), domain::FlowInterval::Day);
        assert_eq!(
            rows[1].top_level()[0].investor(),
            domain::InvestorClass::Foreigner
        );
        assert_eq!(rows[1].top_level()[0].net().to_string(), "200000000000");
        assert_eq!(
            rows[1].top_level()[1].investor(),
            domain::InvestorClass::Individual
        );
        assert_eq!(rows[1].top_level()[1].net().to_string(), "-150000000000");
        assert_eq!(
            rows[1].foreign_scope(),
            domain::ForeignInvestorScope::RegisteredAndUnregistered
        );
        let captured = index_server.finish().expect("requests");
        assert!(
            String::from_utf8_lossy(&captured)
                .contains("/api/v1/market-indicators/KOSPI/investor-trading?")
        );
    }

    fn call_flow(route: usize, server: &ScriptedHttpServer) -> Result<(), application::PortError> {
        let reader = adapter(server);
        if route == 0 {
            run(reader.stock_investor_flow(&instrument(), date("2026-07-16"), date("2026-07-17")))
                .map(|_| ())
        } else {
            run(reader.indicator_investor_flow(
                domain::IndicatorCode::Kospi,
                date("2026-06-10"),
                date("2026-06-11"),
                domain::FlowInterval::Day,
            ))
            .map(|_| ())
        }
    }
    #[test]
    fn investor_flows_reject_bad_numbers_and_handle_401_and_429() {
        for (route, name) in ["stock-flow", "indicator-flow"].into_iter().enumerate() {
            let mut input = recording(name)["body"].clone();
            if route == 0 {
                input["result"]["records"][0]["foreigner"]["netBuyVolume"] = json!("1");
            } else {
                input["result"]["records"][0]["individual"]["buyAmount"] = json!("bad");
            }
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)])
                .expect("server");
            call_flow(route, &server).expect_err("invalid");
            server.finish().expect("requests");
            let revoked =
                json!({"error":{"code":"token-revoked","message":"synthetic","requestId":"fake"}});
            let retry_server = ScriptedHttpServer::start(vec![
                issued(),
                response("401 Unauthorized", &revoked),
                issued(),
                response("200 OK", &recording(name)["body"]),
            ])
            .expect("server");
            call_flow(route, &retry_server).expect("refresh");
            retry_server.finish().expect("requests");
            let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
            let mut responses = vec![issued()];
            responses.extend((0_usize..4).map(|_| response("429 Too Many Requests", &limited)));
            let limited_server = ScriptedHttpServer::start(responses).expect("server");
            let error = call_flow(route, &limited_server).expect_err("limited");
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
                    )
                }
            );
            limited_server.finish().expect("requests");
        }
    }

    #[test]
    fn supply_pages_reject_duplicate_out_of_order_and_invalid_cursor() {
        let fixture = recording("program")["body"].clone();
        let mut duplicate = fixture.clone();
        duplicate["result"]["records"][1]["date"] =
            duplicate["result"]["records"][0]["date"].clone();
        let mut ascending = fixture.clone();
        ascending["result"]["records"]
            .as_array_mut()
            .expect("rows")
            .reverse();
        let mut invalid_cursor = fixture.clone();
        invalid_cursor["result"]["nextUntil"] = json!("2026-07-16");
        let mut future = fixture;
        future["result"]["records"][0]["date"] = json!("2026-07-18");
        for body in [duplicate, ascending, invalid_cursor, future] {
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)])
                .expect("server");
            let error = run(adapter(&server).program_trades(
                &instrument(),
                date("2026-07-16"),
                date("2026-07-17"),
            ))
            .expect_err("invalid page");
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
    fn supply_pages_do_not_discard_a_failing_later_page() {
        let fixture = recording("program")["body"].clone();
        let mut first = fixture.clone();
        first["result"]["records"] = json!([first["result"]["records"][0].clone()]);
        first["result"]["nextUntil"] = json!("2026-07-16");
        let mut second = fixture;
        second["result"]["records"] = json!([second["result"]["records"][1].clone()]);
        second["result"]["records"][0]["arbitrage"]["netBuyVolume"] = json!("invalid");
        second["result"]["nextUntil"] = Value::Null;
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &first),
            response("200 OK", &second),
        ])
        .expect("server");
        let error = run(adapter(&server).program_trades(
            &instrument(),
            date("2026-07-16"),
            date("2026-07-17"),
        ))
        .expect_err("invalid second page");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        server.finish().expect("requests");
    }
    fn us_instrument() -> QuoteInstrument {
        use domain::{
            CommonShare, Currency, DisplayInstrumentName, InstrumentId, ListingStatus, MasterCode,
            MasterInstrument, MasterInstrumentRow, MasterSecurityType, Symbol,
        };
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("NYSE").expect("market"),
            symbol: Symbol::try_new("SPY").expect("symbol"),
            instrument_id: InstrumentId::try_new("synthetic-us-etf").expect("id"),
            name: DisplayInstrumentName::try_new("합성 미국 ETF").expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("FOREIGN_ETF").expect("type"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: crate::support::time().now(),
        })
        .expect("master")
        .quote_instrument()
        .expect("quote")
    }
    #[test]
    fn unsupported_market_interval_and_reversed_dates_send_no_request() {
        let server = ScriptedHttpServer::start(vec![]).expect("server");
        let reader = adapter(&server);
        let unknown_market =
            run(reader.program_trades(&us_instrument(), date("2026-07-16"), date("2026-07-17")))
                .expect_err("US market");
        let reversed =
            run(reader.program_trades(&instrument(), date("2026-07-17"), date("2026-07-16")))
                .expect_err("dates");
        let unknown_indicator = run(reader.indicator_investor_flow(
            domain::IndicatorCode::Unknown,
            date("2026-06-10"),
            date("2026-06-11"),
            domain::FlowInterval::Day,
        ))
        .expect_err("indicator");
        let unknown_interval = run(reader.indicator_investor_flow(
            domain::IndicatorCode::Kospi,
            date("2026-06-10"),
            date("2026-06-11"),
            domain::FlowInterval::Unknown,
        ))
        .expect_err("interval");
        for error in [
            unknown_market,
            reversed,
            unknown_indicator,
            unknown_interval,
        ] {
            assert_eq!(
                error.kind(),
                application::PortErrorKind::InvalidRequest(
                    application::RequestRejection::UnsupportedQuery
                )
            );
        }
        assert_eq!(server.finish().expect("no request").len(), 0);
    }
    fn program_page_with_rows(count: i64) -> Value {
        let mut body = recording("program")["body"].clone();
        let template = body["result"]["records"][0].clone();
        let end = time::Date::from_calendar_date(2026, time::Month::July, 17).expect("date");
        let mut rows = Vec::new();
        for offset in 0_i64..count {
            let mut row = template.clone();
            row["date"] = json!(
                end.checked_sub(time::Duration::days(offset))
                    .expect("date")
                    .to_string()
            );
            rows.push(row);
        }
        body["result"]["records"] = json!(rows);
        body["result"]["nextUntil"] = Value::Null;
        body
    }
    #[test]
    fn supply_page_accepts_100_rows_and_rejects_101() {
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &program_page_with_rows(100)),
        ])
        .expect("server");
        let result = run(adapter(&server).program_trades(
            &instrument(),
            date("2026-04-01"),
            date("2026-07-17"),
        ))
        .expect("100 rows");
        assert_eq!(result.len(), 100);
        assert_eq!(
            result.first().expect("first").context().date(),
            date("2026-04-09")
        );
        server.finish().expect("requests");
        let over_server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &program_page_with_rows(101)),
        ])
        .expect("server");
        let error = run(adapter(&over_server).program_trades(
            &instrument(),
            date("2026-04-01"),
            date("2026-07-17"),
        ))
        .expect_err("101 rows");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        over_server.finish().expect("requests");
    }
    #[test]
    fn supply_page_limit_rejects_incomplete_coverage() {
        let mut responses = vec![issued()];
        let fixture = recording("program")["body"].clone();
        let end = time::Date::from_calendar_date(2026, time::Month::July, 17).expect("date");
        for offset in 0_i64..64 {
            let mut page = fixture.clone();
            let mut row = page["result"]["records"][0].clone();
            let day = end.checked_sub(time::Duration::days(offset)).expect("date");
            let next = day.checked_sub(time::Duration::days(1)).expect("date");
            row["date"] = json!(day.to_string());
            page["result"]["records"] = json!([row]);
            page["result"]["nextUntil"] = json!(next.to_string());
            responses.push(response("200 OK", &page));
        }
        let server = ScriptedHttpServer::start(responses).expect("server");
        let error = run(adapter(&server).program_trades(
            &instrument(),
            date("2026-01-01"),
            date("2026-07-17"),
        ))
        .expect_err("page cap");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        server.finish().expect("requests");
    }
}
