#[cfg(test)]
mod tests {
    use application::{
        Clock, EventBus, EventBusBuilder, EventSubscriber, SecretKey, SecretStore, SecretValue,
        SubscriberFailure,
    };
    use broker_toss::{
        BrokerConnection, MAX_TOPICS_PER_CONNECTION, RequestWait, TokenManager, TossEndpoint,
        TossHttpClient, TossWsClient, TossWsConnector, TossWsDeps, TossWsEndpoint,
        WsAccountSlotPool, WsClientError, WsConfig, WsDisconnectCause, WsDispatchedEvent,
        WsOrderEventKind, WsProtocolError, WsRejectedCode, WsServerEvent, WsSubscriptionSet,
        WsTopic,
    };
    use domain::{
        AccountId, CommonShare, Currency, DisplayInstrumentName, DomainEvent, DomainEventKind,
        InstrumentId, KnownMarket, KoreanMasterDetail, LiquidationTrading, ListingStatus,
        MasterCode, MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow,
        MasterSecurityType, Nanosecond, NxtTrading, OffsetSeconds, Symbol, Timestamp,
        TradingSuspension, UnixSeconds,
    };
    use std::collections::BTreeSet;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;
    use testkit::{FakeWsServer, FakeWsSessionPlan, FakeWsStep, WsUpgradeReply, serve_sequence};
    use testkit_ports::memory::MemoryReply;
    use testkit_ports::system_fakes::MemorySecretStore;
    use tokio::sync::mpsc;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum WaitMode {
        AutoAdvance,
        RealBriefPause,
    }

    struct SteppingClock {
        nanos: AtomicU64,
        waits: mpsc::UnboundedSender<Duration>,
        mode: WaitMode,
    }

    impl SteppingClock {
        fn new(mode: WaitMode) -> (Arc<Self>, mpsc::UnboundedReceiver<Duration>) {
            let (tx, rx) = mpsc::unbounded_channel();
            (
                Arc::new(Self {
                    nanos: AtomicU64::new(0),
                    waits: tx,
                    mode,
                }),
                rx,
            )
        }

        fn advance(&self, duration: Duration) {
            let add = u64::try_from(duration.as_nanos()).expect("nanos");
            self.nanos.fetch_add(add, Ordering::SeqCst);
        }
    }

    impl Clock for SteppingClock {
        fn now(&self) -> Timestamp {
            let total = self.nanos.load(Ordering::SeqCst);
            let secs = i64::try_from(total.checked_div(1_000_000_000).expect("div")).expect("secs");
            let sub = u32::try_from(total.checked_rem(1_000_000_000).expect("rem")).expect("sub");
            Timestamp::from_parts(
                UnixSeconds::new(1_800_000_000_i64.saturating_add(secs)),
                Nanosecond::new(sub),
                OffsetSeconds::new(0),
            )
            .expect("timestamp")
        }
    }

    struct SharedClock(Arc<SteppingClock>);

    impl Clock for SharedClock {
        fn now(&self) -> Timestamp {
            self.0.now()
        }
    }

