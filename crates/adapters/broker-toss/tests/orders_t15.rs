mod support;
#[cfg(test)]
mod support_tests;

#[cfg(test)]
mod tests {
    use application::{
        BrokerRead, Clock, EventBus, HttpStatus, InstrumentIdIssuer, InstrumentMasterAuthority,
        InstrumentMasterSession, MappingStoreError, MasterNotReady, MissingData, PortErrorKind,
        ReadEndpoint, RequestRejection, ResponseMismatch, SecretKey, SecretStore, SecretValue,
        SystemPortError, WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
        TossTimestamp,
    };
    use domain::{
        AccountId, BrokerOrderId, BrokerOrderMoney, BrokerOrderPrice, BrokerOrderSize,
        BrokerOrderStatus, BrokerOrderTime, BrokerOrderType, BrokerTimeInForce,
        ClosedOrderCompleteness, ClosedOrderIncomplete, CommonShare, Currency,
        DisplayInstrumentName, InstrumentId, InstrumentMappingError, InstrumentSource,
        KoreanMasterDetail, LiquidationTrading, ListingStatus, MappingAssignment, MasterCode,
        MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, Money,
        NetPurchaseError, NetPurchaseIncomplete, NetPurchaseScope, NumericInput, NxtTrading,
        OpenOrderCoverage, OppositeOpenOrder, OrderDateFilter, OrderPageExhaustion, OrderSide,
        OrderSymbolFilter, Price, Quantity, ResolvedOrderInstrument, Symbol, TimeInterval,
        Timestamp, TradingSuspension, UnfilledQuantity, net_purchase_amount, net_purchases,
        opposite_open_order,
    };
    use serde_json::{Value, json};
    use std::{
        fs,
        future::Future,
        sync::{
            Arc,
            atomic::{AtomicI64, Ordering},
        },
    };
    use testkit::ScriptedHttpServer;

    struct Store;
    impl SecretStore for Store {
        fn load(
            &self,
            key: SecretKey,
        ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
            std::future::ready(Ok(Some(SecretValue::new(match key {
                SecretKey::TossClientId => b"fake-t15-id".to_vec(),
                SecretKey::TossClientSecret => b"fake-t15-secret".to_vec(),
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

    struct DummyAuthority;
    struct DummySession;
    impl application::InstrumentMappingRepository for DummySession {
        fn resolve(
            &self,
            _: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            std::future::ready(Err(MappingStoreError::Mapping(
                InstrumentMappingError::Missing,
            )))
        }
        fn lookup(
            &self,
            _: &InstrumentSource,
        ) -> impl Future<Output = Result<domain::InstrumentMappingRecord, MappingStoreError>> + Send
        {
            std::future::ready(Err(MappingStoreError::Mapping(
                InstrumentMappingError::Missing,
            )))
        }
        fn for_id(
            &self,
            _: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<domain::InstrumentMappingRecord>, MappingStoreError>> + Send
        {
            std::future::ready(Ok(Vec::new()))
        }
        fn ensure(
            &self,
            _: &InstrumentSource,
            _: domain::IssuedInstrumentId,
        ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send {
            std::future::ready(Err(MappingStoreError::Mapping(
                InstrumentMappingError::Missing,
            )))
        }
        fn retire(
            &self,
            _: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            std::future::ready(Err(MappingStoreError::Mapping(
                InstrumentMappingError::Missing,
            )))
        }
    }
    impl InstrumentMasterSession for DummySession {
        fn verify_observation(
            &self,
            _: &[application::InstrumentMasterObservation],
        ) -> impl Future<Output = Result<(), MasterNotReady>> + Send {
            std::future::ready(Ok(()))
        }
    }
    impl InstrumentMasterAuthority for DummyAuthority {
        type Session<'a> = DummySession;
        fn begin(
            &self,
            _: domain::KnownMarket,
        ) -> impl Future<Output = Result<Self::Session<'_>, MasterNotReady>> + Send {
            std::future::ready(Ok(DummySession))
        }
    }
    struct DummyIssuer;
    impl InstrumentIdIssuer for DummyIssuer {
        fn issue(
            &self,
        ) -> impl Future<Output = Result<domain::IssuedInstrumentId, SystemPortError>> + Send
        {
            std::future::ready(Err(SystemPortError::new(
                application::SystemPort::Repository,
                application::SystemFailure::InvalidData,
            )))
        }
    }

    #[derive(Clone)]
    struct FixedClock(Arc<AtomicI64>);
    impl FixedClock {
        fn at(raw: &str) -> Self {
            Self(Arc::new(AtomicI64::new(*ts(raw).utc_seconds().as_i64())))
        }
    }
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            Timestamp::from_parts(
                domain::UnixSeconds::new(self.0.load(Ordering::SeqCst)),
                domain::Nanosecond::new(0),
                domain::OffsetSeconds::new(32_400),
            )
            .expect("fixed clock")
        }
    }

    #[derive(Clone)]
    struct SharedTime(Arc<crate::support::FakeTime>);
    impl Clock for SharedTime {
        fn now(&self) -> Timestamp {
            self.0.now()
        }
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }

    fn reply(status: &str, body: &Value) -> Vec<u8> {
        let text = body.to_string();
        format!(
            "HTTP/1.1 {status}\r\nRetry-After: 4\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
            text.len()
        )
        .into_bytes()
    }

    fn issued() -> Vec<u8> {
        reply(
            "200 OK",
            &json!({"access_token": "fake-access-t15", "token_type": "Bearer", "expires_in": 3600}),
        )
    }

    fn ts(raw: &str) -> Timestamp {
        TossTimestamp::try_from(raw).expect("ts").into_domain()
    }

    fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("num"), currency).expect("money")
    }

    fn qty(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("num")).expect("qty")
    }

    fn price(text: &str, currency: Currency) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("num"), currency).expect("price")
    }

