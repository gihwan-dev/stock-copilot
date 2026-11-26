mod support;
#[cfg(test)]
mod tests {
    use application::{
        BrokerRead, Clock, DiagnosticEvent, DiagnosticsSink, EventBus, SecretKey, SecretStore,
        SecretValue, SystemPortError, WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
    };
    use domain::{AccountId, Currency, MasterInstrumentFacts, Timestamp};
    use serde_json::{Value, json};
    use std::sync::{Arc, OnceLock, RwLock};
    use std::{future::Future, sync::LazyLock};
    use testkit::ScriptedHttpServer;

    const CLIENT_ID: &str = "tsck_live_fake-id-19";
    const CLIENT_SECRET: &str = "s_fake-key-73";
    const ACCESS_TOKEN: &str = "fake-access-91";
    const ACCOUNT_SEQ: i64 = 70_219;
    const ACCOUNT_NO: &str = "00000070219";
    const AMOUNT: &str = "98765432";

    static EVENTS: LazyLock<RwLock<Vec<String>>> = LazyLock::new(|| RwLock::new(Vec::new()));
    static INSTALLED: OnceLock<()> = OnceLock::new();

    struct Capture;
    impl DiagnosticsSink for Capture {
        fn record(&self, event: &DiagnosticEvent) {
            EVENTS.write().expect("events").push(format!("{event:?}"));
        }
    }

    fn capture() {
        INSTALLED.get_or_init(|| {
            let _installed = broker_toss::install_diagnostics(Arc::new(Capture));
        });
    }

    fn captured() -> String {
        EVENTS.read().expect("events").join("\n")
    }

    fn assert_redacted(log: &str) {
        for secret in [
            CLIENT_ID,
            CLIENT_SECRET,
            ACCESS_TOKEN,
            ACCOUNT_NO,
            "70219",
            AMOUNT,
            "Bearer fake",
        ] {
            assert!(!log.contains(secret), "{secret} leaked:\n{log}");
        }
    }

    struct Store;
    impl SecretStore for Store {
        fn load(
            &self,
            key: SecretKey,
        ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
            std::future::ready(Ok(Some(SecretValue::new(match key {
                SecretKey::TossClientId => CLIENT_ID.as_bytes().to_vec(),
                SecretKey::TossClientSecret => CLIENT_SECRET.as_bytes().to_vec(),
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

    fn response(status: &str, body: &Value) -> Vec<u8> {
        let text = body.to_string();
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
            text.len()
        )
        .into_bytes()
    }

    fn token(kind: &str) -> Vec<u8> {
        response(
            "200 OK",
            &json!({"access_token": ACCESS_TOKEN, "token_type": kind, "expires_in": 3600}),
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

    #[test]
    fn decode_failures_are_logged_with_shape_but_without_values() {
        capture();
        let server = ScriptedHttpServer::start(vec![
            token("Bearer"),
            response(
                "200 OK",
                &json!({"result": {"currency": "KRW", "cashBuyingPower": 98_765_432_i64}}),
            ),
        ])
        .expect("server");
        let reader = adapter(&server);
        let result = run(BrokerRead::buying_power(
            &reader,
            &AccountId::new(ACCOUNT_SEQ),
            Currency::Krw,
        ));
        let _failure = result.expect_err("integer buying power");
        let requests = String::from_utf8(server.finish().expect("finish")).expect("requests");
        assert!(requests.contains(ACCOUNT_NO.trim_start_matches('0')));
        let log = captured();
        assert!(
            log.contains("path: DiagnosticText(\"/oauth2/token\")"),
            "{log}"
        );
        assert!(
            log.contains("path: DiagnosticText(\"/api/v1/buying-power\")"),
            "{log}"
        );
        assert!(log.contains("TokenIssue(Issued)"), "{log}");
        assert!(
            log.contains("endpoint: BuyingPower, mismatch: Schema, detail: DiagnosticText(\"data: invalid type: integer <value>, expected a string\")"),
            "{log}"
        );
        assert_redacted(&log);
    }

    #[test]
    fn token_shape_and_error_codes_are_logged_without_secrets() {
        capture();
        let server = ScriptedHttpServer::start(vec![token("bearer")]).expect("server");
        let lowercase = adapter(&server);
        let _token_failure = run(BrokerRead::accounts(&lowercase)).expect_err("lowercase type");
        let _token_requests = server.finish().expect("finish");
        let rejected = ScriptedHttpServer::start(vec![
            token("Bearer"),
            response(
                "403 Forbidden",
                &json!({"error": {"code": "ip-not-allowed", "requestId": "req-731", "message": ACCOUNT_NO}}),
            ),
        ])
        .expect("server");
        let forbidden = adapter(&rejected);
        let _forbidden_failure = run(BrokerRead::accounts(&forbidden)).expect_err("forbidden");
        let _forbidden_requests = rejected.finish().expect("finish");
        let log = captured();
        assert!(
            log.contains("InvalidResponse(DiagnosticText(\"data: unknown variant <value>, expected `Bearer` at line 1 column"),
            "{log}"
        );
        assert!(
            log.contains("code: Some(DiagnosticText(\"ip-not-allowed\"))"),
            "{log}"
        );
        assert!(
            log.contains("request: Some(DiagnosticText(\"req-731\"))"),
            "{log}"
        );
        assert!(log.contains("Status(HttpStatusCode(403))"), "{log}");
        assert_redacted(&log);
    }
}
