use application::{AiRunId, EstimateRunContext, EstimateTaskMaterials, ReasoningEffort};
use domain::{
    AdjustmentBasis, CalibrationModel, Candle, CandleObservation, CandlePeriod, CandleStatus,
    CandleTime, CommissionRateFact, CommissionValidity, CommonShare, CostEligibleTask, CostRate,
    Currency, DisplayInstrumentName, EstimateContext, EstimateCostPeriod, EstimateMetadata,
    EstimatePrompt, EstimateTask, EvidenceBundleId, Horizon, HorizonSelection, HoursPermission,
    InstrumentId, InvestorProfile, KoreanMasterDetail, LiquidationTrading, ListingStatus, Market,
    MarketCalendar, MarketSession, MasterCode, MasterInstrument, MasterInstrumentRow,
    MasterSecurityType, ModelIdentityPart, Nanosecond, NumericInput, NxtTrading, OffsetSeconds,
    Ohlcv, Price, PriceAdjustment, PromptHash, Quantity, Ratio, RiskAppetite, ScorecardPromptId,
    SessionData, SessionInformation, Symbol, TimeInterval, Timestamp, TradeCost, TradeTaxFact,
    TradingSuspension, UnixSeconds, Volatility,
};

fn price(value: &str) -> Option<Price> {
    Price::parse(&NumericInput::try_new(value).ok()?, Currency::Krw).ok()
}

fn quantity(value: &str) -> Option<Quantity> {
    Quantity::parse(&NumericInput::try_new(value).ok()?).ok()
}

fn ratio(value: &str) -> Option<Ratio> {
    Ratio::parse(&NumericInput::try_new(value).ok()?).ok()
}

fn at(value: i64) -> Option<Timestamp> {
    Timestamp::from_parts(
        UnixSeconds::new(value),
        Nanosecond::new(0_u32),
        OffsetSeconds::new(0_i32),
    )
    .ok()
}

fn day(value: i64) -> Option<Timestamp> {
    at(value.checked_mul(86_400_i64)?)
}

fn local(time: Timestamp) -> Option<Timestamp> {
    Timestamp::from_parts(
        time.utc_seconds(),
        time.nanosecond(),
        OffsetSeconds::new(32_400_i32),
    )
    .ok()
}

fn local_midnight(value: i64) -> Option<Timestamp> {
    local(at(day(value)?
        .utc_seconds()
        .as_i64()
        .checked_sub(32_400_i64)?)?)
}

fn master() -> Option<MasterInstrument> {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").ok()?,
        instrument_id: InstrumentId::try_new("sample-a").ok()?,
        symbol: Symbol::try_new("005930").ok()?,
        name: DisplayInstrumentName::try_new("고정 종목").ok()?,
        security_type: MasterSecurityType::from_code(MasterCode::try_new("STOCK").ok()?),
        currency: Currency::Krw,
        listing_status: ListingStatus::Active,
        common_share: CommonShare::Common,
        korean_detail: Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        }),
        observed_at: day(60)?,
    })
    .ok()
}

fn candle(source: &MasterInstrument, index: i64) -> Option<Candle> {
    let fixture = price(if index.rem_euclid(2) == 0 {
        "100"
    } else {
        "102"
    })?;
    Candle::try_new(
        source.row().instrument_id.clone(),
        CandleTime::try_new(local_midnight(index)?, CandlePeriod::Daily).ok()?,
        PriceAdjustment::Adjusted(AdjustmentBasis::new(day(61)?)),
        Ohlcv::try_new(fixture, fixture, fixture, fixture, quantity("13")?).ok()?,
        CandleObservation::new(day(61)?, CandleStatus::Finalized),
    )
    .ok()
}

