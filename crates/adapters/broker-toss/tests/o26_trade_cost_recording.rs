mod support;

#[cfg(test)]
mod tests {
    use application::{
        BrokerRead, Clock, EventBus, SecretKey, SecretStore, SecretValue, SystemPortError,
        TransactionRead, WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossDate, TossEndpoint,
        TossHttpClient, TossTimestamp,
    };
    use domain::{
        AccountId, BrokerOrderId, CommonShare, Currency, DisplayInstrumentName, EstimatedTradeCost,
        InstrumentId, KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode,
        MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, Money,
        NumericInput, NxtTrading, SettlementWindow, Symbol, Timestamp, TradeCostDisplay,
        TradingSuspension, Transaction, TransactionFilter, trade_cost_display,
    };
    use serde_json::{Value, json};
    use std::{fs, future::Future, sync::Arc};
    use testkit::ScriptedHttpServer;

    struct Store;
    impl SecretStore for Store {
        fn load(
            &self,
            key: SecretKey,
        ) -> impl Future<Output = Result<Option<SecretValue>, SystemPortError>> + Send {
            std::future::ready(Ok(Some(SecretValue::new(match key {
                SecretKey::TossClientId => b"fake-o26-id".to_vec(),
                SecretKey::TossClientSecret => b"fake-o26-secret".to_vec(),
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
    struct FixedClock(Timestamp);
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            self.0
        }
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }

    fn reply(body: &Value) -> Vec<u8> {
        let text = body.to_string();
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
            text.len()
        )
        .into_bytes()
    }

    fn issued() -> Vec<u8> {
        reply(
            &json!({"access_token": "fake-access-o26", "token_type": "Bearer", "expires_in": 3600}),
        )
    }

    fn recording(name: &str) -> Value {
        let text = fs::read_to_string(format!("tests/recordings/{name}.json")).expect("녹화 파일");
        serde_json::from_str::<Value>(&text).expect("녹화 JSON")["body"].clone()
    }

    fn ts(raw: &str) -> Timestamp {
        TossTimestamp::try_from(raw).expect("시각").into_domain()
    }

    fn money(text: &str) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("숫자"), Currency::Krw).expect("금액")
    }

    fn master() -> MasterInstrumentFacts {
        MasterInstrumentFacts::try_new(vec![
            MasterInstrument::try_new(MasterInstrumentRow {
                listing_market: MasterCode::try_new("KOSPI").expect("시장"),
                symbol: Symbol::try_new("005930").expect("종목"),
                instrument_id: InstrumentId::try_new("inst-kr-005930").expect("ID"),
                name: DisplayInstrumentName::try_new("Synthetic Corp").expect("이름"),
                security_type: MasterSecurityType::from_code(
                    MasterCode::try_new("STOCK").expect("유형"),
                ),
                currency: Currency::Krw,
                listing_status: ListingStatus::Active,
                common_share: CommonShare::Common,
                korean_detail: Some(KoreanMasterDetail {
                    liquidation: LiquidationTrading::Regular,
                    krx: TradingSuspension::Clear,
                    nxt: NxtTrading::Unsupported,
                }),
                observed_at: ts("2026-03-28T08:00:00+09:00"),
            })
            .expect("마스터"),
        ])
        .expect("마스터 목록")
    }

    fn reader(server: &ScriptedHttpServer) -> TossAccountRead<Store, FixedClock, Renewal> {
        let waiter = crate::support::time();
        let clock = FixedClock(ts("2026-03-31T12:00:00+09:00"));
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
        TossAccountRead::new(tokens, clock, master())
    }

    fn window() -> SettlementWindow {
        SettlementWindow::try_new(
            TossDate::try_from("2026-03-28")
                .expect("날짜")
                .into_domain(),
            TossDate::try_from("2026-03-31")
                .expect("날짜")
                .into_domain(),
        )
        .expect("구간")
    }

    fn estimate() -> EstimatedTradeCost {
        EstimatedTradeCost {
            commission: Some(money("1400")),
            tax: Some(money("0")),
        }
    }

    fn display_with(transactions_body: &Value) -> TradeCostDisplay {
        let server = ScriptedHttpServer::start(vec![
            issued(),
            reply(&recording("t15-order-detail-kr")),
            reply(transactions_body),
        ])
        .expect("server");
        let read = reader(&server);
        let account = AccountId::new(413);
        let order = run(read.order(
            &account,
            &BrokerOrderId::try_new("synthetic-order-kr-001").expect("주문번호"),
        ))
        .expect("주문 조회");
        let observation = run(read.transactions(&account, window(), &TransactionFilter::all()))
            .expect("거래내역 조회");
        let transactions: Vec<Transaction> = match observation {
            TransactionRead::Complete(complete) => complete.rows().as_slice().to_vec(),
            TransactionRead::Incomplete(_) => panic!("완전한 거래내역이어야 함"),
        };
        trade_cost_display(&order, estimate(), &transactions)
    }

    #[test]
    fn settled_recording_replaces_estimate_with_toss_values() {
        let TradeCostDisplay::Confirmed(confirmed) = display_with(&recording("o26-trades-kr-buy"))
        else {
            panic!("확정값이어야 함");
        };
        assert_eq!(confirmed.commission(), money("1050"));
        assert_eq!(confirmed.tax(), money("0"));
        assert_eq!(confirmed.net_amount(), Some(money("701050")));
    }

    #[test]
    fn unsettled_recording_keeps_the_estimate() {
        let mut body = recording("o26-trades-kr-buy");
        let row = &mut body["result"]["transactions"][0];
        row["settled"] = json!(false);
        row["transactionTime"] = Value::Null;
        assert_eq!(display_with(&body), TradeCostDisplay::Estimated(estimate()));
    }
}
