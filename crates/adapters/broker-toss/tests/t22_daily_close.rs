mod support;
#[cfg(test)]
mod tests {
    use application::{
        BrokerRead, Clock, EventBus, SecretKey, SecretStore, SecretValue, SystemPortError,
        WriteCompleted,
    };
    use broker_toss::{
        TokenError, TokenManager, TokenRenewal, TossAccountRead, TossEndpoint, TossHttpClient,
        TossTimestamp,
    };
    use domain::{
        AdjustmentBasis, CandlePeriod, CandleStatus, CommonShare, Currency, DisplayInstrumentName,
        InstrumentId, KoreanMasterDetail, LiquidationTrading, ListingStatus, MarketSession,
        MasterCode, MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow,
        MasterSecurityType, NumericInput, NxtTrading, Price, PriceAdjustment, SessionData,
        SessionInformation, Symbol, TimeInterval, Timestamp, TradingSuspension,
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
    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(future)
    }
    fn at(text: &str) -> Timestamp {
        TossTimestamp::try_from(text)
            .expect("timestamp")
            .into_domain()
    }
    fn instrument() -> domain::QuoteInstrument {
        master(at("2026-03-25T12:00:00+09:00")).instruments()[0]
            .quote_instrument()
            .expect("quote")
    }
    fn window() -> TimeInterval {
        TimeInterval::try_new(
            at("2026-03-24T00:00:00+09:00"),
            at("2026-03-26T00:00:00+09:00"),
        )
        .expect("window")
    }
    fn sessions() -> Vec<MarketSession> {
        ["2026-03-24", "2026-03-25"]
            .map(|date| {
                let open = at(&format!("{date}T09:00:00+09:00"));
                let close = at(&format!("{date}T15:30:00+09:00"));
                MarketSession::try_new(
                    open.offset_calendar_date(),
                    SessionInformation::Kr {
                        pre_market: None,
                        regular_market: Some(
                            SessionData::try_new(
                                TimeInterval::try_new(open, close).expect("session"),
                                None,
                            )
                            .expect("data"),
                        ),
                        after_market: None,
                    },
                )
                .expect("calendar")
            })
            .to_vec()
    }

