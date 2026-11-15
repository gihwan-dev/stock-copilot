#[cfg(test)]
mod tests {
    use application::{
        Clock, EventBus, SecretKey, SecretStore, SecretValue, SystemPortError, WriteCompleted,
    };
    use broker_toss::{
        BrokerConnection, TokenError, TokenManager, TokenRenewal, TossEndpoint, TossHttpClient,
    };
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use std::future::Future;
    use std::sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    };
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
    #[derive(Clone)]
    struct FakeClock(Arc<AtomicI64>);
    impl Clock for FakeClock {
        fn now(&self) -> Timestamp {
            Timestamp::from_parts(
                UnixSeconds::new(self.0.load(Ordering::SeqCst)),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("time")
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
        fn reconcile_orders(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready(Ok(()))
        }
    }
    fn response(status: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }
    fn issued(value: &str) -> Vec<u8> {
        response(
            "200 OK",
            &format!(r#"{{"access_token":"{value}","token_type":"Bearer","expires_in":120}}"#),
        )
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }
    fn manager(
        server: &ScriptedHttpServer,
        clock: FakeClock,
    ) -> TokenManager<Store, FakeClock, Renewal> {
        TokenManager::new(
            TossHttpClient::new(&TossEndpoint::local(server.port()).expect("endpoint"))
                .expect("client"),
            Store,
            clock,
            Renewal,
            EventBus::default(),
        )
    }

    async fn issue_shared(
        manager: Arc<TokenManager<Store, FakeClock, Renewal>>,
    ) -> Result<broker_toss::TokenLease, TokenError> {
        manager.token().await
    }
    #[test]
    fn simultaneous_requests_share_one_issue_and_secret_debug_is_redacted() {
        let server = ScriptedHttpServer::start(vec![issued("fake-token-913")]).expect("server");
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        let manager = Arc::new(manager(&server, clock));
        runtime().block_on(async {
            let mut tasks = Vec::new();
            for _ in 0_u32..32 {
                let shared = Arc::clone(&manager);
                tasks.push(tokio::spawn(issue_shared(shared)));
            }
            for task in tasks {
                let token = task.await.expect("join").expect("token");
                assert_eq!(token.secret().as_exposed_secret(), b"fake-token-913");
                assert!(!format!("{token:?}").contains("913"));
            }
            assert_eq!(manager.connection().await, BrokerConnection::Connected);
        });
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("/oauth2/token HTTP/1.1").count(), 1);
        assert!(requests.contains("client_id=fake-id-19&client_secret=fake-key-73"));
    }
    #[test]
    fn refresh_boundary_is_before_expiry_and_failure_never_reuses_invalidated_token() {
        let server = ScriptedHttpServer::start(vec![
            issued("old-913"),
            response("503 Service Unavailable", "{}"),
            issued("new-417"),
        ])
        .expect("server");
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        let manager = manager(&server, clock.clone());
        runtime().block_on(async {
            assert_eq!(
                manager
                    .token()
                    .await
                    .expect("first")
                    .secret()
                    .as_exposed_secret(),
                b"old-913"
            );
            clock.0.store(1059, Ordering::SeqCst);
            assert_eq!(
                manager
                    .token()
                    .await
                    .expect("before")
                    .secret()
                    .as_exposed_secret(),
                b"old-913"
            );
            clock.0.store(1060, Ordering::SeqCst);
            manager.token().await.expect_err("issuance failure");
            clock.0.store(1120, Ordering::SeqCst);
            assert_eq!(
                manager
                    .token()
                    .await
                    .expect("recovery")
                    .secret()
                    .as_exposed_secret(),
                b"new-417"
            );
        });
        assert_eq!(
            String::from_utf8(server.finish().expect("requests"))
                .expect("utf8")
                .matches("/oauth2/token HTTP/1.1")
                .count(),
            3
        );
    }
    fn check_event(event: &domain::DomainEvent) {
        assert_eq!(event, &domain::DomainEvent::BrokerConnectionChanged);
    }
    struct Counter(Arc<std::sync::atomic::AtomicUsize>);
    impl application::EventSubscriber for Counter {
        fn on_event(
            &self,
            event: &domain::DomainEvent,
        ) -> Result<(), application::SubscriberFailure> {
            check_event(event);
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }
    #[test]
    fn expiry_failures_and_recovery_publish_only_changed_state() {
        let server = ScriptedHttpServer::start(vec![
            issued("first-19"),
            response("503 Busy", "{}"),
            response("503 Busy", "{}"),
            issued("second-73"),
        ])
        .expect("server");
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut builder = application::EventBusBuilder::default();
        builder.subscribe(
            std::collections::BTreeSet::from([domain::DomainEventKind::BrokerConnectionChanged]),
            Counter(Arc::clone(&count)),
        );
        let manager = TokenManager::new(
            TossHttpClient::new(&TossEndpoint::local(server.port()).expect("endpoint"))
                .expect("client"),
            Store,
            clock.clone(),
            Renewal,
            builder.build(),
        );
        runtime().block_on(async {
            manager.token().await.expect("initial");
            assert_eq!(count.load(Ordering::SeqCst), 1);
            clock.0.store(1119, Ordering::SeqCst);
            manager.token().await.expect_err("issuance failure");
            assert_eq!(count.load(Ordering::SeqCst), 2);
            clock.0.store(1120, Ordering::SeqCst);
            manager.token().await.expect_err("issuance failure");
            assert_eq!(manager.connection().await, BrokerConnection::Disconnected);
            assert_eq!(count.load(Ordering::SeqCst), 2);
            clock.0.store(1121, Ordering::SeqCst);
            manager.token().await.expect("recovery");
            manager.token().await.expect("cached");
            assert_eq!(count.load(Ordering::SeqCst), 3);
            assert_eq!(
                manager.delivery_report().await.expect("report").failures,
                Vec::new()
            );
        });
        server.finish().expect("requests");
    }
    #[test]
    fn revoked_reissues_once_then_stops_all_requests() {
        let revoked = response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#);
        let server = ScriptedHttpServer::start(vec![
            issued("one-19"),
            revoked.clone(),
            issued("two-73"),
            revoked,
        ])
        .expect("server");
        let manager = manager(&server, FakeClock(Arc::new(AtomicI64::new(1000))));
        runtime().block_on(async {
            let lookup = broker_toss::ReadRequest::new(broker_toss::ReadRoute::Accounts);
            assert_eq!(
                manager.get(&lookup).await.expect_err("repeat"),
                TokenError::RevokedRepeated
            );
            assert_eq!(manager.connection().await, BrokerConnection::DuplicateKey);
            assert_eq!(
                manager.get(&lookup).await.expect_err("stopped"),
                TokenError::RevokedRepeated
            );
        });
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("/oauth2/token HTTP/1.1").count(), 2);
        assert_eq!(requests.matches("GET /api/v1/accounts").count(), 2);
    }
    #[test]
    fn revoked_retry_can_recover_and_other_statuses_do_not_reissue() {
        let server = ScriptedHttpServer::start(vec![
            issued("one-19"),
            response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#),
            issued("two-73"),
            response("200 OK", "{}"),
            response("503 Busy", "{}"),
        ])
        .expect("server");
        let manager = manager(&server, FakeClock(Arc::new(AtomicI64::new(1000))));
        runtime().block_on(async {
            let lookup = broker_toss::ReadRequest::new(broker_toss::ReadRoute::Accounts);
            assert_eq!(manager.get(&lookup).await.expect("recovered").status(), 200);
            assert_eq!(
                manager.get(&lookup).await.expect("raw failure").status(),
                503
            );
        });
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("/oauth2/token HTTP/1.1").count(), 2);
        assert!(requests.contains("Bearer two-73"));
    }
    struct OrderedRenewal(Arc<std::sync::RwLock<Vec<&'static str>>>);
    impl TokenRenewal for OrderedRenewal {
        fn close(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready({
                self.0.write().expect("lock").push("close");
                Ok(())
            })
        }
        fn reconnect(
            &self,
            token: &SecretValue,
        ) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready({
                assert_eq!(token.as_exposed_secret(), b"sequence-37");
                self.0.write().expect("lock").push("reconnect");
                Ok(())
            })
        }
        fn resubscribe(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready({
                self.0.write().expect("lock").push("resubscribe");
                Ok(())
            })
        }
        fn reconcile_orders(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready({
                self.0.write().expect("lock").push("orders");
                Ok(())
            })
        }
    }
    #[test]
    fn issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation() {
        let server = ScriptedHttpServer::start(vec![issued("sequence-37")]).expect("server");
        let order = Arc::new(std::sync::RwLock::new(Vec::new()));
        let manager = TokenManager::new(
            TossHttpClient::new(&TossEndpoint::local(server.port()).expect("endpoint"))
                .expect("client"),
            Store,
            FakeClock(Arc::new(AtomicI64::new(1000))),
            OrderedRenewal(Arc::clone(&order)),
            EventBus::default(),
        );
        runtime().block_on(manager.token()).expect("token");
        assert_eq!(
            *order.read().expect("lock"),
            ["close", "reconnect", "resubscribe", "orders"]
        );
        server.finish().expect("requests");
    }
    #[test]
    fn malformed_tokens_are_closed_errors_and_never_become_credentials() {
        for body in [
            "{}",
            r#"{"access_token":"sensitive-913","token_type":"Basic","expires_in":120}"#,
            r#"{"access_token":"","token_type":"Bearer","expires_in":120}"#,
            r#"{"access_token":"bad\r\nvalue","token_type":"Bearer","expires_in":120}"#,
            r#"{"access_token":"x","token_type":"Bearer","expires_in":0}"#,
            r#"{"access_token":"x","token_type":"Bearer","expires_in":1}"#,
            r#"{"access_token":"x","token_type":"Bearer","expires_in":18446744073709551615}"#,
        ] {
            let server = ScriptedHttpServer::start(vec![response("200 OK", body)]).expect("server");
            let manager = manager(&server, FakeClock(Arc::new(AtomicI64::new(1000))));
            let error = runtime().block_on(manager.token()).expect_err("invalid");
            assert_eq!(error, TokenError::InvalidResponse);
            assert!(!format!("{error:?}{manager:?}").contains("913"));
            server.finish().expect("requests");
        }
    }
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Stage {
        Close,
        Connect,
        Subscribe,
        Orders,
    }
    struct FailingRenewal(Stage, Arc<std::sync::RwLock<Vec<Stage>>>);
    fn record_stage(calls: &std::sync::RwLock<Vec<Stage>>, stage: Stage) {
        calls.write().expect("lock").push(stage);
    }
    impl FailingRenewal {
        fn step(&self, stage: Stage) -> Result<(), TokenError> {
            record_stage(&self.1, stage);
            if self.0 == stage {
                Err(TokenError::Renewal)
            } else {
                Ok(())
            }
        }
    }
    impl TokenRenewal for FailingRenewal {
        fn close(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready(self.step(Stage::Close))
        }
        fn reconnect(
            &self,
            _: &SecretValue,
        ) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready(self.step(Stage::Connect))
        }
        fn resubscribe(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready(self.step(Stage::Subscribe))
        }
        fn reconcile_orders(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready(self.step(Stage::Orders))
        }
    }
    #[test]
    fn failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed() {
        for (stage, expected) in [
            (Stage::Close, vec![Stage::Close]),
            (Stage::Connect, vec![Stage::Close, Stage::Connect]),
            (
                Stage::Subscribe,
                vec![Stage::Close, Stage::Connect, Stage::Subscribe],
            ),
            (
                Stage::Orders,
                vec![
                    Stage::Close,
                    Stage::Connect,
                    Stage::Subscribe,
                    Stage::Orders,
                ],
            ),
        ] {
            let server = ScriptedHttpServer::start(vec![issued("never-used-937")]).expect("server");
            let calls = Arc::new(std::sync::RwLock::new(Vec::new()));
            let manager = TokenManager::new(
                TossHttpClient::new(&TossEndpoint::local(server.port()).expect("endpoint"))
                    .expect("client"),
                Store,
                FakeClock(Arc::new(AtomicI64::new(1000))),
                FailingRenewal(stage, Arc::clone(&calls)),
                EventBus::default(),
            );
            runtime().block_on(async {
                assert_eq!(
                    manager.token().await.expect_err("stage failure"),
                    TokenError::Renewal
                );
                assert_eq!(manager.connection().await, BrokerConnection::Disconnected);
            });
            assert_eq!(*calls.read().expect("lock"), expected);
            server.finish().expect("captured");
        }
    }
}
