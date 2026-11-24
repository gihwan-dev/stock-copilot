#[cfg(test)]
mod tests {
    use application::{OrderConfirmation, SecretValue};
    use broker_toss::TossEndpoint;
    use broker_toss_trade::{TossBrokerTrade, TradeAccessToken, TradeTokenFailure};
    use domain::{
        AccountDisplayFact, AccountId, AdjustmentBasis, BrokerAccountNumberInput, BrokerAccountRow,
        BrokerAccountRows, BrokerAccountTypeCode, BrokerOrderSet, BuyingPower, CalendarDate,
        CalendarYear, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        ClientOrderId, CommonShare, Currency, DayNumber, DisplayInstrumentName, EstimateContext,
        EstimateTask, EstimatedOrderFee, Horizon, HoursPermission, InstrumentId, InvestorProfile,
        KoreanMasterDetail, KrOrderPrice, LiquidationTrading, ListingStatus, MarketCalendar,
        MarketOrderPolicy, MarketSession, MasterCode, MasterInstrument, MasterInstrumentFacts,
        MasterInstrumentRow, MasterSecurityType, Money, MonthNumber, Nanosecond, NumericInput,
        NxtTrading, OffsetSeconds, Ohlcv, OpenOrderCoverage, OrderFxRates, OrderGateFacts,
        OrderIntent, OrderSide, OrderTerms, PendingOrderConfirmation, Policy, PolicySecurityFacts,
        Portfolio, PortfolioValuation, Price, PriceAdjustment, PriceLimits, PriceLimitsInput,
        Proposal, ProposalId, ProposalLifetime, ProposalPriceRange, Quantity, RiskAppetite,
        SectorAssignments, SectorCatalog, SessionData, SessionInformation, StockWarnings, Symbol,
        TimeInterval, Timestamp, TradingSuspension, UnixSeconds, Volatility, WholeOrderQuantity,
        default_market_order_policy, sector_assignment_from_compositions, validate_proposal,
    };
    use order_execution::{
        ApprovalSignatureSlot, ApproveAndSubmitOutcome, OrderOutcome, OrderRejectionReason,
        OrderUnknownCause, PostApprovalGate, PostConfirmationRecheck, RetryEligibility,
        RetryUnknownOutcome, UserApproval, approve_and_submit, retry_unknown_order,
    };
    use std::future::Future;
    use std::sync::{Arc, RwLock};
    use testkit::{ConnectionObservation, ConnectionProbe, ScriptedHttpServer};
    use testkit_ports::ScriptedPresence;

