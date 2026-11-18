use crate::{
    BuySignal, BuySignalRecord, CalibrationEstimateId, EstimateTask, MarketCalendar,
    PriceObservation, ScorecardObservation, ScorecardPromptId, SellSignal, SignalEstimate,
    SignalReturnError, TimeInterval, Timestamp,
};
#[derive(Clone, PartialEq, Eq)]
pub struct ScorecardTaskId(EstimateTask);
impl ScorecardTaskId {
    pub(crate) fn from_task(task: &EstimateTask) -> Self {
        Self(task.clone())
    }
}
impl std::fmt::Debug for ScorecardTaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ScorecardTaskId")
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScorecardSignalDirection {
    Buy,
    SellAvoidance,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardSignalIdentity {
    estimate: CalibrationEstimateId,
    prompt: ScorecardPromptId,
}
impl ScorecardSignalIdentity {
    #[must_use]
    pub const fn new(estimate: CalibrationEstimateId, prompt: ScorecardPromptId) -> Self {
        Self { estimate, prompt }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct ScorecardSignalRecord {
    identity: ScorecardSignalIdentity,
    facts: SignalEstimate,
    entry: PriceObservation,
    interval: TimeInterval,
    direction: ScorecardSignalDirection,
}
impl std::fmt::Debug for ScorecardSignalRecord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ScorecardSignalRecord")
    }
}
impl ScorecardSignalRecord {
    pub fn from_buy(
        identity: ScorecardSignalIdentity,
        signal: &BuySignal,
        entry: &PriceObservation,
        history: &BuySignalRecord,
    ) -> Result<Self, SignalReturnError> {
        if history.instrument() != signal.facts().task().reference().instrument() {
            return Err(SignalReturnError::ConflictingReturn);
        }
        Self::record(
            identity,
            signal.facts(),
            entry,
            history.interval(),
            ScorecardSignalDirection::Buy,
        )
    }
    pub fn from_sell(
        identity: ScorecardSignalIdentity,
        signal: &SellSignal,
        entry: &PriceObservation,
        at: Timestamp,
        calendars: &[MarketCalendar],
    ) -> Result<Self, SignalReturnError> {
        crate::SignalTiming::new(at, calendars)
            .check(
                signal.facts().task(),
                signal.facts().submission().submitted_at(),
            )
            .or(Err(SignalReturnError::ConflictingReturn))?;
        let interval =
            crate::estimate_outcome::buy_signal_interval_from(signal.facts().task(), at, calendars)
                .or(Err(SignalReturnError::ExitUnavailable))?;
        Self::record(
            identity,
            signal.facts(),
            entry,
            interval,
            ScorecardSignalDirection::SellAvoidance,
        )
    }
    fn record(
        identity: ScorecardSignalIdentity,
        facts: &SignalEstimate,
        entry: &PriceObservation,
        interval: TimeInterval,
        direction: ScorecardSignalDirection,
    ) -> Result<Self, SignalReturnError> {
        if entry.observed_at() != interval.start()
            || entry
                .instrument_id()
                .is_some_and(|id| id != facts.task().volatility().instrument())
            || entry.check(facts.task().reference().instrument(), interval.start())
                != crate::CheckResult::Passed
            || entry.price().as_money().currency()
                != facts.task().reference().price().as_money().currency()
            || interval.start() < facts.submission().submitted_at()
        {
            return Err(SignalReturnError::ConflictingReturn);
        }
        PriceObservation::from_identified_quote_parts(
            facts.task().volatility().instrument().clone(),
            entry.instrument().clone(),
            entry.price(),
            interval.start(),
        )
        .map(|identified| Self {
            identity,
            facts: facts.clone(),
            entry: identified,
            interval,
            direction,
        })
        .or(Err(SignalReturnError::Arithmetic))
    }
    pub fn observation(
        &self,
        calendars: &[MarketCalendar],
        adjustment: &crate::OutcomeAdjustment,
        candles: &[crate::Candle],
        now: Timestamp,
    ) -> Result<ScorecardObservation, SignalReturnError> {
        let outcome =
            crate::EstimateOutcome::evaluate(self.task(), calendars, adjustment, candles, now);
        ScorecardObservation::try_new(
            self.identity.estimate.clone(),
            self.facts.submission().calibration_key().clone(),
            self.identity.prompt.clone(),
            self.facts.submission().probability(),
            outcome,
        )
        .map(|record| crate::scorecard_input::identified_scorecard(record, self.task_id()))
        .map_err(SignalReturnError::Scorecard)
    }
    pub(crate) fn key(&self) -> &crate::CalibrationKey {
        self.facts.submission().calibration_key()
    }
    pub(crate) const fn prompt(&self) -> &ScorecardPromptId {
        &self.identity.prompt
    }
    pub(crate) fn probability(&self) -> crate::AiProbability {
        self.facts.submission().probability()
    }
    #[must_use]
    pub const fn task(&self) -> &EstimateTask {
        self.facts.task()
    }
    #[must_use]
    pub fn task_id(&self) -> ScorecardTaskId {
        ScorecardTaskId::from_task(self.task())
    }
    #[must_use]
    pub const fn estimate_id(&self) -> &CalibrationEstimateId {
        &self.identity.estimate
    }
    #[must_use]
    pub const fn entry(&self) -> &PriceObservation {
        &self.entry
    }
    #[must_use]
    pub const fn interval(&self) -> TimeInterval {
        self.interval
    }
    #[must_use]
    pub const fn direction(&self) -> ScorecardSignalDirection {
        self.direction
    }
}

#[cfg(test)]
mod review_tests {
    use super::{ScorecardSignalDirection, ScorecardSignalIdentity, ScorecardSignalRecord};
    use crate::scorecard_signal_review::tests::{raised, record, signal_facts};
    use crate::signal_return_contract::tests::instant;
    use crate::signal_return_scenarios::tests::{calendar, task};
    use crate::{
        CalibrationEstimateId, PriceObservation, ScorecardPromptId, SignalReturnError, TimeInterval,
    };
    fn identity() -> ScorecardSignalIdentity {
        ScorecardSignalIdentity::new(
            CalibrationEstimateId::try_new("review").expect("ID"),
            ScorecardPromptId::try_new("prompt").expect("프롬프트"),
        )
    }
    #[test]
    fn review_record_rejects_mismatched_entry_and_exposes_masked_identity() {
        let sealed = record(0, ScorecardSignalDirection::Buy, "0");
        assert_eq!(format!("{:?}", sealed.task_id()), "ScorecardTaskId");
        assert_eq!(format!("{sealed:?}"), "ScorecardSignalRecord");
        let (facts, _) = signal_facts(sealed.task(), 0, ScorecardSignalDirection::Buy, "0");
        let invalid = PriceObservation::from_quote_parts(
            sealed.entry().instrument().clone(),
            sealed.entry().price(),
            instant(61, 3),
        )
        .expect("다른 관측 시각");
        assert_eq!(
            ScorecardSignalRecord::record(
                identity(),
                &facts,
                &invalid,
                sealed.interval(),
                ScorecardSignalDirection::Buy
            ),
            Err(SignalReturnError::ConflictingReturn)
        );
        let missing = sealed
            .observation(
                &[],
                &crate::OutcomeAdjustment::from_actions(
                    sealed.task(),
                    crate::AdjustmentBasis::new(instant(67, 4)),
                    &[],
                )
                .expect("조정"),
                &[],
                instant(67, 4),
            )
            .expect("판정 불가 관측");
        assert_eq!(
            crate::scorecard_input::scorecard_matches_signal(&missing, &sealed),
            crate::CheckResult::Passed
        );
        let other_prompt = ScorecardSignalRecord {
            identity: identity(),
            ..sealed
        };
        assert_eq!(
            crate::scorecard_input::scorecard_matches_signal(&missing, &other_prompt),
            crate::CheckResult::Failed
        );
    }
    #[test]
    fn review_buy_history_from_another_instrument_is_rejected() {
        let source = task(0);
        let reference = PriceObservation::from_identified_quote_parts(
            source.volatility().instrument().clone(),
            crate::ResolvedOrderInstrument::from_master_lookup_result(
                crate::Symbol::try_new("OTHER").expect("코드"),
                crate::Market::Kr,
            )
            .expect("종목"),
            source.reference().price(),
            source.created_at(),
        )
        .expect("관측");
        let other = crate::EstimateTask::create(
            crate::EstimateContext::new(reference, source.created_at()),
            source.volatility().clone(),
            crate::Horizon::new(crate::HorizonSelection::FewDays),
            crate::HoursPermission::default(),
            crate::RiskAppetite::Neutral,
        )
        .expect("다른 종목 과제");
        let calendars = (61_i64..=67).map(calendar).collect::<Vec<_>>();
        let mut buys = Vec::new();
        for original in [&source, &other] {
            for direction in [
                ScorecardSignalDirection::Buy,
                ScorecardSignalDirection::SellAvoidance,
            ] {
                let (facts, item) = signal_facts(original, 0, direction, "0");
                match raised(original, &facts, item, &calendars) {
                    crate::RaisedSignal::Buy(buy) => buys.push(buy),
                    crate::RaisedSignal::Sell(sell) => assert_eq!(sell.facts().task(), original),
                }
            }
        }
        let history = crate::BuySignalRecord::from_signal(
            crate::BuySignalOccurrenceId::try_new("history").expect("발생 ID"),
            &buys[1],
            source.created_at(),
            &calendars,
        )
        .expect("타 종목 이력");
        assert_eq!(
            ScorecardSignalRecord::from_buy(identity(), &buys[0], source.reference(), &history),
            Err(SignalReturnError::ConflictingReturn)
        );
    }
    #[test]
    fn review_public_sell_rejects_unavailable_timing() {
        let source = task(0);
        let calendars = (61_i64..=67).map(calendar).collect::<Vec<_>>();
        for direction in [
            ScorecardSignalDirection::Buy,
            ScorecardSignalDirection::SellAvoidance,
        ] {
            let (facts, item) = signal_facts(&source, 0, direction, "0");
            match raised(&source, &facts, item, &calendars) {
                crate::RaisedSignal::Buy(buy) => assert_eq!(buy.facts().task(), &source),
                crate::RaisedSignal::Sell(sell) => {
                    assert_eq!(
                        ScorecardSignalRecord::from_sell(
                            identity(),
                            &sell,
                            source.reference(),
                            instant(60, 4),
                            &calendars
                        ),
                        Err(SignalReturnError::ConflictingReturn)
                    );
                    assert_eq!(
                        ScorecardSignalRecord::from_sell(
                            identity(),
                            &sell,
                            source.reference(),
                            instant(63, 4),
                            &calendars
                        ),
                        Err(SignalReturnError::ExitUnavailable)
                    );
                }
            }
        }
        assert_eq!(
            TimeInterval::try_new(instant(61, 4), instant(67, 4))
                .expect("구간")
                .start(),
            source.reference().observed_at()
        );
    }
}
