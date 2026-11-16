mod support;
#[cfg(test)]
mod tests {
    use application::{
        BrokerRead, Clock, EventBus, SecretKey, SecretStore, SecretValue, SystemPortError,
        WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
    };
    use domain::{
        AccountId, MasterInstrumentFacts, SettlementWindow, Timestamp, TransactionFilter,
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
            Time(Arc::<crate::support::FakeTime>::clone(&clock)),
            MasterInstrumentFacts::try_new(vec![]).expect("master"),
        )
    }
    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }

    fn date(value: &str) -> domain::CalendarDate {
        broker_toss::TossDate::try_from(value)
            .expect("date")
            .into_domain()
    }
    fn window() -> SettlementWindow {
        SettlementWindow::try_new(date("2026-09-01"), date("2026-09-30")).expect("window")
    }
    fn page(mut rows: Value, mut cursor: Value) -> Value {
        json!({"result":{"transactions":rows.take(),"hasNext":!cursor.is_null(),"nextCursor":cursor.take()}})
    }
    fn row() -> Value {
        json!({"transactionType":"DEPOSIT","marketCountry":"KR","currency":"KRW",
        "symbol":null,"assetClass":null,"isinCode":null,"quantity":null,"filledPrice":null,
        "amount":"731","commission":"13","tax":"17","netAmount":"701","exchangeRate":null,
        "settlementDate":"2026-09-18","settled":true,"transactionTime":null,"isCancellation":false})
    }
    fn complete(bodies: Vec<Value>) -> domain::CompleteTransactionResponse {
        let mut responses = vec![issued()];
        responses.extend(bodies.into_iter().map(|body| response("200 OK", &body)));
        let server = ScriptedHttpServer::start(responses).expect("server");
        let outcome_1 = run(adapter(&server).transactions(
            &AccountId::new(731),
            window(),
            &TransactionFilter::all(),
        ))
        .expect("read");
        match outcome_1 {
            application::TransactionRead::Complete(completed) => completed,
            application::TransactionRead::Incomplete(_) => panic!("incomplete"),
        }
    }
    #[test]
    fn one_page_preserves_distinct_money_fields() {
        let outcome_2 = complete(vec![page(json!([row(), row()]), Value::Null)]);
        assert_eq!(outcome_2.rows().as_slice().len(), 2);
        let data = outcome_2.rows().as_slice()[0].data();
        let money = |text| {
            domain::Money::parse(
                &domain::NumericInput::try_new(text).expect("numeric"),
                domain::Currency::Krw,
            )
            .expect("money")
        };
        assert_eq!(data.amount, Some(money("731")));
        assert_eq!(data.commission, money("13"));
        assert_eq!(data.tax, money("17"));
        assert_eq!(data.net_amount, Some(money("701")));
    }
    #[test]
    fn empty_and_short_pages_continue_until_last_cursor() {
        assert_eq!(
            complete(vec![page(json!([]), Value::Null)])
                .rows()
                .as_slice(),
            []
        );
        let outcome_3 = complete(vec![
            page(json!([row()]), json!("opaque_1")),
            page(json!([]), json!("opaque_2")),
            page(json!([row()]), Value::Null),
        ]);
        assert_eq!(outcome_3.rows().as_slice().len(), 2);
    }

    fn read_responses(
        responses: Vec<Vec<u8>>,
    ) -> (
        Result<application::TransactionRead, application::PortError>,
        String,
    ) {
        let mut scripted = vec![issued()];
        scripted.extend(responses);
        let server = ScriptedHttpServer::start(scripted).expect("server");
        let outcome_4 = run(adapter(&server).transactions(
            &AccountId::new(731),
            window(),
            &TransactionFilter::all(),
        ));
        let requests = String::from_utf8(server.finish().expect("requests")).expect("UTF8");
        (outcome_4, requests)
    }
    fn ok(mut body: Value) -> Vec<u8> {
        response("200 OK", &body.take())
    }
    fn cursor_error() -> Vec<u8> {
        response(
            "400 Bad Request",
            &json!({"error":{"code":"invalid-request","message":"합성 오류"}}),
        )
    }
    #[test]
    fn cursor_restart_discards_first_attempt_and_reuses_scope() {
        let (outcome_5, requests) = read_responses(vec![
            ok(page(json!([row()]), json!("first_1"))),
            cursor_error(),
            ok(page(json!([]), json!("first_1"))),
            ok(page(json!([row()]), Value::Null)),
        ]);
        let application::TransactionRead::Complete(completed) = outcome_5.expect("restart") else {
            panic!("incomplete")
        };
        assert_eq!(completed.rows().as_slice().len(), 1);
        assert_eq!(requests.matches("cursor=first_1").count(), 2);
        assert_eq!(
            requests
                .matches("GET /api/v1/transactions?limit=100 HTTP")
                .count(),
            2
        );
        assert_eq!(
            requests
                .to_ascii_lowercase()
                .matches("x-tossinvest-account: 731")
                .count(),
            4
        );
    }
    #[test]
    fn cursor_restart_is_bounded_and_repeat_is_rejected() {
        let (outcome_6, _) = read_responses(vec![
            ok(page(json!([]), json!("c1"))),
            cursor_error(),
            ok(page(json!([]), json!("c2"))),
            cursor_error(),
        ]);
        outcome_6.expect_err("closed failure");
        let (outcome_7, _) = read_responses(vec![
            ok(page(json!([]), json!("c1"))),
            ok(page(json!([]), json!("c1"))),
        ]);
        outcome_7.expect_err("closed failure");
        let (outcome_8, _) = read_responses(vec![cursor_error()]);
        outcome_8.expect_err("closed failure");
    }
    #[test]
    fn malformed_money_and_inconsistent_paging_fail_closed() {
        for field in [
            "amount",
            "commission",
            "tax",
            "netAmount",
            "quantity",
            "filledPrice",
            "exchangeRate",
        ] {
            let mut raw = row();
            raw[field] = json!("1e3");
            let (outcome_9, _) = read_responses(vec![ok(page(json!([raw]), Value::Null))]);
            outcome_9.expect_err(field);
        }
        for body in [
            json!({"result":{"transactions":[],"hasNext":true,"nextCursor":null}}),
            json!({"result":{"transactions":[],"hasNext":false,"nextCursor":"cursor"}}),
            json!({"result":{"transactions":[],"hasNext":true,"nextCursor":""}}),
        ] {
            let rejected = read_responses(vec![ok(body)]).0.expect_err("paging");
            assert_eq!(rejected.status(), application::HttpStatus::Success);
            assert_eq!(
                rejected.kind(),
                application::PortErrorKind::InvalidResponse(
                    application::ResponseMismatch::Validation
                )
            );
        }
    }
    #[test]
    fn unknown_currency_is_quarantined_and_pending_outside_window_is_received() {
        let mut raw = row();
        raw["currency"] = json!("EUR");
        let (outcome_10, _) = read_responses(vec![ok(page(json!([raw]), Value::Null))]);
        let application::TransactionRead::Incomplete(partial) = outcome_10.expect("quarantine")
        else {
            panic!("complete")
        };
        assert_eq!(
            partial.reason,
            application::IncompleteReason::QuarantinedRows
        );
        assert_eq!(partial.quarantined.len(), 1);
        let mut pending = row();
        pending["settled"] = json!(false);
        pending["settlementDate"] = json!("2027-01-01");
        let mut old = row();
        old["settlementDate"] = json!("2025-01-01");
        let outcome_11 = complete(vec![page(json!([pending, old]), Value::Null)]);
        assert_eq!(outcome_11.rows().as_slice().len(), 1);
        assert_eq!(
            outcome_11.rows().as_slice()[0].data().settlement,
            domain::Settlement::Pending
        );
    }
    #[test]
    fn authentication_and_rate_limit_preserve_existing_paths() {
        let denied = response(
            "401 Unauthorized",
            &json!({"error":{"code":"invalid-token","message":"합성"}}),
        );
        let (outcome_12, _) = read_responses(vec![denied.clone(), issued(), denied]);
        outcome_12.expect_err("closed failure");
        let limited = response(
            "429 Too Many Requests",
            &json!({"error":{"code":"rate-limit-exceeded","message":"합성"}}),
        );
        let (outcome_13, _) = read_responses(vec![limited; 4]);
        let rate_error = outcome_13.expect_err("rate limited");
        assert_eq!(
            rate_error.endpoint(),
            application::ReadEndpoint::Transactions
        );
        assert_eq!(rate_error.status(), application::HttpStatus::RateLimited);
        assert_eq!(
            rate_error.kind(),
            application::PortErrorKind::Unavailable {
                cause: application::UnavailableCause::RateLimited,
                retry: application::RetryDelay::Known(
                    domain::TimeSpan::try_new(
                        domain::SpanSeconds::new(2),
                        domain::Nanosecond::new(0)
                    )
                    .expect("span")
                )
            }
        );
    }

    #[test]
    fn synthetic_openapi_examples_are_converted_through_the_port() {
        for name in [
            "trades",
            "unsettledAndSettled",
            "cash",
            "dividendAndInterest",
            "transfer",
            "empty",
        ] {
            let recording: Value = serde_json::from_str(
                &std::fs::read_to_string(format!("tests/recordings/t06-{name}.json"))
                    .expect("recording"),
            )
            .expect("JSON");
            let body = recording.get("body").expect("body").clone();
            let has_next = body
                .pointer("/result/hasNext")
                .and_then(Value::as_bool)
                .expect("hasNext");
            let mut scripted = vec![ok(body)];
            if has_next {
                scripted.push(ok(page(json!([]), Value::Null)));
            }
            let (example_result, _) = read_responses(scripted);
            example_result.expect("example");
        }
    }

    #[test]
    fn every_transaction_field_is_preserved_independently() {
        let raw = json!({"transactionType":"SELL","marketCountry":"US","currency":"USD","symbol":"AAPL","assetClass":"STOCK","isinCode":"US0378331005","quantity":"3","filledPrice":"181.23","amount":"543.69","commission":"0.73","tax":"1.19","netAmount":"541.77","exchangeRate":"1387.41","settlementDate":"2026-09-19","settled":true,"transactionTime":"2026-09-19T06:31:17+09:00","isCancellation":true});
        let completed = complete(vec![page(json!([raw]), Value::Null)]);
        let expected_money = |text| {
            domain::Money::parse(
                &domain::NumericInput::try_new(text).expect("numeric"),
                domain::Currency::Usd,
            )
            .expect("money")
        };
        let expected = domain::Transaction::try_new(domain::TransactionData {
            kind: domain::TransactionKind::Sell,
            market: domain::Market::Us,
            currency: domain::Currency::Usd,
            symbol: Some(domain::Symbol::try_new("AAPL").expect("symbol")),
            asset_class: Some(domain::AssetClass::Stock),
            isin: Some(domain::IsinCode::try_new("US0378331005").expect("isin")),
            quantity: Some(
                domain::Quantity::parse(&domain::NumericInput::try_new("3").expect("numeric"))
                    .expect("quantity"),
            ),
            filled_price: Some(
                domain::TransactionPrice::try_new(expected_money("181.23")).expect("price"),
            ),
            amount: Some(expected_money("543.69")),
            commission: expected_money("0.73"),
            tax: expected_money("1.19"),
            net_amount: Some(expected_money("541.77")),
            exchange_rate: Some(
                domain::Ratio::parse(&domain::NumericInput::try_new("1387.41").expect("numeric"))
                    .expect("ratio"),
            ),
            settlement_date: date("2026-09-19"),
            settlement: domain::Settlement::Settled,
            transaction_time: Some(
                broker_toss::TossTimestamp::try_from("2026-09-19T06:31:17+09:00")
                    .expect("timestamp")
                    .into_domain(),
            ),
            cancellation: domain::Cancellation::Cancelled,
        })
        .expect("transaction");
        assert_eq!(completed.rows().as_slice(), [expected]);
    }

    fn partial(
        result: Result<application::TransactionRead, application::PortError>,
    ) -> application::IncompleteTransactions {
        match result.expect("partial result") {
            application::TransactionRead::Incomplete(received) => received,
            application::TransactionRead::Complete(_) => panic!("complete"),
        }
    }
    #[test]
    fn middle_server_failure_preserves_received_rows() {
        let (result, _) = read_responses(vec![
            ok(page(json!([row()]), json!("next"))),
            response(
                "500 Internal Server Error",
                &json!({"error":{"code":"internal-error","message":"合成"}}),
            ),
        ]);
        let received = partial(result);
        assert_eq!(
            received.reason,
            application::IncompleteReason::PageInterrupted
        );
        assert_eq!(received.received.len(), 1);
        assert_eq!(received.received[0].as_slice().len(), 1);
        assert_eq!(received.account, AccountId::new(731));
        assert_eq!(received.window, window());
        assert_eq!(received.filter, TransactionFilter::all());
    }
    #[test]
    fn row_limit_preserves_the_overflowing_page_in_bounded_batches() {
        let (result, _) = read_responses(vec![ok(page(json!(vec![row(); 4100]), Value::Null))]);
        let received = partial(result);
        assert_eq!(
            received.reason,
            application::IncompleteReason::RowLimitExceeded
        );
        assert_eq!(
            received
                .received
                .iter()
                .map(|batch| batch.as_slice().len())
                .sum::<usize>(),
            4100
        );
        assert!(
            received
                .received
                .iter()
                .all(|batch| batch.as_slice().len() <= 4096)
        );
    }
    #[test]
    fn outside_settled_rows_are_excluded_before_conversion_and_quarantine() {
        let mut eur = row();
        eur["currency"] = json!("EUR");
        eur["settlementDate"] = json!("2020-01-01");
        let mut bad_market = eur.clone();
        bad_market["currency"] = json!("KRW");
        bad_market["marketCountry"] = json!("JP");
        let mut bad_amount = eur.clone();
        bad_amount["currency"] = json!("KRW");
        bad_amount["amount"] = json!("-9");
        let completed = complete(vec![page(
            json!([eur, bad_market, bad_amount, row()]),
            Value::Null,
        )]);
        assert_eq!(completed.rows().as_slice().len(), 1);
    }
    #[test]
    fn both_window_boundaries_are_included() {
        let mut rows = Vec::new();
        for value in ["2026-08-31", "2026-09-01", "2026-09-30", "2026-10-01"] {
            let mut entry = row();
            entry["settlementDate"] = json!(value);
            rows.push(entry);
        }
        let completed = complete(vec![page(json!(rows), Value::Null)]);
        assert_eq!(
            completed
                .rows()
                .as_slice()
                .iter()
                .map(|entry| entry.data().settlement_date)
                .collect::<Vec<_>>(),
            vec![date("2026-09-01"), date("2026-09-30")]
        );
    }
    #[test]
    fn partial_filter_uses_comma_separated_codes_on_every_page() {
        let server = ScriptedHttpServer::start(vec![
            issued(),
            ok(page(json!([]), json!("opaque"))),
            ok(page(json!([]), Value::Null)),
        ])
        .expect("server");
        let filter = TransactionFilter::only(vec![
            domain::TransactionKind::Buy,
            domain::TransactionKind::Sell,
        ])
        .expect("filter");
        let result = run(adapter(&server).transactions(&AccountId::new(731), window(), &filter))
            .expect("read");
        let application::TransactionRead::Complete(completed) = result else {
            panic!("partial")
        };
        assert_eq!(completed.filter(), &filter);
        let requests = String::from_utf8(server.finish().expect("requests")).expect("utf8");
        assert_eq!(requests.matches("transactionTypes=BUY%2CSELL").count(), 2);
    }
    #[test]
    fn long_unknown_currency_preserves_utf8_chunks() {
        let mut entry = row();
        entry["currency"] = json!("EUR");
        entry["extra"] = json!("가".repeat(23000));
        let original = entry.to_string();
        let (result, _) = read_responses(vec![ok(page(json!([entry]), Value::Null))]);
        let received = partial(result);
        let application::QuarantinedTransaction::UnknownCurrency(chunks) = &received.quarantined[0];
        assert!(chunks.len() > 1);
        assert!(
            chunks
                .iter()
                .all(|chunk| chunk.as_untrusted_str().len() <= 65536)
        );
        assert_eq!(
            chunks
                .iter()
                .map(domain::UntrustedText::as_untrusted_str)
                .collect::<String>(),
            original
        );
    }

    #[test]
    fn page_limit_keeps_previously_received_rows() {
        let responses = (0_u16..8192)
            .map(|index| {
                let rows = if index == 0 {
                    json!([row()])
                } else {
                    json!([])
                };
                ok(page(rows, json!(format!("cursor_{index}"))))
            })
            .collect();
        let (result, requests) = read_responses(responses);
        let received = partial(result);
        assert_eq!(
            received.reason,
            application::IncompleteReason::PageInterrupted
        );
        assert_eq!(received.received[0].as_slice().len(), 1);
        assert_eq!(requests.matches("GET /api/v1/transactions?").count(), 8192);
    }
}