    impl RequestWait for SteppingClock {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            if duration >= Duration::from_secs(600) {
                return Box::pin(std::future::pending());
            }
            let _sent = self.waits.send(duration);
            match self.mode {
                WaitMode::AutoAdvance => {
                    self.advance(duration);
                    Box::pin(std::future::ready(()))
                }
                WaitMode::RealBriefPause => Box::pin(tokio::time::sleep(Duration::from_millis(5))),
            }
        }
    }

    struct EventCollector {
        tx: mpsc::UnboundedSender<DomainEvent>,
    }

    impl EventSubscriber for EventCollector {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            let _sent = self.tx.send(event.clone());
            Ok(())
        }
    }

    fn event_bus_with_collector() -> (Arc<EventBus>, mpsc::UnboundedReceiver<DomainEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut builder = EventBusBuilder::default();
        let kinds = BTreeSet::from([
            DomainEventKind::PriceUpdated,
            DomainEventKind::OrderFilled,
            DomainEventKind::OrderCancelled,
            DomainEventKind::OrderModified,
            DomainEventKind::BrokerConnectionChanged,
        ]);
        builder.subscribe(kinds, EventCollector { tx });
        (Arc::new(builder.build()), rx)
    }

    fn sample_master() -> Arc<MasterInstrumentFacts> {
        let observed = Timestamp::from_parts(
            UnixSeconds::new(1_800_000_000_i64),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("ts");
        let samsung = MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("kospi"),
            symbol: Symbol::try_new("005930").expect("kr symbol"),
            instrument_id: InstrumentId::try_new("kr-005930").expect("id"),
            name: DisplayInstrumentName::try_new("Samsung Electronics").expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("stock"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Supported(TradingSuspension::Clear),
            }),
            observed_at: observed,
        })
        .expect("samsung");
        let apple = MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("nasdaq"),
            symbol: Symbol::try_new("AAPL").expect("us symbol"),
            instrument_id: InstrumentId::try_new("us-AAPL").expect("id"),
            name: DisplayInstrumentName::try_new("Apple Inc").expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("FOREIGN_STOCK").expect("stock"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: observed,
        })
        .expect("apple");
        Arc::new(MasterInstrumentFacts::try_new(vec![samsung, apple]).expect("master facts"))
    }

    fn instrument_id_for(master: &MasterInstrumentFacts, code: &str) -> InstrumentId {
        master
            .instruments()
            .iter()
            .find(|row| row.row().symbol.as_str() == code)
            .expect("instrument")
            .row()
            .instrument_id
            .clone()
    }

    fn http_json(status: u16, reason: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn orders_envelope(rows: &[serde_json::Value]) -> String {
        serde_json::json!({
            "result": {
                "orders": rows,
                "hasNext": false,
                "nextCursor": null
            },
            "error": null
        })
        .to_string()
    }

    struct OrderFixture<'a> {
        order_id: &'a str,
        status: &'a str,
        price: &'a str,
        qty: &'a str,
        filled_qty: &'a str,
        filled_at: Option<&'a str>,
        canceled_at: Option<&'a str>,
    }

    fn kr_order_json(fixture: &OrderFixture<'_>) -> serde_json::Value {
        let avg_price = if fixture.filled_qty == "0" {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(fixture.price.to_owned())
        };
        let filled_amount = if fixture.filled_qty == "0" {
            serde_json::Value::Null
        } else {
            serde_json::Value::String("725000".to_owned())
        };
        serde_json::json!({
            "orderId": fixture.order_id,
            "symbol": "005930",
            "side": "BUY",
            "orderType": "LIMIT",
            "timeInForce": "DAY",
            "status": fixture.status,
            "price": fixture.price,
            "quantity": fixture.qty,
            "orderAmount": null,
            "currency": "KRW",
            "orderedAt": "2027-01-15T09:00:00+09:00",
            "canceledAt": fixture.canceled_at,
            "execution": {
                "filledQuantity": fixture.filled_qty,
                "averageFilledPrice": avg_price,
                "filledAmount": filled_amount,
                "commission": null,
                "tax": null,
                "filledAt": fixture.filled_at,
                "settlementDate": null
            }
        })
    }

    #[derive(Clone, Copy)]
    struct Ports {
        ws_port: u16,
        http_port: u16,
    }

    fn make_client(
        ports: Ports,
        clock: Arc<SteppingClock>,
        bus: Arc<EventBus>,
        pool: Arc<WsAccountSlotPool>,
        config: WsConfig,
    ) -> TossWsClient {
        let ws_ep = TossWsEndpoint::local(ports.ws_port).expect("ws endpoint");
        let http_ep = TossEndpoint::local(ports.http_port).expect("http endpoint");
        let connector = TossWsConnector::new(&ws_ep).expect("connector");
        let http = TossHttpClient::new(
            &http_ep,
            Arc::<SteppingClock>::clone(&clock),
            Arc::<SteppingClock>::clone(&clock),
        )
        .expect("http");
        TossWsClient::new(
            TossWsDeps {
                connector,
                http,
                clock: Arc::<SteppingClock>::clone(&clock),
                wait: clock,
                master: sample_master(),
                bus,
                slot_pool: pool,
            },
            config,
        )
    }

    fn quiet_config() -> WsConfig {
        WsConfig {
            keepalive_interval: Duration::from_secs(3600),
            ..WsConfig::default()
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    fn reject_plan(status: u16, reason: &'static str, body: &str) -> FakeWsSessionPlan {
        FakeWsSessionPlan {
            upgrade: WsUpgradeReply::Reject {
                status,
                reason,
                body: body.to_owned(),
            },
            steps: Vec::new(),
        }
    }

    fn drain_waits(waits: &mut mpsc::UnboundedReceiver<Duration>) -> Vec<Duration> {
        let mut collected = Vec::new();
        while let Ok(dur) = waits.try_recv() {
            collected.push(dur);
        }
        collected
    }

    fn drain_events(events: &mut mpsc::UnboundedReceiver<DomainEvent>) -> Vec<DomainEvent> {
        let mut collected = Vec::new();
        while let Ok(evt) = events.try_recv() {
            collected.push(evt);
        }
        collected
    }

    #[test]
    fn t17_upgrade_auth_headers_and_401_403_503_rejections() {
        runtime().block_on(async {
            let server = FakeWsServer::start(vec![
                reject_plan(401, "Unauthorized", r#"{"error":"unauthorized"}"#),
                reject_plan(403, "Forbidden", r#"{"error":"ip-not-allowed"}"#),
                reject_plan(503, "Service Unavailable", r#"{"error":"maintenance"}"#),
                FakeWsSessionPlan {
                    upgrade: WsUpgradeReply::Accept,
                    steps: vec![FakeWsStep::WaitClientClose],
                },
            ])
            .expect("ws server");
            let http_stub = serve_sequence(Vec::new()).expect("http stub");
            let (clock, _waits) = SteppingClock::new(WaitMode::AutoAdvance);
            let (bus, _events) = event_bus_with_collector();
            let client = make_client(
                Ports {
                    ws_port: server.port(),
                    http_port: http_stub.port(),
                },
                clock,
                bus,
                Arc::new(WsAccountSlotPool::new(2)),
                quiet_config(),
            );
            let token = SecretValue::new(b"tok-alpha-1".to_vec());
            assert_eq!(
                client.connect(&token).await,
                Err(WsClientError::Unauthorized)
            );
            assert_eq!(client.connect(&token).await, Err(WsClientError::Forbidden));
            assert_eq!(
                client.connect(&token).await,
                Err(WsClientError::ServiceUnavailable)
            );
            client.connect(&token).await.expect("connected");
            assert!(client.is_connected().await);
            client.disconnect().await;
            let records = server.finish().expect("records");
            assert_eq!(records.len(), 4);
            for rec in &records {
                assert_eq!(rec.path, "/ws/v1");
                assert_eq!(rec.authorization.as_deref(), Some("Bearer tok-alpha-1"));
            }
            assert!(records.get(3).is_some_and(|rec| rec.client_closed));
        });
    }

    fn quote_test_server() -> FakeWsServer {
        let ack = serde_json::json!({
            "type": "subscriptions",
            "id": "decl-1",
            "subscribed": ["trade:kr:005930", "orderbook:us:AAPL"],
            "rejected": [{"topic": "trade:kr:000000", "code": "stock-not-found", "message": "missing"}]
        })
        .to_string();
        let trade_msg = serde_json::json!({
            "type": "message",
            "topic": "trade:kr:005930",
            "data": {"price": "72500", "volume": "5"}
        })
        .to_string();
        let book_msg = serde_json::json!({
            "type": "message",
            "topic": "orderbook:us:AAPL",
            "data": {"bids": [{"price": "192.50"}], "asks": [{"price": "192.55"}]}
        })
        .to_string();
        let rate_err = serde_json::json!({
            "type": "error",
            "id": "decl-1",
            "error": {"code": "rate-limit-exceeded", "message": "slow down"}
        })
        .to_string();
        FakeWsServer::start(vec![FakeWsSessionPlan {
            upgrade: WsUpgradeReply::Accept,
            steps: vec![
                FakeWsStep::ExpectText,
                FakeWsStep::SendText(ack),
                FakeWsStep::SendText(trade_msg),
                FakeWsStep::SendText(book_msg),
                FakeWsStep::SendText(rate_err),
                FakeWsStep::ExpectText,
                FakeWsStep::ExpectText,
                FakeWsStep::ExpectText,
                FakeWsStep::ExpectText,
                FakeWsStep::ExpectText,
                FakeWsStep::ExpectText,
                FakeWsStep::WaitClientClose,
            ],
        }])
        .expect("ws server")
    }

    fn assert_topic_cap_enforced() {
        let mut capped = WsSubscriptionSet::new();
        for idx in 0_usize..MAX_TOPICS_PER_CONNECTION {
            let sym = Symbol::try_new(&format!("{idx:06}")).expect("sym");
            capped
                .insert(WsTopic::trade(KnownMarket::Kr, &sym))
                .expect("insert");
        }
        let overflow = Symbol::try_new("999999").expect("sym");
        assert_eq!(
            capped.insert(WsTopic::trade(KnownMarket::Kr, &overflow)),
            Err(WsProtocolError::TooManyTopics)
        );
    }

    async fn verify_quote_stream(
        client: &TossWsClient,
        token: &SecretValue,
        master: &MasterInstrumentFacts,
        events: &mut mpsc::UnboundedReceiver<DomainEvent>,
    ) {
        let WsDispatchedEvent::SubscriptionsAck {
            rejected, pruned, ..
        } = client.next_event(token).await.expect("ack")
        else {
            panic!("expected ack");
        };
        assert_eq!(
            rejected.first().map(|item| &item.code),
            Some(&WsRejectedCode::StockNotFound)
        );
        assert_eq!(pruned.len(), 1);
        assert_eq!(client.desired_subscriptions().await.len(), 2);

        let samsung = instrument_id_for(master, "005930");
        let apple = instrument_id_for(master, "AAPL");
        let WsDispatchedEvent::QuoteUpdated {
            instrument_id: trade_id,
            event: WsServerEvent::Trade(_),
        } = client.next_event(token).await.expect("trade")
        else {
            panic!("expected trade");
        };
        assert_eq!(trade_id, Some(samsung.clone()));
        let WsDispatchedEvent::QuoteUpdated {
            instrument_id: book_id,
            event: WsServerEvent::Orderbook(_),
        } = client.next_event(token).await.expect("book")
        else {
            panic!("expected book");
        };
        assert_eq!(book_id, Some(apple.clone()));
        assert_eq!(
            events.recv().await,
            Some(DomainEvent::PriceUpdated(samsung))
        );
        assert_eq!(events.recv().await, Some(DomainEvent::PriceUpdated(apple)));
    }

    #[test]
    fn t17_quotes_rejected_pruning_rate_limit_and_topic_cap() {
        assert_topic_cap_enforced();
        runtime().block_on(async {
            let server = quote_test_server();
            let http_stub = serve_sequence(Vec::new()).expect("http");
            let (clock, mut waits) = SteppingClock::new(WaitMode::AutoAdvance);
            let (bus, mut events) = event_bus_with_collector();
            let master = sample_master();
            let client = make_client(
                Ports {
                    ws_port: server.port(),
                    http_port: http_stub.port(),
                },
                clock,
                bus,
                Arc::new(WsAccountSlotPool::new(2)),
                quiet_config(),
            );
            let token = SecretValue::new(b"tok-quotes".to_vec());
            client.connect(&token).await.expect("connect");

            let mut set = WsSubscriptionSet::new();
            for topic in [
                WsTopic::trade(KnownMarket::Kr, &Symbol::try_new("005930").expect("kr")),
                WsTopic::trade(KnownMarket::Kr, &Symbol::try_new("000000").expect("bad")),
                WsTopic::orderbook(KnownMarket::Us, &Symbol::try_new("AAPL").expect("us")),
            ] {
                set.insert(topic).expect("insert");
            }
            client.declare_subscriptions(set).await.expect("declare");
            verify_quote_stream(&client, &token, &master, &mut events).await;

            let WsDispatchedEvent::RateLimited { .. } =
                client.next_event(&token).await.expect("rate limit")
            else {
                panic!("expected rate limited");
            };
            assert!(drain_waits(&mut waits).contains(&Duration::from_secs(1)));

            let pruned_set = client.desired_subscriptions().await;
            for _step in 0_usize..6_usize {
                client
                    .declare_subscriptions(pruned_set.clone())
                    .await
                    .expect("rate-limited declare");
            }
            assert!(
                drain_waits(&mut waits)
                    .iter()
                    .any(|dur| *dur > Duration::ZERO && *dur <= Duration::from_secs(1))
            );

            client.disconnect().await;
            let records = server.finish().expect("records");
            let second_decl = records
                .first()
                .and_then(|rec| rec.received_texts.get(1))
                .expect("second decl");
            assert!(!second_decl.contains("000000"));
        });
    }

    fn keepalive_server() -> FakeWsServer {
        let trade_push = serde_json::json!({
            "type": "message",
            "topic": "trade:kr:005930",
            "data": {"price": "73000", "volume": "1"}
        })
        .to_string();
        let ack = serde_json::json!({
            "type": "subscriptions",
            "id": "decl-1",
            "subscribed": ["trade:kr:005930"],
            "rejected": []
        })
        .to_string();
        FakeWsServer::start(vec![FakeWsSessionPlan {
            upgrade: WsUpgradeReply::Accept,
            steps: vec![
                FakeWsStep::ExpectText,
                FakeWsStep::SendText(ack),
                FakeWsStep::SendPing(b"srv-ka".to_vec()),
                FakeWsStep::ExpectPong(b"srv-ka".to_vec()),
                FakeWsStep::SendText(trade_push.clone()),
                FakeWsStep::SendText(trade_push),
                FakeWsStep::ServeUntilClientClose {
                    ack_json: None,
                    push_frames: Vec::new(),
                    expected_pings: 2,
                },
            ],
        }])
        .expect("ws server")
    }

    async fn collect_keepalive_events(
        client: &TossWsClient,
        token: &SecretValue,
    ) -> (usize, usize) {
        let mut pong_count = 0_usize;
        let mut quote_count = 0_usize;
        for _step in 0_usize..10_usize {
            match client.next_event(token).await.expect("event") {
                WsDispatchedEvent::Pong => pong_count = pong_count.saturating_add(1),
                WsDispatchedEvent::QuoteUpdated { .. } => {
                    quote_count = quote_count.saturating_add(1);
                }
                WsDispatchedEvent::SubscriptionsAck { .. } => {}
                WsDispatchedEvent::Disconnected(_) => break,
                WsDispatchedEvent::OrderReconciled { .. }
                | WsDispatchedEvent::RateLimited { .. }
                | WsDispatchedEvent::ProtocolErrorFrame { .. }
                | WsDispatchedEvent::IgnoredTopic { .. } => panic!("unexpected event"),
            }
            if pong_count >= 2 && quote_count >= 2 {
                break;
            }
        }
        (pong_count, quote_count)
    }

    #[test]
    fn t17_keepalive_ping_during_active_stream_and_protocol_pong() {
        runtime().block_on(async {
            let server = keepalive_server();
            let http_stub = serve_sequence(Vec::new()).expect("http");
            let (clock, _waits) = SteppingClock::new(WaitMode::RealBriefPause);
            let (bus, _events) = event_bus_with_collector();
            let client = make_client(
                Ports {
                    ws_port: server.port(),
                    http_port: http_stub.port(),
                },
                clock,
                bus,
                Arc::new(WsAccountSlotPool::new(2)),
                WsConfig::default(),
            );
            let token = SecretValue::new(b"tok-ka".to_vec());
            client.connect(&token).await.expect("connect");

            let mut set = WsSubscriptionSet::new();
            set.insert(WsTopic::trade(
                KnownMarket::Kr,
                &Symbol::try_new("005930").expect("sym"),
            ))
            .expect("kr");
            client.declare_subscriptions(set).await.expect("declare");

            let (pong_count, quote_count) = collect_keepalive_events(&client, &token).await;
            assert!(pong_count >= 2 && quote_count >= 2);
            client.disconnect().await;
            let records = server.finish().expect("records");
            let first_rec = records.first().expect("record");
            assert_eq!(first_rec.received_pongs, vec![b"srv-ka".to_vec()]);
            let sent_pings = first_rec
                .received_texts
                .iter()
                .filter(|msg| msg.as_str() == "PING")
                .count();
            assert!(sent_pings >= 2);
        });
    }

    fn personal_order_http_responses() -> Vec<Vec<u8>> {
        let external_filled = kr_order_json(&OrderFixture {
            order_id: "ext-fill-1",
            status: "FILLED",
            price: "72500",
            qty: "10",
            filled_qty: "10",
            filled_at: Some("2027-01-15T09:05:00+09:00"),
            canceled_at: None,
        });
        let external_canceled = kr_order_json(&OrderFixture {
            order_id: "ext-cancel-2",
            status: "CANCELED",
            price: "72000",
            qty: "5",
            filled_qty: "0",
            filled_at: None,
            canceled_at: Some("2027-01-15T09:06:00+09:00"),
        });
        let open_before = kr_order_json(&OrderFixture {
            order_id: "ext-mod-3",
            status: "PENDING",
            price: "71000",
            qty: "4",
            filled_qty: "0",
            filled_at: None,
            canceled_at: None,
        });
        let open_after = kr_order_json(&OrderFixture {
            order_id: "ext-mod-3",
            status: "PENDING",
            price: "71500",
            qty: "4",
            filled_qty: "0",
            filled_at: None,
            canceled_at: None,
        });
        vec![
            http_json(200, "OK", &orders_envelope(&[open_before])),
            http_json(200, "OK", &orders_envelope(&[])),
            http_json(
                200,
                "OK",
                &orders_envelope(std::slice::from_ref(&open_after)),
            ),
            http_json(
                200,
                "OK",
                &orders_envelope(&[external_filled.clone(), external_canceled.clone()]),
            ),
            http_json(200, "OK", &orders_envelope(&[open_after])),
            http_json(
                200,
                "OK",
                &orders_envelope(&[external_filled, external_canceled]),
            ),
        ]
    }

    fn personal_order_ws_server() -> FakeWsServer {
        let event_one = serde_json::json!({
            "type": "message",
            "topic": "personal:order:3",
            "data": {"eventType": "FILL", "order": {"id": "ext-fill-1"}}
        })
        .to_string();
        let event_unknown = serde_json::json!({
            "type": "message",
            "topic": "personal:order:3",
            "data": {"eventType": "FUTURE_VENDOR_EVENT", "order": {"id": "ext-fill-1"}}
        })
        .to_string();
        FakeWsServer::start(vec![FakeWsSessionPlan {
            upgrade: WsUpgradeReply::Accept,
            steps: vec![
                FakeWsStep::ExpectText,
                FakeWsStep::SendText(event_one),
                FakeWsStep::SendText(event_unknown),
                FakeWsStep::WaitClientClose,
            ],
        }])
        .expect("ws")
    }

    #[test]
    fn t17_personal_order_and_external_orders_reconcile_to_domain_events() {
        runtime().block_on(async {
            let http_server = serve_sequence(personal_order_http_responses()).expect("http");
            let server = personal_order_ws_server();
            let (clock, _waits) = SteppingClock::new(WaitMode::AutoAdvance);
            let (bus, mut events) = event_bus_with_collector();
            let client = make_client(
                Ports {
                    ws_port: server.port(),
                    http_port: http_server.port(),
                },
                clock,
                bus,
                Arc::new(WsAccountSlotPool::new(2)),
                quiet_config(),
            );
            let token = SecretValue::new(b"tok-order".to_vec());
            let account = AccountId::new(3);
            client.connect(&token).await.expect("connect");

            let mut set = WsSubscriptionSet::new();
            set.insert(WsTopic::personal_order(&account))
                .expect("order topic");
            client.declare_subscriptions(set).await.expect("declare");

            let baseline = client
                .reconcile_orders_with_token(&token)
                .await
                .expect("baseline");
            assert_eq!(baseline.published_events, Vec::new());

            let WsDispatchedEvent::OrderReconciled {
                notice: first_notice,
                report: first_report,
            } = client.next_event(&token).await.expect("reconcile")
            else {
                panic!("expected order reconciled");
            };
            assert_eq!(first_notice.account_id, account);
            assert_eq!(first_notice.event_kind, WsOrderEventKind::Fill);
            let expected = vec![
                DomainEvent::OrderFilled(account.clone()),
                DomainEvent::OrderCancelled(account.clone()),
                DomainEvent::OrderModified(account),
            ];
            assert_eq!(first_report.published_events, expected);
            assert_eq!(drain_events(&mut events), expected);

            let WsDispatchedEvent::OrderReconciled {
                notice: second_notice,
                report: second_report,
            } = client.next_event(&token).await.expect("unknown kind")
            else {
                panic!("expected unknown reconciled");
            };
            assert_eq!(
                second_notice.event_kind,
                WsOrderEventKind::Unknown("FUTURE_VENDOR_EVENT".to_owned())
            );
            assert_eq!(second_report.published_events, Vec::new());

            client.disconnect().await;
            let _ws_records = server.finish().expect("ws finish");
            assert_eq!(http_server.finish().expect("http finish").len(), 6);
        });
    }

    fn reconnect_http_responses() -> Vec<Vec<u8>> {
        let filled_during_outage = kr_order_json(&OrderFixture {
            order_id: "outage-fill-9",
            status: "FILLED",
            price: "72500",
            qty: "10",
            filled_qty: "10",
            filled_at: Some("2027-01-15T09:10:00+09:00"),
            canceled_at: None,
        });
        vec![
            http_json(200, "OK", &orders_envelope(&[])),
            http_json(200, "OK", &orders_envelope(&[])),
            http_json(200, "OK", &orders_envelope(&[])),
            http_json(
                200,
                "OK",
                &orders_envelope(std::slice::from_ref(&filled_during_outage)),
            ),
            http_json(200, "OK", &orders_envelope(&[])),
            http_json(200, "OK", &orders_envelope(&[filled_during_outage])),
        ]
    }

    fn reconnect_ws_server() -> FakeWsServer {
        let shutdown_frame = serde_json::json!({
            "type": "error",
            "error": {"code": "server-shutdown", "message": "deploying"}
        })
        .to_string();
        FakeWsServer::start(vec![
            FakeWsSessionPlan {
                upgrade: WsUpgradeReply::Accept,
                steps: vec![
                    FakeWsStep::ExpectText,
                    FakeWsStep::SendText(shutdown_frame),
                    FakeWsStep::WaitClientClose,
                ],
            },
            FakeWsSessionPlan {
                upgrade: WsUpgradeReply::Accept,
                steps: vec![FakeWsStep::ExpectText, FakeWsStep::AbruptClose],
            },
            FakeWsSessionPlan {
                upgrade: WsUpgradeReply::Accept,
                steps: vec![FakeWsStep::ExpectText, FakeWsStep::WaitClientClose],
            },
        ])
        .expect("ws")
    }

    fn assert_reconnect_evidence(records: &[testkit::WsRecordedConnection], waits: &[Duration]) {
        assert_eq!(records.len(), 3);
        let rec1 = records.get(1).expect("rec1");
        let rec2 = records.get(2).expect("rec2");
        assert!(rec1.prior_closed_at_accept && rec2.prior_closed_at_accept);
        assert!(
            rec1.received_texts
                .first()
                .is_some_and(|txt| txt.contains("personal:order"))
        );
        assert!(
            rec2.received_texts
                .first()
                .is_some_and(|txt| txt.contains("personal:order"))
        );
        assert!(waits.contains(&Duration::from_secs(1)));
        assert!(waits.contains(&Duration::from_secs(2)));
    }

    #[test]
    fn t17_reconnect_closes_prior_first_applies_backoff_resubscribes_and_reconciles() {
        runtime().block_on(async {
            let http_server = serve_sequence(reconnect_http_responses()).expect("http");
            let server = reconnect_ws_server();
            let (clock, mut waits) = SteppingClock::new(WaitMode::AutoAdvance);
            let (bus, _events) = event_bus_with_collector();
            let client = make_client(
                Ports {
                    ws_port: server.port(),
                    http_port: http_server.port(),
                },
                clock,
                bus,
                Arc::new(WsAccountSlotPool::new(2)),
                quiet_config(),
            );
            let token = SecretValue::new(b"tok-reconn".to_vec());
            let account = AccountId::new(7);
            client.connect(&token).await.expect("connect 1");
            let mut set = WsSubscriptionSet::new();
            set.insert(WsTopic::personal_order(&account)).expect("acct");
            client.declare_subscriptions(set).await.expect("declare");
            let _baseline = client
                .reconcile_orders_with_token(&token)
                .await
                .expect("baseline");

            assert_eq!(
                client.next_event(&token).await.expect("shutdown"),
                WsDispatchedEvent::Disconnected(WsDisconnectCause::ServerShutdown)
            );
            let first_recovery = client
                .reconnect_with_backoff(&token, 0)
                .await
                .expect("reconnect 1");
            assert_eq!(
                first_recovery.published_events,
                vec![DomainEvent::OrderFilled(account)]
            );
            assert_eq!(
                client.next_event(&token).await.expect("abrupt"),
                WsDispatchedEvent::Disconnected(WsDisconnectCause::ServerClosed(None))
            );
            let second_recovery = client
                .reconnect_with_backoff(&token, 1)
                .await
                .expect("reconnect 2");
            assert_eq!(second_recovery.published_events, Vec::new());

            client.disconnect().await;
            let records = server.finish().expect("ws records");
            assert_reconnect_evidence(&records, &drain_waits(&mut waits));
            let _http_calls = http_server.finish().expect("http finish");
        });
    }

    #[test]
    fn t17_two_connections_per_account_rule_evicts_oldest_connection() {
        runtime().block_on(async {
            let plan = FakeWsSessionPlan {
                upgrade: WsUpgradeReply::Accept,
                steps: vec![FakeWsStep::ExpectText, FakeWsStep::WaitClientClose],
            };
            let server = FakeWsServer::start(vec![plan.clone(), plan.clone(), plan]).expect("ws");
            let http_stub = serve_sequence(Vec::new()).expect("http");
            let pool = Arc::new(WsAccountSlotPool::new(2));
            let account = AccountId::new(11);
            let token = SecretValue::new(b"tok-pool".to_vec());

            let mut clients = Vec::new();
            for _idx in 0_usize..3_usize {
                let (clock, _waits) = SteppingClock::new(WaitMode::AutoAdvance);
                let (bus, _events) = event_bus_with_collector();
                clients.push(make_client(
                    Ports {
                        ws_port: server.port(),
                        http_port: http_stub.port(),
                    },
                    clock,
                    bus,
                    Arc::clone(&pool),
                    quiet_config(),
                ));
            }
            let [first, second, third] = <[TossWsClient; 3]>::try_from(clients).expect("3 clients");

            let mut set = WsSubscriptionSet::new();
            set.insert(WsTopic::personal_order(&account))
                .expect("topic");

            first.connect(&token).await.expect("c1");
            first.declare_subscriptions(set.clone()).await.expect("d1");
            second.connect(&token).await.expect("c2");
            second.declare_subscriptions(set.clone()).await.expect("d2");
            assert_eq!(pool.active_count(&account).await, 2);
            assert_eq!(pool.eviction_count().await, 0);

            third.connect(&token).await.expect("c3");
            third.declare_subscriptions(set).await.expect("d3");
            assert_eq!(pool.active_count(&account).await, 2);
            assert_eq!(pool.eviction_count().await, 1);
            assert_eq!(
                first.next_event(&token).await.expect("evicted"),
                WsDispatchedEvent::Disconnected(WsDisconnectCause::AccountConnectionLimitEvicted)
            );

            second.disconnect().await;
            third.disconnect().await;
            let records = server.finish().expect("records");
            assert!(records.iter().all(|rec| rec.client_closed));
        });
    }

    fn renewal_http_responses() -> Vec<Vec<u8>> {
        let token_one_body = serde_json::json!({
            "access_token": "tok-first-live",
            "token_type": "Bearer",
            "expires_in": 3600_u64
        })
        .to_string();
        let token_two_body = serde_json::json!({
            "access_token": "tok-second-renewed",
            "token_type": "Bearer",
            "expires_in": 3600_u64
        })
        .to_string();
        let filled_order = kr_order_json(&OrderFixture {
            order_id: "renew-fill-1",
            status: "FILLED",
            price: "72500",
            qty: "10",
            filled_qty: "10",
            filled_at: Some("2027-01-15T09:15:00+09:00"),
            canceled_at: None,
        });
        vec![
            http_json(200, "OK", &token_one_body),
            http_json(200, "OK", &orders_envelope(&[])),
            http_json(200, "OK", &orders_envelope(&[])),
            http_json(200, "OK", &token_two_body),
            http_json(200, "OK", &orders_envelope(&[])),
            http_json(200, "OK", &orders_envelope(&[filled_order])),
        ]
    }

    fn renewal_ws_server() -> FakeWsServer {
        let revoked_frame = serde_json::json!({
            "type": "error",
            "error": {"code": "auth-revoked", "message": "token expired"}
        })
        .to_string();
        FakeWsServer::start(vec![
            FakeWsSessionPlan {
                upgrade: WsUpgradeReply::Accept,
                steps: vec![
                    FakeWsStep::ExpectText,
                    FakeWsStep::SendText(revoked_frame),
                    FakeWsStep::WaitClientClose,
                ],
            },
            FakeWsSessionPlan {
                upgrade: WsUpgradeReply::Accept,
                steps: vec![FakeWsStep::ExpectText, FakeWsStep::WaitClientClose],
            },
        ])
        .expect("ws")
    }

    async fn seeded_secret_store() -> MemorySecretStore {
        let store = MemorySecretStore::default();
        store.script_store().push(MemoryReply::Memory);
        store.script_store().push(MemoryReply::Memory);
        for _load in 0_usize..4_usize {
            store.script_load().push(MemoryReply::Memory);
        }
        store
            .store(SecretKey::TossClientId, SecretValue::new(b"cid".to_vec()))
            .await
            .expect("cid");
        store
            .store(
                SecretKey::TossClientSecret,
                SecretValue::new(b"csec".to_vec()),
            )
            .await
            .expect("csec");
        store
    }

    fn assert_renewal_records(ws_records: &[testkit::WsRecordedConnection]) {
        assert_eq!(ws_records.len(), 2);
        assert_eq!(
            ws_records
                .first()
                .and_then(|rec| rec.authorization.as_deref()),
            Some("Bearer tok-first-live")
        );
        assert_eq!(
            ws_records
                .get(1)
                .and_then(|rec| rec.authorization.as_deref()),
            Some("Bearer tok-second-renewed")
        );
        assert!(
            ws_records
                .get(1)
                .is_some_and(|rec| rec.prior_closed_at_accept)
        );
    }

    async fn make_renewal_manager(
        http_port: u16,
        clock: &Arc<SteppingClock>,
        ws_client: &Arc<TossWsClient>,
        bus: &Arc<EventBus>,
    ) -> TokenManager<MemorySecretStore, SharedClock, Arc<TossWsClient>> {
        let store = seeded_secret_store().await;
        let http_client = TossHttpClient::new(
            &TossEndpoint::local(http_port).expect("ep"),
            Arc::<SteppingClock>::clone(clock),
            Arc::<SteppingClock>::clone(clock),
        )
        .expect("http");
        TokenManager::new(
            http_client,
            store,
            SharedClock(Arc::clone(clock)),
            Arc::clone(ws_client),
            Arc::clone(bus),
        )
    }

    #[test]
    fn t17_token_expiry_and_auth_revoked_drive_token_manager_renewal_without_deadlock() {
        runtime().block_on(async {
            let http_server = serve_sequence(renewal_http_responses()).expect("http");
            let ws_server = renewal_ws_server();
            let (clock, _waits) = SteppingClock::new(WaitMode::AutoAdvance);
            let (bus, mut events) = event_bus_with_collector();
            let ws_client = Arc::new(make_client(
                Ports {
                    ws_port: ws_server.port(),
                    http_port: http_server.port(),
                },
                Arc::clone(&clock),
                Arc::clone(&bus),
                Arc::new(WsAccountSlotPool::new(2)),
                quiet_config(),
            ));
            let manager = make_renewal_manager(http_server.port(), &clock, &ws_client, &bus).await;

            let lease_one = manager.token().await.expect("first token");
            assert_eq!(manager.connection().await, BrokerConnection::Connected);
            let account = AccountId::new(21);
            let mut set = WsSubscriptionSet::new();
            set.insert(WsTopic::personal_order(&account))
                .expect("topic");
            ws_client.declare_subscriptions(set).await.expect("declare");
            let _baseline = ws_client
                .reconcile_orders_with_token(lease_one.secret())
                .await
                .expect("baseline");

            assert_eq!(
                ws_client
                    .next_event(lease_one.secret())
                    .await
                    .expect("revoked"),
                WsDispatchedEvent::Disconnected(WsDisconnectCause::AuthRevoked)
            );
            clock.advance(Duration::from_secs(3545));

            let lease_two = manager.token().await.expect("renewed token");
            assert_eq!(
                lease_two.secret().as_exposed_secret(),
                b"tok-second-renewed"
            );
            assert!(ws_client.is_connected().await);
            let seen = drain_events(&mut events);
            assert!(seen.contains(&DomainEvent::BrokerConnectionChanged));
            assert!(seen.contains(&DomainEvent::OrderFilled(account)));

            ws_client.disconnect().await;
            assert_renewal_records(&ws_server.finish().expect("ws finish"));
            assert_eq!(http_server.finish().expect("http finish").len(), 6);
        });
    }
}
