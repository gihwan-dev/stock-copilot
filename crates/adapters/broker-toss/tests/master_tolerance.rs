mod support;
#[cfg(test)]
mod tests {
    use application::{
        BrokerRead, Clock, DiagnosticEvent, DiagnosticsSink, EventBus, InstrumentMappingRepository,
        InstrumentMasterAuthority, InstrumentMasterObservation, InstrumentMasterSession,
        MappingStoreError, MasterNotReady, SecretKey, SecretStore, SecretValue, SystemPortError,
        WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
    };
    use domain::{
        AccountId, InstrumentId, InstrumentMappingRecord, InstrumentMasterEntry, InstrumentSource,
        KnownMarket, MappingAssignment, MasterInstrumentFacts, NxtTrading, Timestamp,
    };
    use serde_json::{Value, json};
    use std::future::Future;
    use std::sync::{Arc, LazyLock, OnceLock, RwLock};
    use testkit::ScriptedHttpServer;
    use testkit_ports::{ScriptedInstrumentIdIssuer, memory::MemoryInstrumentMappingRepository};

    static EVENTS: LazyLock<RwLock<Vec<String>>> = LazyLock::new(|| RwLock::new(Vec::new()));
    static INSTALLED: OnceLock<()> = OnceLock::new();
    static SERIAL: RwLock<()> = RwLock::new(());

    struct Capture;
    impl DiagnosticsSink for Capture {
        fn record(&self, event: &DiagnosticEvent) {
            if let DiagnosticEvent::ResponseRejected { detail, .. } = event {
                EVENTS
                    .write()
                    .expect("events")
                    .push(detail.as_str().to_owned());
            }
        }
    }

