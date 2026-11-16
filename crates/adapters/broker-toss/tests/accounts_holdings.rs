mod support;
#[cfg(test)]
mod support_tests;
#[cfg(test)]
mod tests {
    use application::{
        BrokerRead, Clock, EventBus, PortErrorKind, ResponseMismatch, SecretKey, SecretStore,
        SecretValue, SystemPortError, WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
    };
    use domain::{
        AccountId, BrokerAccountKind, CommonShare, Currency, DisplayInstrumentName, Holding,
        InstrumentId, KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode,
        MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, Money,
        NumericInput, NxtTrading, Price, Quantity, Ratio, Symbol, Timestamp, TradingSuspension,
    };
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
    fn recording(name: &str) -> Value {
        let directory = if name == "accounts-empty" {
            "account_recordings"
        } else {
            "recordings"
        };
        serde_json::from_str(
            &std::fs::read_to_string(format!("tests/{directory}/t05-{name}.json"))
                .expect("recording"),
        )
        .expect("JSON")
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
    fn master(now: Timestamp) -> MasterInstrumentFacts {
        MasterInstrumentFacts::try_new(
            [
                ("005930", "KOSPI", Currency::Krw),
                ("AAPL", "NASDAQ", Currency::Usd),
            ]
            .map(|(symbol, market, currency)| {
                MasterInstrument::try_new(MasterInstrumentRow {
                    listing_market: MasterCode::try_new(market).expect("market"),
                    symbol: Symbol::try_new(symbol).expect("symbol"),
                    instrument_id: InstrumentId::try_new(&format!("synthetic-{symbol}"))
                        .expect("id"),
                    name: DisplayInstrumentName::try_new("합성 종목").expect("name"),
                    security_type: MasterSecurityType::from_code(
                        MasterCode::try_new("STOCK").expect("type"),
                    ),
                    currency,
                    listing_status: ListingStatus::Active,
                    common_share: CommonShare::Common,
                    korean_detail: (currency == Currency::Krw).then_some(KoreanMasterDetail {
                        liquidation: LiquidationTrading::Regular,
                        krx: TradingSuspension::Clear,
                        nxt: NxtTrading::Unsupported,
                    }),
                    observed_at: now,
                })
                .expect("master")
            })
            .to_vec(),
        )
        .expect("facts")
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
            Time(Arc::<crate::support::FakeTime>::clone(&clock)),
            master(clock.now()),
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
    fn all_account_recordings_match_the_current_spec() {
        for name in ["accounts", "accounts-empty", "holdings", "holdings-empty"] {
            crate::support_tests::validation::validate_recording(
                &crate::support_tests::spec(),
                &recording(name),
            )
            .expect("T-20 contract");
        }
        assert_eq!(
            TossAccountRead::<Store, Time, Renewal>::capabilities()
                .support_for(application::BrokerCapability::Instruments),
            application::CapabilitySupport::Unsupported
        );
    }
    #[test]
    fn empty_account_recording_replays_through_t20_support() {
        let empty = recording("accounts-empty");
        let server = crate::support_tests::serve_recordings(&empty);
        let clock = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&clock),
            clock,
        )
        .expect("client");
        let request = broker_toss::ReadRequest::new(crate::support_tests::route(&empty));
        let received =
            run(client.get(&request, &SecretValue::new(b"fake-test-91".to_vec()))).expect("GET");
        assert_eq!(received.status(), 200);
        assert_eq!(
            serde_json::from_slice::<Value>(received.body().as_exposed_secret()).expect("JSON"),
            empty["body"]
        );
        assert!(
            String::from_utf8(server.finish().expect("finish"))
                .expect("request")
                .starts_with("GET /api/v1/accounts HTTP/1.1")
        );
        assert!(
            crate::support_tests::recordings()
                .iter()
                .any(|item| item["path"] == "/api/v1/holdings")
        );
    }
    #[test]
    fn accounts_preserve_key_kind_mask_order_and_time() {
        let mut body = recording("accounts")["body"].clone();
        body["result"].as_array_mut().expect("rows").push(
            json!({"accountSeq":-27_i64,"accountNo":"00000000419","accountType":"PENSION_SAVINGS"}),
        );
        let server =
            ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)]).expect("server");
        let reader = adapter(&server);
        let result = run(BrokerRead::accounts(&reader)).expect("accounts");
        assert_eq!(result.rows().len(), 2);
        assert_eq!(result.rows()[0].account(), &AccountId::new(413));
        assert_eq!(result.rows()[1].account(), &AccountId::new(-27));
        assert_eq!(result.rows()[0].kind(), &BrokerAccountKind::Brokerage);
        assert_eq!(result.rows()[1].kind(), &BrokerAccountKind::PensionSavings);
        assert!(format!("{:?}", result.rows()[0].display_fact()).contains("0731"));
        assert!(!format!("{result:?}").contains("00000000731"));
        assert_eq!(result.rows()[0].observed_at(), crate::support::time().now());
        let requests = String::from_utf8(server.finish().expect("finish")).expect("requests");
        assert!(requests.contains("GET /api/v1/accounts HTTP/1.1"));
        assert!(
            !requests
                .to_ascii_lowercase()
                .contains("x-tossinvest-account")
        );
    }
    #[test]
    fn holdings_preserve_each_distinct_field_and_request_account() {
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("holdings")["body"]),
        ])
        .expect("server");
        let result = run(BrokerRead::holdings(
            &adapter(&server),
            &AccountId::new(413),
        ))
        .expect("holdings");
        assert_eq!(result.holdings().len(), 2);
        check_rows(&result);
        check_summary(&result);
        let requests = String::from_utf8(server.finish().expect("finish"))
            .expect("requests")
            .to_ascii_lowercase();
        assert!(requests.contains("get /api/v1/holdings http/1.1"));
        assert!(requests.contains("x-tossinvest-account: 413\r\n"));
    }
    #[test]
    fn empty_recordings_remain_empty_successes() {
        for name in ["accounts-empty", "holdings-empty"] {
            let server = ScriptedHttpServer::start(vec![
                issued(),
                response("200 OK", &recording(name)["body"]),
            ])
            .expect("server");
            let reader = adapter(&server);
            if name == "accounts-empty" {
                assert_eq!(run(reader.accounts()).expect("empty").rows(), []);
            } else {
                let result = run(reader.holdings(&AccountId::new(413))).expect("empty");
                assert_eq!(result.holdings(), []);
                assert_eq!(result.reported().valuation().amount(Currency::Usd), None);
            }
            server.finish().expect("finish");
        }
    }
    #[test]
    fn malformed_fields_fail_the_whole_response() {
        for (name, pointer, replacement) in [
            ("accounts", "/result/0/accountType", json!("FUTURE")),
            ("accounts", "/result/0/accountSeq", Value::Null),
            ("accounts", "/result/0/accountNo", json!("bad")),
            ("holdings", "/result/items/0/currency", json!("EUR")),
            ("holdings", "/result/items/0/marketCountry", json!("EU")),
            ("holdings", "/result/items/0/quantity", json!("-1")),
            (
                "holdings",
                "/result/items/0/quantity",
                json!("0.00000000000000000000000000001"),
            ),
            ("holdings", "/result/items/0/lastPrice", json!("0")),
            (
                "holdings",
                "/result/items/0/averagePurchasePrice",
                json!("1e3"),
            ),
            (
                "holdings",
                "/result/items/0/marketValue/amount",
                json!("bad"),
            ),
            ("holdings", "/result/items/0/cost/commission", json!("bad")),
            ("holdings", "/result/items/0/symbol", json!("999999")),
            ("holdings", "/result/items/1/currency", Value::Null),
            ("holdings", "/result/totalPurchaseAmount/krw", json!("bad")),
        ] {
            let mut body = recording(name)["body"].clone();
            *body.pointer_mut(pointer).expect("pointer") = replacement;
            let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)])
                .expect("server");
            let reader = adapter(&server);
            let error = if name == "accounts" {
                run(reader.accounts()).expect_err("invalid")
            } else {
                run(reader.holdings(&AccountId::new(413))).expect_err("invalid")
            };
            assert!(
                error.kind() == PortErrorKind::InvalidResponse(ResponseMismatch::Schema)
                    || error.kind() == PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
                "{pointer}: {error:?}"
            );
            server.finish().expect("finish");
        }
    }
    #[test]
    fn revoked_token_and_rate_limit_keep_the_existing_paths() {
        for name in ["accounts", "holdings"] {
            let server = ScriptedHttpServer::start(vec![
                issued(),
                response(
                    "401 Unauthorized",
                    &json!({"error":{"code":"token-revoked"}}),
                ),
                issued(),
                response(
                    "429 Too Many Requests",
                    &json!({"error":{"code":"too-many-requests"}}),
                ),
                response("200 OK", &recording(name)["body"]),
            ])
            .expect("server");
            let reader = adapter(&server);
            if name == "accounts" {
                run(reader.accounts()).expect("accounts");
            } else {
                run(reader.holdings(&AccountId::new(413))).expect("holdings");
            }
            let requests = String::from_utf8(server.finish().expect("finish")).expect("requests");
            assert_eq!(requests.matches("POST /oauth2/token ").count(), 2);
            assert_eq!(requests.matches("GET /api/v1/").count(), 3);
        }
    }
    #[test]
    fn missing_required_fields_and_duplicates_never_become_empty_success() {
        for name in ["accounts", "holdings"] {
            let original = recording(name)["body"].clone();
            let mut missing = original.clone();
            missing.as_object_mut().expect("object").remove("result");
            let mut duplicate = original.clone();
            let pointer = if name == "accounts" {
                "/result"
            } else {
                "/result/items"
            };
            let rows = duplicate
                .pointer_mut(pointer)
                .expect("rows")
                .as_array_mut()
                .expect("array");
            rows.push(rows[0].clone());
            let mut missing_field = original;
            let row_pointer = if name == "accounts" {
                "/result/0"
            } else {
                "/result/items/0"
            };
            let key = if name == "accounts" {
                "accountSeq"
            } else {
                "currency"
            };
            missing_field
                .pointer_mut(row_pointer)
                .expect("row")
                .as_object_mut()
                .expect("object")
                .remove(key);
            for body in [
                missing,
                duplicate,
                missing_field,
                json!({"result":null}),
                json!({"result":[],"error":{"code":"unknown"}}),
            ] {
                let server = ScriptedHttpServer::start(vec![issued(), response("200 OK", &body)])
                    .expect("server");
                let reader = adapter(&server);
                reject_body(name, &reader);
                server.finish().expect("finish");
            }
        }
    }
    #[test]
    fn unresolved_and_suspended_master_fail_without_dropping_rows() {
        let clock = crate::support::time();
        let mut rows = master(clock.now())
            .instruments()
            .iter()
            .map(|item| item.row().clone())
            .collect::<Vec<_>>();
        rows[0].korean_detail.as_mut().expect("KR detail").krx = TradingSuspension::Suspended;
        let suspended = MasterInstrumentFacts::try_new(
            rows.into_iter()
                .map(|row| MasterInstrument::try_new(row).expect("master"))
                .collect(),
        )
        .expect("facts");
        for facts in [
            suspended,
            MasterInstrumentFacts::try_new(vec![]).expect("empty master"),
        ] {
            let server = ScriptedHttpServer::start(vec![
                issued(),
                response("200 OK", &recording("holdings")["body"]),
            ])
            .expect("server");
            let client = TossHttpClient::new(
                &TossEndpoint::local(server.port()).expect("local"),
                Arc::<crate::support::FakeTime>::clone(&clock),
                Arc::<crate::support::FakeTime>::clone(&clock),
            )
            .expect("client");
            let reader = TossAccountRead::new(
                TokenManager::new(
                    client,
                    Store,
                    Time(Arc::clone(&clock)),
                    Renewal,
                    Arc::new(EventBus::default()),
                ),
                Time(Arc::clone(&clock)),
                facts,
            );
            assert_eq!(
                run(reader.holdings(&AccountId::new(413)))
                    .expect_err("unresolved")
                    .kind(),
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation)
            );
            server.finish().expect("finish");
        }
    }
    #[test]
    fn liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate() {
        let clock = crate::support::time();
        let rows = master(clock.now())
            .instruments()
            .iter()
            .map(|item| {
                let mut row = item.row().clone();
                if let Some(detail) = row.korean_detail.as_mut() {
                    detail.liquidation = LiquidationTrading::Liquidation;
                }
                MasterInstrument::try_new(row).expect("master")
            })
            .collect();
        let server = ScriptedHttpServer::start(vec![
            issued(),
            response("200 OK", &recording("holdings")["body"]),
        ])
        .expect("server");
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&clock),
            Arc::<crate::support::FakeTime>::clone(&clock),
        )
        .expect("client");
        let reader = TossAccountRead::new(
            TokenManager::new(
                client,
                Store,
                Time(Arc::clone(&clock)),
                Renewal,
                Arc::new(EventBus::default()),
            ),
            Time(Arc::clone(&clock)),
            MasterInstrumentFacts::try_new(rows).expect("facts"),
        );
        let result = run(reader.holdings(&AccountId::new(413))).expect("sell identity");
        assert_eq!(result.holdings().len(), 2);
        assert_eq!(
            result.reported().return_rate().expect("return").rate(),
            domain::HoldingReturn::Defined(domain::RatioQuotient::Exact(
                Ratio::parse(&NumericInput::try_new("0.1179").expect("input")).expect("rate")
            ))
        );
        assert_eq!(
            result.reported().return_rate().expect("return").basis(),
            domain::ReturnBasis::KrwConverted
        );
        server.finish().expect("finish");
    }
    fn reject_body(name: &str, reader: &TossAccountRead<Store, Time, Renewal>) {
        if name == "accounts" {
            run(reader.accounts()).expect_err("invalid");
        } else {
            run(reader.holdings(&AccountId::new(413))).expect_err("invalid");
        }
    }
    fn check_rows(result: &application::HoldingsObservation) {
        for (row, expected) in result.holdings().iter().zip([
            json!([
                "005930", "KRW", "100", "72000", "65000", "7200000", "700000", "0.1077", "100000",
                "0.0141"
            ]),
            json!([
                "AAPL", "USD", "10", "178.5", "155.3", "1785", "232", "0.1494", "25", "0.0142"
            ]),
        ]) {
            check_row(row, &expected);
        }
    }
    fn check_row(row: &Holding, expected: &Value) {
        let text = |index| expected[index].as_str().expect("expected text");
        let input = |index| NumericInput::try_new(text(index)).expect("input");
        let currency = if text(1) == "KRW" {
            Currency::Krw
        } else {
            Currency::Usd
        };
        assert_eq!(row.instrument().symbol().as_str(), text(0));
        assert_eq!(row.currency(), currency);
        assert_eq!(
            row.quantity(),
            Quantity::parse(&input(2)).expect("quantity")
        );
        assert_eq!(
            row.current_price(),
            Some(Price::parse(&input(3), currency).expect("price"))
        );
        assert_eq!(
            row.acquisition_price().money(),
            Money::parse(&input(4), currency).expect("acquisition")
        );
        let reported = row.reported().expect("reported");
        assert_eq!(
            reported.valuation(),
            Money::parse(&input(5), currency).expect("valuation")
        );
        assert_eq!(
            reported.profit(),
            Money::parse(&input(6), currency).expect("profit")
        );
        assert_eq!(
            reported.return_rate(),
            Ratio::parse(&input(7)).expect("rate")
        );
        assert_eq!(
            reported.daily_profit_loss().expect("daily").amount(),
            Money::parse(&input(8), currency).expect("daily")
        );
        assert_eq!(
            reported.daily_profit_loss().expect("daily").rate(),
            Ratio::parse(&input(9)).expect("daily rate")
        );
        assert_eq!(row.observed_at(), crate::support::time().now());
    }
    fn check_summary(result: &application::HoldingsObservation) {
        let summary = result.reported();
        for (currency, purchase, valuation, profit, daily) in [
            (Currency::Krw, "6500000", "7200000", "700000", "100000"),
            (Currency::Usd, "1553", "1785", "232", "25"),
        ] {
            let money = |text| {
                Money::parse(&NumericInput::try_new(text).expect("input"), currency).expect("money")
            };
            assert_eq!(summary.purchase().amount(currency), Some(money(purchase)));
            assert_eq!(summary.valuation().amount(currency), Some(money(valuation)));
            assert_eq!(summary.profit().amount(currency), Some(money(profit)));
            assert_eq!(
                summary
                    .daily_profit_loss()
                    .expect("daily")
                    .amounts()
                    .amount(currency),
                Some(money(daily))
            );
        }
        assert_eq!(
            summary.daily_profit_loss().expect("daily").rate(),
            Ratio::parse(&NumericInput::try_new("0.0141").expect("input")).expect("rate")
        );
        assert_eq!(summary.observed_at(), crate::support::time().now());
    }
}
