mod support;
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
        fn reconcile_orders(
            &self,
            _: &SecretValue,
        ) -> impl Future<Output = Result<(), TokenError>> + Send {
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
    fn client(server: &ScriptedHttpServer) -> TossHttpClient {
        let time = crate::support::time();
        TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("endpoint"),
            Arc::<crate::support::FakeTime>::clone(&time),
            time,
        )
        .expect("client")
    }
    fn manager(
        server: &ScriptedHttpServer,
        clock: FakeClock,
    ) -> TokenManager<Store, FakeClock, Renewal> {
        TokenManager::new(
            client(server),
            Store,
            clock,
            Renewal,
            Arc::new(EventBus::default()),
        )
    }

    async fn issue_shared<R: TokenRenewal>(
        manager: Arc<TokenManager<Store, FakeClock, R>>,
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
            client(&server),
            Store,
            clock.clone(),
            Renewal,
            Arc::new(builder.build()),
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
        fn reconcile_orders(
            &self,
            token: &SecretValue,
        ) -> impl Future<Output = Result<(), TokenError>> + Send {
            std::future::ready({
                assert_eq!(token.as_exposed_secret(), b"sequence-37");
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
            client(&server),
            Store,
            FakeClock(Arc::new(AtomicI64::new(1000))),
            OrderedRenewal(Arc::clone(&order)),
            Arc::new(EventBus::default()),
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
        fn reconcile_orders(
            &self,
            _: &SecretValue,
        ) -> impl Future<Output = Result<(), TokenError>> + Send {
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
                client(&server),
                Store,
                FakeClock(Arc::new(AtomicI64::new(1000))),
                FailingRenewal(stage, Arc::clone(&calls)),
                Arc::new(EventBus::default()),
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
    #[derive(Clone, Copy)]
    enum AdvanceTarget {
        RefreshAt,
        BeforeExpiry,
        ExpiresAt,
        AfterExpiry,
    }
    struct AdvancingRenewal(FakeClock, AdvanceTarget);
    impl TokenRenewal for AdvancingRenewal {
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
            let time = match self.1 {
                AdvanceTarget::RefreshAt => 1060,
                AdvanceTarget::BeforeExpiry => 1119,
                AdvanceTarget::ExpiresAt => 1120,
                AdvanceTarget::AfterExpiry => 1121,
            };
            self.0.0.store(time, Ordering::SeqCst);
            std::future::ready(Ok(()))
        }
    }
    #[test]
    fn queued_requests_share_success_even_when_refresh_becomes_due_during_issuance() {
        let server = ScriptedHttpServer::start(vec![issued("delayed-937")]).expect("server");
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        let manager = Arc::new(TokenManager::new(
            client(&server),
            Store,
            clock.clone(),
            AdvancingRenewal(clock, AdvanceTarget::RefreshAt),
            Arc::new(EventBus::default()),
        ));
        runtime().block_on(async {
            let mut tasks = Vec::new();
            for _ in 0_u32..8 {
                tasks.push(tokio::spawn(issue_shared(Arc::clone(&manager))));
            }
            for task in tasks {
                assert_eq!(
                    task.await
                        .expect("join")
                        .expect("shared issuance")
                        .secret()
                        .as_exposed_secret(),
                    b"delayed-937"
                );
            }
        });
        assert_eq!(
            String::from_utf8(server.finish().expect("capture"))
                .expect("utf8")
                .matches("/oauth2/token HTTP/1.1")
                .count(),
            1
        );
    }
    struct HangingRenewal(
        Arc<std::sync::atomic::AtomicUsize>,
        Arc<tokio::sync::Notify>,
        usize,
    );
    impl TokenRenewal for HangingRenewal {
        fn close(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            self.0.fetch_add(1, Ordering::SeqCst);
            std::future::ready(Ok(()))
        }
        async fn reconnect(&self, _: &SecretValue) -> Result<(), TokenError> {
            if self.0.load(Ordering::SeqCst) == self.2 {
                self.1.notify_one();
                std::future::pending().await
            } else {
                Ok(())
            }
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
    #[test]
    fn cancelled_renewal_disconnects_once_shares_failure_and_can_recover() {
        let server = ScriptedHttpServer::start(vec![
            issued("initial-913"),
            issued("cancelled-417"),
            issued("recovered-831"),
        ])
        .expect("server");
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        let closed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let notice = Arc::new(tokio::sync::Notify::new());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut builder = application::EventBusBuilder::default();
        builder.subscribe(
            std::collections::BTreeSet::from([domain::DomainEventKind::BrokerConnectionChanged]),
            Counter(Arc::clone(&count)),
        );
        let manager = Arc::new(TokenManager::new(
            client(&server),
            Store,
            clock.clone(),
            HangingRenewal(Arc::clone(&closed), Arc::clone(&notice), 2),
            Arc::new(builder.build()),
        ));
        runtime().block_on(async {
            manager.token().await.expect("initial");
            clock.0.store(1070, Ordering::SeqCst);
            let leader = tokio::spawn(issue_shared(Arc::clone(&manager)));
            tokio::time::timeout(std::time::Duration::from_secs(5), notice.notified())
                .await
                .expect("reconnect reached");
            let waiter = tokio::spawn(issue_shared(Arc::clone(&manager)));
            tokio::task::yield_now().await;
            leader.abort();
            assert!(leader.await.expect_err("cancelled").is_cancelled());
            assert_eq!(
                waiter
                    .await
                    .expect("waiter")
                    .expect_err("shared cancellation"),
                TokenError::Renewal
            );
            assert_eq!(manager.connection().await, BrokerConnection::Disconnected);
            assert_eq!(count.load(Ordering::SeqCst), 2);
            assert_eq!(closed.load(Ordering::SeqCst), 2);
            assert_eq!(
                manager
                    .token()
                    .await
                    .expect("recovery")
                    .secret()
                    .as_exposed_secret(),
                b"recovered-831"
            );
            assert_eq!(count.load(Ordering::SeqCst), 3);
        });
        server.finish().expect("capture");
    }
    #[test]
    fn equivalent_json_escapes_preserve_token_and_bearer_type() {
        for body in [
            r#"{"access_token":"\u0066ake-token-942","token_type":"Bearer","expires_in":120}"#,
            r#"{"access_token":"fake-token-942","token_type":"\u0042earer","expires_in":120}"#,
        ] {
            let decoded: broker_toss_schema::types::OAuth2TokenResponse =
                serde_json::from_str(body).expect("schema accepts equivalent JSON");
            assert_eq!(decoded.access_token, "fake-token-942");
            let server = ScriptedHttpServer::start(vec![response("200 OK", body)]).expect("server");
            let manager = manager(&server, FakeClock(Arc::new(AtomicI64::new(1000))));
            assert_eq!(
                runtime()
                    .block_on(manager.token())
                    .expect("decoded")
                    .secret()
                    .as_exposed_secret(),
                b"fake-token-942"
            );
            server.finish().expect("capture");
        }
    }
    async fn wait_until_held(gate: testkit::ResponseGate) -> testkit::ResponseGate {
        loop {
            match gate.observe() {
                testkit::GateObservation::Held => break gate,
                testkit::GateObservation::Waiting => tokio::task::yield_now().await,
                testkit::GateObservation::Closed => {
                    panic!("gate closed before observing the held response")
                }
            }
        }
    }
    async fn read_accounts<R: TokenRenewal>(
        manager: Arc<TokenManager<Store, FakeClock, R>>,
    ) -> Result<broker_toss::RawResponse, TokenError> {
        manager
            .get(&broker_toss::ReadRequest::new(
                broker_toss::ReadRoute::Accounts,
            ))
            .await
    }
    #[test]
    fn obsolete_revoked_retry_does_not_disconnect_a_newer_successful_token() {
        let revoked = response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#);
        let (server, gate) = ScriptedHttpServer::start_holding(
            vec![
                issued("one-913"),
                revoked.clone(),
                issued("two-417"),
                revoked,
                issued("three-831"),
            ],
            testkit::HoldPosition::new(3),
        )
        .expect("server");
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        let manager = Arc::new(manager(&server, clock.clone()));
        runtime().block_on(async {
            let lookup = tokio::spawn(read_accounts(Arc::clone(&manager)));
            let observed_gate =
                tokio::time::timeout(std::time::Duration::from_secs(5), wait_until_held(gate))
                    .await
                    .expect("retry held");
            clock.0.store(1060, Ordering::SeqCst);
            assert_eq!(
                manager
                    .token()
                    .await
                    .expect("planned renewal")
                    .secret()
                    .as_exposed_secret(),
                b"three-831"
            );
            observed_gate.release().expect("release old response");
            assert_eq!(
                lookup
                    .await
                    .expect("join")
                    .expect_err("request remains closed"),
                TokenError::Superseded
            );
            assert_eq!(manager.connection().await, BrokerConnection::Connected);
            assert_eq!(
                manager
                    .token()
                    .await
                    .expect("latest remains usable")
                    .secret()
                    .as_exposed_secret(),
                b"three-831"
            );
        });
        assert_eq!(
            String::from_utf8(server.finish().expect("capture"))
                .expect("utf8")
                .matches("/oauth2/token HTTP/1.1")
                .count(),
            3
        );
    }
    #[test]
    fn connection_coordinator_can_recover_duplicate_key_without_restarting_manager() {
        let revoked = response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#);
        let server = ScriptedHttpServer::start(vec![
            issued("old-913"),
            revoked.clone(),
            issued("retry-417"),
            revoked,
            issued("recovered-831"),
        ])
        .expect("server");
        let manager = manager(&server, FakeClock(Arc::new(AtomicI64::new(1000))));
        runtime().block_on(async {
            assert_eq!(
                manager
                    .get(&broker_toss::ReadRequest::new(
                        broker_toss::ReadRoute::Accounts
                    ))
                    .await
                    .expect_err("duplicate"),
                TokenError::RevokedRepeated
            );
            assert_eq!(manager.connection().await, BrokerConnection::DuplicateKey);
            assert_eq!(
                manager.token().await.expect_err("normal calls stay closed"),
                TokenError::RevokedRepeated
            );
            assert_eq!(
                manager
                    .retry_connection()
                    .await
                    .expect("coordinator retry")
                    .secret()
                    .as_exposed_secret(),
                b"recovered-831"
            );
            assert_eq!(manager.connection().await, BrokerConnection::Connected);
            assert_eq!(
                manager
                    .retry_connection()
                    .await
                    .expect("already connected")
                    .secret()
                    .as_exposed_secret(),
                b"recovered-831"
            );
        });
        assert_eq!(
            String::from_utf8(server.finish().expect("capture"))
                .expect("utf8")
                .matches("/oauth2/token HTTP/1.1")
                .count(),
            3
        );
    }
    fn advancing_manager(
        server: &ScriptedHttpServer,
        target: AdvanceTarget,
    ) -> Arc<TokenManager<Store, FakeClock, AdvancingRenewal>> {
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        Arc::new(TokenManager::new(
            client(server),
            Store,
            clock.clone(),
            AdvancingRenewal(clock, target),
            Arc::new(EventBus::default()),
        ))
    }
    #[test]
    fn one_second_before_expiry_can_still_be_shared_by_the_waiting_generation() {
        let server = ScriptedHttpServer::start(vec![issued("last-second-937")]).expect("server");
        let manager = advancing_manager(&server, AdvanceTarget::BeforeExpiry);
        runtime().block_on(async {
            let mut tasks = Vec::new();
            for _ in 0_u32..8 {
                tasks.push(tokio::spawn(issue_shared(Arc::clone(&manager))));
            }
            for task in tasks {
                assert_eq!(
                    task.await
                        .expect("join")
                        .expect("still valid")
                        .secret()
                        .as_exposed_secret(),
                    b"last-second-937"
                );
            }
        });
        assert_eq!(
            String::from_utf8(server.finish().expect("capture"))
                .expect("utf8")
                .matches("/oauth2/token HTTP/1.1")
                .count(),
            1
        );
    }
    async fn shared_expiry_failure(manager: Arc<TokenManager<Store, FakeClock, AdvancingRenewal>>) {
        let mut tasks = Vec::new();
        for _ in 0_u32..8 {
            tasks.push(tokio::spawn(issue_shared(Arc::clone(&manager))));
        }
        for task in tasks {
            assert_eq!(
                task.await.expect("join").expect_err("expired response"),
                TokenError::InvalidResponse
            );
        }
        assert_eq!(manager.connection().await, BrokerConnection::Disconnected);
    }
    #[test]
    fn expiry_instant_and_after_expiry_share_failure_and_never_return_expired_token() {
        for target in [AdvanceTarget::ExpiresAt, AdvanceTarget::AfterExpiry] {
            let server = ScriptedHttpServer::start(vec![issued("expired-937")]).expect("server");
            runtime().block_on(shared_expiry_failure(advancing_manager(&server, target)));
            assert_eq!(
                String::from_utf8(server.finish().expect("capture"))
                    .expect("utf8")
                    .matches("/oauth2/token HTTP/1.1")
                    .count(),
                1
            );
        }
    }
    struct EventKinds(Arc<std::sync::RwLock<Vec<domain::DomainEventKind>>>);
    fn record_kind(
        kinds: &std::sync::RwLock<Vec<domain::DomainEventKind>>,
        kind: domain::DomainEventKind,
    ) {
        kinds.write().expect("lock").push(kind);
    }
    impl application::EventSubscriber for EventKinds {
        fn on_event(
            &self,
            event: &domain::DomainEvent,
        ) -> Result<(), application::SubscriberFailure> {
            record_kind(&self.0, event.kind());
            Ok(())
        }
    }
    #[test]
    fn connection_changes_use_the_same_bus_as_other_application_publishers() {
        let server = ScriptedHttpServer::start(vec![issued("shared-bus-937")]).expect("server");
        let kinds = Arc::new(std::sync::RwLock::new(Vec::new()));
        let mut builder = application::EventBusBuilder::default();
        builder.subscribe(
            std::collections::BTreeSet::from([
                domain::DomainEventKind::BrokerConnectionChanged,
                domain::DomainEventKind::SettingsChanged,
            ]),
            EventKinds(Arc::clone(&kinds)),
        );
        let bus = Arc::new(builder.build());
        let manager = TokenManager::new(
            client(&server),
            Store,
            FakeClock(Arc::new(AtomicI64::new(1000))),
            Renewal,
            Arc::clone(&bus),
        );
        runtime().block_on(manager.token()).expect("token");
        let report = bus.publish(domain::DomainEvent::SettingsChanged);
        assert_eq!(report.failures, Vec::new());
        assert_eq!(
            *kinds.read().expect("lock"),
            [
                domain::DomainEventKind::BrokerConnectionChanged,
                domain::DomainEventKind::SettingsChanged
            ]
        );
        server.finish().expect("capture");
    }
    #[test]
    fn invalid_token_also_reissues_once_as_required_by_t04_handoff() {
        let invalid = response("401 Unauthorized", r#"{"error":{"code":"invalid-token"}}"#);
        let server = ScriptedHttpServer::start(vec![
            issued("one-913"),
            invalid,
            issued("two-417"),
            response("200 OK", "{}"),
        ])
        .expect("server");
        let manager = manager(&server, FakeClock(Arc::new(AtomicI64::new(1000))));
        let result = runtime()
            .block_on(manager.get(&broker_toss::ReadRequest::new(
                broker_toss::ReadRoute::Accounts,
            )))
            .expect("recovered");
        assert_eq!(result.status(), 200);
        assert_eq!(
            String::from_utf8(server.finish().expect("capture"))
                .expect("utf8")
                .matches("/oauth2/token HTTP/1.1")
                .count(),
            2
        );
    }
    #[test]
    fn repeated_invalid_token_closes_request_without_reporting_duplicate_key() {
        let invalid = response("401 Unauthorized", r#"{"error":{"code":"invalid-token"}}"#);
        let server = ScriptedHttpServer::start(vec![
            issued("one-913"),
            invalid.clone(),
            issued("two-417"),
            invalid,
        ])
        .expect("server");
        let manager = manager(&server, FakeClock(Arc::new(AtomicI64::new(1000))));
        runtime().block_on(async {
            assert_eq!(
                manager
                    .get(&broker_toss::ReadRequest::new(
                        broker_toss::ReadRoute::Accounts
                    ))
                    .await
                    .expect_err("closed"),
                TokenError::Rejected(application::PortErrorKind::InvalidRequest(
                    application::RequestRejection::InvalidToken
                ))
            );
            assert_eq!(manager.connection().await, BrokerConnection::Disconnected);
        });
        assert_eq!(
            String::from_utf8(server.finish().expect("capture"))
                .expect("utf8")
                .matches("/oauth2/token HTTP/1.1")
                .count(),
            2
        );
    }
    struct CountingRenewal(Arc<std::sync::atomic::AtomicUsize>);
    impl TokenRenewal for CountingRenewal {
        fn close(&self) -> impl Future<Output = Result<(), TokenError>> + Send {
            self.0.fetch_add(1, Ordering::SeqCst);
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
    async fn planned_reuse_run(
        outcome: Vec<Vec<u8>>,
        expected: Result<(), TokenError>,
        expected_connection: BrokerConnection,
        expected_issues: usize,
    ) {
        let revoked = response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#);
        let mut replies = vec![issued("old-319"), revoked, issued("planned-751")];
        replies.extend(outcome);
        let (server, gate) =
            ScriptedHttpServer::start_holding(replies, testkit::HoldPosition::new(1))
                .expect("server");
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        let issues = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let manager = Arc::new(TokenManager::new(
            client(&server),
            Store,
            clock.clone(),
            CountingRenewal(Arc::clone(&issues)),
            Arc::new(EventBus::default()),
        ));
        let lookup = tokio::spawn(read_accounts(Arc::clone(&manager)));
        let observed_gate =
            tokio::time::timeout(std::time::Duration::from_secs(5), wait_until_held(gate))
                .await
                .expect("first lookup held");
        clock.0.store(1060, Ordering::SeqCst);
        assert_eq!(
            manager
                .token()
                .await
                .expect("planned renewal")
                .secret()
                .as_exposed_secret(),
            b"planned-751"
        );
        observed_gate.release().expect("release old lookup");
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), lookup)
            .await
            .expect("bounded recovery")
            .expect("join");
        match expected {
            Ok(()) => assert_eq!(result.expect("recovered").status(), 200),
            Err(error) => assert_eq!(result.expect_err("closed"), error),
        }
        assert_eq!(manager.connection().await, expected_connection);
        assert_eq!(issues.load(Ordering::SeqCst), expected_issues);
        let captured = String::from_utf8(server.finish().expect("capture")).expect("utf8");
        assert_eq!(
            captured.matches("/oauth2/token HTTP/1.1").count(),
            expected_issues
        );
        assert_eq!(captured.matches("Bearer old-319").count(), 1);
        assert_eq!(captured.matches("Bearer planned-751").count(), 1);
    }
    #[test]
    fn rejected_old_token_reuses_planned_token_without_another_issue() {
        runtime().block_on(planned_reuse_run(
            vec![response("200 OK", "{}")],
            Ok(()),
            BrokerConnection::Connected,
            2,
        ));
    }
    #[test]
    fn revoked_shared_planned_token_preserves_one_own_reissue() {
        runtime().block_on(planned_reuse_run(
            vec![
                response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#),
                issued("own-863"),
                response("200 OK", "{}"),
            ],
            Ok(()),
            BrokerConnection::Connected,
            3,
        ));
    }
    #[test]
    fn revoked_after_shared_token_and_own_reissue_closes_without_second_own_issue() {
        runtime().block_on(planned_reuse_run(
            vec![
                response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#),
                issued("own-863"),
                response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#),
            ],
            Err(TokenError::RevokedRepeated),
            BrokerConnection::DuplicateKey,
            3,
        ));
    }
    async fn observe_issuance_at(received_at: i64, expected: Result<(), TokenError>) {
        let (server, gate) = ScriptedHttpServer::start_holding(
            vec![issued("sequence-37")],
            testkit::HoldPosition::new(0),
        )
        .expect("server");
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        let order = Arc::new(std::sync::RwLock::new(Vec::new()));
        let manager = Arc::new(TokenManager::new(
            client(&server),
            Store,
            clock.clone(),
            OrderedRenewal(Arc::clone(&order)),
            Arc::new(EventBus::default()),
        ));
        let issuing = tokio::spawn(issue_shared(Arc::clone(&manager)));
        let observed_gate =
            tokio::time::timeout(std::time::Duration::from_secs(5), wait_until_held(gate))
                .await
                .expect("issuance response held");
        clock.0.store(received_at, Ordering::SeqCst);
        observed_gate.release().expect("release response");
        let result = issuing.await.expect("join");
        match expected {
            Ok(()) => {
                result.expect("still valid");
                assert_eq!(
                    *order.read().expect("order"),
                    ["close", "reconnect", "resubscribe", "orders"]
                );
                assert_eq!(manager.connection().await, BrokerConnection::Connected);
            }
            Err(error) => {
                assert_eq!(result.expect_err("already expired"), error);
                assert!(order.read().expect("order").is_empty());
                assert_eq!(manager.connection().await, BrokerConnection::Disconnected);
            }
        }
        server.finish().expect("capture");
    }
    #[test]
    fn expired_issuance_response_never_starts_renewal_callbacks() {
        for (received_at, expected) in [
            (1119, Ok(())),
            (1120, Err(TokenError::InvalidResponse)),
            (1121, Err(TokenError::InvalidResponse)),
        ] {
            runtime().block_on(observe_issuance_at(received_at, expected));
        }
    }
    #[test]
    fn repeated_invalid_token_discards_the_rejected_cached_token() {
        let invalid = response("401 Unauthorized", r#"{"error":{"code":"invalid-token"}}"#);
        let server = ScriptedHttpServer::start(vec![
            issued("invalid-one-319"),
            invalid.clone(),
            issued("invalid-two-751"),
            invalid,
            issued("recovered-863"),
        ])
        .expect("server");
        let manager = manager(&server, FakeClock(Arc::new(AtomicI64::new(1000))));
        runtime().block_on(async {
            assert_eq!(
                manager
                    .get(&broker_toss::ReadRequest::new(
                        broker_toss::ReadRoute::Accounts
                    ))
                    .await
                    .expect_err("closed"),
                TokenError::Rejected(application::PortErrorKind::InvalidRequest(
                    application::RequestRejection::InvalidToken
                ))
            );
            assert_eq!(manager.connection().await, BrokerConnection::Disconnected);
            assert_eq!(
                manager
                    .token()
                    .await
                    .expect("fresh token")
                    .secret()
                    .as_exposed_secret(),
                b"recovered-863"
            );
            assert_eq!(manager.connection().await, BrokerConnection::Connected);
        });
        let captured = String::from_utf8(server.finish().expect("capture")).expect("utf8");
        assert_eq!(captured.matches("/oauth2/token HTTP/1.1").count(), 3);
    }
    #[test]
    fn late_invalid_retry_after_another_failure_reports_failure_instead_of_superseded() {
        let invalid = response("401 Unauthorized", r#"{"error":{"code":"invalid-token"}}"#);
        let (server, gate) = ScriptedHttpServer::start_holding(
            vec![
                issued("one-319"),
                invalid.clone(),
                issued("two-751"),
                invalid.clone(),
                issued("three-863"),
                invalid.clone(),
                issued("four-947"),
                invalid,
            ],
            testkit::HoldPosition::new(3),
        )
        .expect("server");
        let clock = FakeClock(Arc::new(AtomicI64::new(1000)));
        let manager = Arc::new(manager(&server, clock.clone()));
        runtime().block_on(async {
            let earlier = tokio::spawn(read_accounts(Arc::clone(&manager)));
            let observed_gate =
                tokio::time::timeout(std::time::Duration::from_secs(5), wait_until_held(gate))
                    .await
                    .expect("old retry held");
            clock.0.store(1060, Ordering::SeqCst);
            let error = TokenError::Rejected(application::PortErrorKind::InvalidRequest(
                application::RequestRejection::InvalidToken,
            ));
            assert_eq!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    manager.get(&broker_toss::ReadRequest::new(
                        broker_toss::ReadRoute::Quotes
                    )),
                )
                .await
                .expect("newer lookup remains bounded")
                .expect_err("new request fails"),
                error
            );
            assert_eq!(manager.connection().await, BrokerConnection::Disconnected);
            observed_gate.release().expect("release older failure");
            assert_eq!(
                earlier.await.expect("join").expect_err("same closed state"),
                error
            );
            assert_eq!(manager.connection().await, BrokerConnection::Disconnected);
        });
        let captured = String::from_utf8(server.finish().expect("capture")).expect("utf8");
        assert_eq!(captured.matches("/oauth2/token HTTP/1.1").count(), 4);
    }
    async fn assert_duplicate_stop<R: TokenRenewal>(
        manager: &TokenManager<Store, FakeClock, R>,
        count: &std::sync::atomic::AtomicUsize,
    ) {
        assert_eq!(manager.connection().await, BrokerConnection::DuplicateKey);
        assert_eq!(
            manager
                .token()
                .await
                .expect_err("ordinary issuance stays blocked"),
            TokenError::RevokedRepeated
        );
        assert_eq!(
            manager
                .get(&broker_toss::ReadRequest::new(
                    broker_toss::ReadRoute::Accounts
                ))
                .await
                .expect_err("ordinary read stays blocked"),
            TokenError::RevokedRepeated
        );
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn failed_explicit_recovery_keeps_duplicate_key_stop_until_recovery_succeeds() {
        let revoked = response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#);
        let server = ScriptedHttpServer::start(vec![
            issued("old-319"),
            revoked.clone(),
            issued("retry-751"),
            revoked,
            response("503 Busy", "{}"),
            issued("recovered-863"),
        ])
        .expect("server");
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut builder = application::EventBusBuilder::default();
        builder.subscribe(
            std::collections::BTreeSet::from([domain::DomainEventKind::BrokerConnectionChanged]),
            Counter(Arc::clone(&count)),
        );
        let manager = TokenManager::new(
            client(&server),
            Store,
            FakeClock(Arc::new(AtomicI64::new(1000))),
            Renewal,
            Arc::new(builder.build()),
        );
        runtime().block_on(async {
            assert_eq!(
                manager
                    .get(&broker_toss::ReadRequest::new(
                        broker_toss::ReadRoute::Accounts
                    ))
                    .await
                    .expect_err("duplicate key"),
                TokenError::RevokedRepeated
            );
            assert_eq!(manager.connection().await, BrokerConnection::DuplicateKey);
            assert_eq!(count.load(Ordering::SeqCst), 2);
            manager
                .retry_connection()
                .await
                .expect_err("failed recovery");
            assert_duplicate_stop(&manager, &count).await;
            assert_eq!(
                manager
                    .retry_connection()
                    .await
                    .expect("explicit recovery succeeds")
                    .secret()
                    .as_exposed_secret(),
                b"recovered-863"
            );
            assert_eq!(manager.connection().await, BrokerConnection::Connected);
            assert_eq!(count.load(Ordering::SeqCst), 3);
        });
        let captured = String::from_utf8(server.finish().expect("capture")).expect("utf8");
        assert_eq!(captured.matches("/oauth2/token HTTP/1.1").count(), 4);
    }
    async fn recover_shared<R: TokenRenewal>(
        manager: Arc<TokenManager<Store, FakeClock, R>>,
    ) -> Result<broker_toss::TokenLease, TokenError> {
        manager.retry_connection().await
    }
    #[test]
    fn cancelled_explicit_recovery_keeps_duplicate_key_stop_until_recovery_succeeds() {
        let revoked = response("401 Unauthorized", r#"{"error":{"code":"token-revoked"}}"#);
        let server = ScriptedHttpServer::start(vec![
            issued("old-319"),
            revoked.clone(),
            issued("retry-751"),
            revoked,
            issued("cancelled-863"),
            issued("recovered-947"),
        ])
        .expect("server");
        let closed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let notice = Arc::new(tokio::sync::Notify::new());
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut builder = application::EventBusBuilder::default();
        builder.subscribe(
            std::collections::BTreeSet::from([domain::DomainEventKind::BrokerConnectionChanged]),
            Counter(Arc::clone(&count)),
        );
        let manager = Arc::new(TokenManager::new(
            client(&server),
            Store,
            FakeClock(Arc::new(AtomicI64::new(1000))),
            HangingRenewal(Arc::clone(&closed), Arc::clone(&notice), 3),
            Arc::new(builder.build()),
        ));
        runtime().block_on(async {
            assert_eq!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    read_accounts(Arc::clone(&manager)),
                )
                .await
                .expect("duplicate key lookup remains bounded")
                .expect_err("duplicate key"),
                TokenError::RevokedRepeated
            );
            assert_eq!(manager.connection().await, BrokerConnection::DuplicateKey);
            let recovery = tokio::spawn(recover_shared(Arc::clone(&manager)));
            tokio::time::timeout(std::time::Duration::from_secs(5), notice.notified())
                .await
                .expect("recovery callback reached");
            recovery.abort();
            assert!(recovery.await.expect_err("cancelled").is_cancelled());
            assert_duplicate_stop(&manager, &count).await;
            assert_eq!(
                manager
                    .retry_connection()
                    .await
                    .expect("explicit recovery succeeds")
                    .secret()
                    .as_exposed_secret(),
                b"recovered-947"
            );
            assert_eq!(manager.connection().await, BrokerConnection::Connected);
            assert_eq!(count.load(Ordering::SeqCst), 3);
            assert_eq!(closed.load(Ordering::SeqCst), 4);
        });
        let captured = String::from_utf8(server.finish().expect("capture")).expect("utf8");
        assert_eq!(captured.matches("/oauth2/token HTTP/1.1").count(), 4);
    }
    struct SharedTime(Arc<crate::support::FakeTime>);
    impl Clock for SharedTime {
        fn now(&self) -> Timestamp {
            self.0.now()
        }
    }
    fn integrated_manager(
        server: &ScriptedHttpServer,
        time: Arc<crate::support::FakeTime>,
    ) -> TokenManager<Store, SharedTime, Renewal> {
        TokenManager::new(
            TossHttpClient::new(
                &TossEndpoint::local(server.port()).expect("endpoint"),
                Arc::<crate::support::FakeTime>::clone(&time),
                Arc::<crate::support::FakeTime>::clone(&time),
            )
            .expect("client"),
            Store,
            SharedTime(time),
            Renewal,
            Arc::new(EventBus::default()),
        )
    }
    #[test]
    fn managed_get_keeps_rate_budget_across_token_reissue() {
        let server = ScriptedHttpServer::start(vec![
            issued("old-731"),
            response("401 Unauthorized", r#"{"error":{"code":"invalid-token"}}"#),
            issued("new-419"),
            response("200 OK", "{}"),
            response("200 OK", "{}"),
        ])
        .expect("server");
        let time = crate::support::time();
        let manager = integrated_manager(&server, Arc::clone(&time));
        let request = broker_toss::ReadRequest::new(broker_toss::ReadRoute::Accounts);
        runtime().block_on(async {
            assert_eq!(
                manager.get(&request).await.expect("recovered").status(),
                200
            );
            assert_eq!(
                manager.get(&request).await.expect("shared budget").status(),
                200
            );
        });
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_003);
        assert_eq!(*time.now().nanosecond().as_u32(), 333_333_334);
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("POST /oauth2/token HTTP/1.1").count(), 2);
        assert_eq!(requests.matches("GET /api/v1/accounts HTTP/1.1").count(), 3);
        assert_eq!(requests.matches("Bearer old-731").count(), 1);
        assert_eq!(requests.matches("Bearer new-419").count(), 2);
    }
    #[test]
    fn managed_get_honors_retry_after_without_reissuing_token() {
        let server = ScriptedHttpServer::start(vec![
            issued("quota-913"),
            b"HTTP/1.1 429 Limited\r\nContent-Length: 0\r\nRetry-After: 7\r\nConnection: close\r\n\r\n".to_vec(),
            response("200 OK", "{}"),
        ])
        .expect("server");
        let time = crate::support::time();
        let manager = integrated_manager(&server, Arc::clone(&time));
        let result = runtime()
            .block_on(manager.get(&broker_toss::ReadRequest::new(
                broker_toss::ReadRoute::Accounts,
            )))
            .expect("retry");
        assert_eq!(result.status(), 200);
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_008);
        assert_eq!(*time.now().nanosecond().as_u32(), 0);
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("POST /oauth2/token HTTP/1.1").count(), 1);
        assert_eq!(requests.matches("GET /api/v1/accounts HTTP/1.1").count(), 2);
    }
    #[test]
    fn managed_token_post_is_not_retried_on_429() {
        let server =
            ScriptedHttpServer::start(vec![response("429 Limited", "{}")]).expect("server");
        let time = crate::support::time();
        let manager = integrated_manager(&server, Arc::clone(&time));
        runtime()
            .block_on(manager.token())
            .expect_err("one failed issuance");
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_000);
        assert_eq!(*time.now().nanosecond().as_u32(), 0);
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("POST /oauth2/token HTTP/1.1").count(), 1);
    }
}
