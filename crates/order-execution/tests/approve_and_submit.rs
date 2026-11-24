#[cfg(test)]
mod tests {
    use application::{OrderConfirmation, RiskChangeConfirmation};
    use domain::{
        AccountDisplayFact, AccountId, AdjustmentBasis, BrokerAccountNumberInput, BrokerAccountRow,
        BrokerAccountRows, BrokerAccountTypeCode, BrokerOrderId, BrokerOrderSet, BuyingPower,
        CalendarDate, CalendarYear, Candle, CandleObservation, CandlePeriod, CandleStatus,
        CandleTime, ClientOrderId, CommonShare, Currency, DayNumber, DisplayInstrumentName,
        EstimateContext, EstimateTask, EstimatedOrderFee, Horizon, HoursPermission, InstrumentId,
        InvestorProfile, KoreanMasterDetail, KrOrderPrice, LargeOrderConfirmation,
        LiquidationTrading, ListingStatus, Market, MarketCalendar, MarketOrderPolicy,
        MarketSession, MasterCode, MasterInstrument, MasterInstrumentFacts, MasterInstrumentRow,
        MasterSecurityType, Money, MonthNumber, Nanosecond, NumericInput, NxtTrading,
        OffsetSeconds, Ohlcv, OpenOrderCoverage, OrderFxRates, OrderGateFacts, OrderIntent,
        OrderSide, OrderTerms, PendingOrderConfirmation, Policy, PolicySecurityFacts, Portfolio,
        PortfolioValuation, Price, PriceAdjustment, PriceLimits, PriceLimitsInput, Proposal,
        ProposalExecutionError, ProposalId, ProposalLifetime, ProposalPriceRange, Quantity,
        RiskAppetite, RiskChange, SectorAssignments, SectorCatalog, SessionData,
        SessionInformation, StockWarnings, SummaryLimitPrice, SummaryQuantity, Symbol,
        TimeInterval, Timestamp, TradingSuspension, UnixSeconds, Volatility, WholeOrderQuantity,
        default_market_order_policy, sector_assignment_from_compositions, validate_proposal,
    };
    use order_execution::{
        ApprovalSignatureSlot, ApproveAndSubmitOutcome, ApprovedOrder, BrokerTrade, OrderOutcome,
        OrderRejectionReason, OrderUnknownCause, PostApprovalGate, PostConfirmationRecheck,
        RetryEligibility, RetryUnknownOutcome, UnknownOrderOutcome, UserApproval,
        approve_and_submit, retry_unknown_order,
    };
    use std::collections::VecDeque;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::RwLock;
    use std::task::{Context, Poll, Waker};
    use testkit_ports::{
        PresenceCall, RecordedOrderSummary, ScriptedPresence,
        contracts::{user_presence_order, user_presence_risk_change},
    };

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut pinned = Box::pin(future);
        loop {
            match Pin::as_mut(&mut pinned).poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
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

    #[derive(Clone, Copy)]
    enum BrokerReply {
        Accept,
        Reject(OrderRejectionReason),
        Unknown(OrderUnknownCause),
    }

    struct RecordingFakeBroker {
        replies: RwLock<VecDeque<BrokerReply>>,
        submitted: RwLock<Vec<(ClientOrderId, LargeOrderConfirmation, ApprovalSignatureSlot)>>,
    }

    impl RecordingFakeBroker {
        fn new<Replies: IntoIterator<Item = BrokerReply>>(replies: Replies) -> Self {
            Self {
                replies: RwLock::new(replies.into_iter().collect()),
                submitted: RwLock::new(Vec::new()),
            }
        }

        fn submitted(&self) -> Vec<(ClientOrderId, LargeOrderConfirmation, ApprovalSignatureSlot)> {
            match self.submitted.read() {
                Ok(guard) => guard.clone(),
                Err(poison) => poison.into_inner().clone(),
            }
        }
    }

    impl BrokerTrade for RecordingFakeBroker {
        fn submit(&self, order: ApprovedOrder) -> impl Future<Output = OrderOutcome> + Send {
            let mut log = match self.submitted.write() {
                Ok(guard) => guard,
                Err(poison) => poison.into_inner(),
            };
            log.push((
                order.client_order_id().clone(),
                order.large_order_confirmation(),
                order.signature(),
            ));
            let next = match self.replies.write() {
                Ok(mut guard) => guard.pop_front(),
                Err(poison) => poison.into_inner().pop_front(),
            };
            let outcome = match next.expect("브로커 대본") {
                BrokerReply::Accept => {
                    order.into_accepted(BrokerOrderId::try_new("toss-ord-1").expect("주문번호"))
                }
                BrokerReply::Reject(reason) => order.into_rejected(reason),
                BrokerReply::Unknown(cause) => order.into_unknown(cause),
            };
            std::future::ready(outcome)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum OutcomeTag {
        PresenceDenied,
        PresenceUnavailable,
        PostPresenceRejected(GateError),
        RecheckFailed(ProposalExecutionError),
        CommitFailed(GateError),
        ClockRollbackBeforeSubmit,
        Accepted,
        Rejected(OrderRejectionReason),
        Unknown(OrderUnknownCause),
    }

    fn tag_of(outcome: &ApproveAndSubmitOutcome<GateError>) -> OutcomeTag {
        match outcome {
            ApproveAndSubmitOutcome::PresenceDenied => OutcomeTag::PresenceDenied,
            ApproveAndSubmitOutcome::PresenceUnavailable => OutcomeTag::PresenceUnavailable,
            ApproveAndSubmitOutcome::PostPresenceRejected(err) => {
                OutcomeTag::PostPresenceRejected(*err)
            }
            ApproveAndSubmitOutcome::RecheckFailed(err) => OutcomeTag::RecheckFailed(*err),
            ApproveAndSubmitOutcome::CommitFailed(err) => OutcomeTag::CommitFailed(*err),
            ApproveAndSubmitOutcome::ClockRollbackBeforeSubmit => {
                OutcomeTag::ClockRollbackBeforeSubmit
            }
            ApproveAndSubmitOutcome::Submitted(OrderOutcome::Accepted(_)) => OutcomeTag::Accepted,
            ApproveAndSubmitOutcome::Submitted(OrderOutcome::Rejected(rejection)) => {
                OutcomeTag::Rejected(rejection.reason())
            }
            ApproveAndSubmitOutcome::Submitted(OrderOutcome::Unknown(unknown)) => {
                OutcomeTag::Unknown(unknown.cause())
            }
        }
    }

    fn expect_unknown(outcome: ApproveAndSubmitOutcome<GateError>) -> UnknownOrderOutcome {
        match outcome {
            ApproveAndSubmitOutcome::Submitted(OrderOutcome::Unknown(unknown)) => unknown,
            ApproveAndSubmitOutcome::PresenceDenied
            | ApproveAndSubmitOutcome::PresenceUnavailable
            | ApproveAndSubmitOutcome::PostPresenceRejected(_)
            | ApproveAndSubmitOutcome::RecheckFailed(_)
            | ApproveAndSubmitOutcome::CommitFailed(_)
            | ApproveAndSubmitOutcome::ClockRollbackBeforeSubmit
            | ApproveAndSubmitOutcome::Submitted(
                OrderOutcome::Accepted(_) | OrderOutcome::Rejected(_),
            ) => panic!("Unknown 결과가 기대됨"),
        }
    }

    fn expect_retried_unknown(outcome: RetryUnknownOutcome) -> UnknownOrderOutcome {
        match outcome {
            RetryUnknownOutcome::Retried(OrderOutcome::Unknown(unknown)) => unknown,
            RetryUnknownOutcome::ClockRollback
            | RetryUnknownOutcome::WindowExpired
            | RetryUnknownOutcome::Retried(OrderOutcome::Accepted(_) | OrderOutcome::Rejected(_)) =>
            {
                panic!("재시도 Unknown 결과가 기대됨")
            }
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

    #[test]
    fn confirmed_presence_submits_once_and_denied_or_unavailable_submit_zero() {
        let now = at_kst(1_775_091_600);
        let (master, pending_ok) = build_pending(now, "o17-accept-01");
        let gate_ok = ok_gate(&master);
        let presence = ScriptedPresence::new(
            [
                OrderConfirmation::Confirmed,
                OrderConfirmation::Denied,
                OrderConfirmation::Unavailable,
            ],
            [],
        );
        let broker = RecordingFakeBroker::new([BrokerReply::Accept]);
        assert_eq!(
            tag_of(&block_on(approve_and_submit(
                pending_ok, &presence, &gate_ok, &broker
            ))),
            OutcomeTag::Accepted,
            "승인 전송 성공"
        );
        assert_eq!(gate_ok.counts(), (1, 1), "재확인 1회·전송 전 커밋 1회");
        assert_eq!(broker.submitted().len(), 1, "정확히 1건 전송");

        let gate_never = StubGate::new(
            Err(GateError::PostCheckFailed),
            Err(GateError::CommitFailed),
        );
        let broker_never = RecordingFakeBroker::new([]);
        let (_, pending_denied) = build_pending(now, "o17-denied-02");
        let (_, pending_unavail) = build_pending(now, "o17-unavail-03");
        assert_eq!(
            tag_of(&block_on(approve_and_submit(
                pending_denied,
                &presence,
                &gate_never,
                &broker_never
            ))),
            OutcomeTag::PresenceDenied,
            "거부 시 중단"
        );
        assert_eq!(
            tag_of(&block_on(approve_and_submit(
                pending_unavail,
                &presence,
                &gate_never,
                &broker_never
            ))),
            OutcomeTag::PresenceUnavailable,
            "사용 불가 시 중단"
        );
        assert_eq!(gate_never.counts(), (0, 0), "후속 게이트 0회");
        assert!(broker_never.submitted().is_empty(), "주문 전송 0건");
    }

    #[test]
    fn post_presence_and_recheck_failures_stop_before_commit_and_submit() {
        let now = at_kst(1_775_091_600);
        let (master, pending1) = build_pending(now, "o17-fail-1");
        let presence = ScriptedPresence::new(
            [OrderConfirmation::Confirmed, OrderConfirmation::Confirmed],
            [],
        );
        let broker = RecordingFakeBroker::new([]);
        let gate_verify_err =
            StubGate::new(Err(GateError::PostCheckFailed), Ok(at_kst(1_775_091_602)));
        assert_eq!(
            tag_of(&block_on(approve_and_submit(
                pending1,
                &presence,
                &gate_verify_err,
                &broker
            ))),
            OutcomeTag::PostPresenceRejected(GateError::PostCheckFailed),
            "확인 후 검증 실패 시 중단"
        );
        assert_eq!(gate_verify_err.counts(), (1, 0), "커밋 호출 0회");

        let (_, pending2) = build_pending(now, "o17-fail-2");
        let expired_obs = master
            .price_observation(price("35000", Currency::Krw), at_kst(1_775_092_200))
            .expect("만료 관측");
        let gate_recheck_err = StubGate::new(
            Ok(PostConfirmationRecheck {
                now: at_kst(1_775_092_200),
                observation: expired_obs,
            }),
            Ok(at_kst(1_775_092_201)),
        );
        assert_eq!(
            tag_of(&block_on(approve_and_submit(
                pending2,
                &presence,
                &gate_recheck_err,
                &broker
            ))),
            OutcomeTag::RecheckFailed(ProposalExecutionError::Expired),
            "확인 창 동안 만료 시 중단"
        );
        assert_eq!(gate_recheck_err.counts(), (1, 0), "만료 시 커밋 호출 0회");
        assert!(broker.submitted().is_empty(), "전송 0건");
    }

    #[test]
    fn commit_failure_and_rollback_submit_zero() {
        let now = at_kst(1_775_091_600);
        let (master, pending3) = build_pending(now, "o17-fail-3");
        let fresh_obs = master
            .price_observation(price("35000", Currency::Krw), at_kst(1_775_091_601))
            .expect("관측");
        let presence = ScriptedPresence::new(
            [OrderConfirmation::Confirmed, OrderConfirmation::Confirmed],
            [],
        );
        let broker_empty = RecordingFakeBroker::new([]);
        let gate_commit_err = StubGate::new(
            Ok(PostConfirmationRecheck {
                now: at_kst(1_775_091_601),
                observation: fresh_obs.clone(),
            }),
            Err(GateError::CommitFailed),
        );
        assert_eq!(
            tag_of(&block_on(approve_and_submit(
                pending3,
                &presence,
                &gate_commit_err,
                &broker_empty
            ))),
            OutcomeTag::CommitFailed(GateError::CommitFailed),
            "전송 전 원자 기록 실패 시 중단"
        );
        let (_, pending4) = build_pending(now, "o17-fail-4");
        let gate_rollback = StubGate::new(
            Ok(PostConfirmationRecheck {
                now: at_kst(1_775_091_601),
                observation: fresh_obs,
            }),
            Ok(at_kst(1_775_091_600)),
        );
        assert_eq!(
            tag_of(&block_on(approve_and_submit(
                pending4,
                &presence,
                &gate_rollback,
                &broker_empty
            ))),
            OutcomeTag::ClockRollbackBeforeSubmit,
            "커밋 시각 역행 시 중단"
        );
        assert!(broker_empty.submitted().is_empty(), "전송 0건");
    }

    #[test]
    fn broker_rejection_and_maintenance_forbid_retry() {
        let now = at_kst(1_775_091_600);
        let (master, pending_rej) = build_pending(now, "o17-reject-5");
        let gate_ok = ok_gate(&master);
        let presence = ScriptedPresence::new(
            [OrderConfirmation::Confirmed, OrderConfirmation::Confirmed],
            [],
        );
        let broker_reject = RecordingFakeBroker::new([BrokerReply::Reject(
            OrderRejectionReason::ConfirmHighValueRequired,
        )]);
        assert_eq!(
            tag_of(&block_on(approve_and_submit(
                pending_rej,
                &presence,
                &gate_ok,
                &broker_reject
            ))),
            OutcomeTag::Rejected(OrderRejectionReason::ConfirmHighValueRequired),
            "브로커 거절 사유 보존"
        );
        let (_, pending_maint) = build_pending(now, "o17-maint-1");
        let broker_maint =
            RecordingFakeBroker::new([BrokerReply::Unknown(OrderUnknownCause::Maintenance)]);
        let unknown = expect_unknown(block_on(approve_and_submit(
            pending_maint,
            &presence,
            &gate_ok,
            &broker_maint,
        )));
        assert_eq!(
            unknown.retry_eligibility(),
            RetryEligibility::Forbidden,
            "점검 재시도 불가"
        );
        assert!(unknown.into_retry().is_none(), "점검 재시도 토큰 없음");
    }

    #[test]
    fn transport_unknown_enforces_fixed_ten_minute_retry_window() {
        let now = at_kst(1_775_091_600);
        let (master, pending_net) = build_pending(now, "o17-net-2");
        let gate = ok_gate(&master);
        let presence = ScriptedPresence::new([OrderConfirmation::Confirmed], []);
        let broker_retry = RecordingFakeBroker::new([
            BrokerReply::Unknown(OrderUnknownCause::Transport),
            BrokerReply::Unknown(OrderUnknownCause::ServerError),
            BrokerReply::Accept,
        ]);
        let unknown1 = expect_unknown(block_on(approve_and_submit(
            pending_net,
            &presence,
            &gate,
            &broker_retry,
        )));
        assert_eq!(
            unknown1.retry_eligibility(),
            RetryEligibility::Eligible,
            "재시도 가능"
        );
        let first_permit = unknown1.into_retry().expect("재시도 허가");
        let unknown2 = expect_retried_unknown(block_on(retry_unknown_order(
            first_permit,
            at_kst(1_775_092_201),
            &broker_retry,
        )));
        let second_permit = unknown2.into_retry().expect("두 번째 재시도 허가");
        assert_eq!(
            second_permit.first_sent_at(),
            at_kst(1_775_091_602),
            "최초 시각 고정"
        );
        let expired = block_on(retry_unknown_order(
            second_permit,
            at_kst(1_775_092_202),
            &broker_retry,
        ));
        assert!(
            match expired {
                RetryUnknownOutcome::WindowExpired => true,
                RetryUnknownOutcome::ClockRollback | RetryUnknownOutcome::Retried(_) => false,
            },
            "정확히 10분 경과 시 재시도 금지"
        );
        assert_eq!(broker_retry.submitted().len(), 2, "만료 후 추가 전송 없음");
    }

    #[test]
    fn a04_handoff_verifies_scripted_presence_and_user_presence_order_contract() {
        let now = at_kst(1_775_091_600);
        let (_, pending) = build_pending(now, "abcdefgh-1234");
        let summary = pending.summary();
        let presence = ScriptedPresence::new(
            [
                OrderConfirmation::Confirmed,
                OrderConfirmation::Denied,
                OrderConfirmation::Unavailable,
            ],
            [RiskChangeConfirmation::Confirmed],
        );
        block_on(user_presence_order(
            &presence,
            summary,
            OrderConfirmation::Confirmed,
        ));
        block_on(user_presence_risk_change(
            &presence,
            RiskChange::EnableOrders,
            RiskChangeConfirmation::Confirmed,
        ));
        block_on(user_presence_order(
            &presence,
            summary,
            OrderConfirmation::Denied,
        ));
        block_on(user_presence_order(
            &presence,
            summary,
            OrderConfirmation::Unavailable,
        ));
        let recorded = presence.order_calls();
        assert_eq!(recorded.len(), 3, "주문 확인 3회 호출");
        assert_eq!(
            presence.risk_change_calls().len(),
            1,
            "위험 설정 확인 1회 호출"
        );
        let expected = RecordedOrderSummary::from_summary(summary);
        assert_recorded_ten_fields(&recorded[0], &expected);
        assert_eq!(
            presence.calls(),
            vec![
                PresenceCall::Order(Box::new(expected.clone())),
                PresenceCall::RiskChange(RiskChange::EnableOrders),
                PresenceCall::Order(Box::new(expected.clone())),
                PresenceCall::Order(Box::new(expected)),
            ],
            "FIFO 호출 순서와 요청 종류 보존"
        );
    }

    fn assert_recorded_ten_fields(actual: &RecordedOrderSummary, expected: &RecordedOrderSummary) {
        assert_eq!(actual, expected, "요약 열 필드 일치");
        assert_eq!(actual.instrument_name.as_str(), "테스트 ETF", "종목명");
        assert_eq!(actual.symbol.as_str(), "069500", "종목코드");
        assert_eq!(actual.market, Market::Kr, "시장");
        assert_eq!(actual.side, OrderSide::Buy, "방향");
        assert_eq!(
            actual.quantity,
            SummaryQuantity::Specified(
                Quantity::parse(&NumericInput::try_new("10").expect("입력")).expect("수량")
            ),
            "수량"
        );
        assert_eq!(
            actual.limit_price,
            SummaryLimitPrice::Specified(price("35000", Currency::Krw)),
            "지정가"
        );
        assert_eq!(
            actual.expected_amount.money(),
            money("350000", Currency::Krw),
            "예상 금액"
        );
        assert_eq!(
            actual.expected_fee,
            EstimatedOrderFee::Unknown,
            "예상 수수료"
        );
        assert_eq!(actual.account_suffix.to_string(), "9012", "계좌 뒤 4자리");
        assert_eq!(actual.client_order_prefix.as_str(), "abcdefgh", "키 앞 8자");
    }
}
