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
    use domain::{KnownMarket, RankingCautionFilter, RankingDuration, RankingKind, RankingLimit};
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
    fn body() -> Value {
        serde_json::from_str::<Value>(
            &std::fs::read_to_string("tests/market_recordings/t10-rankings.json").expect("file"),
        )
        .expect("fixture")["body"]
            .clone()
    }
    fn query() -> application::RankingQuery {
        application::RankingQuery::try_new(
            RankingKind::MarketTradingAmount,
            KnownMarket::Kr,
            RankingDuration::Day,
            RankingLimit::try_new(2).expect("limit"),
            RankingCautionFilter::Excluded,
        )
        .expect("query")
    }
    #[test]
    fn ranking_preserves_rank_basis_market_and_observation_time() {
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body())]).expect("server");
        let result = run(adapter(&server).rankings(&query())).expect("ranking");
        assert_eq!(result.market(), KnownMarket::Kr);
        assert_eq!(result.kind(), RankingKind::MarketTradingAmount);
        assert_eq!(result.duration(), RankingDuration::Day);
        assert_eq!(result.caution_filter(), RankingCautionFilter::Excluded);
        assert_eq!(result.items().len(), 2);
        assert_eq!(*result.items()[0].rank().as_usize(), 1);
        assert_eq!(result.items()[0].symbol().as_str(), "005930");
        assert_eq!(result.items()[1].symbol().as_str(), "000660");
        let domain::RankingBasis::Amount(amount) = result.items()[0].basis() else {
            panic!("amount")
        };
        assert_eq!(
            amount,
            domain::Money::parse(
                &domain::NumericInput::try_new("1041436650000").expect("input"),
                domain::Currency::Krw
            )
            .expect("money")
        );
        assert_eq!(
            result.observation().server_at(),
            Some(
                broker_toss::TossTimestamp::try_from("2026-06-10T14:30:00+09:00")
                    .expect("time")
                    .into_domain()
            )
        );
        let captured = server.finish().expect("requests");
        let captured_text = String::from_utf8_lossy(&captured);
        for part in [
            "/api/v1/rankings?",
            "type=MARKET_TRADING_AMOUNT",
            "marketCountry=KR",
            "duration=1d",
            "count=2",
            "excludeInvestmentCaution=true",
        ] {
            assert!(captured_text.contains(part));
        }
    }
    #[test]
    fn ranking_rejects_bad_rows_and_uses_shared_error_path() {
        for (pointer, value) in [
            ("/rankings/0/rank", json!(0_u64)),
            ("/rankings/0/tradingAmount", json!("invalid")),
            ("/rankings/0/currency", json!("USD")),
            ("/rankedAt", json!("invalid")),
        ] {
            let mut input = body();
            *input["result"].pointer_mut(pointer).expect("path") = value;
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)])
                .expect("server");
            run(adapter(&server).rankings(&query())).expect_err("invalid");
            server.finish().expect("request");
        }
        let revoked =
            json!({"error":{"code":"token-revoked","message":"synthetic","requestId":"fake"}});
        let revoked_server = ScriptedHttpServer::start(vec![
            issued(),
            response("401 Unauthorized", &revoked),
            issued(),
            response("200 OK", &body()),
        ])
        .expect("server");
        run(adapter(&revoked_server).rankings(&query())).expect("refresh");
        revoked_server.finish().expect("requests");
        let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
        let mut responses = vec![issued()];
        responses.extend((0_usize..4).map(|_| response("429 Too Many Requests", &limited)));
        let limited_server = ScriptedHttpServer::start(responses).expect("server");
        assert_eq!(
            run(adapter(&limited_server).rankings(&query()))
                .expect_err("limited")
                .kind(),
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
    #[test]
    fn ranking_fixture_matches_spec_and_replays() {
        assert_ne!(crate::support_tests::recordings().len(), 0);
        let fixture: Value = serde_json::from_str(
            &std::fs::read_to_string("tests/market_recordings/t10-rankings.json").expect("file"),
        )
        .expect("fixture");
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
        assert_eq!(
            serde_json::from_slice::<Value>(response.body().as_exposed_secret()).expect("body"),
            fixture["body"]
        );
        server.finish().expect("request");
    }
    fn investor_fixture() -> Value {
        serde_json::from_str(
            &std::fs::read_to_string("tests/market_recordings/t10-investor-rankings.json")
                .expect("file"),
        )
        .expect("json")
    }
    fn assert_investor_observations(result: &domain::InvestorTradingRankings) {
        for (group, expected) in [
            (result.foreigner(), "2026-10-20T15:41:19+09:00"),
            (result.institution(), "2026-10-20T15:37:30+09:00"),
            (result.individual(), "2026-10-20T15:39:18+09:00"),
        ] {
            assert_eq!(
                group.observation().server_at(),
                Some(
                    broker_toss::TossTimestamp::try_from(expected)
                        .expect("time")
                        .into_domain()
                )
            );
        }
        assert_eq!(
            result.foreigner().buy().items()[0]
                .amount()
                .amount()
                .as_decimal()
                .to_string(),
            "386487682000"
        );
        assert_eq!(
            result.foreigner().sell().items()[0]
                .amount()
                .amount()
                .as_decimal()
                .to_string(),
            "391201805000"
        );
    }
    #[test]
    fn investor_rankings_preserve_three_groups_sides_and_distinct_times() {
        let fixture = investor_fixture();
        crate::support_tests::validation::validate_recording(
            &crate::support_tests::spec(),
            &fixture,
        )
        .expect("schema");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &fixture["body"])])
                .expect("server");
        let result =
            run(adapter(&server)
                .investor_trading_rankings(RankingLimit::try_new(2).expect("limit")))
            .expect("rankings");
        assert_eq!(result.market(), KnownMarket::Kr);
        assert_eq!(
            result.foreigner().buy().items()[0].symbol().as_str(),
            "005930"
        );
        assert_eq!(
            result.foreigner().sell().items()[0].symbol().as_str(),
            "000660"
        );
        assert_eq!(
            result.institution().buy().items()[0].symbol().as_str(),
            "353200"
        );
        assert_eq!(
            result.individual().sell().items()[0].symbol().as_str(),
            "005380"
        );
        assert_investor_observations(&result);
        let captured = server.finish().expect("requests");
        assert!(
            String::from_utf8_lossy(&captured)
                .contains("/api/v1/rankings/investor-trading?count=2")
        );
    }
    #[test]
    fn investor_ranking_rejects_invalid_amount_and_retries_errors() {
        let mut input = investor_fixture()["body"].clone();
        input["result"]["foreigner"]["netBuyRankings"][0]["netBuyAmount"] = json!("bad");
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)]).expect("server");
        run(adapter(&server).investor_trading_rankings(RankingLimit::try_new(2).expect("limit")))
            .expect_err("invalid");
        server.finish().expect("requests");
        let revoked =
            json!({"error":{"code":"token-revoked","message":"synthetic","requestId":"fake"}});
        let retry_server = ScriptedHttpServer::start(vec![
            issued(),
            response("401 Unauthorized", &revoked),
            issued(),
            response("200 OK", &investor_fixture()["body"]),
        ])
        .expect("server");
        run(adapter(&retry_server)
            .investor_trading_rankings(RankingLimit::try_new(2).expect("limit")))
        .expect("retry");
        retry_server.finish().expect("requests");
        let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
        let mut responses = vec![issued()];
        responses.extend((0_usize..4).map(|_| response("429 Too Many Requests", &limited)));
        let limited_server = ScriptedHttpServer::start(responses).expect("server");
        let error = run(adapter(&limited_server)
            .investor_trading_rankings(RankingLimit::try_new(2).expect("limit")))
        .expect_err("429");
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

    fn sector_fixture() -> Value {
        serde_json::from_str(
            &std::fs::read_to_string("tests/market_recordings/t10-sector-rankings.json")
                .expect("file"),
        )
        .expect("json")
    }
    fn catalog_fixture() -> Value {
        serde_json::from_str(
            &std::fs::read_to_string("tests/market_recordings/t09-sectors.json").expect("file"),
        )
        .expect("json")
    }
    fn sector_query() -> application::SectorRankingQuery {
        application::SectorRankingQuery::try_new(
            RankingKind::TopGainers,
            KnownMarket::Kr,
            RankingDuration::Day,
        )
        .expect("query")
    }
    #[test]
    fn sector_rankings_validate_catalog_and_keep_unknown_server_time() {
        let fixture = sector_fixture();
        crate::support_tests::validation::validate_recording(
            &crate::support_tests::spec(),
            &fixture,
        )
        .expect("schema");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &catalog_fixture()["body"]),
            response("200 OK", &fixture["body"]),
        ])
        .expect("server");
        let result = run(adapter(&server).sector_rankings(&sector_query())).expect("ranking");
        assert_eq!(result.market(), KnownMarket::Kr);
        assert_eq!(result.kind(), RankingKind::TopGainers);
        assert_eq!(result.items().len(), 3);
        assert_eq!(result.items()[0].sector().as_str(), "100");
        assert_eq!(result.items()[1].sector().as_str(), "42");
        assert_eq!(result.observation().server_at(), None);
        let captured = server.finish().expect("requests");
        assert!(String::from_utf8_lossy(&captured).contains("/api/v1/sectors/rankings?"));
    }
    #[test]
    fn sector_rankings_reject_unlisted_minor_bad_value_and_preserve_catalog_failure() {
        for value in [json!("999"), json!("103")] {
            let mut ranking = sector_fixture()["body"].clone();
            ranking["result"]["rankings"][0]["sectorId"] = value;
            let server = ScriptedHttpServer::start(vec![
                issued(),
                response("200 OK", &catalog_fixture()["body"]),
                response("200 OK", &ranking),
            ])
            .expect("server");
            run(adapter(&server).sector_rankings(&sector_query()))
                .expect_err("invalid catalog entry");
            server.finish().expect("requests");
        }
        let mut ranking = sector_fixture()["body"].clone();
        ranking["result"]["rankings"][0]["changeRate"] = json!("bad");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &catalog_fixture()["body"]),
            response("200 OK", &ranking),
        ])
        .expect("server");
        run(adapter(&server).sector_rankings(&sector_query())).expect_err("invalid number");
        server.finish().expect("requests");
        let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
        let mut responses = vec![issued()];
        responses.extend((0_usize..4).map(|_| response("429 Too Many Requests", &limited)));
        let limited_server = ScriptedHttpServer::start(responses).expect("server");
        let error = run(adapter(&limited_server).sector_rankings(&sector_query()))
            .expect_err("catalog failure");
        assert_eq!(error.endpoint(), application::ReadEndpoint::SectorRankings);
        assert_eq!(error.status(), application::HttpStatus::RateLimited);
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

    #[test]
    fn sector_ranking_endpoint_retries_401_and_preserves_429() {
        let catalog = catalog_fixture();
        let fixture = sector_fixture();
        let revoked =
            json!({"error":{"code":"token-revoked","message":"synthetic","requestId":"fake"}});
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &catalog["body"]),
            response("401 Unauthorized", &revoked),
            issued(),
            response("200 OK", &fixture["body"]),
        ])
        .expect("server");
        run(adapter(&server).sector_rankings(&sector_query())).expect("refresh");
        server.finish().expect("requests");
        let limited = json!({"error":{"code":"rate-limit-exceeded","message":"synthetic","requestId":"fake"}});
        let mut responses = vec![issued(), response("200 OK", &catalog["body"])];
        responses.extend((0_usize..4).map(|_| response("429 Too Many Requests", &limited)));
        let limited_server = ScriptedHttpServer::start(responses).expect("server");
        let error =
            run(adapter(&limited_server).sector_rankings(&sector_query())).expect_err("limited");
        assert_eq!(error.endpoint(), application::ReadEndpoint::SectorRankings);
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
    #[test]
    fn ranking_keeps_unknown_server_time_and_missing_change_rate() {
        let mut input = body();
        input["result"]["rankedAt"] = Value::Null;
        input["result"]["rankings"][0]["price"]["changeRate"] = Value::Null;
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &input)]).expect("server");
        let result = run(adapter(&server).rankings(&query())).expect("ranking");
        assert_eq!(result.observation().server_at(), None);
        assert_eq!(
            result.items()[0].change_rate(),
            domain::RankingChange::Unavailable
        );
        server.finish().expect("requests");
    }
}