    fn block_on<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("런타임")
            .block_on(future)
    }

    struct FixedToken;

    impl TradeAccessToken for FixedToken {
        fn access_token(
            &self,
        ) -> impl Future<Output = Result<SecretValue, TradeTokenFailure>> + Send {
            std::future::ready(Ok(SecretValue::new(b"fake-access-91".to_vec())))
        }
    }

    struct MissingToken;

    impl TradeAccessToken for MissingToken {
        fn access_token(
            &self,
        ) -> impl Future<Output = Result<SecretValue, TradeTokenFailure>> + Send {
            std::future::ready(Err(TradeTokenFailure::Unavailable))
        }
    }

    fn reply(status: &str, body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    fn trade<Tokens: TradeAccessToken>(port: u16, tokens: Tokens) -> TossBrokerTrade<Tokens> {
        TossBrokerTrade::new(
            &TossEndpoint::local(port).expect("루프백"),
            Arc::new(tokens),
        )
        .expect("주문 어댑터")
    }

    fn submit_once<Tokens: TradeAccessToken>(
        broker: &TossBrokerTrade<Tokens>,
        key: &str,
    ) -> ApproveAndSubmitOutcome<GateError> {
        let now = at_kst(1_775_091_600);
        let (master, pending) = build_pending(now, key);
        let gate = ok_gate(&master);
        let presence = ScriptedPresence::new([OrderConfirmation::Confirmed], []);
        block_on(approve_and_submit(pending, &presence, &gate, broker))
    }

    fn requests(server: ScriptedHttpServer) -> Vec<String> {
        String::from_utf8(server.finish().expect("요청"))
            .expect("UTF-8")
            .split("POST ")
            .filter(|part| !part.is_empty())
            .map(|part| format!("POST {part}"))
            .collect()
    }

    #[test]
    fn approved_order_is_posted_once_with_key_account_and_flag() {
        let server = ScriptedHttpServer::start(vec![reply(
            "200 OK",
            r#"{"result":{"orderId":"toss-ord-1","clientOrderId":"t16-accept-01"}}"#,
        )])
        .expect("서버");
        let broker = trade(server.port(), FixedToken);
        let outcome = submit_once(&broker, "t16-accept-01");
        let ApproveAndSubmitOutcome::Submitted(OrderOutcome::Accepted(submitted)) = outcome else {
            panic!("접수 결과가 기대됨: {outcome:?}");
        };
        assert_eq!(submitted.broker_order_id().as_str(), "toss-ord-1");
        assert_eq!(submitted.client_order_id().as_str(), "t16-accept-01");
        let sent = requests(server);
        assert_eq!(sent.len(), 1, "정확히 1건 전송");
        let request = sent.first().expect("요청");
        assert!(
            request.starts_with("POST /api/v1/orders HTTP/1.1"),
            "{request}"
        );
        let lower = request.to_ascii_lowercase();
        assert!(lower.contains("authorization: bearer fake-access-91"));
        assert!(lower.contains("x-tossinvest-account: 1001"));
        let body = request.split("\r\n\r\n").nth(1).expect("본문");
        let json: serde_json::Value = serde_json::from_str(body).expect("JSON");
        assert_eq!(
            json,
            serde_json::json!({"clientOrderId":"t16-accept-01","symbol":"069500","side":"BUY","orderType":"LIMIT","timeInForce":"DAY","quantity":"10","price":"35000","confirmHighValueOrder":false})
        );
    }

    #[test]
    fn high_value_rejection_is_final_and_never_resent() {
        let server = ScriptedHttpServer::start(vec![reply(
            "400 Bad Request",
            r#"{"error":{"requestId":"r","code":"confirm-high-value-required","message":"x"}}"#,
        )])
        .expect("서버");
        let broker = trade(server.port(), FixedToken);
        let outcome = submit_once(&broker, "t16-high-02");
        let ApproveAndSubmitOutcome::Submitted(OrderOutcome::Rejected(rejection)) = outcome else {
            panic!("거절 결과가 기대됨: {outcome:?}");
        };
        assert_eq!(
            rejection.reason(),
            OrderRejectionReason::ConfirmHighValueRequired
        );
        assert_eq!(requests(server).len(), 1, "재전송 0건");
    }

    #[test]
    fn server_error_is_unknown_and_retry_reuses_the_same_key() {
        let server = ScriptedHttpServer::start(vec![
            reply(
                "500 Internal Server Error",
                r#"{"error":{"requestId":"r","code":"internal-error","message":"x"}}"#,
            ),
            reply(
                "200 OK",
                r#"{"result":{"orderId":"toss-ord-2","clientOrderId":"t16-retry-03"}}"#,
            ),
        ])
        .expect("서버");
        let broker = trade(server.port(), FixedToken);
        let outcome = submit_once(&broker, "t16-retry-03");
        let ApproveAndSubmitOutcome::Submitted(OrderOutcome::Unknown(unknown)) = outcome else {
            panic!("결과 모름이 기대됨: {outcome:?}");
        };
        assert_eq!(unknown.cause(), OrderUnknownCause::ServerError);
        assert_eq!(unknown.retry_eligibility(), RetryEligibility::Eligible);
        let permit = unknown.into_retry().expect("재시도 허가");
        let retried = block_on(retry_unknown_order(permit, at_kst(1_775_091_700), &broker));
        let RetryUnknownOutcome::Retried(OrderOutcome::Accepted(_)) = retried else {
            panic!("재시도 접수가 기대됨: {retried:?}");
        };
        let sent = requests(server);
        assert_eq!(sent.len(), 2);
        for request in &sent {
            assert!(
                request.contains(r#""clientOrderId":"t16-retry-03""#),
                "{request}"
            );
        }
    }

    #[test]
    fn maintenance_and_dropped_connection_never_resend_without_a_permit() {
        let server = ScriptedHttpServer::start(vec![reply(
            "500 Internal Server Error",
            r#"{"error":{"requestId":"r","code":"maintenance","message":"x"}}"#,
        )])
        .expect("서버");
        let broker = trade(server.port(), FixedToken);
        let outcome = submit_once(&broker, "t16-maint-04");
        let ApproveAndSubmitOutcome::Submitted(OrderOutcome::Unknown(unknown)) = outcome else {
            panic!("결과 모름이 기대됨: {outcome:?}");
        };
        assert_eq!(unknown.cause(), OrderUnknownCause::Maintenance);
        assert_eq!(unknown.retry_eligibility(), RetryEligibility::Forbidden);
        assert_eq!(requests(server).len(), 1);

        let dropped = ScriptedHttpServer::start(vec![Vec::new()]).expect("서버");
        let dropped_broker = trade(dropped.port(), FixedToken);
        let dropped_outcome = submit_once(&dropped_broker, "t16-drop-05");
        let ApproveAndSubmitOutcome::Submitted(OrderOutcome::Unknown(transport)) = dropped_outcome
        else {
            panic!("결과 모름이 기대됨: {dropped_outcome:?}");
        };
        assert_eq!(transport.cause(), OrderUnknownCause::Transport);
        assert_eq!(requests(dropped).len(), 1);
    }

    #[test]
    fn failed_recheck_or_pre_submit_record_opens_no_connection() {
        let now = at_kst(1_775_091_600);
        for (verify_fails, expected) in [
            (GateError::PostCheckFailed, (1, 0)),
            (GateError::CommitFailed, (1, 1)),
        ] {
            let probe = ConnectionProbe::bind().expect("리스너");
            let broker = trade(probe.port().expect("포트"), FixedToken);
            let (master, pending) = build_pending(now, "t16-gate-07");
            let recheck = ok_gate(&master).verify_result.clone().expect("재확인 입력");
            let gate = match verify_fails {
                GateError::PostCheckFailed => {
                    StubGate::new(Err(GateError::PostCheckFailed), Ok(at_kst(1_775_091_602)))
                }
                GateError::CommitFailed => StubGate::new(Ok(recheck), Err(GateError::CommitFailed)),
            };
            let presence = ScriptedPresence::new([OrderConfirmation::Confirmed], []);
            let outcome = block_on(approve_and_submit(pending, &presence, &gate, &broker));
            if let ApproveAndSubmitOutcome::Submitted(sent) = outcome {
                panic!("전송되면 안 됨: {sent:?}");
            }
            assert_eq!(gate.counts(), expected);
            assert_eq!(
                probe.observe().expect("관측"),
                ConnectionObservation::NoConnection
            );
        }
    }

    #[test]
    fn missing_token_rejects_without_opening_a_connection() {
        let probe = ConnectionProbe::bind().expect("리스너");
        let broker = trade(probe.port().expect("포트"), MissingToken);
        let outcome = submit_once(&broker, "t16-token-06");
        let ApproveAndSubmitOutcome::Submitted(OrderOutcome::Rejected(rejection)) = outcome else {
            panic!("거절 결과가 기대됨: {outcome:?}");
        };
        assert_eq!(
            rejection.reason(),
            OrderRejectionReason::AuthenticationRequired
        );
        assert_eq!(
            probe.observe().expect("관측"),
            ConnectionObservation::NoConnection
        );
    }

    fn at_kst(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각")
    }

    fn money(text: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(text).expect("입력"), currency).expect("금액")
    }

    fn price(text: &str, currency: Currency) -> Price {
        Price::try_new(money(text, currency)).expect("가격")
    }

    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(4),
            DayNumber::new(day),
        )
        .expect("날짜")
    }

    fn kr_etf_master(now: Timestamp) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            symbol: Symbol::try_new("069500").expect("심볼"),
            instrument_id: InstrumentId::try_new("kr-069500").expect("ID"),
            name: DisplayInstrumentName::try_new("테스트 ETF").expect("이름"),
            security_type: MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("유형")),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: now,
        })
        .expect("마스터")
    }

    fn kr_calendar() -> MarketCalendar {
        let day = |number: u8, start: i64, end: i64| {
            MarketSession::try_new(
                date(number),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: Some(
                        SessionData::try_new(
                            TimeInterval::try_new(at_kst(start), at_kst(end)).expect("구간"),
                            None,
                        )
                        .expect("세션"),
                    ),
                    after_market: None,
                },
            )
            .expect("일별")
        };
        MarketCalendar::try_new(
            day(1, 1_775_001_600, 1_775_025_000),
            day(2, 1_775_088_000, 1_775_111_400),
            day(3, 1_775_174_400, 1_775_197_800),
        )
        .expect("달력")
    }

    fn build_volatility(master: &MasterInstrument, now: Timestamp) -> Volatility {
        let midnight_kst = 1_775_055_600_i64;
        let adjustment = PriceAdjustment::Adjusted(AdjustmentBasis::new(now));
        let volume =
            Quantity::parse(&NumericInput::try_new("1000").expect("입력")).expect("거래량");
        let candles: Vec<Candle> = (0_i64..=60_i64)
            .map(|index| {
                let offset_days = 60_i64.saturating_sub(index);
                let stamp = at_kst(midnight_kst.saturating_sub(offset_days.saturating_mul(86_400)));
                let close = if index.rem_euclid(2) == 0 {
                    price("34500", Currency::Krw)
                } else {
                    price("35500", Currency::Krw)
                };
                let values = Ohlcv::try_new(close, close, close, close, volume).expect("봉");
                Candle::try_new(
                    master.row().instrument_id.clone(),
                    CandleTime::try_new(stamp, CandlePeriod::Daily).expect("일봉 시각"),
                    adjustment,
                    values,
                    CandleObservation::new(now, CandleStatus::Finalized),
                )
                .expect("일봉")
            })
            .collect();
        Volatility::calculate(&candles).expect("변동성")
    }

    fn build_draft(
        master: &MasterInstrument,
        account: AccountId,
        now: Timestamp,
        order_key: &str,
    ) -> (Proposal<domain::Draft>, domain::PriceObservation) {
        let resolved = master.order_instrument().expect("종목");
        let obs = master
            .price_observation(price("35000", Currency::Krw), now)
            .expect("관측");
        let task = EstimateTask::create(
            EstimateContext::new(obs.clone(), now),
            build_volatility(master, now),
            Horizon::default(),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제");
        let id = ProposalId::new(ClientOrderId::try_new(order_key).expect("키"));
        let intent = OrderIntent::new(
            account,
            resolved,
            id.as_client_order_id().clone(),
            OrderTerms::KrLimit {
                side: OrderSide::Buy,
                quantity: WholeOrderQuantity::try_new(
                    Quantity::parse(&NumericInput::try_new("10").expect("입력")).expect("수량"),
                )
                .expect("정수"),
                price: KrOrderPrice::try_new(price("35000", Currency::Krw)).expect("지정가"),
            },
        )
        .expect("의도");
        let range =
            ProposalPriceRange::from_estimate_task(task.range_facts(), obs.clone()).expect("범위");
        let draft =
            Proposal::new(id, intent, now, range, ProposalLifetime::default()).expect("초안");
        (draft, obs)
    }

    struct GateEnv {
        display_fact: AccountDisplayFact,
        accounts: BrokerAccountRows,
        calendars: Vec<MarketCalendar>,
        policy: MarketOrderPolicy,
        valuation: PortfolioValuation,
        assignments: SectorAssignments,
        security_facts: PolicySecurityFacts,
        open_orders: BrokerOrderSet,
        warnings: StockWarnings,
        limits: PriceLimits,
    }

    impl GateEnv {
        fn new(master: &MasterInstrument, account: &AccountId, now: Timestamp) -> Self {
            let resolved = master.order_instrument().expect("종목");
            let row = BrokerAccountRow::from_broker_account_result(
                account.clone(),
                BrokerAccountTypeCode::new("BROKERAGE"),
                BrokerAccountNumberInput::new("123456789012".to_owned()),
                now,
            )
            .expect("계좌");
            let display_fact = row.display_fact();
            let accounts = BrokerAccountRows::try_new(vec![row]).expect("계좌 목록");
            let valuation = Portfolio::new(
                Vec::new(),
                BuyingPower::new(money("10000000", Currency::Krw), now),
                BuyingPower::new(money("0", Currency::Usd), now),
            )
            .expect("포트폴리오")
            .evaluate(None);
            let facts_list = MasterInstrumentFacts::try_new(vec![master.clone()]).expect("목록");
            let catalog = SectorCatalog::from_response(Vec::new(), now).expect("카탈로그");
            let assignment =
                sector_assignment_from_compositions(master, &catalog, &[]).expect("섹터");
            let assignments = SectorAssignments::try_new(vec![assignment]).expect("섹터 목록");
            let open_orders = BrokerOrderSet::from_open_lookup_result(
                account.clone(),
                Vec::new(),
                OpenOrderCoverage::Complete,
                now,
            )
            .expect("미체결");
            let limits = PriceLimits::try_new(PriceLimitsInput {
                instrument: master.quote_instrument().expect("시세 종목"),
                currency: Currency::Krw,
                lower: Some(price("25000", Currency::Krw)),
                upper: Some(price("45000", Currency::Krw)),
                timestamp: now,
                collected_at: now,
            })
            .expect("제한");
            Self {
                display_fact,
                accounts,
                calendars: vec![kr_calendar()],
                policy: default_market_order_policy(Policy::new(InvestorProfile::default())),
                valuation,
                assignments,
                security_facts: facts_list.policy_security_facts(),
                open_orders,
                warnings: StockWarnings::from_lookup_result(resolved, Vec::new(), now),
                limits,
            }
        }
    }

    fn build_pending(
        now: Timestamp,
        order_key: &str,
    ) -> (MasterInstrument, PendingOrderConfirmation) {
        let master = kr_etf_master(now);
        let account = AccountId::new(1001);
        let (draft, obs) = build_draft(&master, account.clone(), now, order_key);
        let env = GateEnv::new(&master, &account, now);
        let gate_facts = OrderGateFacts {
            master: &master,
            accounts: &env.accounts,
            calendars: &env.calendars,
            market_order_policy: &env.policy,
            valuation: &env.valuation,
            assignments: &env.assignments,
            security_facts: &env.security_facts,
            buying_power: Some(BuyingPower::new(money("10000000", Currency::Krw), now)),
            sellable_quantity: None,
            open_orders: Some(&env.open_orders),
            stock_warnings: Some(&env.warnings),
            price_limits: Some(&env.limits),
            commission_rate: None,
            order_fx_rates: OrderFxRates::missing(),
        };
        let token = validate_proposal(draft, now, obs.clone(), &gate_facts)
            .expect("검증 통과")
            .into_execution(now, obs)
            .expect("토큰");
        let pending = PendingOrderConfirmation::from_execution(
            token,
            master.name_fact().expect("이름 사실"),
            env.display_fact,
            EstimatedOrderFee::Unknown,
            OrderFxRates::missing(),
        )
        .expect("요약");
        (master, pending)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum GateError {
        PostCheckFailed,
        CommitFailed,
    }

    struct StubGate {
        verify_result: Result<PostConfirmationRecheck, GateError>,
        commit_result: Result<Timestamp, GateError>,
        verify_calls: RwLock<usize>,
        commit_calls: RwLock<usize>,
    }

    impl StubGate {
        fn new(
            verify_result: Result<PostConfirmationRecheck, GateError>,
            commit_result: Result<Timestamp, GateError>,
        ) -> Self {
            Self {
                verify_result,
                commit_result,
                verify_calls: RwLock::new(0),
                commit_calls: RwLock::new(0),
            }
        }

        fn counts(&self) -> (usize, usize) {
            let verified = match self.verify_calls.read() {
                Ok(guard) => *guard,
                Err(poison) => *poison.into_inner(),
            };
            let committed = match self.commit_calls.read() {
                Ok(guard) => *guard,
                Err(poison) => *poison.into_inner(),
            };
            (verified, committed)
        }
    }

    impl PostApprovalGate for StubGate {
        type Error = GateError;

        fn verify_after_presence(
            &self,
            approval: &UserApproval,
        ) -> impl Future<Output = Result<PostConfirmationRecheck, Self::Error>> + Send {
            assert_eq!(
                approval.signature(),
                ApprovalSignatureSlot::PendingSecureEnclave,
                "승인 서명 자리 보존"
            );
            let mut count = match self.verify_calls.write() {
                Ok(guard) => guard,
                Err(poison) => poison.into_inner(),
            };
            *count = count.saturating_add(1);
            std::future::ready(self.verify_result.clone())
        }

        fn commit_before_submit(
            &self,
            _approval: &UserApproval,
        ) -> impl Future<Output = Result<Timestamp, Self::Error>> + Send {
            let mut count = match self.commit_calls.write() {
                Ok(guard) => guard,
                Err(poison) => poison.into_inner(),
            };
            *count = count.saturating_add(1);
            std::future::ready(self.commit_result)
        }
    }

    fn ok_gate(master: &MasterInstrument) -> StubGate {
        let recheck_obs = master
            .price_observation(price("35000", Currency::Krw), at_kst(1_775_091_601))
            .expect("재확인 관측");
        StubGate::new(
            Ok(PostConfirmationRecheck {
                now: at_kst(1_775_091_601),
                observation: recheck_obs,
            }),
            Ok(at_kst(1_775_091_602)),
        )
    }
}
