mod support;
#[cfg(test)]
mod support_tests;

#[cfg(test)]
mod tests {
    use application::{
        AccessDenial, BrokerCapability, BrokerRead, CapabilitySupport, Clock, EventBus,
        MissingData, PortErrorKind, ReadEndpoint, ResponseMismatch, RetryDelay, SecretKey,
        SecretStore, SecretValue, SystemPortError, UnavailableCause, WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossDate, TossEndpoint,
        TossHttpClient, TossTimestamp,
    };
    use domain::{
        AccountId, AcquisitionPrice, AiProbability, AiProbabilityInput, BuyingPower, Calibration,
        CalibrationKey, CalibrationLedgerHead, CalibrationModel, CalibrationPosition,
        CalibrationSnapshot, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        CollectedItem, CommissionRateFact, CommissionValidity, CommonShare, CostEligibleTask,
        CostRate, Currency, DisplayInstrumentName, EstimateContext, EstimateCostPeriod,
        EstimateTask, Evidence, EvidenceRef, EvidenceUrl, FxPair, FxRate, FxRateKind, FxSource,
        Holding, HoldingObservation, Horizon, HorizonSelection, HoursPermission, InstrumentId,
        InvestorProfile, KnownMarket, KoreanMasterDetail, LiquidationTrading, ListingStatus,
        Margin, Market, MarketCalendar, MarketSession, MasterCode, MasterInstrument,
        MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, ModelIdentityPart, Money,
        Nanosecond, NormalizedUrl, NumericInput, NxtTrading, Ohlcv, OrderSide, Policy, Portfolio,
        PortfolioError, PortfolioValuation, Price, PriceAdjustment, Quantity, Ratio,
        ResolvedOrderInstrument, RiskAppetite, SectorAssignments, SectorCatalog,
        SellDraftQuantityFacts, SellOrderQuantity, SellQuantityConstraint, SellSignal,
        SellableQuantityDecision, SessionData, SessionInformation, SignalAssessmentFacts,
        SignalContext, SignalDecision, SignalEstimate, SignalSourceLedger, SignalSubmission,
        SignalTaskKey, SignalTiming, SpanSeconds, StockWarning, StockWarningDecision, Symbol,
        TimeInterval, TimeSpan, Timestamp, TradeCost, TradeTaxFact, TradingSuspension,
        UntrustedText, Volatility, calculate_sell_draft_quantity, calibrate_signal_submission,
        check_sellable_quantity, check_stock_warnings, sector_assignment_from_compositions,
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
                SecretKey::TossClientId => b"fake-t11-id".to_vec(),
                SecretKey::TossClientSecret => b"fake-t11-secret".to_vec(),
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
    struct FixedClock(Arc<AtomicI64>);
    impl FixedClock {
        fn at(raw: &str) -> Self {
            Self(Arc::new(AtomicI64::new(*ts(raw).utc_seconds().as_i64())))
        }
        fn set(&self, value: Timestamp) {
            self.0
                .store(*value.utc_seconds().as_i64(), Ordering::SeqCst);
        }
    }
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            Timestamp::from_parts(
                domain::UnixSeconds::new(self.0.load(Ordering::SeqCst)),
                Nanosecond::new(0),
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
            "HTTP/1.1 {status}\r\nRetry-After: 3\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
            text.len()
        )
        .into_bytes()
    }

    fn issued() -> Vec<u8> {
        reply(
            "200 OK",
            &json!({"access_token": "fake-access-t11", "token_type": "Bearer", "expires_in": 3600}),
        )
    }

