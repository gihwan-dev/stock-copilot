#[cfg(test)]
mod tests {
    use application::{
        AuditKind, HttpStatus, MissingData, OrderGateError, OrderGateOutcome,
        OrderGatePolicyContext, PortError, PortErrorKind, QuoteObservation, ReadEndpoint,
        RetryDelay, SystemFailure, SystemPort, SystemPortError, ValidateOrderProposal,
        ValidationReportRepository,
    };
    use domain::{
        AccountId, AdjustmentBasis, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
        BrokerAccountTypeCode, BuyingPower, CalendarDate, CalendarYear, Candle, CandleObservation,
        CandlePeriod, CandleStatus, CandleTime, CheckResult, ClientOrderId, CommonShare, Currency,
        DayNumber, DisplayInstrumentName, EstimateContext, EstimateTask, Horizon, HoursPermission,
        InstrumentId, InvestorProfile, KoreanMasterDetail, KrOrderPrice, LiquidationTrading,
        ListingStatus, MarketCalendar, MarketSession, MasterCode, MasterInstrument,
        MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, Money, MonthNumber,
        Nanosecond, NumericInput, NxtTrading, OffsetSeconds, Ohlcv, OpenOrderCoverage, OrderIntent,
        OrderSide, OrderTerms, Policy, Portfolio, Price, PriceAdjustment, PriceLimits,
        PriceLimitsInput, Proposal, ProposalId, ProposalLifetime, ProposalPriceRange, Quantity,
        Quote, RiskAppetite, SectorAssignments, SectorCatalog, SessionData, SessionInformation,
        StockWarning, StockWarnings, Symbol, TimeInterval, Timestamp, TradingSuspension,
        UnixSeconds, ValidationCheck, Volatility, WholeOrderQuantity, default_market_order_policy,
        sector_assignment_from_compositions,
    };
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};
    use testkit_ports::{
        SteppedClock,
        memory::MemoryReply,
        proposal_memory::MemoryValidationReportRepository,
        scripted::{ScriptedBrokerRead, ScriptedMarketInfo},
        system_fakes::MemoryAuditLog,
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
        let make_day = |day: u8, start: i64, end: i64| {
            MarketSession::try_new(
                date(day),
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
            make_day(1, 1_775_001_600, 1_775_025_000),
            make_day(2, 1_775_088_000, 1_775_111_400),
            make_day(3, 1_775_174_400, 1_775_197_800),
        )
        .expect("달력")
    }

    struct TestFixture {
        broker: ScriptedBrokerRead,
        market: ScriptedMarketInfo,
        reports: MemoryValidationReportRepository,
        audit: MemoryAuditLog,
        clock: SteppedClock,
        context: OrderGatePolicyContext,
        draft: Proposal<domain::Draft>,
    }

    fn scripted_broker(
        master: &MasterInstrument,
        account: &AccountId,
        now: Timestamp,
        warnings: Vec<StockWarning>,
    ) -> ScriptedBrokerRead {
        let quote_instrument = master.quote_instrument().expect("시세 종목");
        let resolved = master.order_instrument().expect("주문 종목");
        let broker = ScriptedBrokerRead::default();
        broker
            .script_accounts()
            .push(Ok(BrokerAccountRows::try_new(vec![
                BrokerAccountRow::from_broker_account_result(
                    account.clone(),
                    BrokerAccountTypeCode::new("BROKERAGE"),
                    BrokerAccountNumberInput::new("123456789012".to_owned()),
                    now,
                )
                .expect("계좌"),
            ])
            .expect("목록")));
        broker.script_quotes().push(Ok(vec![QuoteObservation {
            instrument: quote_instrument.clone(),
            result: Ok(Quote::try_new(
                resolved.symbol().clone(),
                Currency::Krw,
                Some(price("35000", Currency::Krw)),
                Some(now),
                now,
            )
            .expect("시세")),
        }]));
        broker
            .script_price_limits()
            .push(Ok(PriceLimits::try_new(PriceLimitsInput {
                instrument: quote_instrument,
                currency: Currency::Krw,
                lower: Some(price("25000", Currency::Krw)),
                upper: Some(price("45000", Currency::Krw)),
                timestamp: now,
                collected_at: now,
            })
            .expect("가격 제한")));
        broker
            .script_open_orders()
            .push(Ok(domain::BrokerOrderSet::from_open_lookup_result(
                account.clone(),
                Vec::new(),
                OpenOrderCoverage::Complete,
                now,
            )
            .expect("미체결")));
        broker.script_commission_rate().push(Err(PortError::new(
            ReadEndpoint::CommissionRate,
            HttpStatus::NotFound,
            PortErrorKind::NotFound(MissingData::Order),
        )));
        broker
            .script_buying_power()
            .push(Ok(BuyingPower::new(money("10000000", Currency::Krw), now)));
        broker
            .script_stock_warnings()
            .push(Ok(StockWarnings::from_lookup_result(
                resolved, warnings, now,
            )));
        broker
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
                let values = Ohlcv::try_new(close, close, close, close, volume).expect("봉 값");
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
    ) -> Proposal<domain::Draft> {
        let resolved = master.order_instrument().expect("주문 종목");
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
        .expect("추정 과제");
        let id = ProposalId::new(ClientOrderId::try_new("o01-app-1").expect("키"));
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
                price: KrOrderPrice::try_new(price("35000", Currency::Krw)).expect("가격"),
            },
        )
        .expect("의도");
        let range = ProposalPriceRange::from_estimate_task(task.range_facts(), obs).expect("범위");
        Proposal::new(id, intent, now, range, ProposalLifetime::default()).expect("초안")
    }

    fn setup(warnings: Vec<StockWarning>) -> TestFixture {
        let now = at_kst(1_775_091_600);
        let master = kr_etf_master(now);
        let account = AccountId::new(1001);
        let broker = scripted_broker(&master, &account, now, warnings);
        let market = ScriptedMarketInfo::default();
        market.script_calendar().push(Ok(kr_calendar()));
        let reports = MemoryValidationReportRepository::default();
        reports.script_append().push(MemoryReply::Memory);
        reports.script_history().push(MemoryReply::Memory);
        let audit = MemoryAuditLog::default();
        audit.script_append().push(MemoryReply::Memory);
        let facts = MasterInstrumentFacts::try_new(vec![master.clone()]).expect("목록");
        let catalog = SectorCatalog::from_response(Vec::new(), now).expect("카탈로그");
        let assignment =
            sector_assignment_from_compositions(&master, &catalog, &[]).expect("ETF 섹터");
        let assignments = SectorAssignments::try_new(vec![assignment]).expect("섹터");
        let valuation = Portfolio::new(
            Vec::new(),
            BuyingPower::new(money("10000000", Currency::Krw), now),
            BuyingPower::new(money("0", Currency::Usd), now),
        )
        .expect("포트폴리오")
        .evaluate(None);
        let context = OrderGatePolicyContext {
            master: master.clone(),
            market_order_policy: default_market_order_policy(Policy::new(
                InvestorProfile::default(),
            )),
            valuation,
            assignments,
            security_facts: facts.policy_security_facts(),
        };
        let draft = build_draft(&master, account, now);
        TestFixture {
            broker,
            market,
            reports,
            audit,
            clock: SteppedClock::new(now),
            context,
            draft,
        }
    }

    #[test]
    fn validate_order_proposal_collects_facts_records_report_and_audit() {
        let fixture = setup(Vec::new());
        let id = fixture.draft.id().clone();
        let use_case = ValidateOrderProposal::new(
            &fixture.broker,
            &fixture.market,
            &fixture.reports,
            &fixture.audit,
            &fixture.clock,
        );
        let outcome = block_on(use_case.execute(fixture.draft, &fixture.context)).expect("실행");
        match outcome {
            OrderGateOutcome::Passed {
                validated,
                collected,
                ..
            } => {
                assert_eq!(validated.id(), &id, "제안 ID 일치");
                assert_eq!(
                    collected.observation.price(),
                    price("35000", Currency::Krw),
                    "수집 시세 보존"
                );
            }
            OrderGateOutcome::Rejected { report } => {
                panic!("통과해야 할 제안이 거부됨: {report:?}");
            }
        }
        let history = block_on(fixture.reports.history(&id)).expect("보고서 이력");
        assert_eq!(history.len(), 1, "검증 보고서 1건 저장");
        assert_eq!(fixture.audit.entries().len(), 1, "감사 로그 1건 기록");
        assert_eq!(
            fixture.audit.entries()[0].kind(),
            &AuditKind::ValidationRecorded(id),
            "검증 감사 항목 일치"
        );
    }

    #[test]
    fn validate_order_proposal_rejects_warnings_and_fails_closed_on_audit_error() {
        let fixture = setup(vec![StockWarning::Overheated]);
        let use_case = ValidateOrderProposal::new(
            &fixture.broker,
            &fixture.market,
            &fixture.reports,
            &fixture.audit,
            &fixture.clock,
        );
        let outcome = block_on(use_case.execute(fixture.draft, &fixture.context)).expect("실행");
        match outcome {
            OrderGateOutcome::Rejected { report } => {
                assert_eq!(
                    report.result(ValidationCheck::BuyWarnings),
                    CheckResult::Failed,
                    "유의사항 실패 기록"
                );
            }
            OrderGateOutcome::Passed { .. } => panic!("유의사항 종목은 통과하면 안 된다"),
        }

        let fixture2 = setup(Vec::new());
        let failing_audit = MemoryAuditLog::default();
        let audit_err = SystemPortError::new(
            SystemPort::AuditLog,
            SystemFailure::Unavailable(RetryDelay::Unknown),
        );
        failing_audit
            .script_append()
            .push(MemoryReply::Return(Err(audit_err)));
        let use_case2 = ValidateOrderProposal::new(
            &fixture2.broker,
            &fixture2.market,
            &fixture2.reports,
            &failing_audit,
            &fixture2.clock,
        );
        assert_eq!(
            block_on(use_case2.execute(fixture2.draft, &fixture2.context)).expect_err("감사 실패"),
            OrderGateError::Storage(audit_err),
            "감사 기록 실패 시 검증 성공을 돌려주지 않는다"
        );
    }
}
