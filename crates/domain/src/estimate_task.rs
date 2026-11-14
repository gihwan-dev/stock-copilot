use crate::{
    BaseProbability, CheckResult, EstimateDistance, EstimateRangeFacts, Horizon, HorizonSelection,
    HoursPermission, HoursPermissionState, PriceObservation, ProposalVolatility, RiskAppetite,
    Timestamp, TradingDayHorizon, Volatility,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateContext {
    reference: PriceObservation,
    created_at: Timestamp,
}
impl EstimateContext {
    #[must_use]
    pub fn new(reference: PriceObservation, created_at: Timestamp) -> Self {
        Self {
            reference,
            created_at,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoEstimateTask {
    HoursNotAllowed,
    InsufficientSamples,
    InstrumentMismatch,
    StaleVolatility,
    FutureVolatility,
    InvalidReference,
    NumericalDomain,
    CoefficientRegression,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateTask {
    context: EstimateContext,
    volatility: Volatility,
    horizon: TradingDayHorizon,
    target: EstimateDistance,
    stop: EstimateDistance,
    base_probability: BaseProbability,
    appetite: RiskAppetite,
}
impl EstimateTask {
    pub fn create(
        context: EstimateContext,
        volatility: Volatility,
        horizon: Horizon,
        permission: HoursPermission,
        appetite: RiskAppetite,
    ) -> Result<Self, NoEstimateTask> {
        if horizon.selection() == HorizonSelection::Hours
            && permission.state() != HoursPermissionState::Enabled
        {
            return Err(NoEstimateTask::HoursNotAllowed);
        }
        if *volatility.samples().as_usize() < 60 {
            return Err(NoEstimateTask::InsufficientSamples);
        }
        if context.reference.instrument_id() != Some(volatility.instrument()) {
            return Err(NoEstimateTask::InstrumentMismatch);
        }
        let observed_day = context.reference.observed_at().offset_calendar_date();
        let created_day = context
            .created_at
            .date_at_offset(context.reference.observed_at().offset_seconds())
            .or(Err(NoEstimateTask::InvalidReference))?;
        let expected_currency = if context.reference.instrument().market() == crate::Market::Kr {
            crate::Currency::Krw
        } else {
            crate::Currency::Usd
        };
        if context.reference.price().as_money().currency() != expected_currency
            || context
                .reference
                .check(context.reference.instrument(), context.created_at)
                != CheckResult::Passed
        {
            return Err(NoEstimateTask::InvalidReference);
        }
        if volatility.as_of() > observed_day || observed_day > created_day {
            return Err(NoEstimateTask::FutureVolatility);
        }
        volatility
            .as_of()
            .days_until(created_day)
            .ok()
            .filter(|elapsed| (0..=7).contains(elapsed.as_i64()))
            .ok_or(NoEstimateTask::StaleVolatility)?;
        let trading_horizon = TradingDayHorizon::from_estimate_policy(horizon.selection());
        let days_ratio = trading_horizon.as_ratio();
        let (target, stop, base_probability) =
            crate::base_probability::distances(appetite, volatility.daily(), days_ratio)?;
        Ok(Self {
            context,
            volatility,
            horizon: trading_horizon,
            target,
            stop,
            base_probability,
            appetite,
        })
    }
    #[must_use]
    pub fn volatility(&self) -> &Volatility {
        &self.volatility
    }
    #[must_use]
    pub fn reference(&self) -> &PriceObservation {
        &self.context.reference
    }
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.context.created_at
    }
    #[must_use]
    pub fn horizon(&self) -> TradingDayHorizon {
        self.horizon
    }
    #[must_use]
    pub fn target(&self) -> EstimateDistance {
        self.target
    }
    #[must_use]
    pub fn stop(&self) -> EstimateDistance {
        self.stop
    }
    #[must_use]
    pub fn base_probability(&self) -> BaseProbability {
        self.base_probability
    }
    #[must_use]
    pub fn range_facts(&self) -> EstimateRangeFacts {
        EstimateRangeFacts {
            instrument: self.context.reference.instrument().clone(),
            reference: self.reference().price(),
            volatility: ProposalVolatility::from_daily(self.volatility.daily()),
            horizon: self.horizon,
        }
    }
}

pub(crate) const fn buy_task_risk_appetite(task: &EstimateTask) -> RiskAppetite {
    task.appetite
}

#[cfg(test)]
mod tests {
    use super::{EstimateContext, EstimateTask, NoEstimateTask};
    use crate::candle::tests::{at, price, series};
    use crate::{
        CheckResult, Horizon, HorizonSelection, HoursPermission, HoursPermissionState,
        InstrumentId, PriceObservation, ResolvedOrderInstrument, RiskAppetite, Timestamp,
        Volatility,
    };
    use crate::{Currency, Market, Ratio, Symbol};
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;
    fn local_at(day: i64, market: &Market) -> Timestamp {
        let offset = if *market == Market::Kr {
            32_400_i32
        } else {
            -18_000_i32
        };
        Timestamp::from_parts(
            at(day).utc_seconds(),
            at(day).nanosecond(),
            crate::OffsetSeconds::new(offset),
        )
        .expect("현지 시각")
    }
    fn dated_context(context: &EstimateContext, day: i64) -> EstimateContext {
        let at = local_at(day, &context.reference.instrument().market());
        EstimateContext::new(
            PriceObservation::from_identified_quote_parts(
                InstrumentId::try_new("sample").expect("ID"),
                context.reference.instrument().clone(),
                context.reference.price(),
                at,
            )
            .expect("현지 관측"),
            at,
        )
    }
    #[test]
    fn volatility_dates_reject_3700_day_age_and_future_information() {
        let (context, volatility) = facts(61);
        assert_eq!(
            make(dated_context(&context, 3761), volatility.clone()),
            Err(NoEstimateTask::StaleVolatility)
        );
        assert_eq!(
            make(dated_context(&context, 60), volatility),
            Err(NoEstimateTask::FutureVolatility)
        );
    }
    #[test]
    fn seven_calendar_days_pass_and_eight_days_fail() {
        let (context, volatility) = facts(61);
        assert_eq!(
            make(dated_context(&context, 68), volatility.clone())
                .expect("7일 허용")
                .volatility(),
            &volatility
        );
        assert_eq!(
            make(dated_context(&context, 69), volatility),
            Err(NoEstimateTask::StaleVolatility)
        );
    }
    #[test]
    fn quote_identity_must_be_present_in_the_observation_fact() {
        let (mut context, volatility) = facts(61);
        context.reference = PriceObservation::from_quote_parts(
            context.reference.instrument().clone(),
            context.reference.price(),
            context.reference.observed_at(),
        )
        .expect("기존 D-14 관측");
        assert_eq!(
            make(context, volatility),
            Err(NoEstimateTask::InstrumentMismatch)
        );
    }
    const LOCAL_MIDNIGHT_CASES: [(Market, &str, Currency, i64, u64, i32); 3] = [
        (
            Market::Kr,
            "005930",
            Currency::Krw,
            67,
            54_000_u64,
            32_400_i32,
        ),
        (
            Market::Us,
            "ABC",
            Currency::Usd,
            68,
            18_000_u64,
            -18_000_i32,
        ),
        (
            Market::Us,
            "ABC",
            Currency::Usd,
            68,
            14_400_u64,
            -14_400_i32,
        ),
    ];
    #[test]
    fn market_local_midnight_keeps_seven_day_boundary_for_kr_est_and_edt() {
        let (context, volatility) = facts(61);
        for (market, symbol, currency, day, seconds, offset) in LOCAL_MIDNIGHT_CASES {
            let order = ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new(symbol).expect("심볼"),
                market,
            )
            .expect("시장");
            let midnight = at(day)
                .checked_add(
                    crate::TimeSpan::try_new(
                        crate::SpanSeconds::new(seconds),
                        crate::Nanosecond::new(0),
                    )
                    .expect("초"),
                )
                .expect("경계");
            for (instant, expected) in [
                (
                    midnight
                        .checked_sub(crate::TimeSpan::NANOSECOND)
                        .expect("직전"),
                    CheckResult::Passed,
                ),
                (midnight, CheckResult::Failed),
            ] {
                let local = Timestamp::from_parts(
                    instant.utc_seconds(),
                    instant.nanosecond(),
                    crate::OffsetSeconds::new(offset),
                )
                .expect("현지 표현");
                let observation = PriceObservation::from_identified_quote_parts(
                    volatility.instrument().clone(),
                    order.clone(),
                    price("100", currency),
                    local,
                )
                .expect("정규화 관측");
                let result = make(
                    EstimateContext::new(observation, instant),
                    volatility.clone(),
                );
                assert_eq!(
                    result.map_or(CheckResult::Failed, |_task| CheckResult::Passed),
                    expected
                );
            }
        }
        assert_eq!(
            make(context, volatility.clone())
                .expect("과제")
                .volatility(),
            &volatility
        );
    }
    const LOCAL_OFFSET_CASES: [(Market, &str, Currency, i32, CheckResult); 6] = [
        (
            Market::Kr,
            "005930",
            Currency::Krw,
            32_400_i32,
            CheckResult::Passed,
        ),
        (
            Market::Kr,
            "005930",
            Currency::Krw,
            0_i32,
            CheckResult::Failed,
        ),
        (
            Market::Kr,
            "005930",
            Currency::Krw,
            -18_000_i32,
            CheckResult::Failed,
        ),
        (
            Market::Us,
            "ABC",
            Currency::Usd,
            -18_000_i32,
            CheckResult::Passed,
        ),
        (
            Market::Us,
            "ABC",
            Currency::Usd,
            -14_400_i32,
            CheckResult::Passed,
        ),
        (
            Market::Us,
            "ABC",
            Currency::Usd,
            32_400_i32,
            CheckResult::Failed,
        ),
    ];
    #[test]
    fn identified_observation_requires_market_local_offset_and_preserves_identity() {
        let (context, volatility) = facts(61);
        for (market, symbol, currency, offset, expected) in LOCAL_OFFSET_CASES {
            let order = ResolvedOrderInstrument::from_master_lookup_result(
                Symbol::try_new(symbol).expect("심볼"),
                market,
            )
            .expect("시장");
            let at = Timestamp::from_parts(
                context.created_at.utc_seconds(),
                context.created_at.nanosecond(),
                crate::OffsetSeconds::new(offset),
            )
            .expect("시각");
            let result = PriceObservation::from_identified_quote_parts(
                volatility.instrument().clone(),
                order,
                price("100", currency),
                at,
            );
            assert_eq!(
                result.map_or(CheckResult::Failed, |value| {
                    assert_eq!(value.instrument_id(), Some(volatility.instrument()));
                    assert_eq!(
                        value.observed_at().offset_seconds(),
                        crate::OffsetSeconds::new(offset)
                    );
                    CheckResult::Passed
                }),
                expected
            );
        }
    }
    #[test]
    fn market_local_projection_range_errors_are_invalid_reference() {
        let (mut context, volatility) = facts(61);
        let first = time::Date::from_calendar_date(1, time::Month::January, 1)
            .expect("최초 날짜")
            .midnight()
            .assume_utc();
        context.created_at = Timestamp::from_parts(
            crate::UnixSeconds::new(first.unix_timestamp()),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(0),
        )
        .expect("UTC 시각");
        assert_eq!(
            make(context, volatility),
            Err(NoEstimateTask::InvalidReference)
        );
    }
    #[test]
    fn identified_observation_propagates_time_range_failure() {
        let (_, volatility) = facts(61);
        let final_utc = time::Date::from_calendar_date(9999, time::Month::December, 31)
            .expect("날짜")
            .with_hms(14, 59, 58)
            .expect("시각")
            .assume_utc();
        let local = Timestamp::from_parts(
            crate::UnixSeconds::new(final_utc.unix_timestamp()),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(32_400_i32),
        )
        .expect("범위 끝");
        let order = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("005930").expect("심볼"),
            Market::Kr,
        )
        .expect("시장");
        assert_eq!(
            PriceObservation::from_identified_quote_parts(
                volatility.instrument().clone(),
                order,
                price("100", Currency::Krw),
                local,
            ),
            Err(crate::TimeError::Overflow)
        );
    }
    fn facts(count: usize) -> (EstimateContext, Volatility) {
        let texts = (0..count)
            .map(|index| if index % 2 == 0 { "100" } else { "102" })
            .collect::<Vec<_>>();
        let volatility = Volatility::calculate(&series(&texts)).expect("일간 사실");
        let order = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("ABC").expect("심볼"),
            Market::Us,
        )
        .expect("종목");
        let reference = PriceObservation::from_identified_quote_parts(
            volatility.instrument().clone(),
            order,
            price("100", Currency::Usd),
            local_at(61, &Market::Us),
        )
        .expect("관측");
        let context = EstimateContext::new(reference, local_at(61, &Market::Us));
        (context, volatility)
    }
    fn make(
        context: EstimateContext,
        volatility: Volatility,
    ) -> Result<EstimateTask, NoEstimateTask> {
        EstimateTask::create(
            context,
            volatility,
            Horizon::default(),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
    }
    #[test]
    fn sixty_returns_freeze_reference_and_reuse_proposal_range() {
        let (context, volatility) = facts(61);
        let task = make(context.clone(), volatility.clone()).expect("60 수익률");
        assert_eq!(task.reference(), &context.reference);
        assert_eq!(task.created_at(), context.created_at);
        assert_eq!(task.horizon().as_ratio(), Ratio::new(Decimal::from(40_u32)));
        assert_eq!(task.volatility(), &volatility);
        assert!(task.target() > task.stop());
        assert_ne!(task.base_probability().to_string(), String::new());
        assert_ne!(task.target().to_string(), String::new());
        let range = crate::ProposalPriceRange::from_estimate_task(
            task.range_facts(),
            task.reference().clone(),
        )
        .expect("D-14 연결");
        assert_eq!(range.check(task.reference().price()), CheckResult::Passed);
    }
    #[test]
    fn unavailable_reasons_cover_sample_identity_date_reference_and_zero_variance() {
        let (short_context, short_volatility) = facts(60);
        assert_eq!(
            make(short_context, short_volatility),
            Err(NoEstimateTask::InsufficientSamples)
        );
        let (context, volatility) = facts(61);
        let mut identity = context.clone();
        identity.reference = PriceObservation::from_identified_quote_parts(
            InstrumentId::try_new("other").expect("id"),
            context.reference.instrument().clone(),
            context.reference.price(),
            context.reference.observed_at(),
        )
        .expect("다른 종목 사실");
        assert_eq!(
            make(identity, volatility.clone()),
            Err(NoEstimateTask::InstrumentMismatch)
        );
        let mut expired = context.clone();
        expired.created_at = at(62);
        assert_eq!(
            make(expired, volatility.clone()),
            Err(NoEstimateTask::InvalidReference)
        );
        let mut future = context.clone();
        future.created_at = at(60);
        assert_eq!(
            make(future, volatility),
            Err(NoEstimateTask::InvalidReference)
        );
        let zero = Volatility::calculate(&series(&["100"; 61])).expect("상수");
        assert_eq!(make(context, zero), Err(NoEstimateTask::NumericalDomain));
    }
    #[test]
    fn reference_identity_currency_and_five_second_boundary_are_checked() {
        let (context, volatility) = facts(61);
        let other = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("OTHER").expect("심볼"),
            Market::Us,
        )
        .expect("종목");
        let mut identity = context.clone();
        identity.reference = PriceObservation::from_identified_quote_parts(
            InstrumentId::try_new("other").expect("식별자"),
            other,
            context.reference.price(),
            context.reference.observed_at(),
        )
        .expect("다른 종목");
        assert_eq!(
            make(identity, volatility.clone()),
            Err(NoEstimateTask::InstrumentMismatch)
        );
        let mut currency = context.clone();
        currency.reference = PriceObservation::from_identified_quote_parts(
            volatility.instrument().clone(),
            context.reference.instrument().clone(),
            price("100", Currency::Krw),
            context.reference.observed_at(),
        )
        .expect("통화 사실");
        assert_eq!(
            make(currency, volatility.clone()),
            Err(NoEstimateTask::InvalidReference)
        );
        let seconds = *context.created_at.utc_seconds().as_i64();
        for (nanosecond, expected) in [(999_999_999, CheckResult::Passed), (0, CheckResult::Failed)]
        {
            let elapsed = if nanosecond == 0 { 5_i64 } else { 4_i64 };
            let mut aged = context.clone();
            aged.created_at = Timestamp::from_parts(
                crate::UnixSeconds::new(seconds.checked_add(elapsed).expect("시각")),
                crate::Nanosecond::new(nanosecond),
                crate::OffsetSeconds::new(0),
            )
            .expect("시각");
            assert_eq!(
                make(aged, volatility.clone())
                    .map_or(CheckResult::Failed, |_task| CheckResult::Passed),
                expected
            );
        }
    }
    #[test]
    fn domestic_reference_uses_krw() {
        let (mut context, volatility) = facts(61);
        let order = ResolvedOrderInstrument::from_master_lookup_result(
            Symbol::try_new("005930").expect("심볼"),
            Market::Kr,
        )
        .expect("종목");
        context.reference = PriceObservation::from_identified_quote_parts(
            volatility.instrument().clone(),
            order,
            price("100", Currency::Krw),
            local_at(61, &Market::Kr),
        )
        .expect("시세");
        assert_eq!(
            make(context, volatility)
                .expect("국내 과제")
                .reference()
                .price()
                .as_money()
                .currency(),
            Currency::Krw
        );
    }
    fn assert_policy(
        task: &EstimateTask,
        volatility: &Volatility,
        horizon: HorizonSelection,
        risk: RiskAppetite,
    ) {
        let expected_days = match horizon {
            HorizonSelection::Hours => Decimal::new(25, 2),
            HorizonSelection::FewDays => Decimal::from(5_u32),
            HorizonSelection::WeeksToMonths => Decimal::from(40_u32),
            HorizonSelection::HalfYearOrMore => Decimal::from(126_u32),
        };
        assert_eq!(task.horizon().as_ratio(), Ratio::new(expected_days));
        assert!(
            (Decimal::new(2, 1)..=Decimal::new(4, 1))
                .contains(&task.base_probability().as_decimal())
        );
        let root =
            crate::volatility_arithmetic::square_root(Ratio::new(expected_days)).expect("제곱근");
        let (target_coefficient, stop_coefficient) = match risk {
            RiskAppetite::Conservative => (Decimal::new(8, 1), Decimal::new(4, 1)),
            RiskAppetite::Neutral => (Decimal::ONE, Decimal::new(5, 1)),
            RiskAppetite::Aggressive => (Decimal::new(12, 1), Decimal::new(6, 1)),
        };
        let expected_scale = volatility.daily().as_decimal();
        assert!(
            task.target()
                .as_decimal()
                .checked_div(root.as_decimal())
                .expect("H 제거")
                .checked_div(expected_scale)
                .expect("σ 제거")
                .checked_sub(target_coefficient)
                .expect("계수 차이")
                .abs()
                < Decimal::new(1, 20)
        );
        assert!(
            task.stop()
                .as_decimal()
                .checked_div(root.as_decimal())
                .expect("H 제거")
                .checked_div(expected_scale)
                .expect("σ 제거")
                .checked_sub(stop_coefficient)
                .expect("계수 차이")
                .abs()
                < Decimal::new(1, 20)
        );
    }
    #[test]
    fn hours_permission_is_separate_from_selection_and_all_draft_combinations_are_deterministic() {
        let (context, volatility) = facts(61);
        assert_eq!(
            EstimateTask::create(
                context.clone(),
                volatility.clone(),
                Horizon::new(HorizonSelection::Hours),
                HoursPermission::default(),
                RiskAppetite::Neutral
            ),
            Err(NoEstimateTask::HoursNotAllowed)
        );
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_u8..4, 0_u8..3), |(days_ratio, risk_index)| {
                let horizon = match days_ratio {
                    0 => HorizonSelection::Hours,
                    1 => HorizonSelection::FewDays,
                    2 => HorizonSelection::WeeksToMonths,
                    _ => HorizonSelection::HalfYearOrMore,
                };
                let risk = match risk_index {
                    0 => RiskAppetite::Conservative,
                    1 => RiskAppetite::Neutral,
                    _ => RiskAppetite::Aggressive,
                };
                let permission =
                    HoursPermission::restore_from_saved_state(HoursPermissionState::Enabled);
                let first = EstimateTask::create(
                    context.clone(),
                    volatility.clone(),
                    Horizon::new(horizon),
                    permission,
                    risk,
                )
                .expect("초안");
                assert_policy(&first, &volatility, horizon, risk);
                proptest::prop_assert_eq!(
                    EstimateTask::create(
                        context.clone(),
                        volatility.clone(),
                        Horizon::new(horizon),
                        permission,
                        risk
                    )
                    .expect("반복"),
                    first
                );
                Ok(())
            })
            .expect("결정성");
    }
}
