mod support;
#[cfg(test)]
mod support_tests;
#[cfg(test)]
mod tests {
    use application::{
        BrokerRead, Clock, EventBus, InstrumentMappingRepository, InstrumentMasterAuthority,
        InstrumentMasterObservation, InstrumentMasterSession, MappingStoreError, MarketInfo,
        MasterNotReady, SecretKey, SecretStore, SecretValue, SystemPortError, WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
    };
    use domain::{
        InstrumentId, InstrumentMappingError, InstrumentMappingRecord, InstrumentMasterEntry,
        InstrumentSource, IsinCode, KnownMarket, MappingAssignment, MasterInstrumentFacts, Symbol,
        Timestamp,
    };
    use serde_json::{Value, json};
    use std::{future::Future, sync::Arc};
    use testkit::ScriptedHttpServer;
    use testkit_ports::{ScriptedInstrumentIdIssuer, memory::MemoryInstrumentMappingRepository};

    struct Store;
    impl SecretStore for Store {
        fn load(
            &self,
            key: SecretKey,
        ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
            std::future::ready(Ok(Some(SecretValue::new(match key {
                SecretKey::TossClientId => b"synthetic-id".to_vec(),
                SecretKey::TossClientSecret => b"synthetic-secret".to_vec(),
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
    #[derive(Default)]
    struct Authority {
        repository: Arc<MemoryInstrumentMappingRepository>,
        failure: Option<MasterNotReady>,
        verification_failure: Option<MasterNotReady>,
        mapping_failure: Option<MappingFailure>,
        observations: Arc<std::sync::RwLock<Vec<InstrumentMasterObservation>>>,
        mapping_calls: Arc<std::sync::atomic::AtomicUsize>,
        ensure_calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[derive(Clone, Copy)]
    enum MappingFailure {
        Resolve(SystemPortError),
        Ensure(SystemPortError),
        EnsureSecond(SystemPortError),
        EnsureMissing,
        Retire(SystemPortError),
        RetireUncertain,
    }

    struct Session<'a>(
        &'a MemoryInstrumentMappingRepository,
        Option<MasterNotReady>,
        &'a std::sync::RwLock<Vec<InstrumentMasterObservation>>,
        &'a std::sync::atomic::AtomicUsize,
        Option<MappingFailure>,
        &'a std::sync::atomic::AtomicUsize,
    );
    impl InstrumentMappingRepository for Session<'_> {
        async fn resolve(
            &self,
            source: &InstrumentSource,
        ) -> Result<InstrumentId, MappingStoreError> {
            self.3.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match self.4 {
                Some(MappingFailure::Resolve(error)) => Err(MappingStoreError::Storage(error)),
                Some(
                    MappingFailure::Ensure(_)
                    | MappingFailure::EnsureSecond(_)
                    | MappingFailure::EnsureMissing
                    | MappingFailure::Retire(_)
                    | MappingFailure::RetireUncertain,
                )
                | None => self.0.resolve(source).await,
            }
        }
        fn lookup(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentMappingRecord, MappingStoreError>> + Send
        {
            self.0.lookup(source)
        }
        fn for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<InstrumentMappingRecord>, MappingStoreError>> + Send
        {
            self.0.for_id(id)
        }
        async fn ensure(
            &self,
            source: &InstrumentSource,
            candidate: &InstrumentId,
        ) -> Result<MappingAssignment, MappingStoreError> {
            let call = self.5.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            match (self.4, call) {
                (Some(MappingFailure::Ensure(error)), _)
                | (Some(MappingFailure::EnsureSecond(error)), 1) => {
                    Err(MappingStoreError::Storage(error))
                }
                (Some(MappingFailure::EnsureMissing), _) => {
                    Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
                }
                (
                    Some(
                        MappingFailure::Resolve(_)
                        | MappingFailure::EnsureSecond(_)
                        | MappingFailure::Retire(_)
                        | MappingFailure::RetireUncertain,
                    )
                    | None,
                    _,
                ) => self.0.ensure(source, candidate).await,
            }
        }
        async fn retire(
            &self,
            source: &InstrumentSource,
        ) -> Result<InstrumentId, MappingStoreError> {
            match self.4 {
                Some(MappingFailure::Retire(error)) => Err(MappingStoreError::Storage(error)),
                Some(MappingFailure::RetireUncertain) => Err(MappingStoreError::Mapping(
                    InstrumentMappingError::IdentityUncertain,
                )),
                Some(
                    MappingFailure::Resolve(_)
                    | MappingFailure::Ensure(_)
                    | MappingFailure::EnsureSecond(_)
                    | MappingFailure::EnsureMissing,
                )
                | None => self.0.retire(source).await,
            }
        }
    }
    impl InstrumentMasterSession for Session<'_> {
        fn verify_observation(
            &self,
            observations: &[InstrumentMasterObservation],
        ) -> impl Future<Output = Result<(), MasterNotReady>> + Send {
            *self.2.write().expect("observations") = observations.to_vec();
            std::future::ready(match self.1 {
                Some(reason) => Err(reason),
                None => Ok(()),
            })
        }
    }
    impl InstrumentMasterAuthority for Authority {
        type Session<'a> = Session<'a>;
        fn begin(
            &self,
            _: KnownMarket,
        ) -> impl Future<Output = Result<Self::Session<'_>, MasterNotReady>> + Send {
            std::future::ready(match self.failure {
                Some(reason) => Err(reason),
                None => Ok(Session(
                    &self.repository,
                    self.verification_failure,
                    &self.observations,
                    &self.mapping_calls,
                    self.mapping_failure,
                    &self.ensure_calls,
                )),
            })
        }
    }
    fn response(status: &str, body: &Value) -> Vec<u8> {
        let text = body.to_string();
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
            text.len()
        )
        .into_bytes()
    }
    fn token() -> Vec<u8> {
        response(
            "200 OK",
            &json!({"access_token":"synthetic-access","token_type":"Bearer","expires_in":3600}),
        )
    }
    fn recording(name: &str) -> Value {
        let path = format!("tests/market_recordings/t21-{name}.json");
        let recording: Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("json");
        crate::support_tests::validation::validate_recording(
            &crate::support_tests::spec(),
            &recording,
        )
        .expect("schema");
        recording["body"].clone()
    }
    fn list() -> Value {
        recording("stocks-all")
    }
    fn details() -> Value {
        recording("stocks")
    }
    fn empty() -> Value {
        json!({"result":[]})
    }
    fn reader(
        server: &ScriptedHttpServer,
        clock: Arc<crate::support::FakeTime>,
    ) -> TossAccountRead<Store, Time, Renewal> {
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
                Time(Arc::clone(&clock)),
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
    fn source(symbol: &str, isin: &str) -> InstrumentSource {
        InstrumentSource::new(
            KnownMarket::Kr,
            Symbol::try_new(symbol).expect("symbol"),
            IsinCode::try_new(isin).expect("isin"),
        )
    }
    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("id")
    }
    fn issuer(ids: &[&str]) -> ScriptedInstrumentIdIssuer {
        let issuer = ScriptedInstrumentIdIssuer::default();
        for value in ids {
            issuer.push(Ok(id(value)));
        }
        issuer
    }
    fn responses() -> Vec<Vec<u8>> {
        vec![
            token(),
            response("200 OK", &list()),
            response("200 OK", &empty()),
            response("200 OK", &empty()),
            response("200 OK", &details()),
        ]
    }

    #[test]
    fn saved_list_fixture_matches_the_openapi_contract_and_can_be_replayed() {
        assert_ne!(crate::support_tests::recordings().len(), 0);
        let fixture: Value = serde_json::from_str(
            &std::fs::read_to_string("tests/market_recordings/t21-stocks-all.json")
                .expect("fixture"),
        )
        .expect("json");
        let server = crate::support_tests::serve_recordings(&fixture);
        let clock = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&clock),
            Arc::<crate::support::FakeTime>::clone(&clock),
        )
        .expect("client");
        let request = broker_toss::ReadRequest::new(crate::support_tests::route(&fixture));
        let response = run(client.get(&request, &SecretValue::new(b"synthetic-access".to_vec())))
            .expect("response");
        assert_eq!(response.status(), 200);
        assert_eq!(
            serde_json::from_slice::<Value>(response.body().as_exposed_secret()).expect("body"),
            fixture["body"]
        );
        server.finish().expect("request");
    }

    #[test]
    fn unverified_default_and_authority_fail_before_network_and_mapping() {
        let server = ScriptedHttpServer::start(vec![]).expect("server");
        let reader = reader(&server, crate::support::time());
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("not ready");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::MasterNotReady(MasterNotReady::StatusCoverage)
        );
        let authority = Authority {
            failure: Some(MasterNotReady::IdentityContinuity),
            ..Authority::default()
        };
        let instrument_reader = reader.with_instrument_master(authority, issuer(&[]));
        assert_eq!(
            broker_toss::TossInstrumentRead::<
                Store,
                Time,
                Renewal,
                Authority,
                ScriptedInstrumentIdIssuer,
            >::capabilities()
            .support_for(application::BrokerCapability::Instruments),
            application::CapabilitySupport::Supported
        );
        let continuity_error =
            run(instrument_reader.instruments(KnownMarket::Kr)).expect_err("not ready");
        assert_eq!(
            continuity_error.kind(),
            application::PortErrorKind::MasterNotReady(MasterNotReady::IdentityContinuity)
        );
        assert_eq!(server.finish().expect("requests").len(), 0);
    }

