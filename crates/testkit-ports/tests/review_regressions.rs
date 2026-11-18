#[cfg(test)]
mod tests {
    use application::{
        AuditEntry, AuditKind, FxRateQuery, MarketDataRepository, RiskChangeConfirmation,
        SecretKey, SecretValue, SettingsRepository, SignalRepository, SystemFailure, SystemPort,
        SystemPortError, WriteCompleted,
    };
    use domain::{
        AccountId, AcquisitionPrice, AdjustmentBasis, AiProbability, AiProbabilityInput,
        BuyingPower, Calibration, CalibrationEstimateId, CalibrationKey, CalibrationModel, Candle,
        CandleObservation, CandlePeriod, CandleStatus, CandleTime, ChangeReason, Claim,
        ClaimContent, ClaimId, ClaimRole, ClientOrderId, CollectedItem, CommissionRateFact,
        CommissionValidity, CommonShare, CostEligibleTask, CostRate, Currency,
        DisplayInstrumentName, Draft, EstimateContext, EstimateCostPeriod, EstimateOutcome,
        EstimateTask, Evidence, EvidenceRef, EvidenceUrl, FxPair, FxRate, FxRateKind, FxSource,
        HistoryEntry, Holding, HoldingObservation, Horizon, HorizonSelection, HoursPermission,
        HoursPermissionState, InstrumentId, InvestorProfile, KoreanMasterDetail,
        LiquidationTrading, ListingStatus, Margin, Market, MarketCalendar, MarketSession,
        MasterCode, MasterInstrument, MasterInstrumentRow, MasterSecurityType, ModelIdentityPart,
        Money, Nanosecond, NewsItem, NewsSource, NormalizedUrl, NumericInput, NxtTrading,
        OffsetSeconds, Ohlcv, OrderIntent, OrderPermission, OrderSide, OrderTerms,
        OutcomeAdjustment, OutcomeUnavailable, Policy, PolicySecurityFacts, Portfolio, Price,
        PriceAdjustment, Proposal, ProposalId, ProposalLifetime, ProposalPriceRange,
        ProposedChange, Quantity, Quote, RaisedSignal, Ratio, ReportedHolding,
        ResolvedOrderInstrument, RiskAppetite, RiskChange, RiskSettings, SavedMarketOrderSetting,
        SectorAssignments, SessionData, SessionInformation, SignalContext, SignalDecision,
        SignalEstimate, SignalSourceLedger, SignalSubmission, SignalTaskKey, SignalTiming,
        SnapshotCalendar, Symbol, SyncOutcome, SyncRecord, Thesis, ThesisOperation, ThesisPatch,
        TimeInterval, Timestamp, TradeCost, TradeTaxFact, TradingSuspension, UnixSeconds,
        UntrustedText, UserReason, ValuationSnapshot, Volatility, WholeOrderQuantity,
        calibrate_signal_submission,
    };
    use std::future::Future;
    use testkit_ports::{
        contracts,
        memory::{
            MemoryEstimateRepository, MemoryMarketDataRepository, MemoryNewsRepository,
            MemoryPortfolioRepository, MemoryProposalRepository, MemoryReply,
            MemorySettingsRepository, MemorySignalRepository, MemoryThesisRepository,
            MemoryTransactionRepository, MemoryValuationRepository,
        },
    };
    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
        {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn link_news(repository: &MemoryNewsRepository, instrument: InstrumentId, item: &NewsItem) {
        let domain::InformationSource::News(source) = item.collected().source() else {
            panic!("news source");
        };
        let key = application::InformationKey::News(source.clone(), item.collected().url().clone());
        run(application::InformationLinks::set_instruments(
            repository,
            &key,
            &[instrument],
        ))
        .expect("link");
    }
    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("종목")
    }
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn memory<I, O>(script: &testkit_ports::Script<I, MemoryReply<O>>, count: usize) {
        for _ in 0..count {
            script.push(MemoryReply::Memory);
        }
    }
    #[test]
    fn linked_news_is_visible_only_after_a_successful_store() {
        let repository = MemoryNewsRepository::default();
        let instrument = id("news-a");
        let item = NewsItem::new(
            NewsSource::Yonhap,
            NormalizedUrl::try_new("https://example.com/article").expect("주소"),
            UntrustedText::try_new("기사").expect("제목"),
            None,
        );
        link_news(&repository, instrument.clone(), &item);
        link_news(&repository, instrument.clone(), &item);
        memory(repository.script_for_instrument(), 3);
        run(contracts::news_repository_for_instrument(
            &repository,
            &instrument,
            Ok(vec![]),
        ));
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        repository
            .script_store()
            .push(MemoryReply::Return(Err(error)));
        run(contracts::news_repository_store(
            &repository,
            &item,
            Err(error),
        ));
        run(contracts::news_repository_for_instrument(
            &repository,
            &instrument,
            Ok(vec![]),
        ));
        memory(repository.script_store(), 1);
        run(contracts::news_repository_store(
            &repository,
            &item,
            Ok(WriteCompleted),
        ));
        run(contracts::news_repository_for_instrument(
            &repository,
            &instrument,
            Ok(vec![item]),
        ));
    }
    fn candle(seconds: i64, value: &str) -> Candle {
        candle_for(
            id("candle-a"),
            seconds,
            CandlePeriod::Minute,
            PriceAdjustment::Raw,
            value,
        )
    }
    fn candle_for(
        instrument: InstrumentId,
        seconds: i64,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
        value: &str,
    ) -> Candle {
        let price = Price::parse(&NumericInput::try_new(value).expect("숫자"), Currency::Krw)
            .expect("가격");
        Candle::try_new(
            instrument,
            CandleTime::try_new(at(seconds), period).expect("봉 시각"),
            adjustment,
            Ohlcv::try_new(price, price, price, price, quantity("7")).expect("봉"),
            CandleObservation::new(at(300), CandleStatus::Finalized),
        )
        .expect("봉")
    }
    fn fx_rate(value: &str, pair: FxPair, kind: FxRateKind, start: i64, end: i64) -> FxRate {
        FxRate::parse(
            &NumericInput::try_new(value).expect("숫자"),
            pair,
            kind,
            TimeInterval::try_new(at(start), at(end)).expect("기간"),
            FxSource::TossOpenApi,
        )
        .expect("환율")
    }
    #[test]
    fn identical_candles_in_one_batch_are_stored_once() {
        let repository = MemoryMarketDataRepository::default();
        let value = candle(60, "103");
        memory(repository.script_store_candles(), 1);
        memory(repository.script_candles(), 1);
        run(contracts::market_data_repository_store_candles(
            &repository,
            &id("candle-a"),
            &[value.clone(), value.clone()],
            Ok(WriteCompleted),
        ));
        run(contracts::market_data_repository_candles(
            &repository,
            &id("candle-a"),
            (
                TimeInterval::try_new(at(0), at(120)).expect("기간"),
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            ),
            Ok(vec![value]),
        ));
    }
    fn price(value: &str) -> Price {
        Price::parse(&NumericInput::try_new(value).expect("입력"), Currency::Krw).expect("가격")
    }
    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액")
    }
    fn quantity(value: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(value).expect("입력")).expect("수량")
    }
    fn ratio(value: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(value).expect("입력")).expect("비율")
    }
    fn master() -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            instrument_id: id("sample-a"),
            symbol: Symbol::try_new("005930").expect("코드"),
            name: DisplayInstrumentName::try_new("고정 종목").expect("이름"),
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
            observed_at: day(60),
        })
        .expect("마스터")
    }
    fn other_master() -> MasterInstrument {
        let mut row = master().row().clone();
        row.instrument_id = id("sample-b");
        row.symbol = Symbol::try_new("000660").expect("다른 코드");
        row.name = DisplayInstrumentName::try_new("다른 고정 종목").expect("이름");
        MasterInstrument::try_new(row).expect("다른 마스터")
    }
    fn instrument() -> ResolvedOrderInstrument {
        master().order_instrument().expect("종목")
    }
    fn local(time: Timestamp) -> Timestamp {
        Timestamp::from_parts(
            time.utc_seconds(),
            time.nanosecond(),
            OffsetSeconds::new(32400),
        )
        .expect("현지 시각")
    }
    fn local_midnight(value: i64) -> Timestamp {
        local(at(day(value)
            .utc_seconds()
            .as_i64()
            .checked_sub(32400)
            .expect("현지 자정")))
    }
    fn day(value: i64) -> Timestamp {
        at(value.checked_mul(86400).expect("일"))
    }
    fn session(value: i64, market: &Market) -> MarketSession {
        let start = day(value);
        let end = at(start.utc_seconds().as_i64().checked_add(3600).expect("끝"));
        let data = Some(
            SessionData::try_new(TimeInterval::try_new(start, end).expect("구간"), None)
                .expect("장"),
        );
        let information = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
        };
        MarketSession::try_new(start.offset_calendar_date(), information).expect("세션")
    }
    fn calendar(value: i64, market: &Market) -> MarketCalendar {
        MarketCalendar::try_new(
            session(value.checked_sub(1).expect("전일"), market),
            session(value, market),
            session(value.checked_add(1).expect("익일"), market),
        )
        .expect("달력")
    }
    fn eligible() -> CostEligibleTask {
        eligible_for("102")
    }
    fn eligible_for(high: &str) -> CostEligibleTask {
        eligible_for_master(high, &master())
    }
    fn eligible_for_master(high: &str, source: &MasterInstrument) -> CostEligibleTask {
        let candles: Vec<_> = (0_i64..61)
            .map(|index| {
                let fixture_price = price(if index % 2 == 0 { "100" } else { high });
                Candle::try_new(
                    source.row().instrument_id.clone(),
                    CandleTime::try_new(local_midnight(index), CandlePeriod::Daily)
                        .expect("일봉 시각"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(day(61))),
                    Ohlcv::try_new(
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        quantity("13"),
                    )
                    .expect("OHLCV"),
                    CandleObservation::new(day(61), CandleStatus::Finalized),
                )
                .expect("일봉")
            })
            .collect();
        let volatility = Volatility::calculate(&candles).expect("변동성");
        let task = EstimateTask::create(
            EstimateContext::new(
                source
                    .price_observation(price("100"), local(day(60)))
                    .expect("관측"),
                day(60),
            ),
            volatility,
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제");
        let zero = CostRate::try_new(ratio("0")).expect("비용");
        let period = EstimateCostPeriod::from_calendar_result(
            &task,
            at(day(65)
                .utc_seconds()
                .as_i64()
                .checked_add(3600)
                .expect("끝")),
        )
        .expect("만기");
        let cost = TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                zero,
                CommissionValidity::try_new(None, None).expect("유효"),
            )),
            Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
            period,
        )
        .expect("비용");
        CostEligibleTask::assess(task, &cost).expect("적격")
    }
    fn calibration_key() -> CalibrationKey {
        let part = ModelIdentityPart::try_new("fixture").expect("모델");
        CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::new(HorizonSelection::FewDays),
        )
    }
    fn submission(task: &CostEligibleTask) -> SignalSubmission {
        SignalSubmission::create(
            SignalTaskKey::from_task(task),
            calibration_key(),
            AiProbability::parse(&AiProbabilityInput::try_new("10").expect("확률 입력"))
                .expect("확률"),
            Some(
                Evidence::try_new(vec![EvidenceRef::Web(
                    EvidenceUrl::try_new(
                        NormalizedUrl::try_new("https://example.com/evidence").expect("주소"),
                    )
                    .expect("근거 주소"),
                )])
                .expect("근거"),
            ),
            task.task().created_at(),
        )
        .expect("제출")
    }
    fn signal(held: &str) -> RaisedSignal {
        let task = eligible();
        let key = calibration_key();
        let calibration = Calibration::try_new(key, &[]).expect("보정");
        let submitted = calibrate_signal_submission(&calibration, submission(&task));
        let estimate =
            SignalEstimate::create(&task, &submitted, Margin::default()).expect("신호 입력");
        let at = task.task().created_at();
        let holding = Holding::new(
            instrument(),
            quantity(held),
            AcquisitionPrice::try_new(money("87", Currency::Krw)).expect("원가"),
            HoldingObservation::new(
                None,
                Some(
                    ReportedHolding::new(
                        money("113", Currency::Krw),
                        money("7", Currency::Krw),
                        ratio("0.07"),
                    )
                    .expect("평가"),
                ),
                at,
            ),
        )
        .expect("보유");
        let portfolio = Portfolio::new(
            vec![holding],
            BuyingPower::new(money("131", Currency::Krw), at),
            BuyingPower::new(money("0", Currency::Usd), at),
        )
        .expect("포트")
        .evaluate(None);
        let policy = Policy::new(InvestorProfile::default());
        let assignments = SectorAssignments::try_new(vec![]).expect("분류");
        let security = PolicySecurityFacts::default();
        let item = CollectedItem::from_web(
            NormalizedUrl::try_new("https://example.com/evidence").expect("주소"),
            UntrustedText::try_new("근거").expect("제목"),
            None,
        );
        let items = [item];
        let sources = SignalSourceLedger::new(&items, &[], &[]);
        let calendars: Vec<_> = (60..=66).map(|day| calendar(day, &Market::Kr)).collect();
        let context = SignalContext::new(
            Some(&portfolio),
            &policy,
            (&assignments, &security),
            &sources,
            SignalTiming::new(at, &calendars),
        );
        SignalDecision::assess(&estimate, &context)
            .into_signal()
            .expect("매도 신호")
    }
    fn change(name: &str) -> ProposedChange {
        let text = |value| UntrustedText::try_new(value).expect("텍스트");
        ProposedChange::new(
            ThesisOperation::AddClaim(Claim::new(
                ClaimId::try_new(name).expect("논거 ID"),
                ClaimRole::Supporting,
                ClaimContent::try_new(
                    text("매출"),
                    text("성장"),
                    text("상한"),
                    text("공시"),
                    text("반증"),
                )
                .expect("논거"),
            )),
            Evidence::try_new(vec![EvidenceRef::Web(
                EvidenceUrl::try_new(
                    NormalizedUrl::try_new("https://example.com/a").expect("주소"),
                )
                .expect("근거"),
            )])
            .expect("증거"),
            ChangeReason::try_new(text("근거"), text("model"), text("v1")).expect("이유"),
        )
    }
    fn thesis() -> Thesis {
        let mut thesis = Thesis::new();
        thesis
            .apply_batch(
                thesis.version(),
                ThesisPatch::try_new(vec![change("claim-a"), change("claim-b")]).expect("패치"),
            )
            .expect("적용");
        let history = thesis.history().to_vec();
        let mut targets = history.iter().map(HistoryEntry::sequence);
        thesis
            .confirm_by_user(
                thesis.version(),
                targets.next().expect("첫 변경"),
                UserReason::try_new(UntrustedText::try_new("확인").expect("이유"))
                    .expect("확인 이유"),
            )
            .expect("확인");
        thesis
            .reject_by_user(
                thesis.version(),
                targets.next().expect("둘째 변경"),
                UserReason::try_new(UntrustedText::try_new("거절").expect("이유"))
                    .expect("거절 이유"),
            )
            .expect("거절");
        thesis
    }
    fn draft(sigma: &str) -> Proposal<Draft> {
        draft_for("same-draft", sigma)
    }
    fn draft_for(key_text: &str, sigma: &str) -> Proposal<Draft> {
        let key = ProposalId::new(ClientOrderId::try_new(key_text).expect("ID"));
        let intent = OrderIntent::new(
            AccountId::new(173),
            instrument(),
            key.as_client_order_id().clone(),
            OrderTerms::KrMarket {
                side: OrderSide::Buy,
                quantity: WholeOrderQuantity::try_new(quantity("7")).expect("정수"),
            },
        )
        .expect("주문");
        let task = eligible_for(sigma);
        let range = ProposalPriceRange::from_estimate_task(
            task.task().range_facts(),
            task.task().reference().clone(),
        )
        .expect("범위");
        Proposal::new(key, intent, day(60), range, ProposalLifetime::default()).expect("초안")
    }
    fn snapshot(value: i64) -> ValuationSnapshot {
        let at = day(value);
        let calendar =
            SnapshotCalendar::new(&calendar(value, &Market::Kr), &calendar(value, &Market::Us))
                .expect("평가 달력");
        let portfolio = Portfolio::new(
            vec![],
            BuyingPower::new(money("113", Currency::Krw), at),
            BuyingPower::new(money("0", Currency::Usd), at),
        )
        .expect("포트");
        ValuationSnapshot::observe(&calendar, &portfolio.evaluate(None), at, None).expect("평가")
    }
    #[test]
    fn generic_memory_scenarios_preserve_state_and_boundaries() {
        let first_account = AccountId::new(17);
        let other_account = AccountId::new(29);
        let portfolios = MemoryPortfolioRepository::default();
        memory(portfolios.script_record(), 3);
        memory(portfolios.script_latest(), 3);
        run(contracts::scenarios::portfolio_latest(
            &portfolios,
            (&first_account, &other_account),
            (
                &SyncRecord::new(at(11), SyncOutcome::Failed),
                &SyncRecord::new(at(23), SyncOutcome::AccessBlocked),
            ),
        ));
        let transactions = MemoryTransactionRepository::default();
        memory(transactions.script_record(), 3);
        memory(transactions.script_latest(), 3);
        run(contracts::scenarios::transaction_latest(
            &transactions,
            (&first_account, &other_account),
            (
                &SyncRecord::new(at(11), SyncOutcome::Failed),
                &SyncRecord::new(at(23), SyncOutcome::AccessBlocked),
            ),
        ));
        let theses = MemoryThesisRepository::default();
        memory(theses.script_store(), 3);
        memory(theses.script_load(), 4);
        run(contracts::scenarios::thesis_roundtrip(
            &theses,
            (&id("thesis-a"), &id("thesis-b")),
            (&thesis(), &Thesis::new()),
        ));
    }
    #[test]
    fn generic_proposal_contract_preserves_versions_and_keys() {
        let proposals = MemoryProposalRepository::default();
        memory(proposals.script_store_draft(), 3);
        memory(proposals.script_draft(), 4);
        run(contracts::scenarios::proposal_roundtrip(
            &proposals,
            (&draft("102"), &draft("107")),
            &draft_for("other-draft", "113"),
        ));
    }
    fn run_candle_contract<P: MarketDataRepository>(candles: &P) {
        run(contracts::scenarios::candle_history(
            candles,
            contracts::scenarios::CandleHistoryFacts {
                instrument: &id("candle-a"),
                window: TimeInterval::try_new(at(0), at(120)).expect("기간"),
                boundaries: (&candle(0, "103"), &candle(60, "127")),
                outside: (&candle(-60, "97"), &candle(120, "131")),
                distractors: &[
                    candle_for(
                        id("candle-b"),
                        60,
                        CandlePeriod::Minute,
                        PriceAdjustment::Raw,
                        "151",
                    ),
                    candle_for(
                        id("candle-a"),
                        0,
                        CandlePeriod::Daily,
                        PriceAdjustment::Raw,
                        "157",
                    ),
                    candle_for(
                        id("candle-a"),
                        60,
                        CandlePeriod::Minute,
                        PriceAdjustment::Adjusted(AdjustmentBasis::new(at(300))),
                        "163",
                    ),
                ],
            },
        ));
    }
    #[test]
    fn generic_candle_and_valuation_contracts_preserve_query_scope() {
        let first_account = AccountId::new(17);
        let other_account = AccountId::new(29);
        let candles = MemoryMarketDataRepository::default();
        memory(candles.script_store_candles(), 6);
        memory(candles.script_candles(), 3);
        run_candle_contract(&candles);
        let valuations = MemoryValuationRepository::default();
        memory(valuations.script_append(), 6);
        memory(valuations.script_history(), 2);
        run(contracts::scenarios::valuation_history(
            &valuations,
            (&first_account, &other_account),
            TimeInterval::try_new(day(3), day(5)).expect("기간"),
            &[snapshot(3), snapshot(4)],
            &snapshot(5),
        ));
    }
    fn next_calibration(task: &CostEligibleTask, key: &CalibrationKey) -> Calibration {
        let calendars: Vec<_> = (60..=66).map(|day| calendar(day, &Market::Kr)).collect();
        let basis = AdjustmentBasis::new(local(day(66)));
        let adjustment =
            OutcomeAdjustment::from_actions(task.task(), basis, &[]).expect("수정 기준");
        let candles: Vec<_> = (60..=65)
            .map(|index| {
                let fixture_price = price(if index == 65 { "300" } else { "100" });
                Candle::try_new(
                    id("sample-a"),
                    CandleTime::try_new(local_midnight(index), CandlePeriod::Daily).expect("시각"),
                    PriceAdjustment::Adjusted(basis),
                    Ohlcv::try_new(
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        fixture_price,
                        quantity("13"),
                    )
                    .expect("봉"),
                    CandleObservation::new(day(66), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect();
        let outcome =
            EstimateOutcome::evaluate(task.task(), &calendars, &adjustment, &candles, day(66));
        let fact = outcome
            .calibration_fact(
                CalibrationEstimateId::try_new("calibration-a").expect("ID"),
                key.model().clone(),
                AiProbability::parse(&AiProbabilityInput::try_new("61").expect("입력"))
                    .expect("확률"),
            )
            .unwrap_or_else(|| panic!("종결 판정 필요: {outcome:?}"));
        Calibration::try_new(key.clone(), &[fact]).expect("둘째 보정")
    }
    #[test]
    fn generic_estimates_preserve_first_resolution_and_latest_calibration() {
        let repository = MemoryEstimateRepository::default();
        memory(repository.script_append_submission(), 3);
        memory(repository.script_submissions(), 3);
        memory(repository.script_append_resolution(), 3);
        memory(repository.script_resolution(), 3);
        let task = eligible();
        run(contracts::scenarios::estimate_append_only(
            &repository,
            &submission(&task),
            (
                &EstimateOutcome::Unavailable(OutcomeUnavailable::MissingCandles),
                &EstimateOutcome::Unavailable(OutcomeUnavailable::Calendar),
            ),
            &submission(&eligible_for_master("107", &other_master())),
        ));
        let key = calibration_key();
        let first = Calibration::try_new(key.clone(), &[]).expect("첫 보정");
        let second = next_calibration(&task, &key);
        let other_key = CalibrationKey::new(
            key.model().clone(),
            Horizon::new(HorizonSelection::HalfYearOrMore),
        );
        let other = Calibration::try_new(other_key.clone(), &[]).expect("다른 키 보정");
        memory(repository.script_append_calibration(), 4);
        memory(repository.script_calibration(), 3);
        run(contracts::scenarios::calibration_versions(
            &repository,
            &key,
            (&first, &second),
            (&other_key, &other),
        ));
    }
    #[test]
    fn generic_signal_contract_requires_nonempty_replacement_and_clearing() {
        let repository = MemorySignalRepository::default();
        memory(repository.script_replace(), 4);
        memory(repository.script_current(), 5);
        run(contracts::scenarios::signal_replacement(
            &repository,
            (&id("signal-a"), &id("signal-b")),
            (&[signal("7")], &[signal("11")]),
        ));
    }
    struct DiscardSignals;
    impl SignalRepository for DiscardSignals {
        async fn replace(
            &self,
            _: &InstrumentId,
            _: &[RaisedSignal],
        ) -> Result<WriteCompleted, SystemPortError> {
            std::future::ready(Ok(WriteCompleted)).await
        }
        async fn current(&self, _: &InstrumentId) -> Result<Vec<RaisedSignal>, SystemPortError> {
            std::future::ready(Ok(vec![])).await
        }
    }
    #[test]
    fn generic_scenarios_reject_discarded_writes_and_changed_price_range() {
        let first = [signal("7")];
        let second = [signal("11")];
        std::panic::catch_unwind(|| {
            run(contracts::scenarios::signal_replacement(
                &DiscardSignals,
                (&id("signal-a"), &id("signal-b")),
                (&first, &second),
            ));
        })
        .expect_err("쓰기를 버리는 구현 거부");
        let repository = MemoryProposalRepository::default();
        repository
            .script_draft()
            .push(MemoryReply::Return(Ok(Some(draft("107")))));
        let expected = draft("102");
        std::panic::catch_unwind(|| {
            run(contracts::proposal_repository_draft(
                &repository,
                expected.id(),
                Ok(Some(testkit_ports::DraftRecord::from_draft(&expected))),
            ));
        })
        .expect_err("가격 범위 오염 거부");
    }
    #[test]
    fn clock_risk_and_news_use_public_generic_contracts() {
        let clock = testkit_ports::SteppedClock::new(at(17));
        contracts::clock_now(&clock, at(17));
        clock.advance_to(at(29)).expect("시계 전진");
        contracts::clock_now(&clock, at(29));
        let presence = testkit_ports::ScriptedPresence::new(
            [],
            [
                RiskChangeConfirmation::Confirmed,
                RiskChangeConfirmation::Denied,
                RiskChangeConfirmation::Unavailable,
            ],
        );
        for (change, result) in [
            (RiskChange::EnableOrders, RiskChangeConfirmation::Confirmed),
            (RiskChange::AllowHours, RiskChangeConfirmation::Denied),
            (
                RiskChange::AllowMarketOrders,
                RiskChangeConfirmation::Unavailable,
            ),
        ] {
            run(contracts::user_presence_risk_change(
                &presence, change, result,
            ));
        }
    }
    #[test]
    fn news_contract_preserves_instrument_isolation_and_saved_facts() {
        let repository = MemoryNewsRepository::default();
        let instrument = id("news-c");
        let item = NewsItem::new(
            NewsSource::Yonhap,
            NormalizedUrl::try_new("https://example.com/c").expect("주소"),
            UntrustedText::try_new("기사").expect("제목"),
            None,
        );
        link_news(&repository, instrument.clone(), &item);
        link_news(&repository, instrument.clone(), &item);
        let other_item = NewsItem::new(
            NewsSource::Yonhap,
            NormalizedUrl::try_new("https://example.com/d").expect("주소"),
            UntrustedText::try_new("다른 기사").expect("제목"),
            None,
        );
        link_news(&repository, id("news-other"), &other_item);
        memory(repository.script_store(), 3);
        memory(repository.script_for_instrument(), 3);
        run(contracts::scenarios::news_roundtrip(
            &repository,
            (&instrument, &id("news-other")),
            (&item, &other_item),
        ));
    }
    fn caps(limit: &str, warning: &str, sector: &str) -> domain::WeightCaps {
        domain::WeightCaps::try_new(
            domain::ProfileWeight::try_new(ratio(limit)).expect("종목 상한"),
            domain::ProfileWeight::try_new(ratio(warning)).expect("주의 비중"),
            domain::ProfileWeight::try_new(ratio(sector)).expect("섹터 상한"),
        )
        .expect("상한")
    }
    fn profile_hours(profile: &InvestorProfile, hours: HoursPermissionState) -> InvestorProfile {
        let mut saved = profile.saved_state();
        saved.hours_permission = hours;
        InvestorProfile::restore_from_saved_state(saved).expect("저장된 성향")
    }
    fn run_settings_contract<P: SettingsRepository>(repository: &P) {
        let initial = profile_hours(
            &InvestorProfile::default()
                .with_horizon(Horizon::new(HorizonSelection::HalfYearOrMore))
                .expect("비기본 기간")
                .with_risk_appetite(RiskAppetite::Conservative)
                .with_weight_caps(caps("0.13", "0.07", "0.31")),
            HoursPermissionState::Enabled,
        );
        let changed = profile_hours(
            &initial
                .with_risk_appetite(RiskAppetite::Aggressive)
                .with_weight_caps(caps("0.23", "0.11", "0.47")),
            HoursPermissionState::Disabled,
        );
        let original = RiskSettings::new(
            OrderPermission::Disabled,
            SavedMarketOrderSetting::Disabled,
            initial.hours_permission().state(),
            initial.weight_caps(),
            initial.risk_appetite(),
        );
        let revised = RiskSettings::new(
            OrderPermission::Enabled,
            SavedMarketOrderSetting::Enabled,
            changed.hours_permission().state(),
            changed.weight_caps(),
            changed.risk_appetite(),
        );
        let restored = RiskSettings::new(
            OrderPermission::Enabled,
            SavedMarketOrderSetting::Enabled,
            initial.hours_permission().state(),
            initial.weight_caps(),
            initial.risk_appetite(),
        );
        run(contracts::scenarios::settings_roundtrip(
            repository,
            (&initial, &changed),
            (&original, &revised, &restored),
        ));
    }
    #[test]
    fn settings_queries_reflect_the_same_shared_facts() {
        let repository = MemorySettingsRepository::default();
        memory(repository.script_store_profile(), 2);
        memory(repository.script_store_risk_settings(), 2);
        memory(repository.script_profile(), 3);
        memory(repository.script_risk_settings(), 5);
        run_settings_contract(&repository);
    }
    #[test]
    fn audit_and_secret_lifecycles_are_reusable_generic_scenarios() {
        let audit = testkit_ports::system_fakes::MemoryAuditLog::default();
        memory(audit.script_append(), 3);
        let first = AuditEntry::new(
            at(11),
            AuditKind::RiskChangeConfirmationRecorded {
                change: RiskChange::EnableOrders,
                outcome: RiskChangeConfirmation::Denied,
            },
        );
        let second = AuditEntry::new(
            at(29),
            AuditKind::RiskChangeConfirmationRecorded {
                change: RiskChange::AllowHours,
                outcome: RiskChangeConfirmation::Unavailable,
            },
        );
        run(contracts::scenarios::audit_append_only(
            &audit,
            (&first, &second),
        ));
        let secrets = testkit_ports::system_fakes::MemorySecretStore::default();
        memory(secrets.script_store(), 3);
        memory(secrets.script_load(), 4);
        memory(secrets.script_remove(), 2);
        run(contracts::scenarios::secret_lifecycle(
            &secrets,
            (SecretKey::DartApiKey, SecretKey::FredApiKey),
            (
                &SecretValue::new(vec![17, 29]),
                &SecretValue::new(vec![53, 67, 79]),
            ),
        ));
    }
    #[test]
    fn market_data_latest_queries_have_reusable_contract_sequences() {
        let repository = MemoryMarketDataRepository::default();
        let older = Quote::try_new(
            Symbol::try_new("AAPL").expect("코드"),
            Currency::Usd,
            None,
            Some(at(11)),
            at(13),
        )
        .expect("시세");
        let newer = Quote::try_new(
            Symbol::try_new("AAPL").expect("코드"),
            Currency::Usd,
            None,
            Some(at(23)),
            at(29),
        )
        .expect("시세");
        memory(repository.script_store_quote(), 3);
        memory(repository.script_quote(), 3);
        run(contracts::scenarios::quote_latest(
            &repository,
            (&id("quote-a"), &id("quote-b")),
            (&older, &newer),
        ));
        let rate = |value, start, end| {
            FxRate::parse(
                &NumericInput::try_new(value).expect("숫자"),
                FxPair::UsdKrw,
                FxRateKind::Mid,
                TimeInterval::try_new(at(start), at(end)).expect("기간"),
                FxSource::TossOpenApi,
            )
            .expect("환율")
        };
        memory(repository.script_store_fx_rate(), 4);
        memory(repository.script_fx_rate(), 6);
        run(contracts::scenarios::fx_latest_not_after(
            &repository,
            (&rate("1391", 11, 31), &rate("1433", 23, 43)),
            (at(10), at(22)),
            (
                &fx_rate("0.0007", FxPair::KrwUsd, FxRateKind::Mid, 31, 51),
                &fx_rate("1477", FxPair::UsdKrw, FxRateKind::Buy, 37, 57),
            ),
        ));
    }
    #[test]
    fn settings_risk_first_populates_the_same_profile_facts() {
        let repository = MemorySettingsRepository::default();
        let expected = profile_hours(
            &InvestorProfile::default()
                .with_risk_appetite(RiskAppetite::Aggressive)
                .with_weight_caps(caps("0.17", "0.09", "0.37")),
            HoursPermissionState::Enabled,
        );
        let risk = RiskSettings::new(
            OrderPermission::Enabled,
            SavedMarketOrderSetting::Enabled,
            HoursPermissionState::Enabled,
            expected.weight_caps(),
            expected.risk_appetite(),
        );
        memory(repository.script_profile(), 2);
        memory(repository.script_risk_settings(), 2);
        memory(repository.script_store_risk_settings(), 1);
        run(contracts::scenarios::settings_risk_first(
            &repository,
            &risk,
            &expected,
        ));
    }
    #[test]
    fn candle_store_rejects_mismatched_instrument_without_partial_write() {
        let repository = MemoryMarketDataRepository::default();
        memory(repository.script_store_candles(), 1);
        memory(repository.script_candles(), 2);
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        run(contracts::market_data_repository_store_candles(
            &repository,
            &id("different-instrument"),
            &[
                candle_for(
                    id("different-instrument"),
                    0,
                    CandlePeriod::Minute,
                    PriceAdjustment::Raw,
                    "157",
                ),
                candle(60, "103"),
            ],
            Err(error),
        ));
        run(contracts::market_data_repository_candles(
            &repository,
            &id("different-instrument"),
            (
                TimeInterval::try_new(at(0), at(120)).expect("기간"),
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            ),
            Ok(vec![]),
        ));
        run(contracts::market_data_repository_candles(
            &repository,
            &id("candle-a"),
            (
                TimeInterval::try_new(at(0), at(120)).expect("기간"),
                CandlePeriod::Minute,
                PriceAdjustment::Raw,
            ),
            Ok(vec![]),
        ));
    }

    enum PartialCandleWrite {
        All,
        Matching,
        Other,
    }
    impl PartialCandleWrite {
        fn selected<'candle>(
            &self,
            instrument: &InstrumentId,
            candle: &'candle Candle,
        ) -> Option<&'candle Candle> {
            match self {
                Self::All => Some(candle),
                Self::Matching => (candle.instrument() == instrument).then_some(candle),
                Self::Other => (candle.instrument() != instrument).then_some(candle),
            }
        }
    }
    struct PartialCandles {
        inner: MemoryMarketDataRepository,
        mode: PartialCandleWrite,
    }
    impl MarketDataRepository for PartialCandles {
        async fn store_quote(
            &self,
            instrument: &InstrumentId,
            quote: &Quote,
        ) -> Result<application::CacheWriteOutcome, SystemPortError> {
            self.inner.store_quote(instrument, quote).await
        }
        async fn quote(&self, instrument: &InstrumentId) -> Result<Option<Quote>, SystemPortError> {
            self.inner.quote(instrument).await
        }
        async fn store_candles(
            &self,
            instrument: &InstrumentId,
            candles: &[Candle],
        ) -> Result<WriteCompleted, SystemPortError> {
            let mode = if candles.iter().any(|value| value.instrument() != instrument) {
                &self.mode
            } else {
                &PartialCandleWrite::All
            };
            for value in candles
                .iter()
                .filter_map(|candle| mode.selected(instrument, candle))
            {
                self.inner
                    .store_candles(value.instrument(), std::slice::from_ref(value))
                    .await?;
            }
            if candles.iter().any(|value| value.instrument() != instrument) {
                return Err(SystemPortError::new(
                    SystemPort::Repository,
                    SystemFailure::InvalidData,
                ));
            }
            Ok(WriteCompleted)
        }
        async fn candles(
            &self,
            instrument: &InstrumentId,
            window: TimeInterval,
            period: CandlePeriod,
            adjustment: PriceAdjustment,
        ) -> Result<Vec<Candle>, SystemPortError> {
            self.inner
                .candles(instrument, window, period, adjustment)
                .await
        }
        async fn store_fx_rate(
            &self,
            rate: &FxRate,
        ) -> Result<application::CacheWriteOutcome, SystemPortError> {
            self.inner.store_fx_rate(rate).await
        }
        async fn fx_rate(
            &self,
            pair: FxPair,
            kind: FxRateKind,
            at: FxRateQuery,
        ) -> Result<Option<FxRate>, SystemPortError> {
            self.inner.fx_rate(pair, kind, at).await
        }
    }
    fn partial_candle_contract(mode: PartialCandleWrite) {
        let inner = MemoryMarketDataRepository::default();
        memory(inner.script_store_candles(), 32);
        memory(inner.script_candles(), 16);
        run_candle_contract(&PartialCandles { inner, mode });
    }
    #[test]
    #[should_panic(expected = "거부된 봉 묶음은 어느 종목에도 저장되지 않음")]
    fn public_contract_rejects_candles_written_before_error() {
        partial_candle_contract(PartialCandleWrite::All);
    }
    #[test]
    #[should_panic(expected = "거부된 봉 묶음은 어느 종목에도 저장되지 않음")]
    fn public_contract_rejects_only_matching_candles_written_before_error() {
        partial_candle_contract(PartialCandleWrite::Matching);
    }
    #[test]
    #[should_panic(expected = "거부된 봉 묶음은 어느 종목에도 저장되지 않음")]
    fn public_contract_rejects_only_other_candles_written_before_error() {
        partial_candle_contract(PartialCandleWrite::Other);
    }
    #[derive(Default)]
    struct StaleRisk {
        inner: MemorySettingsRepository,
        cached: std::sync::RwLock<Option<RiskSettings>>,
    }
    impl SettingsRepository for StaleRisk {
        async fn store_profile(
            &self,
            profile: &InvestorProfile,
        ) -> Result<WriteCompleted, SystemPortError> {
            let result = self.inner.store_profile(profile).await?;
            let current = self.inner.risk_settings().await?;
            *self.cached.write().expect("캐시 잠금") = current;
            Ok(result)
        }
        async fn profile(&self) -> Result<Option<InvestorProfile>, SystemPortError> {
            self.inner.profile().await
        }
        async fn store_risk_settings(
            &self,
            settings: &RiskSettings,
        ) -> Result<WriteCompleted, SystemPortError> {
            let result = self.inner.store_risk_settings(settings).await?;
            let current = self.inner.risk_settings().await?;
            let mut cached = self.cached.write().expect("캐시 잠금");
            if cached.is_none() {
                *cached = current;
            }
            Ok(result)
        }
        async fn risk_settings(&self) -> Result<Option<RiskSettings>, SystemPortError> {
            let current = *self.cached.read().expect("캐시 잠금");
            std::future::ready(Ok(current)).await
        }
    }
    #[test]
    #[should_panic(expected = "ADR 포트 계약: 입력에 대응하는 사실과 오류를 보존")]
    fn public_contract_rejects_stale_risk_after_successful_write() {
        let port = StaleRisk::default();
        memory(port.inner.script_store_profile(), 2);
        memory(port.inner.script_store_risk_settings(), 2);
        memory(port.inner.script_profile(), 3);
        memory(port.inner.script_risk_settings(), 8);
        run_settings_contract(&port);
    }
}
