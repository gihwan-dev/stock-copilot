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
    };
    use domain::{KnownMarket, SectorId};
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
    fn master() -> MasterInstrumentFacts {
        use domain::{
            CommonShare, Currency, DisplayInstrumentName, InstrumentId, KoreanMasterDetail,
            LiquidationTrading, ListingStatus, MasterCode, MasterInstrument, MasterInstrumentRow,
            MasterSecurityType, NxtTrading, Symbol, TradingSuspension,
        };
        let observed = crate::support::time().now();
        let rows = ["091160", "396500"].map(|symbol| {
            MasterInstrument::try_new(MasterInstrumentRow {
                listing_market: MasterCode::try_new("KOSPI").expect("market"),
                symbol: Symbol::try_new(symbol).expect("symbol"),
                instrument_id: InstrumentId::try_new(&format!("synthetic-{symbol}")).expect("id"),
                name: DisplayInstrumentName::try_new("합성 ETF").expect("name"),
                security_type: MasterSecurityType::from_code(
                    MasterCode::try_new("ETF").expect("type"),
                ),
                currency: Currency::Krw,
                listing_status: ListingStatus::Active,
                common_share: CommonShare::Common,
                korean_detail: Some(KoreanMasterDetail {
                    liquidation: LiquidationTrading::Regular,
                    krx: TradingSuspension::Clear,
                    nxt: NxtTrading::Unsupported,
                }),
                observed_at: observed,
            })
            .expect("master")
        });
        MasterInstrumentFacts::try_new(rows.to_vec()).expect("facts")
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
    fn recording(name: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(format!("tests/market_recordings/t09-{name}.json"))
                .expect("fixture"),
        )
        .expect("json")
    }
    fn id() -> SectorId {
        SectorId::try_new("100").expect("sector")
    }
    fn fetch(route: usize, server: &ScriptedHttpServer) -> Result<(), application::PortError> {
        let reader = adapter(server);
        match route {
            0 => run(reader.sectors()).map(|_| ()),
            1 => run(reader.sector(&id(), KnownMarket::Kr)).map(|_| ()),
            2 => run(reader.sector_composition(&id(), KnownMarket::Kr)).map(|_| ()),
            _ => run(reader.sector_etfs(&id(), KnownMarket::Kr)).map(|_| ()),
        }
    }
    #[test]
    fn sector_catalog_preserves_hierarchy_and_recordings_match_schema() {
        for name in ["sectors", "sector", "sector-stocks", "sector-etfs"] {
            let fixture = recording(name);
            crate::support_tests::validation::validate_recording(
                &crate::support_tests::spec(),
                &fixture,
            )
            .expect("schema");
        }
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("sectors")["body"]),
        ])
        .expect("server");
        let result = run(adapter(&server).sectors()).expect("catalog");
        assert_eq!(result.sectors().len(), 4);
        assert_eq!(result.sectors()[0].id().as_str(), "10");
        assert_eq!(result.sectors()[2].id().as_str(), "100");
        assert_eq!(result.sectors()[2].parent().expect("parent").as_str(), "10");
        server.finish().expect("requests");
    }
    #[test]
    fn sector_detail_preserves_identity_market_and_six_rates() {
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("sector")["body"]),
        ])
        .expect("server");
        let result = run(adapter(&server).sector(&id(), KnownMarket::Kr)).expect("detail");
        assert_eq!(result.metadata().id(), &id());
        assert_eq!(result.market(), KnownMarket::Kr);
        assert_eq!(result.rates().len(), 6);
        assert_eq!(result.rates()[0].duration(), domain::RankingDuration::Day);
        assert_eq!(result.rates()[5].duration(), domain::RankingDuration::Year);
        assert_eq!(
            *result.rates()[0].change_rate(),
            domain::RankingChange::Available(
                domain::Ratio::parse(&domain::NumericInput::try_new("0.0179").expect("input"))
                    .expect("ratio")
            )
        );
        assert_eq!(
            *result.rates()[4].change_rate(),
            domain::RankingChange::Unavailable
        );
        assert_eq!(
            *result.rates()[5].change_rate(),
            domain::RankingChange::Available(
                domain::Ratio::parse(&domain::NumericInput::try_new("0.3542").expect("input"))
                    .expect("ratio")
            )
        );
        let captured = server.finish().expect("requests");
        assert!(
            String::from_utf8_lossy(&captured).contains("/api/v1/sectors/100?marketCountry=KR")
        );
    }
    #[test]
    fn sector_stocks_preserve_symbols_and_market() {
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("sector-stocks")["body"]),
        ])
        .expect("server");
        let result =
            run(adapter(&server).sector_composition(&id(), KnownMarket::Kr)).expect("stocks");
        assert_eq!(result.symbols()[0].as_str(), "005930");
        assert_eq!(result.symbols()[1].as_str(), "000660");
        assert_eq!(result.market(), KnownMarket::Kr);
        let money = |text| {
            domain::SectorAmount::Available(
                domain::Money::parse(
                    &domain::NumericInput::try_new(text).expect("input"),
                    domain::Currency::Krw,
                )
                .expect("money"),
            )
        };
        assert_eq!(
            result.stocks()[0].market_capitalization(),
            money("388035900000000")
        );
        assert_eq!(result.stocks()[0].trading_amount(), money("1041436650000"));
        assert_eq!(
            result.stocks()[1].market_capitalization(),
            money("140150000000000")
        );
        assert_eq!(result.stocks()[1].trading_amount(), money("927910250000"));
        server.finish().expect("requests");
    }
    #[test]
    fn sector_etfs_preserve_symbols_and_sector() {
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("sector-etfs")["body"]),
        ])
        .expect("server");
        let result = run(adapter(&server).sector_etfs(&id(), KnownMarket::Kr)).expect("etfs");
        assert_eq!(result.etfs()[0].symbol().as_str(), "091160");
        assert_eq!(result.etfs()[1].symbol().as_str(), "396500");
        assert_eq!(result.etfs()[0].name().as_str(), "KODEX 반도체");
        assert_eq!(result.etfs()[1].name().as_str(), "TIGER Fn반도체TOP10");
        assert_eq!(result.sector(), &id());
        server.finish().expect("requests");
    }
    #[test]
    fn sectors_reject_bad_hierarchy_identity_and_numbers() {
        let mut catalog = recording("sectors")["body"].clone();
        catalog["result"]["sectors"][2]["parentSectorId"] = json!("999");
        let mut detail = recording("sector")["body"].clone();
        detail["result"]["sectorId"] = json!("101");
        let mut stocks = recording("sector-stocks")["body"].clone();
        stocks["result"]["stocks"][0]["marketCapitalization"] = json!("bad");
        let mut etfs = recording("sector-etfs")["body"].clone();
        etfs["result"]["etfs"][1]["symbol"] = etfs["result"]["etfs"][0]["symbol"].clone();
        for (route, input) in [catalog, detail, stocks, etfs].into_iter().enumerate() {
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)])
                .expect("server");
            fetch(route, &server).expect_err("invalid");
            server.finish().expect("requests");
        }
    }
    #[test]
    fn sector_routes_handle_401_and_429_through_shared_pipeline() {
        for (route, name) in ["sectors", "sector", "sector-stocks", "sector-etfs"]
            .into_iter()
            .enumerate()
        {
            let revoked =
                json!({"error":{"code":"token-revoked","message":"synthetic","requestId":"fake"}});
            let server = ScriptedHttpServer::start(vec![
                issued(),
                response("401 Unauthorized", &revoked),
                issued(),
                response("200 OK", &recording(name)["body"]),
            ])
            .expect("server");
            fetch(route, &server).expect("refresh");
            server.finish().expect("requests");
            let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
            let mut responses = vec![issued()];
            responses.extend((0_usize..4).map(|_| response("429 Too Many Requests", &limited)));
            let limited_server = ScriptedHttpServer::start(responses).expect("server");
            assert_rate_limit(fetch(route, &limited_server).expect_err("rate limited"));
            limited_server.finish().expect("requests");
        }
    }
    #[test]
    fn recording_replays_using_t20_transport() {
        assert_ne!(crate::support_tests::recordings().len(), 0);
        let fixture = recording("sectors");
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
    async fn held(gate: testkit::ResponseGate) -> testkit::ResponseGate {
        loop {
            match gate.observe() {
                testkit::GateObservation::Held => return gate,
                testkit::GateObservation::Waiting => tokio::task::yield_now().await,
                testkit::GateObservation::Closed => panic!("gate closed"),
            }
        }
    }
    #[test]
    fn stock_all_limiter_delays_second_request_at_least_one_second() {
        let body = recording("stocks-all")["body"].clone();
        let replies = vec![response("200 OK", &body), response("200 OK", &body)];
        let (server, gate) =
            ScriptedHttpServer::start_holding(replies, testkit::HoldPosition::new(1))
                .expect("server");
        let clock = crate::support::time();
        let client = Arc::new(
            TossHttpClient::new(
                &TossEndpoint::local(server.port()).expect("local"),
                Arc::<crate::support::FakeTime>::clone(&clock),
                Arc::<crate::support::FakeTime>::clone(&clock),
            )
            .expect("client"),
        );
        let observed = run(async {
            let first_request = broker_toss::ReadRequest::new(broker_toss::ReadRoute::Instruments)
                .with_query(broker_toss::QueryKey::Market, "KOSPI".to_owned());
            client
                .get(&first_request, &SecretValue::new(b"fake-token".to_vec()))
                .await
                .expect("first");
            let first_time = clock.now();
            let second_client = Arc::<TossHttpClient>::clone(&client);
            let task = tokio::spawn(async move {
                let request = broker_toss::ReadRequest::new(broker_toss::ReadRoute::Instruments)
                    .with_query(broker_toss::QueryKey::Market, "KOSPI".to_owned());
                second_client
                    .get(&request, &SecretValue::new(b"fake-token".to_vec()))
                    .await
            });
            let held_gate = tokio::time::timeout(std::time::Duration::from_secs(5), held(gate))
                .await
                .expect("second request received");
            let at_receipt = clock.now();
            held_gate.release().expect("release");
            task.await.expect("task").expect("second");
            first_time.elapsed_until(at_receipt).expect("elapsed")
        });
        assert!(
            observed
                >= domain::TimeSpan::try_new(
                    domain::SpanSeconds::new(1),
                    domain::Nanosecond::new(0)
                )
                .expect("second")
        );
        let captured = server.finish().expect("requests");
        assert_eq!(
            String::from_utf8_lossy(&captured)
                .matches("GET /api/v1/stocks/all?")
                .count(),
            2
        );
    }

    #[test]
    fn invalid_sector_path_is_rejected_before_transport() {
        let server = ScriptedHttpServer::start(vec![]).expect("server");
        let reader = adapter(&server);
        for text in [
            "../accounts",
            "a?count=1",
            "x#y",
            "a/b",
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNO",
        ] {
            let sector = SectorId::try_new(text).expect("domain id");
            let detail = run(reader.sector(&sector, KnownMarket::Kr)).expect_err("invalid");
            assert_eq!(
                detail.kind(),
                application::PortErrorKind::InvalidRequest(
                    application::RequestRejection::MalformedRequest
                )
            );
            let composition =
                run(reader.sector_composition(&sector, KnownMarket::Kr)).expect_err("invalid");
            assert_eq!(
                composition.kind(),
                application::PortErrorKind::InvalidRequest(
                    application::RequestRejection::MalformedRequest
                )
            );
            let etfs = run(reader.sector_etfs(&sector, KnownMarket::Kr)).expect_err("invalid");
            assert_eq!(
                etfs.kind(),
                application::PortErrorKind::InvalidRequest(
                    application::RequestRejection::MalformedRequest
                )
            );
        }
        assert_eq!(server.finish().expect("requests").len(), 0);
    }

    #[test]
    fn sector_etfs_require_matching_master_in_the_requested_market() {
        let mut input = recording("sector-etfs")["body"].clone();
        input["result"]["etfs"][0]["symbol"] = json!("123456");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)]).expect("server");
        let error =
            run(adapter(&server).sector_etfs(&id(), KnownMarket::Kr)).expect_err("unknown master");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        server.finish().expect("requests");
    }
    #[test]
    fn generated_sector_models_decode_recordings_and_reject_invalid_shapes() {
        let catalog = recording("sectors")["body"]["result"].clone();
        let detail = recording("sector")["body"]["result"].clone();
        let stocks = recording("sector-stocks")["body"]["result"].clone();
        let etfs = recording("sector-etfs")["body"]["result"].clone();
        let _catalog: broker_toss_schema::types::SectorsResponse =
            serde_json::from_value(catalog.clone()).expect("catalog");
        let _detail: broker_toss_schema::types::SectorDetailResponse =
            serde_json::from_value(detail.clone()).expect("detail");
        let _stocks: broker_toss_schema::types::SectorStocksResponse =
            serde_json::from_value(stocks.clone()).expect("stocks");
        let _etfs: broker_toss_schema::types::SectorEtfsResponse =
            serde_json::from_value(etfs.clone()).expect("etfs");
        let mut bad = catalog;
        bad["sectors"][0]["depth"] = json!("bad");
        serde_json::from_value::<broker_toss_schema::types::SectorsResponse>(bad)
            .expect_err("invalid catalog");
        let mut bad_detail = detail;
        bad_detail["performance"]["changeRates"] = json!("bad");
        serde_json::from_value::<broker_toss_schema::types::SectorDetailResponse>(bad_detail)
            .expect_err("invalid detail");
        let mut bad_stocks = stocks;
        bad_stocks["stocks"][0]["currency"] = json!(false);
        serde_json::from_value::<broker_toss_schema::types::SectorStocksResponse>(bad_stocks)
            .expect_err("invalid stocks");
        let mut bad_etfs = etfs;
        bad_etfs["etfs"][0]["symbol"] = json!(42_i32);
        serde_json::from_value::<broker_toss_schema::types::SectorEtfsResponse>(bad_etfs)
            .expect_err("invalid etfs");
    }
    #[test]
    fn sector_detail_preserves_us_market_in_request_and_result() {
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("sector")["body"]),
        ])
        .expect("server");
        let result = run(adapter(&server).sector(&id(), KnownMarket::Us)).expect("detail");
        assert_eq!(result.market(), KnownMarket::Us);
        let captured = server.finish().expect("requests");
        assert!(String::from_utf8_lossy(&captured).contains("marketCountry=US"));
    }
    #[test]
    fn sector_responses_reject_unusable_catalog_and_parent_ids() {
        let catalog = json!({"result":{"sectors":[{"sectorId":"../accounts", "name":"합성", "depth":0_i32, "parentSectorId":null}]}});
        let catalog_server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &catalog)])
                .expect("server");
        let error = run(adapter(&catalog_server).sectors()).expect_err("invalid catalog id");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        catalog_server.finish().expect("requests");
        let mut detail = recording("sector")["body"].clone();
        detail["result"]["parentSectorId"] = json!("../accounts");
        let detail_server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &detail)]).expect("server");
        let detail_error = run(adapter(&detail_server).sector(&id(), KnownMarket::Kr))
            .expect_err("invalid parent id");
        assert_eq!(
            detail_error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        detail_server.finish().expect("requests");
    }
    fn many_sectors(count: usize) -> Value {
        let rows = (0..count).map(|index| json!({"sectorId":format!("{index}"), "name":"합성", "depth":0_i32, "parentSectorId":null})).collect::<Vec<_>>();
        json!({"result":{"sectors":rows}})
    }
    #[test]
    fn sector_catalog_bounds_rows_at_8192() {
        let allowed_server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &many_sectors(8192))])
                .expect("server");
        let catalog = run(adapter(&allowed_server).sectors()).expect("boundary");
        assert_eq!(catalog.sectors().len(), 8192);
        allowed_server.finish().expect("requests");
        let denied_server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &many_sectors(8193))])
                .expect("server");
        let error = run(adapter(&denied_server).sectors()).expect_err("over limit");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        denied_server.finish().expect("requests");
    }
}