    fn fixture(name: &str) -> Value {
        serde_json::from_str(
            &std::fs::read_to_string(format!("tests/market_recordings/t07-{name}.json"))
                .expect("fixture"),
        )
        .expect("json")
    }
    fn serve(values: &[Value]) -> ScriptedHttpServer {
        ScriptedHttpServer::start(
            std::iter::once(issued())
                .chain(values.iter().map(|value| response("200 OK", value)))
                .collect(),
        )
        .expect("server")
    }
    #[derive(Clone)]
    struct FixedTime(Timestamp);
    impl Clock for FixedTime {
        fn now(&self) -> Timestamp {
            self.0
        }
    }
    fn adapter_at(
        server: &ScriptedHttpServer,
        now: Timestamp,
    ) -> TossAccountRead<Store, FixedTime, Renewal> {
        let clock = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("local"),
            Arc::<crate::support::FakeTime>::clone(&clock),
            clock,
        )
        .expect("client");
        TossAccountRead::new(
            TokenManager::new(
                client,
                Store,
                FixedTime(now),
                Renewal,
                Arc::new(EventBus::default()),
            ),
            FixedTime(now),
            master(now),
        )
    }
    fn invalid_response<T: std::fmt::Debug>(result: Result<T, application::PortError>) {
        let error = result.expect_err("invalid response");
        assert_eq!(
            error.kind(),
            application::PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation)
        );
    }
    fn us_sessions(dates: [(&str, &str); 4]) -> Vec<MarketSession> {
        dates
            .into_iter()
            .enumerate()
            .map(|(index, (date, offset))| {
                let open = at(&format!("{date}T09:30:00{offset}"));
                let close = at(&format!("{date}T16:00:00{offset}"));
                let regular = (index == 0 || index == 3).then(|| {
                    SessionData::try_new(TimeInterval::try_new(open, close).expect("range"), None)
                        .expect("data")
                });
                MarketSession::try_new(
                    open.offset_calendar_date(),
                    SessionInformation::Us {
                        day_market: None,
                        pre_market: None,
                        regular_market: regular,
                        after_market: None,
                    },
                )
                .expect("session")
            })
            .collect()
    }
    #[test]
    fn t22_extended_session_does_not_delay_daily_finalization() {
        let mut calendar = sessions();
        let regular = SessionData::try_new(
            TimeInterval::try_new(
                at("2026-03-25T09:00:00+09:00"),
                at("2026-03-25T15:30:00+09:00"),
            )
            .expect("regular interval"),
            None,
        )
        .expect("regular session");
        let after = SessionData::try_new(
            TimeInterval::try_new(
                at("2026-03-25T15:30:00+09:00"),
                at("2026-03-25T20:00:00+09:00"),
            )
            .expect("after interval"),
            None,
        )
        .expect("after session");
        calendar[1] = MarketSession::try_new(
            at("2026-03-25T00:00:00+09:00").offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(regular),
                after_market: Some(after),
            },
        )
        .expect("extended calendar");
        for (now, status) in [
            ("2026-03-25T15:00:00+09:00", CandleStatus::InProgress),
            ("2026-03-25T15:29:59+09:00", CandleStatus::InProgress),
            ("2026-03-25T15:30:00+09:00", CandleStatus::Finalized),
            ("2026-03-25T16:00:00+09:00", CandleStatus::Finalized),
            ("2026-03-25T19:59:59+09:00", CandleStatus::Finalized),
            ("2026-03-25T20:00:00+09:00", CandleStatus::Finalized),
        ] {
            let server = serve(&[fixture("daily")["body"].clone()]);
            let result = run(adapter_at(&server, at(now))
                .with_candle_sessions(calendar.clone())
                .candles(
                    &instrument(),
                    window(),
                    CandlePeriod::Daily,
                    PriceAdjustment::Raw,
                ))
            .expect("daily");
            assert_eq!(result[1].status(), status, "{now}");
            assert_eq!(
                result[1].values().close(),
                Price::try_new(
                    domain::Money::parse(
                        &NumericInput::try_new("72000").expect("amount"),
                        Currency::Krw,
                    )
                    .expect("money")
                )
                .expect("price")
            );
            let _captured = server.finish().expect("requests");
        }
    }

    #[test]
    fn t22_half_day_and_missing_regular_session_remain_distinct() {
        let regular = SessionData::try_new(
            TimeInterval::try_new(
                at("2026-03-25T09:00:00+09:00"),
                at("2026-03-25T12:00:00+09:00"),
            )
            .expect("half day"),
            None,
        )
        .expect("regular");
        let after = SessionData::try_new(
            TimeInterval::try_new(
                at("2026-03-25T12:00:00+09:00"),
                at("2026-03-25T20:00:00+09:00"),
            )
            .expect("after"),
            None,
        )
        .expect("after");
        for (now, status) in [
            ("2026-03-25T11:59:59+09:00", CandleStatus::InProgress),
            ("2026-03-25T12:00:00+09:00", CandleStatus::Finalized),
            ("2026-03-25T16:00:00+09:00", CandleStatus::Finalized),
        ] {
            let mut rows = sessions();
            rows[1] = MarketSession::try_new(
                at(now).offset_calendar_date(),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: Some(regular),
                    after_market: Some(after),
                },
            )
            .expect("half day");
            let server = serve(&[fixture("daily")["body"].clone()]);
            let candles = run(adapter_at(&server, at(now))
                .with_candle_sessions(rows)
                .candles(
                    &instrument(),
                    window(),
                    CandlePeriod::Daily,
                    PriceAdjustment::Raw,
                ))
            .expect("candles");
            assert_eq!(candles[1].status(), status);
            let _captured = server.finish().expect("requests");
        }
    }
    #[test]
    fn t22_missing_regular_and_holiday_reject_returned_candles() {
        let after = SessionData::try_new(
            TimeInterval::try_new(
                at("2026-03-25T12:00:00+09:00"),
                at("2026-03-25T20:00:00+09:00"),
            )
            .expect("after"),
            None,
        )
        .expect("after");
        for extra in [None, Some(after)] {
            let mut rows = sessions();
            rows[1] = MarketSession::try_new(
                at("2026-03-25T00:00:00+09:00").offset_calendar_date(),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: None,
                    after_market: extra,
                },
            )
            .expect("missing regular");
            let server = serve(&[fixture("daily")["body"].clone()]);
            invalid_response(run(adapter_at(&server, at("2026-03-25T16:00:00+09:00"))
                .with_candle_sessions(rows)
                .candles(
                    &instrument(),
                    window(),
                    CandlePeriod::Daily,
                    PriceAdjustment::Raw,
                )));
            let _captured = server.finish().expect("requests");
        }
    }

    #[test]
    fn t22_us_previous_local_day_is_finalized_at_korean_16() {
        let now = at("2026-03-25T16:00:00+09:00");
        let mut body = fixture("daily")["body"].clone();
        body["result"]["candles"]
            .as_array_mut()
            .expect("candles")
            .truncate(1);
        body["result"]["candles"][0]["timestamp"] = json!("2026-03-24T00:00:00-04:00");
        body["result"]["candles"][0]["currency"] = json!("USD");
        body["result"]["nextBefore"] = Value::Null;
        let open = at("2026-03-24T09:30:00-04:00");
        let close = at("2026-03-24T16:00:00-04:00");
        let after = at("2026-03-24T20:00:00-04:00");
        let row = MarketSession::try_new(
            open.offset_calendar_date(),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(open, close).expect("regular"),
                        None,
                    )
                    .expect("regular"),
                ),
                after_market: Some(
                    SessionData::try_new(TimeInterval::try_new(close, after).expect("after"), None)
                        .expect("after"),
                ),
            },
        )
        .expect("us");
        let instrument = master(now).instruments()[1]
            .quote_instrument()
            .expect("us instrument");
        let range = TimeInterval::try_new(
            at("2026-03-24T00:00:00-04:00"),
            at("2026-03-25T00:00:00-04:00"),
        )
        .expect("range");
        let server = serve(&[body]);
        let candles = run(adapter_at(&server, now)
            .with_candle_sessions(vec![row])
            .candles(
                &instrument,
                range,
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            ))
        .expect("us candles");
        assert_eq!(candles.len(), 1);
        assert_eq!(candles[0].time().date(), open.offset_calendar_date());
        assert_eq!(candles[0].status(), CandleStatus::Finalized);
        let _captured = server.finish().expect("requests");
    }

    fn t22_money(text: &str, currency: Currency) -> domain::Money {
        domain::Money::parse(&NumericInput::try_new(text).expect("input"), currency).expect("money")
    }
    fn t22_quantity() -> domain::Quantity {
        domain::Quantity::parse(&NumericInput::try_new("3").expect("input")).expect("quantity")
    }
    fn t22_kr_rows(now: Timestamp) -> Vec<MarketSession> {
        let mut kr = sessions();
        let session = |start, end| {
            SessionData::try_new(
                TimeInterval::try_new(at(start), at(end)).expect("interval"),
                None,
            )
            .expect("session")
        };
        kr[1] = MarketSession::try_new(
            now.offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(session(
                    "2026-03-25T09:00:00+09:00",
                    "2026-03-25T15:30:00+09:00",
                )),
                after_market: Some(session(
                    "2026-03-25T15:30:00+09:00",
                    "2026-03-25T20:00:00+09:00",
                )),
            },
        )
        .expect("kr today");
        kr
    }
    fn t22_calendar(kr: &[MarketSession]) -> domain::SnapshotCalendar {
        let next_open = at("2026-03-26T09:00:00+09:00");
        let next_close = at("2026-03-26T15:30:00+09:00");
        let next = MarketSession::try_new(
            next_open.offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(next_open, next_close).expect("next"),
                        None,
                    )
                    .expect("next"),
                ),
                after_market: None,
            },
        )
        .expect("next");
        let kr_calendar = domain::MarketCalendar::try_new(kr[0].clone(), kr[1].clone(), next)
            .expect("kr calendar");
        let us = us_sessions([
            ("2026-03-24", "-04:00"),
            ("2026-03-25", "-04:00"),
            ("2026-03-26", "-04:00"),
            ("2026-03-27", "-04:00"),
        ]);
        let us_calendar =
            domain::MarketCalendar::try_new(us[0].clone(), us[1].clone(), us[2].clone())
                .expect("us calendar");
        domain::SnapshotCalendar::new(&kr_calendar, &us_calendar).expect("calendar")
    }
    fn t22_transaction() -> domain::Transaction {
        let quantity = t22_quantity();
        domain::Transaction::try_new(domain::TransactionData {
            kind: domain::TransactionKind::Buy,
            market: domain::Market::Kr,
            currency: Currency::Krw,
            symbol: Some(instrument().symbol().clone()),
            asset_class: Some(domain::AssetClass::Stock),
            isin: None,
            quantity: Some(quantity),
            filled_price: None,
            amount: Some(t22_money("210000", Currency::Krw)),
            commission: t22_money("0", Currency::Krw),
            tax: t22_money("0", Currency::Krw),
            net_amount: Some(t22_money("210000", Currency::Krw)),
            exchange_rate: None,
            settlement_date: at("2026-03-24T00:00:00+09:00").offset_calendar_date(),
            settlement: domain::Settlement::Settled,
            transaction_time: Some(at("2026-03-24T10:00:00+09:00")),
            cancellation: domain::Cancellation::Regular,
        })
        .expect("transaction")
    }
    fn t22_rows(
        now: Timestamp,
    ) -> (
        domain::SettledRowSet,
        domain::SettlementWindow,
        domain::LedgerEvidence,
    ) {
        use domain::AccountId;
        let transaction = t22_transaction();
        let account = AccountId::new(71);
        let rows = domain::SettledRowSet::try_restore(
            account.clone(),
            vec![
                domain::StoredTransaction::try_restore(transaction.clone(), now, None)
                    .expect("stored"),
            ],
            domain::TransactionBatch::try_new(vec![]).expect("pending"),
            Some(now),
        )
        .expect("rows");
        let range = domain::SettlementWindow::try_new(
            at("2026-03-24T00:00:00+09:00").offset_calendar_date(),
            now.offset_calendar_date(),
        )
        .expect("range");
        let response = domain::CompleteTransactionResponse::after_last_page_without_quarantine(
            account,
            range,
            domain::TransactionFilter::all(),
            domain::TransactionBatch::try_new(vec![transaction]).expect("transactions"),
            domain::CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
        )
        .expect("complete response");
        let ledger_evidence =
            domain::LedgerEvidence::from_complete_response(response).expect("ledger evidence");
        (rows, range, ledger_evidence)
    }
    fn t22_ledger(
        context: (
            &domain::SettledRowSet,
            domain::SettlementWindow,
            &domain::LedgerEvidence,
            Timestamp,
        ),
        currency: Currency,
        amounts: (&str, &str),
    ) -> domain::CashLedger {
        let (rows, range, ledger_evidence, now) = context;
        let (opening, ending) = amounts;
        domain::CashLedger::reconstruct(
            rows,
            domain::CashLedgerInput {
                window: range,
                currency,
                opening: Some(t22_money(opening, currency)),
                evidence: Some(ledger_evidence.clone()),
                reported: Some(domain::BuyingPower::new(t22_money(ending, currency), now)),
            },
        )
    }
    fn t22_period(
        entry: &domain::CashLedger,
        range: domain::SettlementWindow,
    ) -> domain::ConfirmedFlowPeriod {
        match entry.external_flows(&[]) {
            domain::ExternalFlows::Reconciled(flows) => match flows.period(range) {
                domain::FlowPeriod::Confirmed(value) => value,
                domain::FlowPeriod::Uncertain(_) => panic!("unconfirmed period"),
            },
            domain::ExternalFlows::Unavailable(_) => panic!("unreconciled ledger"),
        }
    }
    fn t22_recompute(calendar: &domain::SnapshotCalendar, close: &domain::Candle, now: Timestamp) {
        let (rows, range, ledger_evidence) = t22_rows(now);
        let kr_ledger = t22_ledger(
            (&rows, range, &ledger_evidence, now),
            Currency::Krw,
            ("211000", "1000"),
        );
        let us_ledger = t22_ledger(
            (&rows, range, &ledger_evidence, now),
            Currency::Usd,
            ("0", "0"),
        );
        let kr_period = t22_period(&kr_ledger, range);
        let us_period = t22_period(&us_ledger, range);
        let history = domain::SnapshotInstrumentHistory::new(
            domain::LotIdentity::new(
                instrument().instrument_id().clone(),
                instrument().symbol().clone(),
                domain::Market::Kr,
                Currency::Krw,
            ),
            vec![],
            vec![close.clone()],
        )
        .expect("history");
        let histories = [history];
        let prices = [(
            instrument().instrument_id().clone(),
            domain::ValuationPriceEvidence::SessionClose {
                price: close.values().close(),
                date: close.time().date(),
            },
        )];
        let input = domain::ValuationRecomputeInput::new(
            calendar,
            &rows,
            domain::ValuationHistoryEvidence::new(&histories, &prices),
            domain::DatedSnapshotLedgers::new(
                now.offset_calendar_date(),
                domain::SnapshotLedgers::new((&kr_ledger, &kr_period), (&us_ledger, &us_period)),
            ),
            domain::SnapshotObservation::new(now, now, None),
        )
        .expect("recompute input");
        let recomputed = domain::ValuationSnapshot::recompute(input).expect("recompute");
        assert_eq!(
            recomputed.stock_value(Currency::Krw),
            Ok(t22_money("216000", Currency::Krw))
        );
        assert_eq!(
            recomputed.account_value(Currency::Krw),
            Ok(t22_money("217000", Currency::Krw))
        );
    }
    fn t22_portfolio(now: Timestamp, price: Price) -> domain::Portfolio {
        use domain::{BuyingPower, Holding, HoldingObservation, Portfolio};
        let master = master(now);
        let held = Holding::new(
            master.instruments()[0]
                .order_instrument()
                .expect("instrument"),
            domain::Quantity::parse(&NumericInput::try_new("3").expect("input")).expect("quantity"),
            domain::AcquisitionPrice::try_new(t22_money("70000", Currency::Krw)).expect("cost"),
            HoldingObservation::new(Some(price), None, now),
        )
        .expect("holding");
        Portfolio::new(
            vec![held],
            BuyingPower::new(t22_money("1000", Currency::Krw), now),
            BuyingPower::new(t22_money("0", Currency::Usd), now),
        )
        .expect("portfolio")
    }
    fn t22_bad_candles(close: &domain::Candle, previous: &domain::Candle) -> Vec<domain::Candle> {
        let now = close.collected_at();
        let variants = [
            (
                InstrumentId::try_new("different").expect("id"),
                PriceAdjustment::Raw,
                now,
                CandleStatus::Finalized,
            ),
            (
                close.instrument().clone(),
                PriceAdjustment::Raw,
                now,
                CandleStatus::InProgress,
            ),
            (
                close.instrument().clone(),
                PriceAdjustment::Adjusted(AdjustmentBasis::new(now)),
                now,
                CandleStatus::Finalized,
            ),
            (
                close.instrument().clone(),
                PriceAdjustment::Raw,
                at("2026-03-25T16:01:00+09:00"),
                CandleStatus::Finalized,
            ),
            (
                close.instrument().clone(),
                PriceAdjustment::Raw,
                at("2026-03-25T15:00:00+09:00"),
                CandleStatus::Finalized,
            ),
        ];
        variants
            .into_iter()
            .map(|(id, adjustment, collected, status)| {
                domain::Candle::try_new(
                    id,
                    close.time(),
                    adjustment,
                    close.values(),
                    domain::CandleObservation::new(collected, status),
                )
                .expect("synthetic candle")
            })
            .chain(std::iter::once(previous.clone()))
            .collect()
    }
    fn t22_invalid_candles(
        calendar: &domain::SnapshotCalendar,
        portfolio: &domain::Portfolio,
        previous: &domain::Candle,
        evidence: &domain::ObservedCloseEvidence,
    ) {
        let close = evidence.candle();
        let now = close.collected_at();
        let evaluated = portfolio.evaluate(None);
        let observe = |entries: &[domain::ObservedCloseEvidence]| {
            domain::ValuationSnapshot::observe_with_evidence(
                calendar, &evaluated, now, None, entries,
            )
        };
        assert_eq!(
            observe(&[]).err(),
            Some(domain::SnapshotError::MissingPrice)
        );
        assert_eq!(
            observe(&[evidence.clone(), evidence.clone()]).err(),
            Some(domain::SnapshotError::DuplicateInstrument)
        );
        for invalid in t22_bad_candles(close, previous) {
            let bad = domain::ObservedCloseEvidence::new(instrument(), invalid);
            assert_eq!(
                observe(&[bad]).err(),
                Some(domain::SnapshotError::InvalidCandle)
            );
        }
        let unrelated = domain::ObservedCloseEvidence::new(
            master(now).instruments()[1].quote_instrument().expect("us"),
            close.clone(),
        );
        assert_eq!(
            observe(&[unrelated]).err(),
            Some(domain::SnapshotError::MissingPrice)
        );
    }
    fn t22_invalid_reports(
        calendar: &domain::SnapshotCalendar,
        now: Timestamp,
        portfolio: &domain::Portfolio,
        evidence: &domain::ObservedCloseEvidence,
    ) {
        use domain::{BuyingPower, Holding, HoldingObservation, Portfolio};
        let quantity = t22_quantity();
        let master = master(now);
        let mismatch = Holding::new(
            master.instruments()[0]
                .order_instrument()
                .expect("instrument"),
            quantity,
            domain::AcquisitionPrice::try_new(t22_money("70000", Currency::Krw)).expect("cost"),
            HoldingObservation::new(
                Some(Price::try_new(t22_money("71900", Currency::Krw)).expect("price")),
                None,
                now,
            ),
        )
        .expect("holding");
        let different = Portfolio::new(
            vec![mismatch],
            BuyingPower::new(t22_money("1000", Currency::Krw), now),
            BuyingPower::new(t22_money("0", Currency::Usd), now),
        )
        .expect("portfolio");
        assert_eq!(
            domain::ValuationSnapshot::observe_with_evidence(
                calendar,
                &different.evaluate(None),
                now,
                None,
                std::slice::from_ref(evidence)
            )
            .err(),
            Some(domain::SnapshotError::InvalidCandle)
        );
        let summary = domain::CurrencyAmounts::new(t22_money("215999", Currency::Krw), None)
            .expect("amounts");
        let zero = domain::CurrencyAmounts::new(t22_money("0", Currency::Krw), None).expect("zero");
        let different_summary = portfolio
            .clone()
            .with_reported(domain::ReportedPortfolio::new(summary, zero, zero, now));
        assert_eq!(
            domain::ValuationSnapshot::observe_with_evidence(
                calendar,
                &different_summary.evaluate(None),
                now,
                None,
                std::slice::from_ref(evidence)
            )
            .err(),
            Some(domain::SnapshotError::MissingPrice)
        );
    }
    fn t22_supply_evidence(
        market: &testkit_ports::memory::MemoryMarketDataRepository,
        evidence: &domain::ObservedCloseEvidence,
    ) {
        let candle = evidence.candle();
        let later = domain::Candle::try_new(
            candle.instrument().clone(),
            candle.time(),
            candle.adjustment(),
            candle.values(),
            domain::CandleObservation::new(
                at("2026-03-25T16:01:00+09:00"),
                CandleStatus::Finalized,
            ),
        )
        .expect("later candle");
        market.supply_close_evidence(domain::ObservedCloseEvidence::new(instrument(), later));
        market.supply_close_evidence(evidence.clone());
    }
    fn t22_invalid_holding_report(
        calendar: &domain::SnapshotCalendar,
        now: Timestamp,
        evidence: &domain::ObservedCloseEvidence,
    ) {
        let ratio =
            domain::Ratio::parse(&NumericInput::try_new("0").expect("input")).expect("ratio");
        let report = domain::ReportedHolding::new(
            t22_money("215999", Currency::Krw),
            t22_money("0", Currency::Krw),
            ratio,
        )
        .expect("reported");
        let holding = domain::Holding::new(
            master(now).instruments()[0]
                .order_instrument()
                .expect("instrument"),
            t22_quantity(),
            domain::AcquisitionPrice::try_new(t22_money("70000", Currency::Krw)).expect("cost"),
            domain::HoldingObservation::new(
                Some(evidence.candle().values().close()),
                Some(report),
                now,
            ),
        )
        .expect("holding");
        let portfolio = domain::Portfolio::new(
            vec![holding],
            domain::BuyingPower::new(t22_money("1000", Currency::Krw), now),
            domain::BuyingPower::new(t22_money("0", Currency::Usd), now),
        )
        .expect("portfolio");
        assert_eq!(
            domain::ValuationSnapshot::observe_with_evidence(
                calendar,
                &portfolio.evaluate(None),
                now,
                None,
                std::slice::from_ref(evidence)
            )
            .err(),
            Some(domain::SnapshotError::InvalidCandle)
        );
    }
    fn t22_save(
        calendar: &domain::SnapshotCalendar,
        now: Timestamp,
        portfolio: domain::Portfolio,
        evidence: domain::ObservedCloseEvidence,
    ) {
        use application::TakeValuationSnapshot;
        use domain::{AccountId, SyncOutcome, SyncRecord};
        use testkit_ports::{
            SteppedClock,
            memory::{
                MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
                MemoryValuationRepository,
            },
        };
        let account = AccountId::new(71);
        let store = MemoryPortfolioRepository::default();
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                now,
                SyncOutcome::Complete(portfolio),
            )))));
        let market = MemoryMarketDataRepository::default();
        market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
        t22_supply_evidence(&market, &evidence);
        let repository = MemoryValuationRepository::default();
        repository.script_append().push(MemoryReply::Memory);
        let clock = SteppedClock::new(now);
        let bus = EventBus::default();
        let report = run(
            TakeValuationSnapshot::new(&store, &market, &repository, &clock, &bus)
                .execute(&AccountId::new(71), calendar),
        )
        .expect("recorded observed snapshot");
        assert_eq!(
            report.snapshot.stock_value(Currency::Krw),
            Ok(t22_money("216000", Currency::Krw))
        );
        assert_eq!(
            report.snapshot.price_date(Currency::Krw),
            now.offset_calendar_date()
        );
        assert_eq!(report.snapshot.observed_close_evidence(), &[evidence]);
        assert_eq!(report.recorded, application::RecordOutcome::Recorded);
        assert_eq!(repository.calls().len(), 1);
        repository.script_history().push(MemoryReply::Memory);
        let saved = run(application::ValuationRepository::history(
            &repository,
            &account,
            window(),
        ))
        .expect("saved history");
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0], report.snapshot);
    }
    #[test]
    fn t22_evidence_query_uses_holding_observation_time() {
        use application::TakeValuationSnapshot;
        use domain::{AccountId, SyncOutcome, SyncRecord};
        use testkit_ports::{
            SteppedClock,
            memory::{
                MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
                MemoryValuationRepository,
            },
        };
        let now = at("2026-03-25T16:00:00+09:00");
        let observed = at("2026-03-25T15:59:00+09:00");
        let calendar = t22_calendar(&t22_kr_rows(now));
        let server = serve(&[fixture("daily")["body"].clone()]);
        let bars = run(adapter_at(&server, observed)
            .with_candle_sessions(t22_kr_rows(now))
            .candles(
                &instrument(),
                window(),
                CandlePeriod::Daily,
                PriceAdjustment::Raw,
            ))
        .expect("candles");
        let original = &bars[1];
        let later = domain::Candle::try_new(
            original.instrument().clone(),
            original.time(),
            original.adjustment(),
            original.values(),
            domain::CandleObservation::new(now, CandleStatus::Finalized),
        )
        .expect("later");
        let expected = domain::ObservedCloseEvidence::new(instrument(), original.clone());
        let store = MemoryPortfolioRepository::default();
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                now,
                SyncOutcome::Complete(t22_portfolio(observed, original.values().close())),
            )))));
        let market = MemoryMarketDataRepository::default();
        market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
        market.supply_close_evidence(domain::ObservedCloseEvidence::new(instrument(), later));
        market.supply_close_evidence(expected.clone());
        let repository = MemoryValuationRepository::default();
        repository.script_append().push(MemoryReply::Memory);
        let report = run(TakeValuationSnapshot::new(
            &store,
            &market,
            &repository,
            &SteppedClock::new(now),
            &EventBus::default(),
        )
        .execute(&AccountId::new(71), &calendar))
        .expect("observed snapshot");
        assert_eq!(report.snapshot.observed_close_evidence(), &[expected]);
        let _captured = server.finish().expect("requests");
    }

    #[test]
    fn t22_evidence_repository_error_prevents_snapshot_storage() {
        use application::{TakeValuationSnapshot, ValuationSnapshotError};
        use domain::{AccountId, SyncOutcome, SyncRecord};
        use testkit_ports::{
            SteppedClock,
            memory::{
                MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
                MemoryValuationRepository,
            },
        };
        let now = at("2026-03-25T16:00:00+09:00");
        let calendar = t22_calendar(&t22_kr_rows(now));
        let price = Price::try_new(t22_money("72000", Currency::Krw)).expect("price");
        let store = MemoryPortfolioRepository::default();
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                now,
                SyncOutcome::Complete(t22_portfolio(now, price)),
            )))));
        let market = MemoryMarketDataRepository::default();
        market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
        let error = SystemPortError::new(
            application::SystemPort::Repository,
            application::SystemFailure::PermissionDenied,
        );
        market.fail_close_evidence(error);
        let repository = MemoryValuationRepository::default();
        let clock = SteppedClock::new(now);
        let bus = EventBus::default();
        let result = run(
            TakeValuationSnapshot::new(&store, &market, &repository, &clock, &bus)
                .execute(&AccountId::new(71), &calendar),
        );
        assert_eq!(result, Err(ValuationSnapshotError::Repository(error)));
        assert_eq!(repository.calls().len(), 0);
    }

    fn t22_flat_candle(
        quote: &domain::QuoteInstrument,
        price: Price,
        date: Timestamp,
        collected_at: Timestamp,
    ) -> domain::Candle {
        domain::Candle::try_new(
            quote.instrument_id().clone(),
            domain::CandleTime::try_new(date, CandlePeriod::Daily).expect("day"),
            PriceAdjustment::Raw,
            domain::Ohlcv::try_new(price, price, price, price, t22_quantity()).expect("values"),
            domain::CandleObservation::new(collected_at, CandleStatus::Finalized),
        )
        .expect("candle")
    }

    fn t22_us_extended_calendar(now: Timestamp) -> domain::SnapshotCalendar {
        let interval = |start, end| {
            SessionData::try_new(TimeInterval::try_new(start, end).expect("interval"), None)
                .expect("session")
        };
        let us_session = |date: &str| {
            let open = at(&format!("{date}T09:30:00-04:00"));
            let close = at(&format!("{date}T16:00:00-04:00"));
            let end = at(&format!("{date}T20:00:00-04:00"));
            MarketSession::try_new(
                open.offset_calendar_date(),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: Some(interval(open, close)),
                    after_market: Some(interval(close, end)),
                },
            )
            .expect("us session")
        };
        let kr = t22_kr_rows(now);
        let next_open = at("2026-03-26T09:00:00+09:00");
        let next_close = at("2026-03-26T15:30:00+09:00");
        let next = MarketSession::try_new(
            next_open.offset_calendar_date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(interval(next_open, next_close)),
                after_market: None,
            },
        )
        .expect("kr next");
        let kr_calendar = domain::MarketCalendar::try_new(kr[0].clone(), kr[1].clone(), next)
            .expect("kr calendar");
        let us_calendar = domain::MarketCalendar::try_new(
            us_session("2026-03-24"),
            us_session("2026-03-25"),
            us_session("2026-03-26"),
        )
        .expect("us calendar");
        domain::SnapshotCalendar::new(&kr_calendar, &us_calendar).expect("calendar")
    }

    #[test]
    fn t22_multiple_close_observations_remain_ambiguous_even_when_prices_match() {
        use application::{TakeValuationSnapshot, ValuationSnapshotError};
        use domain::{AccountId, SyncOutcome, SyncRecord};
        use testkit_ports::{
            SteppedClock,
            memory::{
                MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
                MemoryValuationRepository,
            },
        };
        let now = at("2026-03-25T16:00:00+09:00");
        let observed = at("2026-03-25T15:59:00+09:00");
        let earlier = at("2026-03-25T15:45:00+09:00");
        let calendar = t22_calendar(&t22_kr_rows(now));
        let quote = instrument();
        let last = Price::try_new(t22_money("72000", Currency::Krw)).expect("last");
        for first_text in ["72000", "71000"] {
            let first = Price::try_new(t22_money(first_text, Currency::Krw)).expect("first");
            let day = at("2026-03-25T00:00:00+09:00");
            let store = MemoryPortfolioRepository::default();
            store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    now,
                    SyncOutcome::Complete(t22_portfolio(observed, last)),
                )))));
            let market = MemoryMarketDataRepository::default();
            market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
            market.supply_close_evidence(domain::ObservedCloseEvidence::new(
                quote.clone(),
                t22_flat_candle(&quote, first, day, earlier),
            ));
            market.supply_close_evidence(domain::ObservedCloseEvidence::new(
                quote.clone(),
                t22_flat_candle(&quote, last, day, observed),
            ));
            let repository = MemoryValuationRepository::default();
            let result = run(TakeValuationSnapshot::new(
                &store,
                &market,
                &repository,
                &SteppedClock::new(now),
                &EventBus::default(),
            )
            .execute(&AccountId::new(71), &calendar));
            assert_eq!(
                result,
                Err(ValuationSnapshotError::Snapshot(
                    domain::SnapshotError::DuplicateInstrument
                ))
            );
            assert_eq!(repository.calls().len(), 0);
        }
    }

    #[test]
    fn t22_intraday_previous_close_keeps_missing_price_without_querying_evidence() {
        use application::{TakeValuationSnapshot, ValuationSnapshotError};
        use domain::{AccountId, SyncOutcome, SyncRecord};
        use testkit_ports::{
            SteppedClock,
            memory::{
                MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
                MemoryValuationRepository,
            },
        };
        let now = at("2026-03-25T15:00:00+09:00");
        let calendar = t22_calendar(&t22_kr_rows(now));
        let price = Price::try_new(t22_money("72000", Currency::Krw)).expect("price");
        let quote = instrument();
        let previous = t22_flat_candle(
            &quote,
            price,
            at("2026-03-24T00:00:00+09:00"),
            at("2026-03-24T16:00:00+09:00"),
        );
        for state in ["available", "repository-error"] {
            let store = MemoryPortfolioRepository::default();
            store
                .script_latest()
                .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                    now,
                    SyncOutcome::Complete(t22_portfolio(now, price)),
                )))));
            let market = MemoryMarketDataRepository::default();
            market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
            market.supply_close_evidence(domain::ObservedCloseEvidence::new(
                quote.clone(),
                previous.clone(),
            ));
            if state == "repository-error" {
                market.fail_close_evidence(SystemPortError::new(
                    application::SystemPort::Repository,
                    application::SystemFailure::PermissionDenied,
                ));
            }
            let repository = MemoryValuationRepository::default();
            let result = run(TakeValuationSnapshot::new(
                &store,
                &market,
                &repository,
                &SteppedClock::new(now),
                &EventBus::default(),
            )
            .execute(&AccountId::new(71), &calendar));
            assert_eq!(
                result,
                Err(ValuationSnapshotError::Snapshot(
                    domain::SnapshotError::MissingPrice
                )),
                "{state}"
            );
            assert_eq!(repository.calls().len(), 0);
        }
    }

    #[test]
    fn t22_us_extended_session_before_close_keeps_previous_price_rejected() {
        use application::{TakeValuationSnapshot, ValuationSnapshotError};
        use domain::{AccountId, SyncOutcome, SyncRecord};
        use testkit_ports::{
            SteppedClock,
            memory::{
                MemoryMarketDataRepository, MemoryPortfolioRepository, MemoryReply,
                MemoryValuationRepository,
            },
        };
        let now = at("2026-03-25T16:00:00+09:00");
        let calendar = t22_us_extended_calendar(now);
        let master = master(now);
        let quote = master.instruments()[1].quote_instrument().expect("quote");
        let price = Price::try_new(t22_money("17", Currency::Usd)).expect("price");
        let previous = t22_flat_candle(
            &quote,
            price,
            at("2026-03-24T00:00:00-04:00"),
            at("2026-03-24T16:05:00-04:00"),
        );
        let holding = domain::Holding::new(
            master.instruments()[1].order_instrument().expect("order"),
            t22_quantity(),
            domain::AcquisitionPrice::try_new(t22_money("10", Currency::Usd)).expect("cost"),
            domain::HoldingObservation::new(Some(price), None, now),
        )
        .expect("holding");
        let portfolio = domain::Portfolio::new(
            vec![holding],
            domain::BuyingPower::new(t22_money("0", Currency::Krw), now),
            domain::BuyingPower::new(t22_money("0", Currency::Usd), now),
        )
        .expect("portfolio");
        let store = MemoryPortfolioRepository::default();
        store
            .script_latest()
            .push(MemoryReply::Return(Ok(Some(SyncRecord::new(
                now,
                SyncOutcome::Complete(portfolio),
            )))));
        let market = MemoryMarketDataRepository::default();
        market.script_fx_rate().push(MemoryReply::Return(Ok(None)));
        market.supply_close_evidence(domain::ObservedCloseEvidence::new(quote, previous));
        let repository = MemoryValuationRepository::default();
        let result = run(TakeValuationSnapshot::new(
            &store,
            &market,
            &repository,
            &SteppedClock::new(now),
            &EventBus::default(),
        )
        .execute(&AccountId::new(71), &calendar));
        assert_eq!(
            result,
            Err(ValuationSnapshotError::Snapshot(
                domain::SnapshotError::MissingPrice
            ))
        );
        assert_eq!(repository.calls().len(), 0);
    }

    #[test]
    fn t22_recorded_candle_supplies_observed_snapshot_evidence() {
        let now = at("2026-03-25T16:00:00+09:00");
        let kr = t22_kr_rows(now);
        let calendar = t22_calendar(&kr);
        let server = serve(&[fixture("daily")["body"].clone()]);
        let candles = run(adapter_at(&server, now).with_candle_sessions(kr).candles(
            &instrument(),
            window(),
            CandlePeriod::Daily,
            PriceAdjustment::Raw,
        ))
        .expect("recorded candles");
        let close = &candles[1];
        assert_eq!(close.status(), CandleStatus::Finalized);
        let evidence = domain::ObservedCloseEvidence::new(instrument(), close.clone());
        t22_recompute(&calendar, close, now);
        let portfolio = t22_portfolio(now, close.values().close());
        t22_invalid_candles(&calendar, &portfolio, &candles[0], &evidence);
        t22_invalid_reports(&calendar, now, &portfolio, &evidence);
        t22_invalid_holding_report(&calendar, now, &evidence);
        t22_save(&calendar, now, portfolio, evidence);
        let _captured = server.finish().expect("requests");
    }
}