    fn load_recording(dir: &str, name: &str) -> Value {
        serde_json::from_str(
            &fs::read_to_string(format!("tests/{dir}/{name}.json")).expect("recording file"),
        )
        .expect("recording JSON")
    }

    #[derive(Clone, Copy)]
    struct MasterSpec<'a> {
        listing: &'a str,
        symbol: &'a str,
        id: &'a str,
        currency: Currency,
        status: ListingStatus,
        kr_detail: Option<KoreanMasterDetail>,
    }

    fn master_row(spec: MasterSpec<'_>) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new(spec.listing).expect("listing"),
            symbol: Symbol::try_new(spec.symbol).expect("sym"),
            instrument_id: InstrumentId::try_new(spec.id).expect("id"),
            name: DisplayInstrumentName::try_new("Synthetic Corp").expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new(if spec.currency == Currency::Krw {
                    "STOCK"
                } else {
                    "FOREIGN_STOCK"
                })
                .expect("sec"),
            ),
            currency: spec.currency,
            listing_status: spec.status,
            common_share: CommonShare::Common,
            korean_detail: spec.kr_detail,
            observed_at: ts("2026-03-28T08:00:00+09:00"),
        })
        .expect("master")
    }

    fn active_kr_detail() -> KoreanMasterDetail {
        KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        }
    }

    fn default_master() -> MasterInstrumentFacts {
        MasterInstrumentFacts::try_new(vec![
            master_row(MasterSpec {
                listing: "KOSPI",
                symbol: "005930",
                id: "inst-kr-005930",
                currency: Currency::Krw,
                status: ListingStatus::Active,
                kr_detail: Some(active_kr_detail()),
            }),
            master_row(MasterSpec {
                listing: "NASDAQ",
                symbol: "AAPL",
                id: "inst-us-aapl",
                currency: Currency::Usd,
                status: ListingStatus::Active,
                kr_detail: None,
            }),
            master_row(MasterSpec {
                listing: "KOSPI",
                symbol: "000660",
                id: "inst-kr-000660",
                currency: Currency::Krw,
                status: ListingStatus::Delisted,
                kr_detail: Some(active_kr_detail()),
            }),
            master_row(MasterSpec {
                listing: "KOSPI",
                symbol: "035420",
                id: "inst-kr-035420",
                currency: Currency::Krw,
                status: ListingStatus::Active,
                kr_detail: Some(KoreanMasterDetail {
                    liquidation: LiquidationTrading::Liquidation,
                    krx: TradingSuspension::Clear,
                    nxt: NxtTrading::Unsupported,
                }),
            }),
        ])
        .expect("master facts")
    }

    fn adapter_with<C: Clock + Clone + 'static>(
        server: &ScriptedHttpServer,
        clock: C,
        waiter: Arc<crate::support::FakeTime>,
        master: MasterInstrumentFacts,
    ) -> TossAccountRead<Store, C, Renewal> {
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&waiter),
            waiter,
        )
        .expect("client");
        let tokens = TokenManager::new(
            client,
            Store,
            clock.clone(),
            Renewal,
            Arc::new(EventBus::default()),
        );
        TossAccountRead::new(tokens, clock, master)
    }

    fn kr_instrument(symbol: &str) -> ResolvedOrderInstrument {
        master_row(MasterSpec {
            listing: "KOSPI",
            symbol,
            id: "inst-kr-helper",
            currency: Currency::Krw,
            status: ListingStatus::Active,
            kr_detail: Some(active_kr_detail()),
        })
        .order_gate_facts()
        .expect("gate")
        .instrument_for(OrderSide::Sell)
        .expect("kr")
    }

    fn us_instrument(symbol: &str) -> ResolvedOrderInstrument {
        master_row(MasterSpec {
            listing: "NASDAQ",
            symbol,
            id: "inst-us-helper",
            currency: Currency::Usd,
            status: ListingStatus::Active,
            kr_detail: None,
        })
        .order_gate_facts()
        .expect("gate")
        .instrument_for(OrderSide::Sell)
        .expect("us")
    }

    #[derive(Clone, Copy)]
    struct ClosedOrderFixture<'a> {
        id: &'a str,
        symbol: &'a str,
        currency: &'a str,
        side: &'a str,
        filled_amount: &'a str,
        ordered_at: &'a str,
        filled_at: &'a str,
    }

    fn sample_closed_order(fixture: ClosedOrderFixture<'_>) -> Value {
        json!({
            "orderId": fixture.id,
            "symbol": fixture.symbol,
            "side": fixture.side,
            "orderType": "LIMIT",
            "timeInForce": "DAY",
            "status": "FILLED",
            "price": if fixture.currency == "KRW" { "70000" } else { "185.25" },
            "quantity": "10",
            "orderAmount": null,
            "currency": fixture.currency,
            "orderedAt": fixture.ordered_at,
            "canceledAt": null,
            "execution": {
                "filledQuantity": "10",
                "averageFilledPrice": if fixture.currency == "KRW" { "70000" } else { "185.25" },
                "filledAmount": fixture.filled_amount,
                "commission": "0",
                "tax": "0",
                "filledAt": fixture.filled_at,
                "settlementDate": "2026-03-30"
            }
        })
    }

    #[test]
    fn t15_recordings_validate_and_replay_against_openapi_spec() {
        let spec = crate::support_tests::spec();
        for (dir, name) in [
            ("recordings", "t15-orders-open"),
            ("recordings", "t15-orders-closed-page1"),
            ("recordings", "t15-orders-closed-empty"),
            ("recordings", "t15-order-detail-kr"),
            ("recordings", "t15-order-detail-us-amount"),
            ("order_recordings", "t15-order-404-not-found"),
            ("order_recordings", "t15-orders-400-invalid-cursor"),
            ("order_recordings", "t15-orders-401-token-revoked"),
            ("order_recordings", "t15-orders-429-rate-limit"),
        ] {
            let rec = load_recording(dir, name);
            crate::support_tests::validation::validate_recording(&spec, &rec).expect(name);
        }
        assert!(
            crate::support_tests::recordings()
                .iter()
                .any(|item| item["path"] == "/api/v1/orders")
        );
        let sample = load_recording("recordings", "t15-order-detail-kr");
        let replay = crate::support_tests::serve_recordings(&sample);
        let time = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(replay.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&time),
            time,
        )
        .expect("client");
        let resp = run(client.get(
            &broker_toss::ReadRequest::new(crate::support_tests::route(&sample)),
            &SecretValue::new(b"fake-t15-token".to_vec()),
        ))
        .expect("replay");
        assert_eq!(resp.status(), 200);
        let _wire = replay.finish().expect("finish");
    }

    #[test]
    fn order_detail_parses_kr_limit_and_us_amount_orders() {
        let kr_rec = load_recording("recordings", "t15-order-detail-kr");
        let us_amt = load_recording("recordings", "t15-order-detail-us-amount");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &kr_rec["body"]),
            reply("200 OK", &us_amt["body"]),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-29T12:00:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let account = AccountId::new(413);

        let kr_id = BrokerOrderId::try_new("synthetic-order-kr-001").expect("id");
        let kr_order = run(reader.order(&account, &kr_id)).expect("kr order");
        assert_eq!(kr_order.account_id(), &account);
        assert_eq!(kr_order.order_id(), &kr_id);
        assert_eq!(kr_order.instrument(), &kr_instrument("005930"));
        assert_eq!(kr_order.side(), OrderSide::Buy);
        assert_eq!(kr_order.status(), &BrokerOrderStatus::Filled);
        assert_eq!(kr_order.size(), BrokerOrderSize::Quantity(qty("10")));
        assert_eq!(kr_order.filled_quantity(), qty("10"));
        assert_eq!(
            kr_order.unfilled_quantity(),
            UnfilledQuantity::Known(qty("0"))
        );
        assert_eq!(
            kr_order.price(),
            BrokerOrderPrice::Known(price("70000", Currency::Krw))
        );
        assert_eq!(
            kr_order.filled_amount(),
            BrokerOrderMoney::Known(money("700000", Currency::Krw))
        );
        assert_eq!(kr_order.observed_at(), ts("2026-03-29T12:00:00+09:00"));

        let us_id = BrokerOrderId::try_new("synthetic-order-us-amt-002").expect("id");
        let us_order = run(reader.order(&account, &us_id)).expect("us amount order");
        assert_eq!(
            us_order.size(),
            BrokerOrderSize::Amount {
                reported_quantity: qty("0"),
                order_amount: money("250.5", Currency::Usd),
            }
        );
        assert_eq!(
            us_order.unfilled_quantity(),
            UnfilledQuantity::UnknownAmountOrder
        );
        assert_eq!(us_order.filled_quantity(), qty("1.352226"));
        assert_eq!(us_order.price(), BrokerOrderPrice::Absent);

        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        assert!(wire.contains("GET /api/v1/orders/synthetic-order-kr-001 HTTP/1.1"));
        assert!(wire.contains("x-tossinvest-account: 413"));
    }

    fn unknown_metadata_order_payload() -> Value {
        json!({
            "result": {
                "orderId": "synthetic-order-unknown-003",
                "symbol": "005930",
                "side": "SELL",
                "orderType": "FUTURE_TYPE",
                "timeInForce": "FUTURE_TIF",
                "status": "FUTURE_STATUS",
                "price": null,
                "quantity": "3",
                "orderAmount": null,
                "currency": "KRW",
                "orderedAt": "2026-03-28T09:30:00+09:00",
                "canceledAt": "2026-03-28T09:35:00+09:00",
                "execution": {
                    "filledQuantity": "0",
                    "averageFilledPrice": null,
                    "filledAmount": null,
                    "commission": null,
                    "tax": null,
                    "filledAt": null,
                    "settlementDate": null
                }
            }
        })
    }

    #[test]
    fn order_detail_preserves_unknown_metadata_and_maps_order_not_found() {
        let not_found = load_recording("order_recordings", "t15-order-404-not-found");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &unknown_metadata_order_payload()),
            reply("404 Not Found", &not_found["body"]),
            reply("400 Bad Request", &not_found["body"]),
            reply("404 Not Found", &not_found["body"]),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-29T12:00:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let account = AccountId::new(413);

        let unk_id = BrokerOrderId::try_new("synthetic-order-unknown-003").expect("id");
        let unk_order = run(reader.order(&account, &unk_id)).expect("unknown meta");
        assert_eq!(
            unk_order.order_type(),
            &BrokerOrderType::Unknown(MasterCode::try_new("FUTURE_TYPE").expect("c"))
        );
        assert_eq!(
            unk_order.time_in_force(),
            &BrokerTimeInForce::Unknown(MasterCode::try_new("FUTURE_TIF").expect("c"))
        );
        assert_eq!(
            unk_order.status(),
            &BrokerOrderStatus::Unknown(MasterCode::try_new("FUTURE_STATUS").expect("c"))
        );
        assert_eq!(
            unk_order.canceled_at(),
            BrokerOrderTime::Known(ts("2026-03-28T09:35:00+09:00"))
        );

        let missing_id = BrokerOrderId::try_new("synthetic-order-missing-404").expect("id");
        let err_404 = run(reader.order(&account, &missing_id)).expect_err("404");
        assert_eq!(err_404.endpoint(), ReadEndpoint::Order);
        assert_eq!(err_404.status(), HttpStatus::NotFound);
        assert_eq!(err_404.kind(), PortErrorKind::NotFound(MissingData::Order));

        let err_400 = run(reader.order(&account, &missing_id)).expect_err("400 status mismatch");
        assert_eq!(
            err_400.kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        );
        let non_order = run(reader.open_orders(&account)).expect_err("non-order endpoint");
        assert_eq!(
            non_order.kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        );
        let _wire = server.finish().expect("finish");
    }

    fn mutated_order(pointer: &str, value: Value) -> Value {
        let mut body = load_recording("recordings", "t15-order-detail-kr")["body"].clone();
        *body.pointer_mut(pointer).expect("pointer") = value;
        body
    }

    fn invalid_order_detail_replies() -> Vec<Vec<u8>> {
        vec![
            issued(),
            reply("200 OK", &mutated_order("/result/orderId", json!("other"))),
            reply("200 OK", &mutated_order("/result/symbol", json!("009999"))),
            reply("200 OK", &mutated_order("/result/symbol", json!("009999"))),
            reply("200 OK", &mutated_order("/result/symbol", json!("000660"))),
            reply("200 OK", &mutated_order("/result/currency", json!("EUR"))),
            reply("200 OK", &mutated_order("/result/side", json!("SHORT"))),
            reply("200 OK", &mutated_order("/result/currency", json!("USD"))),
            reply(
                "200 OK",
                &mutated_order("/result/execution/commission", json!("1e2")),
            ),
            reply(
                "200 OK",
                &mutated_order("/result/execution/settlementDate", json!("2026-02-30")),
            ),
            reply(
                "200 OK",
                &mutated_order(
                    "/result/execution/filledAt",
                    json!("2026-03-28T09:29:59+09:00"),
                ),
            ),
            reply(
                "200 OK",
                &mutated_order("/result/canceledAt", json!("2026-03-28T09:29:59+09:00")),
            ),
            reply(
                "200 OK",
                &mutated_order("/result/orderAmount", json!("700000")),
            ),
        ]
    }

    #[test]
    fn order_detail_rejects_id_mismatch_unsafe_ids_missing_master_and_bad_fields() {
        let server = ScriptedHttpServer::start(invalid_order_detail_replies()).expect("server");
        let clock = FixedClock::at("2026-03-29T12:00:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let account = AccountId::new(413);

        for unsafe_raw in ["a/b", "a?b", "..", &"a".repeat(129)] {
            let unsafe_id = BrokerOrderId::try_new(unsafe_raw).expect("domain id");
            let err = run(reader.order(&account, &unsafe_id)).expect_err("unsafe route");
            assert_eq!(err.status(), HttpStatus::NotReceived);
            assert_eq!(
                err.kind(),
                PortErrorKind::InvalidRequest(RequestRejection::MalformedRequest)
            );
        }

        let target_id = BrokerOrderId::try_new("synthetic-order-kr-001").expect("id");
        assert_eq!(
            run(reader.order(&account, &target_id))
                .expect_err("mismatch")
                .kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        );
        assert_eq!(
            run(reader.order(&account, &target_id))
                .expect_err("legacy missing")
                .kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        );
        let dedicated = reader.with_instrument_master(DummyAuthority, DummyIssuer);
        assert_eq!(
            run(dedicated.order(&account, &target_id))
                .expect_err("dedicated missing")
                .kind(),
            PortErrorKind::MasterNotReady(MasterNotReady::MissingInstrument)
        );
        for _case in 0_u8..9 {
            assert_eq!(
                run(dedicated.order(&account, &target_id))
                    .expect_err("invalid row")
                    .kind(),
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
            );
        }
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn open_orders_fetches_unfiltered_complete_and_checks_opposite_open_order() {
        let open_rec = load_recording("recordings", "t15-orders-open");
        let empty_rec = load_recording("recordings", "t15-orders-closed-empty");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &open_rec["body"]),
            reply("200 OK", &empty_rec["body"]),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-29T12:00:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let account = AccountId::new(413);

        let complete = run(reader.open_orders(&account)).expect("open complete");
        assert_eq!(complete.coverage(), OpenOrderCoverage::Complete);
        assert_eq!(complete.account_id(), &account);
        assert_eq!(complete.orders().len(), 2);
        assert_eq!(
            opposite_open_order(
                &complete,
                &account,
                &kr_instrument("005930"),
                OrderSide::Sell
            ),
            OppositeOpenOrder::Present
        );
        assert_eq!(
            opposite_open_order(
                &complete,
                &account,
                &kr_instrument("005930"),
                OrderSide::Buy
            ),
            OppositeOpenOrder::Absent
        );
        assert_eq!(
            opposite_open_order(&complete, &account, &us_instrument("AAPL"), OrderSide::Buy),
            OppositeOpenOrder::NotApplicable
        );

        let empty = run(reader.open_orders(&account)).expect("empty open");
        assert_eq!(empty.coverage(), OpenOrderCoverage::Complete);
        assert!(empty.orders().is_empty());

        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        assert!(wire.contains("GET /api/v1/orders?status=OPEN HTTP/1.1"));
        assert!(!wire.contains("symbol="));
        assert!(!wire.contains("from="));
        assert!(!wire.contains("to="));
        assert!(wire.contains("x-tossinvest-account: 413"));
    }

    fn open_fault_payloads() -> Vec<Value> {
        let open_rec = load_recording("recordings", "t15-orders-open");
        let delisted_row = json!({
            "orderId": "synthetic-open-delisted-003",
            "symbol": "000660",
            "side": "BUY",
            "orderType": "LIMIT",
            "timeInForce": "DAY",
            "status": "PENDING",
            "price": "100000",
            "quantity": "1",
            "orderAmount": null,
            "currency": "KRW",
            "orderedAt": "2026-03-29T09:35:00+09:00",
            "canceledAt": null,
            "execution": {
                "filledQuantity": "0",
                "averageFilledPrice": null,
                "filledAmount": null,
                "commission": null,
                "tax": null,
                "filledAt": null,
                "settlementDate": null
            }
        });
        let mut with_unidentified = open_rec["body"].clone();
        with_unidentified["result"]["orders"]
            .as_array_mut()
            .expect("arr")
            .push(delisted_row.clone());
        let mut with_closed = open_rec["body"].clone();
        with_closed["result"]["orders"][0]["status"] = json!("FILLED");
        let mut with_has_next = open_rec["body"].clone();
        with_has_next["result"]["hasNext"] = json!(true);
        with_has_next["result"]["nextCursor"] = json!("unexpected-cursor");
        let only_unidentified =
            json!({"result": {"orders": [delisted_row], "hasNext": false, "nextCursor": null}});
        let bogus_empty_next =
            json!({"result": {"orders": [], "hasNext": true, "nextCursor": "c"}});
        let broken_schema = json!({"result": {"orders": [{"orderId": 123_i32}], "hasNext": false, "nextCursor": null}});
        vec![
            with_unidentified,
            with_closed,
            with_has_next,
            only_unidentified,
            bogus_empty_next,
            broken_schema,
        ]
    }

    fn day_window(start: &str, end: &str) -> TimeInterval {
        TimeInterval::try_new(ts(start), ts(end)).expect("window")
    }

    fn closed_page2_payload() -> Value {
        json!({
            "result": {
                "orders": [
                    sample_closed_order(ClosedOrderFixture {
                        id: "synthetic-closed-kr-002",
                        symbol: "005930",
                        currency: "KRW",
                        side: "SELL",
                        filled_amount: "200000",
                        ordered_at: "2026-03-28T10:00:00+09:00",
                        filled_at: "2026-03-28T10:05:00+09:00",
                    }),
                    sample_closed_order(ClosedOrderFixture {
                        id: "synthetic-closed-us-003",
                        symbol: "AAPL",
                        currency: "USD",
                        side: "BUY",
                        filled_amount: "1852.5",
                        ordered_at: "2026-03-28T11:00:00+09:00",
                        filled_at: "2026-03-28T11:01:00+09:00",
                    })
                ],
                "nextCursor": null,
                "hasNext": false
            }
        })
    }

    fn partial_filled_closed_payload() -> Value {
        json!({
            "result": {
                "orders": [
                    sample_closed_order(ClosedOrderFixture {
                        id: "synthetic-closed-kr-ok",
                        symbol: "005930",
                        currency: "KRW",
                        side: "BUY",
                        filled_amount: "700000",
                        ordered_at: "2026-03-28T09:30:00+09:00",
                        filled_at: "2026-03-28T09:31:00+09:00",
                    }),
                    {
                        "orderId": "synthetic-closed-partial",
                        "symbol": "005930",
                        "side": "BUY",
                        "orderType": "LIMIT",
                        "timeInForce": "DAY",
                        "status": "PARTIAL_FILLED",
                        "price": "70000",
                        "quantity": "10",
                        "orderAmount": null,
                        "currency": "KRW",
                        "orderedAt": "2026-03-28T09:32:00+09:00",
                        "canceledAt": null,
                        "execution": {
                            "filledQuantity": "4",
                            "averageFilledPrice": "70000",
                            "filledAmount": "280000",
                            "commission": "0",
                            "tax": "0",
                            "filledAt": "2026-03-28T09:33:00+09:00",
                            "settlementDate": null
                        }
                    }
                ],
                "nextCursor": null,
                "hasNext": false
            }
        })
    }

    fn server_error_4s() -> PortErrorKind {
        PortErrorKind::Unavailable {
            cause: application::UnavailableCause::ServerError,
            retry: application::RetryDelay::Known(
                domain::TimeSpan::try_new(domain::SpanSeconds::new(4), domain::Nanosecond::new(0))
                    .expect("span"),
            ),
        }
    }

    #[test]
    fn open_orders_marks_incomplete_on_unidentified_closed_pagination_and_errors() {
        let payloads = open_fault_payloads();
        let mut responses = vec![issued()];
        responses.extend(payloads.iter().map(|body| reply("200 OK", body)));
        responses.push(reply(
            "500 Internal Server Error",
            &json!({"error": {"requestId": "o-500", "code": "internal-error", "message": "err"}}),
        ));
        let server = ScriptedHttpServer::start(responses).expect("server");
        let reader = adapter_with(
            &server,
            FixedClock::at("2026-03-29T12:00:00+09:00"),
            crate::support::time(),
            default_master(),
        );
        let account = AccountId::new(413);

        let partial = run(reader.open_orders(&account)).expect("unidentified row");
        assert_eq!(partial.coverage(), OpenOrderCoverage::Incomplete);
        assert_eq!(partial.orders().len(), 2);
        assert_eq!(
            opposite_open_order(&partial, &account, &kr_instrument("005930"), OrderSide::Buy),
            OppositeOpenOrder::Incomplete
        );
        assert_eq!(
            opposite_open_order(
                &partial,
                &account,
                &kr_instrument("005930"),
                OrderSide::Sell
            ),
            OppositeOpenOrder::Present
        );

        let closed_mix = run(reader.open_orders(&account)).expect("closed in open");
        assert_eq!(closed_mix.coverage(), OpenOrderCoverage::Incomplete);
        assert_eq!(closed_mix.orders().len(), 2);
        assert_eq!(
            run(reader.open_orders(&account))
                .expect("hasNext")
                .coverage(),
            OpenOrderCoverage::Incomplete
        );
        let only_unid = run(reader.open_orders(&account)).expect("only unidentified");
        assert_eq!(only_unid.coverage(), OpenOrderCoverage::Incomplete);
        assert!(only_unid.orders().is_empty());

        assert_eq!(
            run(reader.open_orders(&account)).expect_err("bogus").kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        );
        assert_eq!(
            run(reader.open_orders(&account))
                .expect_err("schema")
                .kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Schema)
        );
        assert_eq!(
            run(reader.open_orders(&account)).expect_err("500").kind(),
            server_error_4s()
        );
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn closed_orders_paginates_proves_completeness_and_feeds_d38_net_purchases() {
        let page1 = load_recording("recordings", "t15-orders-closed-page1");
        let page2 = closed_page2_payload();
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &page1["body"]),
            reply("200 OK", &page2),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-28T15:30:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let account = AccountId::new(413);
        let window = day_window("2026-03-28T09:00:00+09:00", "2026-03-28T15:00:00+09:00");

        let closed = run(reader.closed_orders(&account, window)).expect("closed");
        assert_eq!(closed.completeness(), ClosedOrderCompleteness::Complete);
        assert_eq!(closed.orders().len(), 3);
        let proof = closed.completeness_proof().expect("proof");
        assert_eq!(proof.scope().account_id(), &account);
        assert_eq!(proof.scope().evaluation_window(), window);
        assert_eq!(proof.scope().date_filter(), OrderDateFilter::Unfiltered);
        assert_eq!(
            proof.scope().symbol_filter(),
            &OrderSymbolFilter::AllSymbols
        );
        assert_eq!(proof.exhaustion(), OrderPageExhaustion::Exhausted);
        let totals = net_purchases(&closed, window);
        assert_eq!(
            totals.currency(Currency::Krw),
            Ok(money("500000", Currency::Krw))
        );
        assert_eq!(
            totals.currency(Currency::Usd),
            Ok(money("1852.5", Currency::Usd))
        );

        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        assert!(wire.contains("GET /api/v1/orders?status=CLOSED&limit=100 HTTP/1.1"));
        assert!(wire.contains(
            "GET /api/v1/orders?status=CLOSED&limit=100&cursor=synthetic-cursor-page-2 HTTP/1.1"
        ));
        assert!(!wire.contains("from="));
        assert!(!wire.contains("to="));
        assert!(!wire.contains("symbol="));
    }

    #[test]
    fn closed_orders_handles_cursor_restart_and_repeated_cursor() {
        let invalid_cursor = load_recording("order_recordings", "t15-orders-400-invalid-cursor");
        let page1 = load_recording("recordings", "t15-orders-closed-page1");
        let page_final = json!({
            "result": {
                "orders": [sample_closed_order(ClosedOrderFixture {
                    id: "synthetic-closed-kr-restart",
                    symbol: "005930",
                    currency: "KRW",
                    side: "BUY",
                    filled_amount: "350000",
                    ordered_at: "2026-03-28T09:30:00+09:00",
                    filled_at: "2026-03-28T09:31:00+09:00",
                })],
                "nextCursor": null,
                "hasNext": false
            }
        });
        let repeated_page2 = json!({"result": {"orders": [], "nextCursor": "synthetic-cursor-page-2", "hasNext": true}});
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &page1["body"]),
            reply("400 Bad Request", &invalid_cursor["body"]),
            reply("200 OK", &page_final),
            reply("200 OK", &page1["body"]),
            reply("200 OK", &repeated_page2),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-28T15:30:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let account = AccountId::new(413);
        let window = day_window("2026-03-28T09:00:00+09:00", "2026-03-28T15:00:00+09:00");

        let restarted = run(reader.closed_orders(&account, window)).expect("cursor restart");
        assert_eq!(restarted.completeness(), ClosedOrderCompleteness::Complete);
        assert!(
            restarted
                .orders()
                .contains_key(&BrokerOrderId::try_new("synthetic-closed-kr-restart").expect("id"))
        );

        let repeated = run(reader.closed_orders(&account, window)).expect("repeated cursor");
        assert_eq!(
            repeated.completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );
        assert_eq!(
            net_purchase_amount(&repeated, window, NetPurchaseScope::Currency(Currency::Krw)),
            Err(NetPurchaseError::Incomplete(NetPurchaseIncomplete::Query(
                ClosedOrderIncomplete::MissingPages
            )))
        );
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn closed_orders_handles_mid_failure_page1_failure_and_partial_filled() {
        let page1 = load_recording("recordings", "t15-orders-closed-page1");
        let partial_filled_page = partial_filled_closed_payload();
        let err_500 =
            json!({"error": {"requestId": "c-500", "code": "internal-error", "message": "err"}});
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &page1["body"]),
            reply("500 Internal Server Error", &err_500),
            reply("500 Internal Server Error", &err_500),
            reply("200 OK", &partial_filled_page),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-28T15:30:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let account = AccountId::new(413);
        let window = day_window("2026-03-28T09:00:00+09:00", "2026-03-28T15:00:00+09:00");

        let mid_failed = run(reader.closed_orders(&account, window)).expect("mid failure");
        assert_eq!(
            mid_failed.completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RequestFailed)
        );
        assert_eq!(mid_failed.orders().len(), 1);

        let page1_failed = run(reader.closed_orders(&account, window)).expect_err("page1 fail");
        assert_eq!(page1_failed.endpoint(), ReadEndpoint::ClosedOrders);

        let partial = run(reader.closed_orders(&account, window)).expect("partial rejected");
        assert_eq!(
            partial.completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RejectedRows)
        );
        assert_eq!(
            net_purchase_amount(&partial, window, NetPurchaseScope::Currency(Currency::Krw)),
            Err(NetPurchaseError::Incomplete(NetPurchaseIncomplete::Query(
                ClosedOrderIncomplete::RejectedRows
            )))
        );
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn closed_orders_preserves_old_order_for_d38_and_handles_early_or_future_windows() {
        let old_order_page = json!({
            "result": {
                "orders": [sample_closed_order(ClosedOrderFixture {
                    id: "synthetic-closed-old-fill",
                    symbol: "005930",
                    currency: "KRW",
                    side: "BUY",
                    filled_amount: "700000",
                    ordered_at: "2026-03-27T14:00:00+09:00",
                    filled_at: "2026-03-28T09:31:00+09:00",
                })],
                "nextCursor": null,
                "hasNext": false
            }
        });
        let empty = load_recording("recordings", "t15-orders-closed-empty");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &old_order_page),
            reply("200 OK", &empty["body"]),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-28T12:00:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let account = AccountId::new(413);

        let past = day_window("2026-03-28T09:00:00+09:00", "2026-03-28T11:00:00+09:00");
        let old = run(reader.closed_orders(&account, past)).expect("old");
        assert_eq!(old.completeness(), ClosedOrderCompleteness::Complete);
        assert_eq!(
            net_purchase_amount(&old, past, NetPurchaseScope::Currency(Currency::Krw)),
            Err(NetPurchaseError::Incomplete(
                NetPurchaseIncomplete::UnallocatableExecution
            ))
        );

        let straddling = day_window("2026-03-28T09:00:00+09:00", "2026-03-28T15:00:00+09:00");
        let early = run(reader.closed_orders(&account, straddling)).expect("early");
        assert_eq!(
            early.completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );
        assert_eq!(
            net_purchase_amount(
                &early,
                straddling,
                NetPurchaseScope::Currency(Currency::Krw)
            ),
            Err(NetPurchaseError::Incomplete(
                NetPurchaseIncomplete::ObservationTooEarly
            ))
        );

        for start_at in ["2026-03-28T12:00:00+09:00", "2026-03-28T13:00:00+09:00"] {
            let win = day_window(start_at, "2026-03-28T15:00:00+09:00");
            assert_eq!(
                run(reader.closed_orders(&account, win))
                    .expect_err("future or equal")
                    .kind(),
                PortErrorKind::InvalidRequest(RequestRejection::UnsupportedQuery)
            );
        }
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn review1_rejects_empty_next_pages_and_execution_or_future_time_contradictions() {
        let page1 = load_recording("recordings", "t15-orders-closed-page1");
        let empty_next =
            json!({"result": {"orders": [], "hasNext": true, "nextCursor": "c-empty"}});
        let mut zero_avg = load_recording("recordings", "t15-order-detail-kr")["body"].clone();
        zero_avg["result"]["execution"]["filledQuantity"] = json!("0");
        zero_avg["result"]["execution"]["filledAmount"] = json!(null);
        zero_avg["result"]["execution"]["filledAt"] = json!(null);
        zero_avg["result"]["execution"]["averageFilledPrice"] = json!("70000");
        let neg_comm = mutated_order("/result/execution/commission", json!("-10"));
        let neg_tax = mutated_order("/result/execution/tax", json!("-5"));
        let future_order = mutated_order("/result/orderedAt", json!("2027-01-01T09:30:00+09:00"));
        let future_open = json!({"result": {"orders": [future_order["result"].clone()], "hasNext": false, "nextCursor": null}});

        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &empty_next),
            reply("200 OK", &page1["body"]),
            reply("200 OK", &empty_next),
            reply("200 OK", &zero_avg),
            reply("200 OK", &neg_comm),
            reply("200 OK", &neg_tax),
            reply("200 OK", &future_order),
            reply("200 OK", &future_open),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-28T15:30:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let account = AccountId::new(413);
        let window = day_window("2026-03-28T09:00:00+09:00", "2026-03-28T15:00:00+09:00");
        let id = BrokerOrderId::try_new("synthetic-order-kr-001").expect("id");

        assert_eq!(
            run(reader.closed_orders(&account, window))
                .expect_err("empty p1")
                .kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        );
        let mid_empty = run(reader.closed_orders(&account, window)).expect("empty p2");
        assert_eq!(
            mid_empty.completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::RequestFailed)
        );
        for _case in 0_u8..4 {
            assert_eq!(
                run(reader.order(&account, &id))
                    .expect_err("bad detail")
                    .kind(),
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
            );
        }
        let open_future = run(reader.open_orders(&account)).expect("future open");
        assert_eq!(open_future.coverage(), OpenOrderCoverage::Incomplete);
        assert!(open_future.orders().is_empty());
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn closed_orders_rejects_schema_only_page() {
        let bad_schema = json!({"result": {"orders": [{"orderId": 123_i32}], "nextCursor": null, "hasNext": false}});
        let server =
            ScriptedHttpServer::start(vec![issued(), reply("200 OK", &bad_schema)]).expect("srv");
        let clock = FixedClock::at("2026-03-28T12:00:00+09:00");
        let reader = adapter_with(&server, clock, crate::support::time(), default_master());
        let past = day_window("2026-03-28T09:00:00+09:00", "2026-03-28T11:00:00+09:00");
        assert_eq!(
            run(reader.closed_orders(&AccountId::new(413), past))
                .expect_err("schema only")
                .kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Schema)
        );
        let _wire = server.finish().expect("finish");
    }

    fn huge_closed_page(count: u32) -> Value {
        let orders = (0_u32..count)
            .map(|idx| {
                sample_closed_order(ClosedOrderFixture {
                    id: &format!("synthetic-row-{idx:05}"),
                    symbol: "005930",
                    currency: "KRW",
                    side: "BUY",
                    filled_amount: "70000",
                    ordered_at: "2026-03-28T09:30:00+09:00",
                    filled_at: "2026-03-28T09:31:00+09:00",
                })
            })
            .collect::<Vec<_>>();
        json!({"result": {"orders": orders, "nextCursor": null, "hasNext": false}})
    }

    #[test]
    fn closed_orders_marks_row_limit_incomplete_and_shares_order_history_rate_limit() {
        let huge_page = huge_closed_page(4097);
        let empty = load_recording("recordings", "t15-orders-closed-empty");
        let detail = load_recording("recordings", "t15-order-detail-kr");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &huge_page),
            reply("200 OK", &empty["body"]),
            reply("200 OK", &detail["body"]),
            reply("200 OK", &empty["body"]),
            reply("200 OK", &detail["body"]),
            reply("200 OK", &empty["body"]),
        ])
        .expect("server");
        let time = crate::support::time();
        let reader = adapter_with(
            &server,
            SharedTime(Arc::<crate::support::FakeTime>::clone(&time)),
            Arc::<crate::support::FakeTime>::clone(&time),
            default_master(),
        );
        let account = AccountId::new(413);
        let window = day_window("2026-03-28T09:00:00+09:00", "2026-03-28T11:00:00+09:00");
        let id = BrokerOrderId::try_new("synthetic-order-kr-001").expect("id");
        let start = time.now();

        let step = domain::TimeSpan::try_new(
            domain::SpanSeconds::new(0),
            domain::Nanosecond::new(333_333_334),
        )
        .expect("order-history step");
        let limited = run(reader.closed_orders(&account, window)).expect("row limit");
        assert_eq!(time.now(), start);
        assert_eq!(
            limited.completeness(),
            ClosedOrderCompleteness::Incomplete(ClosedOrderIncomplete::MissingPages)
        );
        assert_eq!(limited.orders().len(), 4096);
        assert_eq!(
            limited.completeness_proof().expect("p").exhaustion(),
            OrderPageExhaustion::MorePages
        );
        run(reader.open_orders(&account)).expect("2");
        assert_eq!(time.now(), start.checked_add(step).expect("t1"));
        run(reader.order(&account, &id)).expect("3");
        assert_eq!(
            time.now(),
            start
                .checked_add(step)
                .and_then(|first| first.checked_add(step))
                .expect("t2")
        );
        run(reader.open_orders(&account)).expect("4");
        run(reader.order(&account, &id)).expect("5");
        let before_sixth = time.now();
        run(reader.open_orders(&account)).expect("6 waits");
        assert_eq!(time.now(), before_sixth.checked_add(step).expect("t5"));
        let _wire = server.finish().expect("finish");
    }
}