    fn captured_during<T>(work: impl FnOnce() -> T) -> (T, Vec<String>) {
        let _serial = match SERIAL.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        INSTALLED.get_or_init(|| {
            let _installed = broker_toss::install_diagnostics(Arc::new(Capture));
        });
        EVENTS.write().expect("events").clear();
        let result = work();
        (result, EVENTS.read().expect("events").clone())
    }

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
    struct Authority(MemoryInstrumentMappingRepository);
    struct Session<'a>(&'a MemoryInstrumentMappingRepository);
    impl InstrumentMappingRepository for Session<'_> {
        fn resolve(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.0.resolve(source)
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
        fn ensure(
            &self,
            source: &InstrumentSource,
            candidate: domain::IssuedInstrumentId,
        ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send {
            self.0.ensure(source, candidate)
        }
        fn retire(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.0.retire(source)
        }
    }
    impl InstrumentMasterSession for Session<'_> {
        fn verify_observation(
            &self,
            _: &[InstrumentMasterObservation],
        ) -> impl Future<Output = Result<(), MasterNotReady>> + Send {
            std::future::ready(Ok(()))
        }
    }
    impl InstrumentMasterAuthority for Authority {
        type Session<'a> = Session<'a>;
        fn begin(
            &self,
            _: KnownMarket,
        ) -> impl Future<Output = Result<Self::Session<'_>, MasterNotReady>> + Send {
            std::future::ready(Ok(Session(&self.0)))
        }
    }

    fn issuer(count: usize) -> ScriptedInstrumentIdIssuer {
        let issuer = ScriptedInstrumentIdIssuer::default();
        let mut generator = domain::IndependentInstrumentIdIssuer::new();
        for _ in 0..count {
            issuer.push(Ok(generator.issue().expect("issued")));
        }
        issuer
    }

    fn response(body: &Value) -> Vec<u8> {
        let text = body.to_string();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
            text.len()
        )
        .into_bytes()
    }

    fn rows(rows: &Value) -> Vec<u8> {
        response(&json!({ "result": rows }))
    }

    fn token() -> Vec<u8> {
        response(
            &json!({"access_token":"synthetic-access","token_type":"Bearer","expires_in":3600}),
        )
    }

    fn reader(server: &ScriptedHttpServer) -> TossAccountRead<Store, Time, Renewal> {
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
            MasterInstrumentFacts::try_new(Vec::new()).expect("facts"),
        )
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }

    fn listed(symbol: &str, isin: &str) -> Value {
        json!({"symbol":symbol,"name":"합성 종목","securityType":"STOCK","isCommonShare":true,"isinCode":isin})
    }

    fn detail(symbol: &str, isin: &str) -> Value {
        json!({"symbol":symbol,"name":"합성 종목","englishName":"Synthetic","isinCode":isin,"market":"KOSPI","securityType":"STOCK","isCommonShare":true,"status":"ACTIVE","currency":"KRW","listDate":null,"delistDate":null,"sharesOutstanding":"1","leverageFactor":null,"koreanMarketDetail":{"liquidationTrading":false,"nxtSupported":true,"krxTradingSuspended":false,"nxtTradingSuspended":false}})
    }

    fn odd_kr_script() -> Vec<Vec<u8>> {
        let mut nxt_mismatch = detail("000020", "KR7000020008");
        nxt_mismatch["koreanMarketDetail"] = json!({"liquidationTrading":false,"nxtSupported":false,"krxTradingSuspended":false,"nxtTradingSuspended":false});
        nxt_mismatch["name"] = json!("상세 이름");
        let mut unknown_status = detail("000030", "KR7000030007");
        unknown_status["status"] = json!("HALTED");
        let mut wrong_currency = detail("000040", "KR7000040006");
        wrong_currency["currency"] = json!("JPY");
        vec![
            token(),
            rows(&json!([
                listed("000010", "KR7000010009"),
                listed("000020", "KR7000020008"),
                listed("000030", "KR7000030007"),
                listed("000040", "KR7000040006"),
                listed("000050", ""),
                listed("000060", "KR7000060004"),
                {"symbol":"000070","name":"합성 종목"},
                listed("000010", "KR7000010009"),
            ])),
            rows(&json!([])),
            rows(&json!([])),
            rows(&json!([
                detail("000010", "KR7000010009"),
                nxt_mismatch,
                unknown_status,
                wrong_currency,
                detail("999999", "KR7999999000"),
            ])),
        ]
    }

    #[test]
    fn product_master_skips_odd_rows_and_logs_each_reason_with_a_count() {
        let server = ScriptedHttpServer::start(odd_kr_script()).expect("server");
        let (result, log) = captured_during(|| {
            run(reader(&server).instrument_master(
                &Authority::default(),
                &issuer(2),
                KnownMarket::Kr,
            ))
        });
        let master = result.expect("odd rows must not fail the whole master");
        let available = master
            .entries()
            .iter()
            .filter_map(|entry| match entry {
                InstrumentMasterEntry::Available(instrument) => Some(instrument.row().clone()),
                InstrumentMasterEntry::Unavailable { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            available
                .iter()
                .map(|row| row.symbol.as_str().to_owned())
                .collect::<Vec<_>>(),
            vec![String::from("000010"), String::from("000020")]
        );
        assert_eq!(available[1].name.as_str(), "상세 이름");
        assert_eq!(
            available[1].korean_detail.map(|detail| detail.nxt),
            Some(NxtTrading::Unsupported)
        );
        for expected in [
            "skipped 1 instrument rows: listed ISIN is empty or invalid",
            "skipped 1 instrument rows: duplicate listed symbol",
            "skipped 1 instrument rows: detail listing status is unknown",
            "skipped 1 instrument rows: detail currency is unknown",
            "skipped 1 instrument rows: listed symbol has no detail row",
            "skipped 1 instrument rows: detail row for a symbol that was not requested",
        ] {
            assert!(
                log.iter().any(|line| line == expected),
                "{expected}: {log:?}"
            );
        }
        assert!(
            log.iter().any(|line| line
                .starts_with("skipped 1 instrument rows: listing row does not match the schema")),
            "{log:?}"
        );
        assert!(!log.iter().any(|line| line.contains("000050")), "{log:?}");
        server.finish().expect("requests");
    }

    #[test]
    fn strict_master_port_names_the_failed_validation_step() {
        let server = ScriptedHttpServer::start(vec![
            token(),
            rows(&json!([
                listed("000010", "KR7000010009"),
                listed("000050", "")
            ])),
        ])
        .expect("server");
        let (result, log) = captured_during(|| {
            run(reader(&server)
                .with_instrument_master(Authority::default(), issuer(0))
                .instruments(KnownMarket::Kr))
        });
        assert_eq!(
            result.expect_err("strict").kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
        assert_eq!(log, vec![String::from("listed ISIN is empty or invalid")]);
        server.finish().expect("requests");
    }

    #[test]
    fn a_master_where_every_row_is_rejected_is_not_saved_as_empty() {
        let server = ScriptedHttpServer::start(vec![
            token(),
            rows(&json!([listed("000050", "")])),
            rows(&json!([])),
            rows(&json!([])),
        ])
        .expect("server");
        let (result, log) = captured_during(|| {
            run(reader(&server).instrument_master(
                &Authority::default(),
                &issuer(0),
                KnownMarket::Kr,
            ))
        });
        let _error = result.expect_err("nothing usable");
        assert!(
            log.iter()
                .any(|line| line == "every instrument row was rejected"),
            "{log:?}"
        );
        server.finish().expect("requests");
    }

    #[test]
    fn us_rows_keep_valid_tickers_and_drop_a_stray_korean_detail() {
        let mut apple = detail("AAPL", "US0378331005");
        apple["market"] = json!("NASDAQ");
        apple["currency"] = json!("USD");
        let server = ScriptedHttpServer::start(vec![
            token(),
            rows(&json!([])),
            rows(&json!([
                listed("AAPL", "US0378331005"),
                listed("BRK/B", "US0846707026")
            ])),
            rows(&json!([])),
            rows(&json!([])),
            rows(&json!([apple])),
        ])
        .expect("server");
        let (result, log) = captured_during(|| {
            run(reader(&server).instrument_master(
                &Authority::default(),
                &issuer(1),
                KnownMarket::Us,
            ))
        });
        let facts = result
            .expect("master")
            .complete_facts()
            .expect("available rows only");
        assert_eq!(facts.instruments().len(), 1);
        assert_eq!(facts.instruments()[0].row().korean_detail, None);
        assert!(
            log.iter().any(
                |line| line == "skipped 1 instrument rows: listed symbol is not a valid symbol"
            ),
            "{log:?}"
        );
        server.finish().expect("requests");
    }

    fn holdings_body() -> Value {
        let recorded: Value = serde_json::from_str(
            &std::fs::read_to_string("tests/recordings/t05-holdings.json").expect("fixture"),
        )
        .expect("json");
        recorded["body"].clone()
    }

    #[test]
    fn holdings_missing_from_the_master_are_kept_with_the_reported_name() {
        let server =
            ScriptedHttpServer::start(vec![token(), response(&holdings_body())]).expect("server");
        let (result, log) = captured_during(|| {
            run(BrokerRead::holdings(
                &reader(&server).showing_unmatched_holdings(),
                &AccountId::new(413),
            ))
        });
        let observation = result.expect("holdings without a master");
        let shown = observation
            .holdings()
            .iter()
            .map(|holding| {
                (
                    holding.instrument().symbol().as_str().to_owned(),
                    holding.reported_name().map(|name| name.as_str().to_owned()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            shown,
            vec![
                (String::from("005930"), Some(String::from("삼성전자"))),
                (String::from("AAPL"), Some(String::from("Apple Inc."))),
            ]
        );
        assert_eq!(
            log,
            vec![String::from(
                "2 holdings not matched to the instrument master are shown as reported"
            )]
        );
        server.finish().expect("requests");
    }

    #[test]
    fn a_holding_without_a_name_is_kept_with_the_name_unknown() {
        let mut body = holdings_body();
        body["result"]["items"][0]["name"] = json!("");
        let server = ScriptedHttpServer::start(vec![token(), response(&body)]).expect("server");
        let (result, _log) = captured_during(|| {
            run(BrokerRead::holdings(
                &reader(&server).showing_unmatched_holdings(),
                &AccountId::new(413),
            ))
        });
        let observation = result.expect("holdings");
        assert_eq!(observation.holdings()[0].reported_name(), None);
        server.finish().expect("requests");
    }
}