#[must_use]
pub fn sample_cost_eligible_task() -> Option<CostEligibleTask> {
    let source = master()?;
    let candles = (0_i64..61_i64)
        .map(|index| candle(&source, index))
        .collect::<Option<Vec<_>>>()?;
    let task = EstimateTask::create(
        EstimateContext::new(
            source
                .price_observation(price("100")?, local(day(60)?)?)
                .ok()?,
            day(60)?,
        ),
        Volatility::calculate(&candles).ok()?,
        Horizon::new(HorizonSelection::FewDays),
        HoursPermission::default(),
        RiskAppetite::Neutral,
    )
    .ok()?;
    let zero = CostRate::try_new(ratio("0")?).ok()?;
    let maturity = at(day(65)?.utc_seconds().as_i64().checked_add(3_600_i64)?)?;
    let period = EstimateCostPeriod::from_calendar_result(&task, maturity).ok()?;
    let cost = TradeCost::from_facts(
        Some(CommissionRateFact::from_commissions_result(
            Market::Kr,
            zero,
            CommissionValidity::try_new(None, None).ok()?,
        )),
        Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
        period,
    )
    .ok()?;
    CostEligibleTask::assess(task, &cost).ok()
}

fn identity(value: &str) -> Option<ModelIdentityPart> {
    ModelIdentityPart::try_new(value).ok()
}

#[must_use]
pub fn sample_estimate_run_context(run: &str) -> Option<EstimateRunContext> {
    Some(EstimateRunContext {
        run: AiRunId::try_new(run).ok()?,
        model: CalibrationModel::new(identity("claude")?, identity("model")?, identity("v1")?),
        horizon: Horizon::new(HorizonSelection::FewDays),
        metadata: EstimateMetadata {
            effort: ReasoningEffort::High,
            cli_version: identity("2.7.3")?,
            prompt: EstimatePrompt {
                id: ScorecardPromptId::try_new("estimate-prompt").ok()?,
                version: identity("v4")?,
                hash: PromptHash::try_new(
                    "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
                )
                .ok()?,
            },
            evidence_bundle: EvidenceBundleId::try_new("bundle-nine").ok()?,
        },
    })
}

#[must_use]
pub fn sample_day(value: i64) -> Option<Timestamp> {
    day(value)
}

#[must_use]
pub fn sample_local(time: Timestamp) -> Option<Timestamp> {
    local(time)
}

#[must_use]
pub fn sample_master() -> Option<MasterInstrument> {
    master()
}

#[must_use]
pub fn sample_estimate_materials() -> Option<EstimateTaskMaterials> {
    let source = master()?;
    let candles = (0_i64..61_i64)
        .map(|index| candle(&source, index))
        .collect::<Option<Vec<_>>>()?;
    let zero = CostRate::try_new(ratio("0")?).ok()?;
    Some(EstimateTaskMaterials {
        context: EstimateContext::new(
            source
                .price_observation(price("100")?, local(day(60)?)?)
                .ok()?,
            day(60)?,
        ),
        volatility: Volatility::calculate(&candles).ok()?,
        profile: InvestorProfile::default()
            .with_horizon(Horizon::new(HorizonSelection::FewDays))
            .ok()?,
        commission: Some(CommissionRateFact::from_commissions_result(
            Market::Kr,
            zero,
            CommissionValidity::try_new(None, None).ok()?,
        )),
        tax: Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
    })
}

fn kr_row(index: i64) -> Option<MarketSession> {
    let open = local(day(index)?)?;
    let close = local(at(day(index)?
        .utc_seconds()
        .as_i64()
        .checked_add(23_400_i64)?)?)?;
    MarketSession::try_new(
        open.offset_calendar_date(),
        SessionInformation::Kr {
            pre_market: None,
            regular_market: Some(
                SessionData::try_new(TimeInterval::try_new(open, close).ok()?, None).ok()?,
            ),
            after_market: None,
        },
    )
    .ok()
}

#[must_use]
pub fn sample_kr_calendars(first: i64, last: i64) -> Option<Vec<MarketCalendar>> {
    (first..=last)
        .map(|index| {
            MarketCalendar::try_new(
                kr_row(index.checked_sub(1)?)?,
                kr_row(index)?,
                kr_row(index.checked_add(1)?)?,
            )
            .ok()
        })
        .collect()
}