    #[test]
    fn every_exchange_and_detail_is_joined_without_losing_fields() {
        let server = ScriptedHttpServer::start(responses()).expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&["opaque-91", "opaque-72"]));
        let master = run(reader.instruments(KnownMarket::Kr)).expect("master");
        let facts = master.complete_facts().expect("complete");
        assert_eq!(facts.instruments().len(), 2);
        let first = facts.instruments()[0].row();
        assert_eq!(first.symbol.as_str(), "005930");
        assert_eq!(first.instrument_id, id("opaque-91"));
        assert_eq!(first.name.as_str(), "합성 하나");
        assert_eq!(first.listing_market.as_str(), "KOSPI");
        assert_eq!(first.security_type.code().as_str(), "STOCK");
        assert_eq!(first.listing_status, domain::ListingStatus::Scheduled);
        assert_eq!(first.common_share, domain::CommonShare::Common);
        assert_eq!(
            first.korean_detail.expect("kr").liquidation,
            domain::LiquidationTrading::Liquidation
        );
        assert_eq!(
            first.korean_detail.expect("kr").nxt,
            domain::NxtTrading::Supported(domain::TradingSuspension::Suspended)
        );
        let second = facts.instruments()[1].row();
        assert_eq!(second.symbol.as_str(), "069500");
        assert_eq!(second.instrument_id, id("opaque-72"));
        assert_eq!(second.security_type.code().as_str(), "ETF");
        assert_eq!(second.common_share, domain::CommonShare::Other);
        assert_eq!(
            second.korean_detail.expect("kr").krx,
            domain::TradingSuspension::Suspended
        );
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        for market in ["KOSPI", "KOSDAQ", "KR_ETC"] {
            assert!(requests.contains(&format!("/api/v1/stocks/all?market={market}")));
        }
        assert!(!requests.contains("status="));
        assert!(!requests.contains("securityType="));
        assert!(!requests.contains("commonShare="));
        assert!(requests.contains("/api/v1/stocks?symbols=005930%2C069500"));
    }

    #[test]
    fn uncertain_identity_is_kept_in_the_result_without_losing_other_rows() {
        let server = ScriptedHttpServer::start(responses()).expect("server");
        let authority = Authority::default();
        run(authority
            .repository
            .ensure(&source("005930", "KR7005930099"), &id("existing-37")))
        .expect("seed");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(authority, issuer(&["opaque-72"]));
        let master = run(reader.instruments(KnownMarket::Kr)).expect("master");
        assert_eq!(master.entries().len(), 2);
        assert_eq!(
            master.entries()[0],
            InstrumentMasterEntry::Unavailable {
                source: source("005930", "KR7005930003"),
                reason: InstrumentMappingError::IdentityUncertain
            }
        );
        match &master.entries()[1] {
            InstrumentMasterEntry::Available(value) => {
                assert_eq!(value.row().instrument_id, id("opaque-72"));
            }
            InstrumentMasterEntry::Unavailable { .. } => panic!("expected available instrument"),
        }
        assert_eq!(
            master.complete_facts(),
            Err(domain::InstrumentMasterError::UnavailableInstrument)
        );
        server.finish().expect("requests");
    }

    #[test]
    fn a_failed_exchange_or_missing_detail_never_returns_a_complete_list() {
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &list()),
            response("500 Internal Server Error", &json!({})),
        ])
        .expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&[]));
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("failed exchange");
        assert_eq!(error.endpoint(), application::ReadEndpoint::Instruments);
        server.finish().expect("requests");
    }

    #[test]
    fn retired_and_collision_results_are_preserved_per_instrument() {
        let server = ScriptedHttpServer::start(responses()).expect("server");
        let authority = Authority::default();
        let retired = source("005930", "KR7005930003");
        run(authority
            .repository
            .ensure(&retired, &id("already-retired")))
        .expect("seed retired");
        run(authority.repository.retire(&retired)).expect("retire");
        run(authority
            .repository
            .ensure(&source("000001", "KR7000000001"), &id("collision-91")))
        .expect("seed collision");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(authority, issuer(&["collision-91"]));
        let master = run(reader.instruments(KnownMarket::Kr)).expect("master");
        assert_eq!(
            master.entries(),
            &[
                InstrumentMasterEntry::Unavailable {
                    source: retired,
                    reason: InstrumentMappingError::Retired
                },
                InstrumentMasterEntry::Unavailable {
                    source: source("069500", "KR7069500007"),
                    reason: InstrumentMappingError::IdAlreadyUsed
                },
            ]
        );
        assert_eq!(
            master.complete_facts(),
            Err(domain::InstrumentMasterError::UnavailableInstrument)
        );
        server.finish().expect("requests");
    }

    #[test]
    fn existing_ids_are_reused_without_consuming_candidates() {
        let server = ScriptedHttpServer::start(responses()).expect("server");
        let authority = Authority::default();
        run(authority
            .repository
            .ensure(&source("005930", "KR7005930003"), &id("old-19")))
        .expect("seed first");
        run(authority
            .repository
            .ensure(&source("069500", "KR7069500007"), &id("old-37")))
        .expect("seed second");
        let reader =
            reader(&server, crate::support::time()).with_instrument_master(authority, issuer(&[]));
        let master = run(reader.instruments(KnownMarket::Kr))
            .expect("master")
            .complete_facts()
            .expect("complete");
        assert_eq!(master.instruments()[0].row().instrument_id, id("old-19"));
        assert_eq!(master.instruments()[1].row().instrument_id, id("old-37"));
        server.finish().expect("requests");
    }

    fn invalid_details() -> Vec<Value> {
        vec![
            json!({"result":[]}),
            {
                let mut value = details();
                value["result"][0]["isinCode"] = json!("KR7069500099");
                value
            },
            {
                let mut value = details();
                value["result"][0]["market"] = json!("KOSDAQ");
                value
            },
            {
                let mut value = details();
                value["result"][0]["name"] = json!("다른 이름");
                value
            },
            {
                let mut value = details();
                value["result"][0]["securityType"] = json!("STOCK");
                value
            },
            {
                let mut value = details();
                value["result"][0]["isCommonShare"] = json!(true);
                value
            },
            {
                let mut value = details();
                value["result"][0]["currency"] = json!("USD");
                value
            },
            {
                let mut value = details();
                value["result"][0]["koreanMarketDetail"]["nxtTradingSuspended"] = json!(true);
                value
            },
            {
                let mut value = details();
                value["result"][0] = value["result"][1].clone();
                value
            },
            {
                let mut value = details();
                value["result"][0]["symbol"] = json!("000001");
                value
            },
        ]
    }

    #[test]
    fn mismatched_and_missing_details_fail_before_the_issuer_is_called() {
        for detail in invalid_details() {
            let server = ScriptedHttpServer::start(vec![
                token(),
                response("200 OK", &list()),
                response("200 OK", &empty()),
                response("200 OK", &empty()),
                response("200 OK", &detail),
            ])
            .expect("server");
            let reader = reader(&server, crate::support::time())
                .with_instrument_master(Authority::default(), issuer(&[]));
            let error = run(reader.instruments(KnownMarket::Kr)).expect_err("invalid detail");
            assert_eq!(error.endpoint(), application::ReadEndpoint::Instruments);
            server.finish().expect("requests");
        }
    }

    #[test]
    fn duplicate_symbols_across_exchanges_fail_before_details_and_assignment() {
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &list()),
            response("200 OK", &list()),
        ])
        .expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&[]));
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("duplicate symbols");
        assert_eq!(error.endpoint(), application::ReadEndpoint::Instruments);
        server.finish().expect("requests");
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
    fn market_requests_are_rate_limited_before_the_second_request_arrives() {
        let replies = responses();
        let (server, gate) =
            ScriptedHttpServer::start_holding(replies, testkit::HoldPosition::new(2))
                .expect("server");
        let clock = crate::support::time();
        let initial = clock.now();
        let reader = reader(&server, Arc::clone(&clock))
            .with_instrument_master(Authority::default(), issuer(&["opaque-91", "opaque-72"]));
        let received = run(async {
            let task = tokio::spawn(async move { reader.instruments(KnownMarket::Kr).await });
            let held_gate = tokio::time::timeout(std::time::Duration::from_secs(5), held(gate))
                .await
                .expect("second request received");
            let received = clock.now();
            held_gate.release().expect("release");
            task.await.expect("task").expect("master");
            received
        });
        assert!(
            initial.elapsed_until(received).expect("elapsed")
                >= domain::TimeSpan::try_new(
                    domain::SpanSeconds::new(1),
                    domain::Nanosecond::new(0)
                )
                .expect("one second")
        );
        server.finish().expect("requests");
    }

    #[test]
    fn us_exchanges_preserve_source_case_and_use_the_existing_id() {
        let us_list = json!({"result":[{"symbol":"aapl","name":"합성 미국","securityType":"FOREIGN_STOCK","isCommonShare":true,"isinCode":"US0378331005"}]});
        let us_details = json!({"result":[{"symbol":"aapl","name":"합성 미국","englishName":"Synthetic US","isinCode":"US0378331005","market":"NASDAQ","securityType":"FOREIGN_STOCK","isCommonShare":true,"status":"ACTIVE","currency":"USD","sharesOutstanding":"17","koreanMarketDetail":null}]});
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &empty()),
            response("200 OK", &us_list),
            response("200 OK", &empty()),
            response("200 OK", &empty()),
            response("200 OK", &us_details),
        ])
        .expect("server");
        let authority = Authority::default();
        let source = InstrumentSource::new(
            KnownMarket::Us,
            Symbol::try_new("aapl").expect("symbol"),
            IsinCode::try_new("US0378331005").expect("isin"),
        );
        run(authority.repository.ensure(&source, &id("existing-us-81"))).expect("seed");
        let reader =
            reader(&server, crate::support::time()).with_instrument_master(authority, issuer(&[]));
        let master = run(reader.instruments(KnownMarket::Us))
            .expect("master")
            .complete_facts()
            .expect("complete");
        let row = master.instruments()[0].row();
        assert_eq!(row.instrument_id, id("existing-us-81"));
        assert_eq!(row.symbol.as_str(), "AAPL");
        assert_eq!(row.listing_market.as_str(), "NASDAQ");
        assert_eq!(row.currency, domain::Currency::Usd);
        assert_eq!(row.korean_detail, None);
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        for market in ["NYSE", "NASDAQ", "AMEX", "US_ETC"] {
            assert!(requests.contains(&format!("/api/v1/stocks/all?market={market}")));
        }
    }

    #[test]
    fn later_detail_batch_failure_does_not_assign_any_id() {
        let mut listed = Vec::new();
        let mut details = Vec::new();
        for number in 0_u32..201_u32 {
            let symbol = format!("{number:06}");
            let isin = format!("KR{number:010}");
            listed.push(json!({"symbol":symbol,"name":"합성 주식","securityType":"STOCK","isCommonShare":true,"isinCode":isin}));
            details.push(json!({"symbol":symbol,"name":"합성 주식","englishName":"Synthetic","isinCode":isin,"market":"KOSPI","securityType":"STOCK","isCommonShare":true,"status":"ACTIVE","currency":"KRW","sharesOutstanding":"1","koreanMarketDetail":null}));
        }
        details.pop();
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &json!({"result":listed})),
            response("200 OK", &empty()),
            response("200 OK", &empty()),
            response("200 OK", &json!({"result":details})),
            response("500 Internal Server Error", &json!({})),
        ])
        .expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&[]));
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("failed detail batch");
        assert_eq!(error.endpoint(), application::ReadEndpoint::Instruments);
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("GET /api/v1/stocks?symbols=").count(), 2);
        assert!(requests.contains("GET /api/v1/stocks?symbols=000200 HTTP"));
    }

    #[test]
    fn unverified_observation_fails_before_any_candidate_is_requested() {
        let server = ScriptedHttpServer::start(responses()).expect("server");
        let mapping_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let authority = Authority {
            verification_failure: Some(MasterNotReady::IdentityContinuity),
            mapping_calls: Arc::clone(&mapping_calls),
            ..Authority::default()
        };
        let reader =
            reader(&server, crate::support::time()).with_instrument_master(authority, issuer(&[]));
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("unverified observation");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::MasterNotReady(MasterNotReady::IdentityContinuity)
        );
        assert_eq!(error.status(), application::HttpStatus::Success);
        assert_eq!(mapping_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        server.finish().expect("requests");
    }

    #[test]
    fn authority_receives_listing_status_market_and_observation_time_before_mapping() {
        let server = ScriptedHttpServer::start(responses()).expect("server");
        let observations = Arc::new(std::sync::RwLock::new(Vec::new()));
        let authority = Authority {
            observations: Arc::clone(&observations),
            ..Authority::default()
        };
        let clock = crate::support::time();
        let reader = reader(&server, Arc::clone(&clock))
            .with_instrument_master(authority, issuer(&["opaque-91", "opaque-72"]));
        run(reader.instruments(KnownMarket::Kr)).expect("master");
        let seen = observations.write().expect("observations");
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].source, source("005930", "KR7005930003"));
        assert_eq!(seen[0].listing_market.as_str(), "KOSPI");
        assert_eq!(seen[0].listing_status, domain::ListingStatus::Scheduled);
        assert_eq!(seen[1].source, source("069500", "KR7069500007"));
        assert_eq!(seen[1].listing_status, domain::ListingStatus::Active);
        assert_eq!(seen[0].observed_at, clock.now());
        assert_eq!(seen[1].observed_at, clock.now());
        server.finish().expect("requests");
    }

    #[test]
    fn returned_master_can_supply_sector_members_and_etfs() {
        let mut replies = responses();
        replies.push(response("200 OK", &json!({"result":{"stocks":[{"symbol":"005930","name":"합성 하나","volume":"14","tradingAmount":"57","marketCapitalization":"73","currency":"KRW"}]}})));
        replies.push(response(
            "200 OK",
            &json!({"result":{"etfs":[{"symbol":"069500","name":"합성 둘"}]}}),
        ));
        let server = ScriptedHttpServer::start(replies).expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&["opaque-91", "opaque-72"]));
        let master = run(reader.instruments(KnownMarket::Kr))
            .expect("master")
            .complete_facts()
            .expect("complete");
        let populated_reader = reader.with_market_master(master);
        let sector = domain::SectorId::try_new("100").expect("sector");
        let composition = run(populated_reader.sector_composition(&sector, KnownMarket::Kr))
            .expect("composition");
        assert_eq!(
            composition.symbols(),
            &[Symbol::try_new("005930").expect("symbol")]
        );
        let etfs = run(populated_reader.sector_etfs(&sector, KnownMarket::Kr)).expect("etfs");
        assert_eq!(
            etfs.etfs()[0].symbol(),
            &Symbol::try_new("069500").expect("symbol")
        );
        server.finish().expect("requests");
    }

    #[test]
    fn sector_queries_report_the_missing_master_as_not_ready() {
        use application::ClassifiedError;
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &json!({"result":{"stocks":[{"symbol":"005930","name":"합성 하나","volume":"14","tradingAmount":"57","marketCapitalization":"73","currency":"KRW"}]}})),
            response("200 OK", &json!({"result":{"etfs":[{"symbol":"069500","name":"합성 둘"}]}})),
        ]).expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&[]));
        let sector = domain::SectorId::try_new("100").expect("sector");
        let composition =
            run(reader.sector_composition(&sector, KnownMarket::Kr)).expect_err("missing master");
        let etfs = run(reader.sector_etfs(&sector, KnownMarket::Kr)).expect_err("missing master");
        for error in [composition, etfs] {
            assert_eq!(
                error.kind(),
                application::PortErrorKind::MasterNotReady(MasterNotReady::MissingInstrument)
            );
            assert_eq!(
                error.class(),
                application::ErrorClass::Wait(application::RetryDelay::Unknown)
            );
            assert_eq!(error.status(), application::HttpStatus::Success);
        }
        server.finish().expect("requests");
    }
    fn probe_list_of_size(count: usize) -> Value {
        let rows = (0..count)
            .map(|index| {
                json!({
                    "symbol": format!("{index:06}"),
                    "name": "합성 종목",
                    "securityType": "STOCK",
                    "isCommonShare": true,
                    "isinCode": "KR7000000001"
                })
            })
            .collect::<Vec<_>>();
        json!({"result": rows})
    }

    #[test]
    fn review_limit_exactly_8192_proceeds_to_the_next_exchange() {
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &probe_list_of_size(8192)),
            response("500 Internal Server Error", &json!({})),
        ])
        .expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&[]));
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("second exchange failure");
        assert_eq!(error.status(), application::HttpStatus::ServerError);
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert!(requests.contains("market=KOSDAQ"));
    }

    #[test]
    fn review_limit_above_8192_fails_before_next_exchange() {
        let server =
            ScriptedHttpServer::start(vec![token(), response("200 OK", &probe_list_of_size(8193))])
                .expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&[]));
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("oversized");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert!(!requests.contains("market=KOSDAQ"));
    }

    #[test]
    fn review_explicit_delisted_status_cannot_become_tradable() {
        let mut delisted_details = details();
        delisted_details["result"][1]["status"] = json!("DELISTED");
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &list()),
            response("200 OK", &empty()),
            response("200 OK", &empty()),
            response("200 OK", &delisted_details),
        ])
        .expect("server");
        let authority = Authority::default();
        let observations = Arc::clone(&authority.observations);
        let repository = Arc::clone(&authority.repository);
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(authority, issuer(&["opaque-b"]));
        let master = run(reader.instruments(KnownMarket::Kr)).expect("master");
        assert_eq!(
            master.entries()[0],
            InstrumentMasterEntry::Unavailable {
                source: source("005930", "KR7005930003"),
                reason: InstrumentMappingError::IdentityUncertain,
            }
        );
        assert_eq!(
            observations.read().expect("observations")[0].listing_status,
            domain::ListingStatus::Delisted
        );
        assert_eq!(
            run(repository.resolve(&source("005930", "KR7005930003"))),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        server.finish().expect("requests");
    }

    #[test]
    fn review_regression_verified_delisting_retires_existing_mapping() {
        let mut details = details();
        details["result"][1]["status"] = json!("DELISTED");
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &list()),
            response("200 OK", &empty()),
            response("200 OK", &empty()),
            response("200 OK", &details),
        ])
        .expect("server");
        let authority = Authority::default();
        let repository = Arc::clone(&authority.repository);
        let delisted = source("005930", "KR7005930003");
        run(repository.ensure(&delisted, &id("old-delisted"))).expect("seed");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(authority, issuer(&["other"]));
        let master = run(reader.instruments(KnownMarket::Kr)).expect("master");
        assert_eq!(
            master.entries()[0],
            InstrumentMasterEntry::Unavailable {
                source: delisted.clone(),
                reason: InstrumentMappingError::Retired,
            }
        );
        assert_eq!(
            run(repository.resolve(&delisted)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
        );
        server.finish().expect("requests");
    }

    #[test]
    fn review_retirement_failure_preserves_the_system_error_and_existing_mapping() {
        use application::{ClassifiedError, SystemFailure, SystemPort};
        let mut details = details();
        details["result"][1]["status"] = json!("DELISTED");
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &list()),
            response("200 OK", &empty()),
            response("200 OK", &empty()),
            response("200 OK", &details),
        ])
        .expect("server");
        let original =
            SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        let authority = Authority {
            mapping_failure: Some(MappingFailure::Retire(original)),
            ..Authority::default()
        };
        let repository = Arc::clone(&authority.repository);
        let delisted = source("005930", "KR7005930003");
        run(repository.ensure(&delisted, &id("old-delisted"))).expect("seed");
        let reader =
            reader(&server, crate::support::time()).with_instrument_master(authority, issuer(&[]));
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("retire failure");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::MasterNotReady(MasterNotReady::Storage(original))
        );
        assert_eq!(error.class(), original.class());
        assert_eq!(run(repository.resolve(&delisted)), Ok(id("old-delisted")));
        server.finish().expect("requests");
    }

    #[test]
    fn review_regression_issuer_errors_preserve_the_system_classification() {
        use application::{ClassifiedError, SystemFailure, SystemPort};
        for failure in [
            SystemFailure::RepositoryIntegrity,
            SystemFailure::InvalidData,
            SystemFailure::PermissionDenied,
            SystemFailure::Unavailable(application::RetryDelay::Unknown),
        ] {
            let server = ScriptedHttpServer::start(responses()).expect("server");
            let issuer = issuer(&[]);
            let source_error = SystemPortError::new(SystemPort::Repository, failure);
            issuer.push(Err(source_error));
            let reader = reader(&server, crate::support::time())
                .with_instrument_master(Authority::default(), issuer);
            let error = run(reader.instruments(KnownMarket::Kr)).expect_err("issuer failure");
            assert_eq!(error.class(), source_error.class());
            server.finish().expect("requests");
        }
    }

    #[test]
    fn review_regression_repository_errors_keep_their_exact_cause() {
        use application::{ClassifiedError, SystemFailure, SystemPort};
        for failure in [
            SystemFailure::RepositoryIntegrity,
            SystemFailure::InvalidData,
            SystemFailure::PermissionDenied,
            SystemFailure::Unavailable(application::RetryDelay::Unknown),
        ] {
            let original = SystemPortError::new(SystemPort::Repository, failure);
            for mapping_failure in [
                MappingFailure::Resolve(original),
                MappingFailure::Ensure(original),
            ] {
                let server = ScriptedHttpServer::start(responses()).expect("server");
                let authority = Authority {
                    mapping_failure: Some(mapping_failure),
                    ..Authority::default()
                };
                let reader = reader(&server, crate::support::time())
                    .with_instrument_master(authority, issuer(&["candidate"]));
                let error = run(reader.instruments(KnownMarket::Kr)).expect_err("repository error");
                assert_eq!(
                    error.kind(),
                    application::PortErrorKind::MasterNotReady(MasterNotReady::Storage(original))
                );
                assert_eq!(error.class(), original.class());
                server.finish().expect("requests");
            }
        }
    }

    #[test]
    fn review_retry_reuses_successful_mapping_after_later_write_failure() {
        use application::{SystemFailure, SystemPort};
        let mut replies = responses();
        replies.extend(responses().into_iter().skip(1));
        let server = ScriptedHttpServer::start(replies).expect("server");
        let authority = Authority {
            mapping_failure: Some(MappingFailure::EnsureSecond(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::Unavailable(application::RetryDelay::Unknown),
            ))),
            ..Authority::default()
        };
        let repository = Arc::clone(&authority.repository);
        let reader = reader(&server, crate::support::time()).with_instrument_master(
            authority,
            issuer(&["first-id", "failed-candidate", "retry-id"]),
        );
        run(reader.instruments(KnownMarket::Kr)).expect_err("second write failed");
        assert_eq!(
            run(repository.resolve(&source("005930", "KR7005930003"))),
            Ok(id("first-id"))
        );
        assert_eq!(
            run(repository.resolve(&source("069500", "KR7069500007"))),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        let retried = run(reader.instruments(KnownMarket::Kr))
            .expect("retried")
            .complete_facts()
            .expect("complete");
        assert_eq!(retried.instruments()[0].row().instrument_id, id("first-id"));
        assert_eq!(retried.instruments()[1].row().instrument_id, id("retry-id"));
        server.finish().expect("requests");
    }

    #[test]
    fn review_regression_same_isin_never_grants_either_ambiguous_source_an_id() {
        let mut first_list = list();
        first_list["result"][1]["isinCode"] = json!("KR7005930003");
        let mut first_details = details();
        first_details["result"][0]["isinCode"] = json!("KR7005930003");
        let server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &first_list),
            response("200 OK", &empty()),
            response("200 OK", &empty()),
            response("200 OK", &first_details),
        ])
        .expect("server");
        let authority = Authority::default();
        let repository = Arc::clone(&authority.repository);
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(authority, issuer(&["ambiguous-1", "ambiguous-2"]));
        let master = run(reader.instruments(KnownMarket::Kr)).expect("per-instrument result");
        assert_eq!(
            master.entries(),
            &[
                InstrumentMasterEntry::Unavailable {
                    source: source("005930", "KR7005930003"),
                    reason: InstrumentMappingError::IdentityUncertain,
                },
                InstrumentMasterEntry::Unavailable {
                    source: source("069500", "KR7005930003"),
                    reason: InstrumentMappingError::IdentityUncertain,
                },
            ]
        );
        for symbol in ["005930", "069500"] {
            assert_eq!(
                run(repository.resolve(&source(symbol, "KR7005930003"))),
                Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
            );
        }
        server.finish().expect("requests");
    }
    #[test]
    fn review_regression_holdings_separates_missing_master_from_bad_response() {
        let recording: Value = serde_json::from_str(
            &std::fs::read_to_string("tests/recordings/t05-holdings.json").expect("fixture"),
        )
        .expect("json");
        for (body, expected) in [
            (
                recording["body"].clone(),
                application::PortErrorKind::MasterNotReady(MasterNotReady::MissingInstrument),
            ),
            (
                {
                    let mut invalid = recording["body"].clone();
                    invalid["result"]["items"][0]["quantity"] = json!("invalid");
                    invalid
                },
                application::PortErrorKind::InvalidResponse(
                    application::ResponseMismatch::Validation,
                ),
            ),
        ] {
            let server = ScriptedHttpServer::start(vec![token(), response("200 OK", &body)])
                .expect("server");
            let reader = reader(&server, crate::support::time())
                .with_instrument_master(Authority::default(), issuer(&[]));
            let error = run(reader.holdings(&domain::AccountId::new(413))).expect_err("error");
            assert_eq!(error.kind(), expected);
            server.finish().expect("requests");
        }
    }

    fn review2_active_holdings_master() -> MasterInstrumentFacts {
        let mut details = details();
        details["result"][1]["status"] = json!("ACTIVE");
        details["result"][1]["koreanMarketDetail"]["liquidationTrading"] = json!(false);
        let mut replies = responses();
        *replies.last_mut().expect("details reply") = response("200 OK", &details);
        let server = ScriptedHttpServer::start(replies).expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&["stock-id", "etf-id"]));
        let facts = run(reader.instruments(KnownMarket::Kr))
            .expect("master")
            .complete_facts()
            .expect("complete");
        server.finish().expect("requests");
        facts
    }

    fn review2_holdings_body() -> Value {
        let recording: Value = serde_json::from_str(
            &std::fs::read_to_string("tests/recordings/t05-holdings.json").expect("fixture"),
        )
        .expect("json");
        recording["body"].clone()
    }

    fn review2_single_holdings_body() -> Value {
        let mut body = review2_holdings_body();
        body["result"]["items"]
            .as_array_mut()
            .expect("items")
            .truncate(1);
        body
    }

    #[test]
    fn review2_bad_holdings_are_rejected_before_master_lookup() {
        let facts = review2_active_holdings_master();
        let empty = MasterInstrumentFacts::try_new(vec![]).expect("empty master");
        let mut cases = Vec::new();
        for symbol in ["", "@@@", "005930 ", "12345", "00593a"] {
            let mut body = review2_single_holdings_body();
            body["result"]["items"][0]["symbol"] = json!(symbol);
            cases.push(body);
        }
        let mut lowercase_us = review2_holdings_body();
        let us_rows = lowercase_us["result"]["items"]
            .as_array_mut()
            .expect("items");
        us_rows[0] = us_rows[1].clone();
        us_rows.truncate(1);
        us_rows[0]["symbol"] = json!("aapl");
        cases.push(lowercase_us);
        for (field, value) in [
            ("quantity", "-1"),
            ("lastPrice", "-1"),
            ("averagePurchasePrice", "-1"),
        ] {
            let mut body = review2_single_holdings_body();
            body["result"]["items"][0][field] = json!(value);
            cases.push(body);
        }
        let mut negative_valuation = review2_single_holdings_body();
        negative_valuation["result"]["items"][0]["marketValue"]["amount"] = json!("-1");
        cases.push(negative_valuation);
        let mut later_bad_row = review2_holdings_body();
        later_bad_row["result"]["items"][1]["lastPrice"] = json!("-1");
        cases.push(later_bad_row);
        let mut duplicate = review2_single_holdings_body();
        let rows = duplicate["result"]["items"].as_array_mut().expect("items");
        rows.push(rows[0].clone());
        cases.push(duplicate);
        for master in [&empty, &facts] {
            for body in &cases {
                let server = ScriptedHttpServer::start(vec![token(), response("200 OK", body)])
                    .expect("server");
                let reader = reader(&server, crate::support::time())
                    .with_market_master(master.clone())
                    .with_instrument_master(Authority::default(), issuer(&[]));
                let error =
                    run(reader.holdings(&domain::AccountId::new(413))).expect_err("bad row");
                assert_eq!(
                    error.kind(),
                    application::PortErrorKind::InvalidResponse(
                        application::ResponseMismatch::Validation,
                    )
                );
                assert_eq!(error.status(), application::HttpStatus::Success);
                server.finish().expect("requests");
            }
        }
    }

    #[test]
    fn review2_valid_holding_requires_a_matching_master() {
        let facts = review2_active_holdings_master();
        let mut body = review2_holdings_body();
        body["result"]["items"]
            .as_array_mut()
            .expect("items")
            .truncate(1);
        for master in [
            MasterInstrumentFacts::try_new(vec![]).expect("empty"),
            facts,
        ] {
            let server = ScriptedHttpServer::start(vec![token(), response("200 OK", &body)])
                .expect("server");
            let reader = reader(&server, crate::support::time())
                .with_market_master(master.clone())
                .with_instrument_master(Authority::default(), issuer(&[]));
            let result = run(reader.holdings(&domain::AccountId::new(413)));
            if master.instruments().is_empty() {
                assert_eq!(
                    result.expect_err("missing master").kind(),
                    application::PortErrorKind::MasterNotReady(MasterNotReady::MissingInstrument)
                );
            } else {
                let holdings = result.expect("valid holding");
                assert_eq!(holdings.holdings().len(), 1);
                assert_eq!(
                    holdings.holdings()[0].instrument().symbol().as_str(),
                    "005930"
                );
            }
            server.finish().expect("requests");
        }
    }

    #[test]
    fn review2_issuer_error_preserves_its_origin_and_response_status() {
        use application::{SystemFailure, SystemPort};
        let original =
            SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        let server = ScriptedHttpServer::start(responses()).expect("server");
        let issuer = issuer(&[]);
        issuer.push(Err(original));
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer);
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("issuer failure");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::MasterNotReady(MasterNotReady::Issuer(original))
        );
        assert_eq!(error.status(), application::HttpStatus::Success);
        server.finish().expect("requests");
    }

    #[test]
    fn review2_ensure_missing_is_a_repository_error_without_assignment() {
        use application::{SystemFailure, SystemPort};
        let server = ScriptedHttpServer::start(responses()).expect("server");
        let authority = Authority {
            mapping_failure: Some(MappingFailure::EnsureMissing),
            ..Authority::default()
        };
        let repository = Arc::clone(&authority.repository);
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(authority, issuer(&["candidate"]));
        let error = run(reader.instruments(KnownMarket::Kr)).expect_err("invalid ensure result");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::MasterNotReady(MasterNotReady::Storage(
                SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
            ))
        );
        assert_eq!(error.status(), application::HttpStatus::Success);
        assert_eq!(
            run(repository.resolve(&source("005930", "KR7005930003"))),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        server.finish().expect("requests");
    }

    #[test]
    fn review2_uncertain_retirement_preserves_the_existing_mapping() {
        let mut details = details();
        details["result"][1]["status"] = json!("DELISTED");
        let mut replies = responses();
        *replies.last_mut().expect("details reply") = response("200 OK", &details);
        let server = ScriptedHttpServer::start(replies).expect("server");
        let authority = Authority {
            mapping_failure: Some(MappingFailure::RetireUncertain),
            ..Authority::default()
        };
        let repository = Arc::clone(&authority.repository);
        let original = source("005930", "KR7005930003");
        run(repository.ensure(&original, &id("old-id"))).expect("seed");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(authority, issuer(&["other-id"]));
        let master = run(reader.instruments(KnownMarket::Kr)).expect("master");
        assert_eq!(
            master.entries()[0],
            InstrumentMasterEntry::Unavailable {
                source: original.clone(),
                reason: InstrumentMappingError::IdentityUncertain,
            }
        );
        assert_eq!(run(repository.resolve(&original)), Ok(id("old-id")));
        server.finish().expect("requests");
    }

    #[test]
    fn review3_real_ledger_rejects_uncertain_retirement_without_changing_history() {
        let mut details = details();
        details["result"][1]["status"] = json!("DELISTED");
        let mut replies = responses();
        *replies.last_mut().expect("details reply") = response("200 OK", &details);
        let server = ScriptedHttpServer::start(replies).expect("server");
        let authority = Authority::default();
        let repository = Arc::clone(&authority.repository);
        let existing = source("005930", "KR7005930099");
        let observed = source("005930", "KR7005930003");
        run(repository.ensure(&existing, &id("existing-id"))).expect("seed");
        let before = run(repository.lookup(&existing)).expect("existing record");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(authority, issuer(&["other-id"]));
        let master = run(reader.instruments(KnownMarket::Kr)).expect("master");
        assert_eq!(
            master.entries()[0],
            InstrumentMasterEntry::Unavailable {
                source: observed.clone(),
                reason: InstrumentMappingError::IdentityUncertain,
            }
        );
        assert_eq!(run(repository.lookup(&existing)), Ok(before));
        assert_eq!(
            run(repository.resolve(&observed)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdentityUncertain
            ))
        );
        server.finish().expect("requests");
    }

    #[test]
    fn review3_unknown_etf_type_is_not_a_proven_response_conflict() {
        let mut list = list();
        list["result"][0]["securityType"] = json!("NEW_KIND");
        let mut details = details();
        details["result"][1]["securityType"] = json!("NEW_KIND");
        let master_server = ScriptedHttpServer::start(vec![
            token(),
            response("200 OK", &list),
            response("200 OK", &empty()),
            response("200 OK", &empty()),
            response("200 OK", &details),
        ])
        .expect("server");
        let master_reader = reader(&master_server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&["unknown-id", "etf-id"]));
        let master = run(master_reader.instruments(KnownMarket::Kr))
            .expect("master")
            .complete_facts()
            .expect("complete");
        master_server.finish().expect("requests");
        let body = json!({"result":{"etfs":[{"symbol":"005930","name":"합성 하나"}]}});
        let sector = domain::SectorId::try_new("100").expect("sector");
        let legacy_server =
            ScriptedHttpServer::start(vec![token(), response("200 OK", &body)]).expect("server");
        let legacy =
            reader(&legacy_server, crate::support::time()).with_market_master(master.clone());
        let legacy_error = run(legacy.sector_etfs(&sector, KnownMarket::Kr)).expect_err("unknown");
        assert_eq!(
            legacy_error.kind(),
            application::PortErrorKind::Unsupported(application::BrokerCapability::Instruments)
        );
        legacy_server.finish().expect("requests");
        let server =
            ScriptedHttpServer::start(vec![token(), response("200 OK", &body)]).expect("server");
        let reader = reader(&server, crate::support::time())
            .with_market_master(master)
            .with_instrument_master(Authority::default(), issuer(&[]));
        let error = run(reader.sector_etfs(&sector, KnownMarket::Kr)).expect_err("unknown");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::Unsupported(application::BrokerCapability::Instruments)
        );
        assert_eq!(error.status(), application::HttpStatus::Success);
        server.finish().expect("requests");
    }

    #[test]
    fn review_regression_etf_type_conflict_is_not_a_missing_instrument() {
        let mut replies = responses();
        replies.push(response(
            "200 OK",
            &json!({"result":{"etfs":[{"symbol":"005930","name":"합성 하나"}]}}),
        ));
        let server = ScriptedHttpServer::start(replies).expect("server");
        let reader = reader(&server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&["stock-probe", "etf-probe"]));
        let master = run(reader.instruments(KnownMarket::Kr))
            .expect("master")
            .complete_facts()
            .expect("complete");
        assert_eq!(master.instruments()[0].row().symbol.as_str(), "005930");
        assert_eq!(
            master.instruments()[0].row().security_type.code().as_str(),
            "STOCK"
        );
        let populated_reader = reader.with_market_master(master);
        let error = run(populated_reader.sector_etfs(
            &domain::SectorId::try_new("100").expect("sector"),
            KnownMarket::Kr,
        ))
        .expect_err("wrong type");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        server.finish().expect("requests");
    }

    fn review_timestamp(text: &str) -> Timestamp {
        broker_toss::TossTimestamp::try_from(text)
            .expect("timestamp")
            .into_domain()
    }

    fn review_candle_sessions() -> Vec<domain::MarketSession> {
        use domain::{MarketSession, SessionData, SessionInformation, TimeInterval};
        ["2026-03-24", "2026-03-25"]
            .map(|date| {
                let open = review_timestamp(&format!("{date}T09:00:00+09:00"));
                let close = review_timestamp(&format!("{date}T15:30:00+09:00"));
                MarketSession::try_new(
                    open.offset_calendar_date(),
                    SessionInformation::Kr {
                        pre_market: None,
                        regular_market: Some(
                            SessionData::try_new(
                                TimeInterval::try_new(open, close).expect("window"),
                                None,
                            )
                            .expect("session data"),
                        ),
                        after_market: None,
                    },
                )
                .expect("session")
            })
            .to_vec()
    }

    #[test]
    fn review_regression_adjusted_daily_missing_master_is_a_dedicated_cause() {
        use domain::TimeInterval;
        let master_server = ScriptedHttpServer::start(responses()).expect("server");
        let populated = reader(&master_server, crate::support::time())
            .with_instrument_master(Authority::default(), issuer(&["one", "two"]));
        let facts = run(populated.instruments(KnownMarket::Kr))
            .expect("master")
            .complete_facts()
            .expect("facts");
        let instrument = facts.instruments()[0].quote_instrument().expect("quote");
        master_server.finish().expect("requests");
        let fixture: Value = serde_json::from_str(
            &std::fs::read_to_string("tests/market_recordings/t07-daily.json").expect("fixture"),
        )
        .expect("json");
        let server = ScriptedHttpServer::start(vec![token(), response("200 OK", &fixture["body"])])
            .expect("server");
        let reader = reader(&server, crate::support::time())
            .with_candle_sessions(review_candle_sessions())
            .with_instrument_master(Authority::default(), issuer(&[]));
        let error = run(reader.candles(
            &instrument,
            TimeInterval::try_new(
                review_timestamp("2026-03-24T00:00:00+09:00"),
                review_timestamp("2026-03-26T00:00:00+09:00"),
            )
            .expect("window"),
            domain::CandlePeriod::Daily,
            domain::PriceAdjustment::Adjusted(domain::AdjustmentBasis::new(review_timestamp(
                "2026-03-26T00:00:00+09:00",
            ))),
        ))
        .expect_err("missing master");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::MasterNotReady(MasterNotReady::MissingInstrument)
        );
        server.finish().expect("requests");
    }
}
