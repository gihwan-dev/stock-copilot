use crate::{
    BaseProbability, CalendarDate, CheckResult, EstimateDistance, EstimateRangeFacts, Horizon,
    HorizonSelection, HoursPermission, HoursPermissionState, InstrumentId, PriceObservation,
    ProposalVolatility, ResolvedOrderInstrument, RiskAppetite, Timestamp, TradingDayHorizon,
    Volatility,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateInstrument {
    id: InstrumentId,
    order: ResolvedOrderInstrument,
}
impl EstimateInstrument {
    #[must_use]
    pub fn from_master_lookup(id: InstrumentId, order: ResolvedOrderInstrument) -> Self {
        Self { id, order }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateContext {
    instrument: EstimateInstrument,
    reference: PriceObservation,
    latest_closed_day: CalendarDate,
    created_at: Timestamp,
}
impl EstimateContext {
    #[must_use]
    pub fn new(
        instrument: EstimateInstrument,
        reference: PriceObservation,
        latest_closed_day: CalendarDate,
        created_at: Timestamp,
    ) -> Self {
        Self {
            instrument,
            reference,
            latest_closed_day,
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
    InvalidReference,
    NumericalDomain,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateTask {
    context: EstimateContext,
    volatility: Volatility,
    horizon: TradingDayHorizon,
    target: EstimateDistance,
    stop: EstimateDistance,
    base_probability: BaseProbability,
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
        if volatility.instrument() != &context.instrument.id
            || context.reference.instrument() != &context.instrument.order
        {
            return Err(NoEstimateTask::InstrumentMismatch);
        }
        if volatility.as_of() != context.latest_closed_day {
            return Err(NoEstimateTask::StaleVolatility);
        }
        let expected_currency = if context.instrument.order.market() == crate::Market::Kr {
            crate::Currency::Krw
        } else {
            crate::Currency::Usd
        };
        if context.reference.price().as_money().currency() != expected_currency
            || context
                .reference
                .check(&context.instrument.order, context.created_at)
                != CheckResult::Passed
        {
            return Err(NoEstimateTask::InvalidReference);
        }
        let trading_horizon = TradingDayHorizon::from_estimate_policy(horizon.selection());
        let days_ratio = trading_horizon.as_ratio();
        let (target, stop, base_probability) =
            crate::base_probability::distances(appetite, volatility.daily(), days_ratio)
                .ok_or(NoEstimateTask::NumericalDomain)?;
        Ok(Self {
            context,
            volatility,
            horizon: trading_horizon,
            target,
            stop,
            base_probability,
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
            instrument: self.context.instrument.order.clone(),
            reference: self.reference().price(),
            volatility: ProposalVolatility::from_daily(self.volatility.daily()),
            horizon: self.horizon,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{EstimateContext, EstimateInstrument, EstimateTask, NoEstimateTask};
    use crate::candle::tests::{at, price, series};
    use crate::{
        CheckResult, Horizon, HorizonSelection, HoursPermission, HoursPermissionState,
        InstrumentId, PriceObservation, ResolvedOrderInstrument, RiskAppetite, Timestamp,
        Volatility,
    };
    use crate::{Currency, Market, Ratio, Symbol};
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;
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
        let reference =
            PriceObservation::from_quote_parts(order.clone(), price("100", Currency::Usd), at(61))
                .expect("관측");
        let context = EstimateContext::new(
            EstimateInstrument::from_master_lookup(
                InstrumentId::try_new("sample").expect("id"),
                order,
            ),
            reference,
            volatility.as_of(),
            at(61),
        );
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
        identity.instrument.id = InstrumentId::try_new("other").expect("id");
        assert_eq!(
            make(identity, volatility.clone()),
            Err(NoEstimateTask::InstrumentMismatch)
        );
        let mut date = context.clone();
        date.latest_closed_day = at(59)
            .date_at_offset(crate::OffsetSeconds::new(0))
            .expect("날짜");
        assert_eq!(
            make(date, volatility.clone()),
            Err(NoEstimateTask::StaleVolatility)
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
        identity.reference = PriceObservation::from_quote_parts(
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
        currency.reference = PriceObservation::from_quote_parts(
            context.instrument.order.clone(),
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
        context.instrument.order = order.clone();
        context.reference = PriceObservation::from_quote_parts(
            order,
            price("100", Currency::Krw),
            context.created_at,
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