    fn adapter_with_clock<C: Clock + Clone + 'static>(
        server: &ScriptedHttpServer,
        clock: C,
        waiter: Arc<crate::support::FakeTime>,
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
        TossAccountRead::new(
            tokens,
            clock,
            MasterInstrumentFacts::try_new(vec![]).expect("empty master"),
        )
    }

    fn adapter(
        server: &ScriptedHttpServer,
    ) -> (
        TossAccountRead<Store, SharedTime, Renewal>,
        Arc<crate::support::FakeTime>,
    ) {
        let clock = crate::support::time();
        let reader = adapter_with_clock(
            server,
            SharedTime(Arc::<crate::support::FakeTime>::clone(&clock)),
            Arc::<crate::support::FakeTime>::clone(&clock),
        );
        (reader, clock)
    }

    fn load_recording(dir: &str, name: &str) -> Value {
        serde_json::from_str(
            &fs::read_to_string(format!("tests/{dir}/{name}.json")).expect("recording file"),
        )
        .expect("recording JSON")
    }

    fn kr_master(symbol: &str) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("listing"),
            symbol: Symbol::try_new(symbol).expect("symbol"),
            instrument_id: InstrumentId::try_new("inst-kr-005930").expect("id"),
            name: DisplayInstrumentName::try_new("Synthetic KR").expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("code"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: ts("2026-03-03T09:00:00+09:00"),
        })
        .expect("kr master")
    }

    fn us_master(symbol: &str) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("listing"),
            symbol: Symbol::try_new(symbol).expect("symbol"),
            instrument_id: InstrumentId::try_new("inst-us-aapl").expect("id"),
            name: DisplayInstrumentName::try_new("Synthetic US").expect("name"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("FOREIGN_STOCK").expect("code"),
            ),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: ts("2026-03-03T09:00:00+09:00"),
        })
        .expect("us master")
    }

    fn kr_instrument(symbol: &str) -> ResolvedOrderInstrument {
        kr_master(symbol)
            .order_gate_facts()
            .expect("gate")
            .instrument_for(OrderSide::Sell)
            .expect("kr inst")
    }

    fn us_instrument(symbol: &str) -> ResolvedOrderInstrument {
        us_master(symbol)
            .order_gate_facts()
            .expect("gate")
            .instrument_for(OrderSide::Sell)
            .expect("us inst")
    }

    fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("num"), currency).expect("money")
    }

    fn qty(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("num")).expect("qty")
    }

    fn date(text: &str) -> domain::CalendarDate {
        TossDate::try_from(text).expect("date").into_domain()
    }

    fn ts(text: &str) -> Timestamp {
        TossTimestamp::try_from(text).expect("ts").into_domain()
    }

    #[test]
    fn t11_recordings_validate_against_openapi_and_capabilities_include_new_endpoints() {
        let spec = crate::support_tests::spec();
        for (dir, name) in [
            ("recordings", "t11-buying-power-krw"),
            ("recordings", "t11-buying-power-usd"),
            ("recordings", "t11-sellable-quantity-kr"),
            ("recordings", "t11-sellable-quantity-us"),
            ("recordings", "t11-commissions"),
            ("recordings", "t11-stock-warnings"),
            ("recordings", "t11-stock-warnings-empty"),
            ("order_recordings", "t11-buying-power-404-account"),
            ("order_recordings", "t11-sellable-404-stock"),
            ("order_recordings", "t11-warnings-404-stock"),
        ] {
            let rec = load_recording(dir, name);
            crate::support_tests::validation::validate_recording(&spec, &rec).expect(name);
        }
        assert!(
            crate::support_tests::recordings()
                .iter()
                .any(|item| item["path"] == "/api/v1/buying-power")
        );
        let sample = load_recording("recordings", "t11-buying-power-krw");
        let replay = crate::support_tests::serve_recordings(&sample);
        let replay_time = crate::support::time();
        let replay_client = TossHttpClient::new(
            &TossEndpoint::local(replay.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&replay_time),
            replay_time,
        )
        .expect("client");
        let replay_resp = run(replay_client.get(
            &broker_toss::ReadRequest::new(crate::support_tests::route(&sample)),
            &SecretValue::new(b"fake-t11-token".to_vec()),
        ))
        .expect("replay");
        assert_eq!(replay_resp.status(), 200);
        let _wire = replay.finish().expect("finish");
        let capabilities = TossAccountRead::<Store, SharedTime, Renewal>::capabilities();
        for cap in [
            BrokerCapability::SellableQuantity,
            BrokerCapability::OpenOrders,
            BrokerCapability::ClosedOrders,
            BrokerCapability::Order,
            BrokerCapability::StockWarnings,
            BrokerCapability::CommissionRate,
        ] {
            assert_eq!(
                capabilities.support_for(cap),
                CapabilitySupport::Supported,
                "{cap:?}"
            );
        }
    }

    #[test]
    fn buying_power_preserves_exact_krw_usd_negative_and_account_header() {
        let krw_rec = load_recording("recordings", "t11-buying-power-krw");
        let usd_rec = load_recording("recordings", "t11-buying-power-usd");
        let krw_trailing = json!({"result": {"currency": "KRW", "cashBuyingPower": "5000000.00"}});
        let neg_krw = json!({"result": {"currency": "KRW", "cashBuyingPower": "-12500"}});
        let neg_usd = json!({"result": {"currency": "USD", "cashBuyingPower": "-12.75"}});
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &krw_rec["body"]),
            reply("200 OK", &usd_rec["body"]),
            reply("200 OK", &krw_trailing),
            reply("200 OK", &neg_krw),
            reply("200 OK", &neg_usd),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-29T10:15:00+09:00");
        let reader = adapter_with_clock(&server, clock.clone(), crate::support::time());

        let krw = run(reader.buying_power(&AccountId::new(413), Currency::Krw)).expect("krw");
        assert_eq!(krw.currency(), Currency::Krw);
        assert_eq!(krw.money(), Ok(money("5000000", Currency::Krw)));
        assert_eq!(krw.observed_at(), ts("2026-03-29T10:15:00+09:00"));

        clock.set(ts("2026-03-29T10:16:00+09:00"));
        let usd = run(reader.buying_power(&AccountId::new(731), Currency::Usd)).expect("usd");
        assert_eq!(usd.currency(), Currency::Usd);
        assert_eq!(usd.money(), Ok(money("3500.525", Currency::Usd)));
        assert_eq!(usd.observed_at(), ts("2026-03-29T10:16:00+09:00"));

        let trailing = run(reader.buying_power(&AccountId::new(413), Currency::Krw)).expect("krw0");
        assert_eq!(trailing.money(), Ok(money("5000000.00", Currency::Krw)));

        let negative_krw =
            run(reader.buying_power(&AccountId::new(413), Currency::Krw)).expect("neg krw");
        assert_eq!(
            negative_krw.money(),
            Err(PortfolioError::NegativeBuyingPower)
        );
        let negative_usd =
            run(reader.buying_power(&AccountId::new(731), Currency::Usd)).expect("neg usd");
        assert_eq!(
            negative_usd.money(),
            Err(PortfolioError::NegativeBuyingPower)
        );

        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        assert!(wire.contains("GET /api/v1/buying-power?currency=KRW HTTP/1.1"));
        assert!(wire.contains("GET /api/v1/buying-power?currency=USD HTTP/1.1"));
        assert!(wire.contains("x-tossinvest-account: 413"));
        assert!(wire.contains("x-tossinvest-account: 731"));
    }

    fn buying_power_invalid_cases() -> Vec<(Currency, Value, ResponseMismatch)> {
        vec![
            (
                Currency::Krw,
                json!({"result": {"currency": "USD", "cashBuyingPower": "5000000"}}),
                ResponseMismatch::Validation,
            ),
            (
                Currency::Usd,
                json!({"result": {"currency": "KRW", "cashBuyingPower": "5000000"}}),
                ResponseMismatch::Validation,
            ),
            (
                Currency::Krw,
                json!({"result": {"currency": "EUR", "cashBuyingPower": "5000000"}}),
                ResponseMismatch::Validation,
            ),
            (
                Currency::Krw,
                json!({"result": {"currency": "KRW", "cashBuyingPower": "5000000.5"}}),
                ResponseMismatch::Validation,
            ),
            (
                Currency::Usd,
                json!({"result": {"currency": "USD", "cashBuyingPower": "1e3"}}),
                ResponseMismatch::Validation,
            ),
            (
                Currency::Usd,
                json!({"result": {"currency": "USD", "cashBuyingPower": "NaN"}}),
                ResponseMismatch::Validation,
            ),
            (
                Currency::Usd,
                json!({"result": {"currency": "USD", "cashBuyingPower": "1234567890123456789012345678901"}}),
                ResponseMismatch::Schema,
            ),
            (
                Currency::Krw,
                json!({"result": {"currency": "KRW", "cashBuyingPower": 5000_i32}}),
                ResponseMismatch::Schema,
            ),
            (
                Currency::Krw,
                json!({"result": {"currency": "KRW"}}),
                ResponseMismatch::Schema,
            ),
            (
                Currency::Krw,
                json!({"result": {"currency": "KRW", "cashBuyingPower": "100"}, "error": {"code": "x"}}),
                ResponseMismatch::Schema,
            ),
        ]
    }

    #[test]
    fn buying_power_rejects_currency_mismatch_krw_fraction_bad_numbers_and_maps_errors() {
        let err_404 = load_recording("order_recordings", "t11-buying-power-404-account");
        let cases = buying_power_invalid_cases();
        let mut responses = vec![issued()];
        for (_, body, _) in &cases {
            responses.push(reply("200 OK", body));
        }
        responses.push(reply("404 Not Found", &err_404["body"]));
        let limited = json!({"error": {"requestId": "bp-429", "code": "rate-limit-exceeded", "message": "limit"}});
        for _ in 0..4_u8 {
            responses.push(reply("429 Too Many Requests", &limited));
        }
        let server = ScriptedHttpServer::start(responses).expect("server");
        let (reader, _) = adapter(&server);
        for (currency, _, expected) in cases {
            let err = run(reader.buying_power(&AccountId::new(413), currency)).expect_err("reject");
            assert_eq!(err.endpoint(), ReadEndpoint::BuyingPower);
            assert_eq!(err.kind(), PortErrorKind::InvalidResponse(expected));
        }
        let not_found =
            run(reader.buying_power(&AccountId::new(413), Currency::Krw)).expect_err("404");
        assert_eq!(
            not_found.kind(),
            PortErrorKind::AccessDenied(AccessDenial::AccountNotFound)
        );
        let rate_limited =
            run(reader.buying_power(&AccountId::new(413), Currency::Krw)).expect_err("429");
        assert_eq!(
            rate_limited.kind(),
            PortErrorKind::Unavailable {
                cause: UnavailableCause::RateLimited,
                retry: RetryDelay::Known(
                    TimeSpan::try_new(SpanSeconds::new(3), Nanosecond::new(0)).expect("span")
                ),
            }
        );
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn sellable_quantity_binds_account_instrument_clock_and_feeds_d34() {
        let kr_rec = load_recording("recordings", "t11-sellable-quantity-kr");
        let us_rec = load_recording("recordings", "t11-sellable-quantity-us");
        let kr_zero = json!({"result": {"sellableQuantity": "0"}});
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &kr_rec["body"]),
            reply("200 OK", &us_rec["body"]),
            reply("200 OK", &kr_zero),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-03T09:05:00+09:00");
        let reader = adapter_with_clock(&server, clock, crate::support::time());
        let account = AccountId::new(413);
        let kr = kr_instrument("005930");
        let us = us_instrument("AAPL");

        let fact_kr = run(reader.sellable_quantity(&account, &kr)).expect("kr sellable");
        assert_eq!(fact_kr.account_id(), &account);
        assert_eq!(fact_kr.instrument(), &kr);
        assert_eq!(fact_kr.quantity(), qty("100"));
        assert_eq!(fact_kr.observed_at(), ts("2026-03-03T09:05:00+09:00"));
        assert_eq!(
            check_sellable_quantity(
                &fact_kr,
                &account,
                &kr,
                SellOrderQuantity::try_new(qty("50")).expect("qty")
            ),
            SellableQuantityDecision::Allowed
        );
        assert_eq!(
            check_sellable_quantity(
                &fact_kr,
                &AccountId::new(999),
                &kr,
                SellOrderQuantity::try_new(qty("50")).expect("qty")
            ),
            SellableQuantityDecision::DifferentFact
        );

        let fact_us = run(reader.sellable_quantity(&account, &us)).expect("us sellable");
        assert_eq!(fact_us.quantity(), qty("5.5125"));

        let fact_zero = run(reader.sellable_quantity(&account, &kr)).expect("zero sellable");
        assert_eq!(fact_zero.quantity(), qty("0"));

        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        assert!(wire.contains("GET /api/v1/sellable-quantity?symbol=005930 HTTP/1.1"));
        assert!(wire.contains("GET /api/v1/sellable-quantity?symbol=AAPL HTTP/1.1"));
        assert!(wire.contains("x-tossinvest-account: 413"));
    }

    #[test]
    fn sellable_quantity_fact_feeds_d57_sell_draft_quantity_directly() {
        let kr_dot_zero = json!({"result": {"sellableQuantity": "11.0"}});
        let server =
            ScriptedHttpServer::start(vec![issued(), reply("200 OK", &kr_dot_zero)]).expect("srv");
        let clock = FixedClock::at("2026-03-03T09:00:05+09:00");
        let reader = adapter_with_clock(&server, clock, crate::support::time());
        let account = AccountId::new(413);
        let master = kr_master("005930");
        let kr = master
            .order_gate_facts()
            .expect("gate")
            .instrument_for(OrderSide::Sell)
            .expect("kr");

        let fact = run(reader.sellable_quantity(&account, &kr)).expect("d57 sellable");
        let (signal, valuation, calendars, signal_at) = build_d57_sell_signal(&master, qty("17"));
        let calc_at = signal_at
            .checked_add(TimeSpan::try_new(SpanSeconds::new(10), Nanosecond::new(0)).expect("span"))
            .expect("later");
        let facts = SellDraftQuantityFacts::new(&account, &valuation, Some(&fact));
        let plan = calculate_sell_draft_quantity(&signal, &facts, calc_at, &calendars)
            .into_plan()
            .expect("d57 plan");
        assert_eq!(plan.account_id(), &account);
        assert_eq!(plan.instrument(), &kr);
        assert_eq!(plan.sellable_quantity().quantity(), qty("11"));
        assert_eq!(
            plan.constraint(),
            SellQuantityConstraint::SellableQuantityCap
        );
        assert_eq!(plan.sellable_observed_at(), ts("2026-03-03T09:00:05+09:00"));
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn sellable_quantity_rejects_fractional_kr_bad_values_and_mismatched_symbols() {
        let err_404 = load_recording("order_recordings", "t11-sellable-404-stock");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &json!({"result": {"sellableQuantity": "10.5"}})),
            reply("200 OK", &json!({"result": {"sellableQuantity": "-1"}})),
            reply("200 OK", &json!({"result": {"sellableQuantity": "1e2"}})),
            reply(
                "200 OK",
                &json!({"result": {"sellableQuantity": "1234567890123456789012345678901"}}),
            ),
            reply("200 OK", &json!({"result": {"sellableQuantity": 10_i32}})),
            reply("404 Not Found", &err_404["body"]),
        ])
        .expect("server");
        let (reader, _) = adapter(&server);
        let account = AccountId::new(413);
        let kr = kr_instrument("005930");
        let us = us_instrument("AAPL");

        for (inst, expected) in [
            (&kr, ResponseMismatch::Validation),
            (&us, ResponseMismatch::Validation),
            (&us, ResponseMismatch::Validation),
            (&us, ResponseMismatch::Schema),
            (&kr, ResponseMismatch::Schema),
        ] {
            let err = run(reader.sellable_quantity(&account, inst)).expect_err("reject");
            assert_eq!(err.kind(), PortErrorKind::InvalidResponse(expected));
        }
        let missing = run(reader.sellable_quantity(&account, &kr)).expect_err("404");
        assert_eq!(missing.kind(), PortErrorKind::NotFound(MissingData::Stock));
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn commission_rate_selects_market_preserves_validity_and_isolates_accounts() {
        let rec = load_recording("recordings", "t11-commissions");
        let other_account = json!({
            "result": [
                {"marketCountry": "KR", "commissionRate": "0.00025", "startDate": null, "endDate": null},
                {"marketCountry": "US", "commissionRate": "0.0025", "startDate": null, "endDate": null}
            ]
        });
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &rec["body"]),
            reply("200 OK", &rec["body"]),
            reply("200 OK", &other_account),
        ])
        .expect("server");
        let (reader, _) = adapter(&server);

        let kr_a = run(reader.commission_rate(&AccountId::new(413), KnownMarket::Kr)).expect("kr");
        assert_eq!(kr_a.market(), &Market::Kr);
        assert_eq!(
            kr_a.rate(),
            CostRate::try_new(
                Ratio::parse(&NumericInput::try_new("0.00015").expect("n")).expect("r")
            )
            .expect("rate")
        );
        assert_eq!(
            kr_a.validity(),
            CommissionValidity::try_new(Some(date("2026-01-01")), Some(date("2026-12-31")))
                .expect("val")
        );

        let us_a = run(reader.commission_rate(&AccountId::new(413), KnownMarket::Us)).expect("us");
        assert_eq!(us_a.market(), &Market::Us);
        assert_eq!(
            us_a.validity(),
            CommissionValidity::try_new(None, Some(date("2026-06-30"))).expect("val")
        );

        let kr_b =
            run(reader.commission_rate(&AccountId::new(731), KnownMarket::Kr)).expect("kr b");
        assert_eq!(
            kr_b.rate(),
            CostRate::try_new(
                Ratio::parse(&NumericInput::try_new("0.00025").expect("n")).expect("r")
            )
            .expect("rate")
        );
        assert_eq!(
            kr_b.validity(),
            CommissionValidity::try_new(None, None).expect("unlimited")
        );

        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        assert!(wire.contains("GET /api/v1/commissions HTTP/1.1"));
        assert!(wire.contains("x-tossinvest-account: 413"));
        assert!(wire.contains("x-tossinvest-account: 731"));
    }

    fn commission_invalid_cases() -> Vec<(KnownMarket, Value, ResponseMismatch)> {
        let row = |mc: &str, rate: &str| json!({"marketCountry": mc, "commissionRate": rate});
        vec![
            (
                KnownMarket::Kr,
                json!({"result": [row("US", "0.001")]}),
                ResponseMismatch::Validation,
            ),
            (
                KnownMarket::Kr,
                json!({"result": [row("JP", "0.001")]}),
                ResponseMismatch::Validation,
            ),
            (
                KnownMarket::Kr,
                json!({"result": [row("KR", "0.00015"), row("JP", "0.001")]}),
                ResponseMismatch::Validation,
            ),
            (
                KnownMarket::Kr,
                json!({"result": [row("KR", "0.00015"), row("KR", "0.0002")]}),
                ResponseMismatch::Validation,
            ),
            (
                KnownMarket::Us,
                json!({"result": [row("US", "0.001"), row("US", "0.002")]}),
                ResponseMismatch::Validation,
            ),
            (
                KnownMarket::Kr,
                json!({"result": [row("KR", "-0.0001")]}),
                ResponseMismatch::Validation,
            ),
            (
                KnownMarket::Kr,
                json!({"result": [row("KR", "1.0001")]}),
                ResponseMismatch::Validation,
            ),
            (
                KnownMarket::Kr,
                json!({"result": [{"marketCountry": "KR", "commissionRate": "0.00015", "startDate": "2026-12-31", "endDate": "2026-01-01"}]}),
                ResponseMismatch::Validation,
            ),
            (
                KnownMarket::Kr,
                json!({"result": [row("KR", "0.00015"), {"marketCountry": "US", "commissionRate": "0.001", "endDate": "2026-02-30"}]}),
                ResponseMismatch::Validation,
            ),
            (
                KnownMarket::Kr,
                json!({"result": {"not": "an array"}}),
                ResponseMismatch::Schema,
            ),
        ]
    }

    #[test]
    fn commission_rate_rejects_unknown_market_duplicates_invalid_rates_dates_and_missing_market() {
        let bad_cases = commission_invalid_cases();
        let mut responses = vec![issued()];
        for (_, body, _) in &bad_cases {
            responses.push(reply("200 OK", body));
        }
        let server = ScriptedHttpServer::start(responses).expect("server");
        let (reader, _) = adapter(&server);
        for (market, _, expected) in bad_cases {
            let err = run(reader.commission_rate(&AccountId::new(413), market)).expect_err("bad");
            assert_eq!(err.endpoint(), ReadEndpoint::CommissionRate);
            assert_eq!(err.kind(), PortErrorKind::InvalidResponse(expected));
        }
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn stock_warnings_preserves_known_and_unknown_without_account_header() {
        let rec = load_recording("recordings", "t11-stock-warnings");
        let empty = load_recording("recordings", "t11-stock-warnings-empty");
        let unknown_and_dup = json!({
            "result": [
                {"warningType": "LIQUIDATION_TRADING", "exchange": "KRX", "startDate": "2026-03-01", "endDate": "2026-03-10"},
                {"warningType": "LIQUIDATION_TRADING", "exchange": "NXT", "startDate": "2026-03-01", "endDate": "2026-03-10"},
                {"warningType": "NEW_EXCHANGE_ALERT", "exchange": null, "startDate": null, "endDate": null}
            ]
        });
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &rec["body"]),
            reply("200 OK", &empty["body"]),
            reply("200 OK", &unknown_and_dup),
        ])
        .expect("server");
        let clock = FixedClock::at("2026-03-29T11:00:00+09:00");
        let reader = adapter_with_clock(&server, clock, crate::support::time());
        let kr = kr_instrument("005930");

        let warnings = run(reader.stock_warnings(&kr)).expect("warnings");
        assert_eq!(warnings.instrument(), &kr);
        assert_eq!(warnings.observed_at(), ts("2026-03-29T11:00:00+09:00"));
        assert_eq!(
            warnings.warnings(),
            &[StockWarning::Overheated, StockWarning::ViStatic]
        );
        assert_eq!(
            check_stock_warnings(&warnings, &kr, OrderSide::Buy),
            StockWarningDecision::Blocked
        );
        assert_eq!(
            check_stock_warnings(&warnings, &kr, OrderSide::Sell),
            StockWarningDecision::NotApplicable
        );

        let clear = run(reader.stock_warnings(&kr)).expect("empty");
        assert_eq!(clear.warnings(), []);
        assert_eq!(
            check_stock_warnings(&clear, &kr, OrderSide::Buy),
            StockWarningDecision::Clear
        );

        let with_unknown = run(reader.stock_warnings(&kr)).expect("unknown");
        assert_eq!(
            with_unknown.warnings(),
            &[
                StockWarning::LiquidationTrading,
                StockWarning::Unknown(MasterCode::try_new("NEW_EXCHANGE_ALERT").expect("code")),
            ]
        );
        assert_eq!(
            check_stock_warnings(&with_unknown, &kr, OrderSide::Buy),
            StockWarningDecision::Blocked
        );

        let wire = String::from_utf8(server.finish().expect("finish")).expect("utf8");
        assert!(wire.contains("GET /api/v1/stocks/005930/warnings HTTP/1.1"));
        assert!(!wire.to_ascii_lowercase().contains("x-tossinvest-account"));
    }

    #[test]
    fn stock_warnings_rejects_bad_dates_exchange_codes_and_maps_404() {
        let err_404 = load_recording("order_recordings", "t11-warnings-404-stock");
        let bad_cases: Vec<(Value, ResponseMismatch)> = vec![
            (
                json!({"result": [{"warningType": ""}]}),
                ResponseMismatch::Validation,
            ),
            (
                json!({"result": [{"warningType": "OVERHEATED", "exchange": " "}]}),
                ResponseMismatch::Validation,
            ),
            (
                json!({"result": [{"warningType": "OVERHEATED", "startDate": "2026-03-28", "endDate": "2026-03-20"}]}),
                ResponseMismatch::Validation,
            ),
            (
                json!({"result": [{"warningType": "OVERHEATED", "startDate": "2026-13-01"}]}),
                ResponseMismatch::Validation,
            ),
            (
                json!({"result": [{"exchange": "KRX"}]}),
                ResponseMismatch::Schema,
            ),
        ];
        let mut responses = vec![issued()];
        for (body, _) in &bad_cases {
            responses.push(reply("200 OK", body));
        }
        responses.push(reply("404 Not Found", &err_404["body"]));
        let server = ScriptedHttpServer::start(responses).expect("server");
        let (reader, _) = adapter(&server);
        let kr = kr_instrument("005930");
        for (_, expected) in bad_cases {
            let err = run(reader.stock_warnings(&kr)).expect_err("bad");
            assert_eq!(err.endpoint(), ReadEndpoint::StockWarnings);
            assert_eq!(err.kind(), PortErrorKind::InvalidResponse(expected));
        }
        let missing = run(reader.stock_warnings(&kr)).expect_err("404");
        assert_eq!(missing.kind(), PortErrorKind::NotFound(MissingData::Stock));
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn stock_warnings_enforces_256_item_limit_boundary() {
        let entry = json!({
            "warningType": "VI_STATIC",
            "exchange": "KRX",
            "startDate": "2026-03-01",
            "endDate": "2026-03-31"
        });
        let at_limit = json!({"result": vec![entry.clone(); 256]});
        let over_limit = json!({"result": vec![entry; 257]});
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &at_limit),
            reply("200 OK", &over_limit),
        ])
        .expect("server");
        let (reader, _) = adapter(&server);
        let kr = kr_instrument("005930");
        let ok = run(reader.stock_warnings(&kr)).expect("256 allowed");
        assert_eq!(ok.warnings(), &[StockWarning::ViStatic]);
        let over = run(reader.stock_warnings(&kr)).expect_err("257 rejected");
        assert_eq!(over.endpoint(), ReadEndpoint::StockWarnings);
        assert_eq!(
            over.kind(),
            PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
        );
        let _wire = server.finish().expect("finish");
    }

    #[test]
    fn order_info_endpoints_share_three_per_second_rate_limit_group() {
        let bp = load_recording("recordings", "t11-buying-power-krw");
        let sq = load_recording("recordings", "t11-sellable-quantity-kr");
        let cm = load_recording("recordings", "t11-commissions");
        let sw = load_recording("recordings", "t11-stock-warnings-empty");
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply("200 OK", &bp["body"]),
            reply("200 OK", &sw["body"]),
            reply("200 OK", &sq["body"]),
            reply("200 OK", &cm["body"]),
            reply("200 OK", &bp["body"]),
        ])
        .expect("server");
        let (reader, clock) = adapter(&server);
        let start = clock.now();
        let account = AccountId::new(413);
        let kr = kr_instrument("005930");
        let step = TimeSpan::try_new(SpanSeconds::new(0), Nanosecond::new(555_555_556))
            .expect("order-info step");
        run(reader.buying_power(&account, Currency::Krw)).expect("1");
        assert_eq!(clock.now(), start);
        run(reader.stock_warnings(&kr)).expect("separate stock group");
        assert_eq!(clock.now(), start);
        run(reader.sellable_quantity(&account, &kr)).expect("2");
        assert_eq!(clock.now(), start.checked_add(step).expect("t1"));
        run(reader.commission_rate(&account, KnownMarket::Kr)).expect("3");
        assert_eq!(
            clock.now(),
            start
                .checked_add(step)
                .and_then(|first| first.checked_add(step))
                .expect("t2")
        );
        run(reader.buying_power(&account, Currency::Krw)).expect("4");
        assert!(clock.now() > start.checked_add(step).expect("after"));
        let _wire = server.finish().expect("finish");
    }

    fn day_offset(base_day: Timestamp, offset: i64) -> Timestamp {
        let seconds = offset.checked_mul(86_400).expect("sec");
        let span = TimeSpan::try_new(SpanSeconds::new(seconds.unsigned_abs()), Nanosecond::new(0))
            .expect("span");
        if seconds >= 0 {
            base_day.checked_add(span).expect("add")
        } else {
            base_day.checked_sub(span).expect("sub")
        }
    }

    fn kr_calendars(base_day: Timestamp) -> Vec<MarketCalendar> {
        let session_for = |offset: i64| {
            let start = day_offset(base_day, offset);
            let end = start
                .checked_add(
                    TimeSpan::try_new(SpanSeconds::new(3600), Nanosecond::new(0)).expect("h"),
                )
                .expect("end");
            MarketSession::try_new(
                start.offset_calendar_date(),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: Some(
                        SessionData::try_new(TimeInterval::try_new(start, end).expect("iv"), None)
                            .expect("sd"),
                    ),
                    after_market: None,
                },
            )
            .expect("ms")
        };
        (0_i64..=14)
            .map(|offset| {
                MarketCalendar::try_new(
                    session_for(offset.checked_sub(1).expect("prev")),
                    session_for(offset),
                    session_for(offset.checked_add(1).expect("next")),
                )
                .expect("cal")
            })
            .collect()
    }

    fn kr_candles(base_day: Timestamp) -> Vec<Candle> {
        let midnight = ts("2026-03-03T00:00:00+09:00");
        (0_i64..61)
            .map(|index| {
                let open = if index.wrapping_rem(2) == 0 {
                    "100"
                } else {
                    "102"
                };
                let close = if index.wrapping_rem(2) == 0 {
                    "102"
                } else {
                    "100"
                };
                let start = day_offset(midnight, index.checked_sub(62).expect("past"));
                Candle::try_new(
                    InstrumentId::try_new("inst-kr-005930").expect("id"),
                    CandleTime::try_new(start, CandlePeriod::Daily).expect("ct"),
                    PriceAdjustment::Adjusted(domain::AdjustmentBasis::new(base_day)),
                    Ohlcv::try_new(
                        Price::parse(&NumericInput::try_new(open).expect("n"), Currency::Krw)
                            .expect("o"),
                        Price::parse(&NumericInput::try_new("105").expect("n"), Currency::Krw)
                            .expect("h"),
                        Price::parse(&NumericInput::try_new("95").expect("n"), Currency::Krw)
                            .expect("l"),
                        Price::parse(&NumericInput::try_new(close).expect("n"), Currency::Krw)
                            .expect("c"),
                        qty("100"),
                    )
                    .expect("ohlcv"),
                    CandleObservation::new(base_day, CandleStatus::Finalized),
                )
                .expect("candle")
            })
            .collect()
    }

    fn d57_eligible(master: &MasterInstrument, base_day: Timestamp) -> CostEligibleTask {
        let volatility = Volatility::calculate(&kr_candles(base_day)).expect("vol");
        let reference = master
            .price_observation(
                Price::try_new(money("100", Currency::Krw)).expect("p"),
                base_day,
            )
            .expect("ref");
        let task = EstimateTask::create(
            EstimateContext::new(reference, base_day),
            volatility,
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("task");
        let zero =
            CostRate::try_new(Ratio::parse(&NumericInput::try_new("0").expect("n")).expect("r"))
                .expect("zero");
        let cost = TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                zero,
                CommissionValidity::try_new(None, None).expect("val"),
            )),
            Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
            EstimateCostPeriod::from_calendar_result(&task, day_offset(base_day, 10))
                .expect("period"),
        )
        .expect("cost");
        CostEligibleTask::assess(task, &cost).expect("eligible")
    }

    fn d57_estimate(
        master: &MasterInstrument,
        base_day: Timestamp,
    ) -> (SignalEstimate, CollectedItem, CalibrationLedgerHead) {
        let eligible = d57_eligible(master, base_day);
        let part = ModelIdentityPart::try_new("t11").expect("part");
        let key = CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::new(HorizonSelection::FewDays),
        );
        let item = CollectedItem::from_web(
            NormalizedUrl::try_new("https://example.com/t11").expect("url"),
            UntrustedText::try_new("근거").expect("title"),
            None,
        );
        let evidence = Some(
            Evidence::try_new(vec![EvidenceRef::Web(
                EvidenceUrl::try_new(item.url().clone()).expect("url"),
            )])
            .expect("ev"),
        );
        let submission = SignalSubmission::create(
            SignalTaskKey::from_task(&eligible),
            key.clone(),
            AiProbability::parse(&AiProbabilityInput::try_new("1").expect("p")).expect("prob"),
            evidence,
            base_day,
        )
        .expect("sub");
        let ledger_head = CalibrationLedgerHead::new(key.clone(), CalibrationPosition::new(0));
        let snapshot = CalibrationSnapshot::try_new(
            Calibration::try_new(key, &[]).expect("cal"),
            ledger_head.clone(),
        )
        .expect("snap");
        let calibrated =
            calibrate_signal_submission(Some(&snapshot), &ledger_head, submission).expect("cal");
        let estimate = SignalEstimate::create(
            &eligible,
            &calibrated,
            Margin::for_appetite(RiskAppetite::Neutral),
        )
        .expect("est");
        (estimate, item, ledger_head)
    }

    fn build_d57_sell_signal(
        master: &MasterInstrument,
        held_qty: Quantity,
    ) -> (
        SellSignal,
        PortfolioValuation,
        Vec<MarketCalendar>,
        Timestamp,
    ) {
        let base_day = ts("2026-03-03T09:00:00+09:00");
        let calendars = kr_calendars(base_day);
        let (estimate, item, ledger_head) = d57_estimate(master, base_day);
        let instrument = master
            .order_gate_facts()
            .expect("gate")
            .instrument_for(OrderSide::Sell)
            .expect("inst");
        let mid = FxRate::parse(
            &NumericInput::try_new("1300").expect("fx"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(day_offset(base_day, -1), day_offset(base_day, 10)).expect("iv"),
            FxSource::TossOpenApi,
        )
        .expect("fx");
        let holding = Holding::new(
            instrument,
            held_qty,
            AcquisitionPrice::try_new(money("100", Currency::Krw)).expect("acq"),
            HoldingObservation::new(
                Some(Price::try_new(money("100", Currency::Krw)).expect("p")),
                None,
                base_day,
            ),
        )
        .expect("holding");
        let valuation = Portfolio::new(
            vec![holding],
            BuyingPower::new(money("10000", Currency::Krw), base_day),
            BuyingPower::new(money("1000", Currency::Usd), base_day),
        )
        .expect("port")
        .evaluate(Some(mid));
        let items = [item];
        let ledger = SignalSourceLedger::new(&items, &[], &[]);
        let policy = Policy::new(InvestorProfile::default());
        let catalog = SectorCatalog::from_response(vec![], base_day).expect("catalog");
        let assignment =
            sector_assignment_from_compositions(master, &catalog, &[]).expect("assign");
        let assignments = SectorAssignments::try_new(vec![assignment]).expect("assigns");
        let security = MasterInstrumentFacts::try_new(vec![master.clone()])
            .expect("facts")
            .policy_security_facts();
        let context = SignalContext::new(
            Some(&valuation),
            &policy,
            (&assignments, &security),
            &ledger,
            SignalAssessmentFacts::new(
                SignalTiming::new(base_day, &calendars),
                Some(estimate.task().reference().clone()),
                Some(ledger_head),
            ),
        );
        let sell = match SignalDecision::assess(&estimate, &context) {
            SignalDecision::Sell(signal) => signal,
            SignalDecision::Buy(_) | SignalDecision::None(_) => panic!("expected sell signal"),
        };
        (sell, valuation, calendars, base_day)
    }
}
