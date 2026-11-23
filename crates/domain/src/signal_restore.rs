use crate::{
    AiProbability, CalibrationError, CalibrationKey, CalibrationLedgerHead, CalibrationSnapshot,
    CollectedItem, CommissionRateFact, CostEligibleTask, Disclosure, EstimateCostPeriod,
    EstimateRestoreError, Evidence, FxRate, HistoricalClassificationError,
    HistoricalClassifications, HistoricalPortfolio, Margin, MarketCalendar, MasterInstrument,
    Money, NoSignalReason, NoThresholds, OrderSide, Policy, Probability, Quantity, RaisedSignal,
    Ratio, SignalAssessmentFacts, SignalContext, SignalDecision, SignalEstimate, SignalOperation,
    SignalPreparationError, SignalSourceLedger, SignalSubmission, SignalSubmissionError,
    SignalTaskKey, SignalTiming, StoredEstimateEvidence, StoredPriceObservation, ThresholdDisplay,
    ThresholdKind, Timestamp, ToolReference, TradeCost, TradeCostError, TradeTaxFact,
};

#[derive(Clone, PartialEq, Eq)]
pub struct HistoricalSignalContext {
    pub portfolio: Option<HistoricalPortfolio>,
    pub rate: Option<FxRate>,
    pub policy: Policy,
    pub classifications: HistoricalClassifications,
    pub items: Vec<CollectedItem>,
    pub disclosures: Vec<Disclosure>,
    pub tools: Vec<ToolReference>,
    pub calendars: Vec<MarketCalendar>,
    pub evaluated_at: Timestamp,
    pub target: Option<StoredPriceObservation>,
    pub calibration_head: Option<CalibrationLedgerHead>,
}

impl std::fmt::Debug for HistoricalSignalContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistoricalSignalContext")
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalRestoreEvidence {
    pub estimate: StoredEstimateEvidence,
    pub commission: CommissionRateFact,
    pub tax: TradeTaxFact,
    pub maturity: Timestamp,
    pub calibration: CalibrationSnapshot,
    pub calibration_key: CalibrationKey,
    pub probability: AiProbability,
    pub source: Option<Evidence>,
    pub submitted_at: Timestamp,
    pub margin: Margin,
    pub assessment: HistoricalSignalContext,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HistoricalSignalQuantity(Quantity);
impl HistoricalSignalQuantity {
    #[must_use]
    pub const fn new(quantity: Quantity) -> Self {
        Self(quantity)
    }
    #[must_use]
    pub fn display_text(self) -> crate::HistoricalDecimalText {
        crate::HistoricalDecimalText::from_amount(crate::Amount::new(self.0.as_decimal()))
    }
    pub(crate) const fn quantity(self) -> Quantity {
        self.0
    }
}
impl std::fmt::Debug for HistoricalSignalQuantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoricalSignalQuantity([비공개])")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HistoricalSignalLimit(Money);
impl HistoricalSignalLimit {
    #[must_use]
    pub const fn new(limit: Money) -> Self {
        Self(limit)
    }
    #[must_use]
    pub fn currency(self) -> crate::Currency {
        self.0.currency()
    }
    #[must_use]
    pub fn display_text(self) -> crate::HistoricalDecimalText {
        crate::HistoricalDecimalText::from_amount(self.0.amount())
    }
}
impl std::fmt::Debug for HistoricalSignalLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoricalSignalLimit([비공개])")
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum HistoricalSignalDirection {
    Buy {
        limit: HistoricalSignalLimit,
    },
    Sell {
        quantity: HistoricalSignalQuantity,
        observed_at: Timestamp,
    },
}

impl std::fmt::Debug for HistoricalSignalDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Buy { .. } => f.write_str("Buy([비공개])"),
            Self::Sell { .. } => f.write_str("Sell([비공개])"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalDisplayFacts {
    pub direction: HistoricalSignalDirection,
    pub probability: Probability,
    pub operation: SignalOperation,
    pub buy_threshold: Ratio,
    pub sell_threshold: Ratio,
    pub breakeven_threshold: Ratio,
}

impl SignalDisplayFacts {
    fn from_signal(signal: &RaisedSignal) -> Self {
        let (direction, estimate) = match signal {
            RaisedSignal::Buy(buy) => (
                HistoricalSignalDirection::Buy {
                    limit: HistoricalSignalLimit::new(buy.limit().amount()),
                },
                buy.facts(),
            ),
            RaisedSignal::Sell(sell) => (
                HistoricalSignalDirection::Sell {
                    quantity: HistoricalSignalQuantity::new(sell.held_quantity()),
                    observed_at: sell.observed_at(),
                },
                sell.facts(),
            ),
        };
        let probability = match estimate.calibration() {
            crate::CalibratedProbability::InsufficientSamples(value) => {
                value.evidence().probability()
            }
            crate::CalibratedProbability::EligibleBeforeE91(value) => {
                value.evidence().probability()
            }
        };
        let threshold = |kind| Ratio::new(estimate.thresholds().value(kind).as_decimal());
        Self {
            direction,
            probability,
            operation: estimate.operation(),
            buy_threshold: threshold(ThresholdKind::Buy),
            sell_threshold: threshold(ThresholdKind::Sell),
            breakeven_threshold: threshold(ThresholdKind::Breakeven),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSignalEvidence {
    pub evidence: SignalRestoreEvidence,
    pub original: SignalRestoreEvidence,
    pub expected: SignalDisplayFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalRestoreError {
    Estimate(EstimateRestoreError),
    Cost(TradeCostError),
    Threshold(NoThresholds),
    Submission(SignalSubmissionError),
    Calibration(CalibrationError),
    Preparation(SignalPreparationError),
    Classification(HistoricalClassificationError),
    Target(crate::DraftRestoreError),
    NoSignal(NoSignalReason),
    ChangedSignal,
}

#[derive(Clone, PartialEq, Eq)]
pub struct HistoricalBuySignal(crate::BuySignal);
impl std::fmt::Debug for HistoricalBuySignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoricalBuySignal(..)")
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct HistoricalSellSignal(crate::SellSignal);
impl std::fmt::Debug for HistoricalSellSignal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("HistoricalSellSignal(..)")
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalSignal {
    Buy(HistoricalBuySignal),
    Sell(HistoricalSellSignal),
}
impl HistoricalSignal {
    #[must_use]
    pub fn from_signal(value: &RaisedSignal) -> Self {
        match value {
            RaisedSignal::Buy(buy) => Self::Buy(HistoricalBuySignal(buy.clone())),
            RaisedSignal::Sell(sell) => Self::Sell(HistoricalSellSignal(sell.clone())),
        }
    }
    fn to_internal_signal(&self) -> RaisedSignal {
        match self {
            Self::Buy(value) => RaisedSignal::Buy(value.0.clone()),
            Self::Sell(value) => RaisedSignal::Sell(value.0.clone()),
        }
    }
    fn estimate(&self) -> &SignalEstimate {
        match self {
            Self::Buy(value) => value.0.facts(),
            Self::Sell(value) => value.0.facts(),
        }
    }
    #[must_use]
    pub fn calibration_key(&self) -> &CalibrationKey {
        self.estimate().submission().calibration_key()
    }
    #[must_use]
    pub fn calibration_head(&self) -> &CalibrationLedgerHead {
        self.estimate().calibration_head()
    }
    #[must_use]
    pub fn side(&self) -> OrderSide {
        match self {
            Self::Buy(_) => OrderSide::Buy,
            Self::Sell(_) => OrderSide::Sell,
        }
    }
    #[must_use]
    pub fn market(&self) -> crate::Market {
        self.estimate().task().reference().instrument().market()
    }
    #[must_use]
    pub fn range_facts(&self) -> crate::EstimateRangeFacts {
        self.estimate().task().range_facts()
    }
    pub fn validate_observation(
        &self,
        observation: &crate::PriceObservation,
        now: Timestamp,
    ) -> Result<(), crate::SignalUnavailable> {
        observation.validate_for_task(self.estimate().task(), now)
    }
    pub fn verify_task_identity(
        &self,
        instrument: &crate::InstrumentId,
        issued: &CostEligibleTask,
        master: &crate::ResolvedOrderInstrument,
    ) -> Result<(), crate::SignalTransitionError> {
        let task = self.estimate().task();
        if task.volatility().instrument() != instrument
            || task.reference().instrument() != master
            || issued.task() != task
        {
            return Err(crate::SignalTransitionError::InstrumentMismatch);
        }
        Ok(())
    }
    pub fn display_probability(&self) -> Result<ThresholdDisplay, NoThresholds> {
        self.estimate().display_probability()
    }
    pub fn threshold_display(&self, kind: ThresholdKind) -> Result<ThresholdDisplay, NoThresholds> {
        self.estimate().thresholds().display(kind)
    }
    pub(crate) fn sell_signal(&self) -> Option<&crate::SellSignal> {
        match self {
            Self::Buy(_) => None,
            Self::Sell(value) => Some(&value.0),
        }
    }
    #[must_use]
    pub fn matches_live(&self, live: &RaisedSignal) -> crate::CheckResult {
        match (self, live) {
            (Self::Buy(_), RaisedSignal::Buy(_)) => {
                if Self::from_signal(live) == *self {
                    crate::CheckResult::Passed
                } else {
                    crate::CheckResult::Failed
                }
            }
            (Self::Sell(stored), RaisedSignal::Sell(fresh)) => {
                crate::sell_signal_matches_reassessed(&stored.0, fresh)
            }
            (Self::Buy(_) | Self::Sell(_), RaisedSignal::Buy(_) | RaisedSignal::Sell(_)) => {
                crate::CheckResult::Failed
            }
        }
    }
    #[must_use]
    pub fn display_facts(&self) -> SignalDisplayFacts {
        SignalDisplayFacts::from_signal(&self.to_internal_signal())
    }
}

pub fn signal_transition_from_history(
    previous: Option<&HistoricalSignal>,
    decision: &SignalDecision,
) -> Result<crate::SignalTransition, crate::SignalTransitionError> {
    let signal = previous.map(HistoricalSignal::to_internal_signal);
    crate::signal_transition(signal.as_ref(), decision)
}

impl SignalRestoreEvidence {
    fn estimate_for(
        &self,
        current: &MasterInstrument,
        calibration: &CalibrationSnapshot,
        mode: crate::estimate_restore::MasterReplayMode,
    ) -> Result<(SignalEstimate, crate::QuoteInstrument), SignalRestoreError> {
        let (task, quote) = self
            .estimate
            .replay(current, mode)
            .map_err(SignalRestoreError::Estimate)?;
        let period = EstimateCostPeriod::from_calendar_result(&task, self.maturity)
            .map_err(SignalRestoreError::Cost)?;
        let cost = TradeCost::from_facts(
            Some(self.commission.clone()),
            Some(self.tax.clone()),
            period,
        )
        .map_err(SignalRestoreError::Cost)?;
        let eligible =
            CostEligibleTask::assess(task, &cost).map_err(SignalRestoreError::Threshold)?;
        let submission = SignalSubmission::create(
            SignalTaskKey::from_task(&eligible),
            self.calibration_key.clone(),
            self.probability,
            self.source.clone(),
            self.submitted_at,
        )
        .map_err(SignalRestoreError::Submission)?;
        let calibrated = crate::calibrate_signal_submission(
            Some(calibration),
            calibration.through(),
            submission,
        )
        .map_err(SignalRestoreError::Calibration)?;
        SignalEstimate::create(&eligible, &calibrated, self.margin)
            .map(|estimate| (estimate, quote))
            .map_err(SignalRestoreError::Preparation)
    }

    fn replay(
        &self,
        current: &MasterInstrument,
        mode: crate::estimate_restore::MasterReplayMode,
    ) -> Result<RaisedSignal, SignalRestoreError> {
        let (estimate, quote) = self.estimate_for(current, &self.calibration, mode)?;
        let input = &self.assessment;
        let valuation = input
            .portfolio
            .as_ref()
            .map(|portfolio| portfolio.evaluate(input.rate));
        let (assignments, security) = input
            .classifications
            .replay()
            .map_err(SignalRestoreError::Classification)?;
        let ledger = SignalSourceLedger::new(&input.items, &input.disclosures, &input.tools);
        let target = input
            .target
            .map(|value| match mode {
                crate::estimate_restore::MasterReplayMode::LiveOrderGate => {
                    value.replay_live(current, self.estimate.evidence.side)
                }
                crate::estimate_restore::MasterReplayMode::HistoricalIdentity => {
                    value.replay_historical(&quote)
                }
            })
            .transpose()
            .map_err(SignalRestoreError::Target)?;
        let assessment = SignalAssessmentFacts::new(
            SignalTiming::new(input.evaluated_at, &input.calendars),
            target,
            input.calibration_head.clone(),
        );
        let context = SignalContext::new(
            valuation.as_ref(),
            &input.policy,
            (&assignments, &security),
            &ledger,
            assessment,
        );
        match SignalDecision::assess(&estimate, &context) {
            SignalDecision::Buy(value) => Ok(RaisedSignal::Buy(value)),
            SignalDecision::Sell(value) => Ok(RaisedSignal::Sell(value)),
            SignalDecision::None(reason) => Err(SignalRestoreError::NoSignal(reason)),
        }
    }

    pub fn capture(
        &self,
        original: &RaisedSignal,
        current: &MasterInstrument,
    ) -> Result<StoredSignalEvidence, SignalRestoreError> {
        let rebuilt = self.replay(
            current,
            crate::estimate_restore::MasterReplayMode::LiveOrderGate,
        )?;
        let actual = SignalDisplayFacts::from_signal(&rebuilt);
        let expected = SignalDisplayFacts::from_signal(original);
        let rebuilt_estimate = signal_estimate(&rebuilt);
        let original_estimate = signal_estimate(original);
        if &rebuilt != original
            || display_offset(&actual) != display_offset(&expected)
            || rebuilt_estimate
                .submission()
                .submitted_at()
                .offset_seconds()
                != original_estimate
                    .submission()
                    .submitted_at()
                    .offset_seconds()
            || crate::estimate_restore::check_task_offsets(
                rebuilt_estimate.task(),
                original_estimate.task(),
            )
            .is_err()
        {
            return Err(SignalRestoreError::ChangedSignal);
        }
        Ok(StoredSignalEvidence {
            evidence: self.clone(),
            original: self.clone(),
            expected: SignalDisplayFacts::from_signal(original),
        })
    }
}

fn signal_estimate(signal: &RaisedSignal) -> &SignalEstimate {
    match signal {
        RaisedSignal::Buy(value) => value.facts(),
        RaisedSignal::Sell(value) => value.facts(),
    }
}

impl StoredSignalEvidence {
    pub fn restore_history(
        &self,
        current: &MasterInstrument,
    ) -> Result<HistoricalSignal, SignalRestoreError> {
        let signal = self.evidence.replay(
            current,
            crate::estimate_restore::MasterReplayMode::HistoricalIdentity,
        )?;
        check_signal_evidence_match(&self.evidence, &self.original)?;
        let actual = SignalDisplayFacts::from_signal(&signal);
        if actual != self.expected || display_offset(&actual) != display_offset(&self.expected) {
            return Err(SignalRestoreError::ChangedSignal);
        }
        Ok(HistoricalSignal::from_signal(&signal))
    }

    pub fn verify_same_as(&self, other: &Self) -> Result<(), SignalRestoreError> {
        check_signal_evidence_match(&self.evidence, &other.evidence)?;
        check_signal_evidence_match(&self.original, &other.original)?;
        if self.expected != other.expected
            || display_offset(&self.expected) != display_offset(&other.expected)
        {
            return Err(SignalRestoreError::ChangedSignal);
        }
        Ok(())
    }
}

fn check_signal_evidence_match(
    left: &SignalRestoreEvidence,
    right: &SignalRestoreEvidence,
) -> Result<(), SignalRestoreError> {
    if left != right
        || left.maturity.offset_seconds() != right.maturity.offset_seconds()
        || left.submitted_at.offset_seconds() != right.submitted_at.offset_seconds()
        || left.assessment.evaluated_at.offset_seconds()
            != right.assessment.evaluated_at.offset_seconds()
        || left
            .assessment
            .target
            .map(|value| value.observed_at.offset_seconds())
            != right
                .assessment
                .target
                .map(|value| value.observed_at.offset_seconds())
    {
        return Err(SignalRestoreError::ChangedSignal);
    }
    left.estimate
        .check_stored_offsets(&right.estimate)
        .map_err(SignalRestoreError::Estimate)?;
    check_context_offsets(&left.assessment, &right.assessment)
}

fn display_offset(facts: &SignalDisplayFacts) -> Option<crate::OffsetSeconds> {
    match facts.direction {
        HistoricalSignalDirection::Buy { .. } => None,
        HistoricalSignalDirection::Sell { observed_at, .. } => Some(observed_at.offset_seconds()),
    }
}

fn check_context_offsets(
    left: &HistoricalSignalContext,
    right: &HistoricalSignalContext,
) -> Result<(), SignalRestoreError> {
    let rate_offsets = |rate: Option<FxRate>| {
        rate.map(|value| {
            (
                value.as_of().offset_seconds(),
                value.valid_until().offset_seconds(),
            )
        })
    };
    if rate_offsets(left.rate) != rate_offsets(right.rate) {
        return Err(SignalRestoreError::ChangedSignal);
    }
    for (first, second) in left.calendars.iter().zip(&right.calendars) {
        check_calendar_offsets(first, second)?;
    }
    Ok(())
}

fn check_calendar_offsets(
    first: &MarketCalendar,
    second: &MarketCalendar,
) -> Result<(), SignalRestoreError> {
    for (left_segment, right_segment) in first.sessions().iter().zip(second.sessions()) {
        let left = left_segment.session();
        let right = right_segment.session();
        let left_interval = left.interval();
        let right_interval = right.interval();
        if left_interval.start().offset_seconds() != right_interval.start().offset_seconds()
            || left_interval.end().offset_seconds() != right_interval.end().offset_seconds()
            || auction_offset(left.auction()) != auction_offset(right.auction())
        {
            return Err(SignalRestoreError::ChangedSignal);
        }
    }
    Ok(())
}

fn auction_offset(auction: Option<crate::AuctionBoundary>) -> Option<crate::OffsetSeconds> {
    match auction {
        Some(crate::AuctionBoundary::StartsAt(value) | crate::AuctionBoundary::EndsAt(value)) => {
            Some(value.offset_seconds())
        }
        None => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSignalOccurrence {
    pub signal: StoredSignalEvidence,
    pub id: crate::BuySignalOccurrenceId,
    pub expected_id: crate::BuySignalOccurrenceId,
    pub raised_at: Timestamp,
    pub interval: crate::TimeInterval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalOccurrenceRestoreError {
    Signal(SignalRestoreError),
    NotBuy,
    Evaluation(NoSignalReason),
    ChangedOccurrence,
}

impl StoredSignalOccurrence {
    pub fn capture(
        original: &crate::BuySignalRecord,
        signal: StoredSignalEvidence,
        current: &MasterInstrument,
    ) -> Result<Self, SignalOccurrenceRestoreError> {
        let evidence = Self {
            signal,
            id: original.id().clone(),
            expected_id: original.id().clone(),
            raised_at: original.raised_at(),
            interval: original.interval(),
        };
        if &evidence.restore(current)?.0 != original {
            return Err(SignalOccurrenceRestoreError::ChangedOccurrence);
        }
        Ok(evidence)
    }

    pub fn restore(
        &self,
        current: &MasterInstrument,
    ) -> Result<HistoricalSignalOccurrence, SignalOccurrenceRestoreError> {
        if self.id != self.expected_id {
            return Err(SignalOccurrenceRestoreError::ChangedOccurrence);
        }
        let history = self
            .signal
            .restore_history(current)
            .map_err(SignalOccurrenceRestoreError::Signal)?;
        let buy = match history {
            HistoricalSignal::Buy(buy) => buy,
            HistoricalSignal::Sell(_) => return Err(SignalOccurrenceRestoreError::NotBuy),
        };
        let record = crate::BuySignalRecord::from_signal_checked(
            self.id.clone(),
            &buy.0,
            self.raised_at,
            &self.signal.evidence.assessment.calendars,
        )
        .map_err(SignalOccurrenceRestoreError::Evaluation)?;
        if record.interval() != self.interval
            || record.interval().start().offset_seconds() != self.interval.start().offset_seconds()
            || record.interval().end().offset_seconds() != self.interval.end().offset_seconds()
        {
            return Err(SignalOccurrenceRestoreError::ChangedOccurrence);
        }
        Ok(HistoricalSignalOccurrence(record))
    }

    pub fn verify_commit_binding(
        &self,
        signal: &StoredSignalEvidence,
        record: &crate::BuySignalRecord,
    ) -> Result<(), SignalOccurrenceRestoreError> {
        self.signal
            .verify_same_as(signal)
            .map_err(SignalOccurrenceRestoreError::Signal)?;
        if self.id != *record.id()
            || self.expected_id != *record.id()
            || self.raised_at != record.raised_at()
            || self.raised_at.offset_seconds() != record.raised_at().offset_seconds()
            || self.interval != record.interval()
            || self.interval.start().offset_seconds() != record.interval().start().offset_seconds()
            || self.interval.end().offset_seconds() != record.interval().end().offset_seconds()
        {
            return Err(SignalOccurrenceRestoreError::ChangedOccurrence);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalSignalOccurrence(crate::BuySignalRecord);

impl HistoricalSignalOccurrence {
    #[must_use]
    pub fn from_record(record: &crate::BuySignalRecord) -> Self {
        Self(record.clone())
    }
    pub(crate) fn record(&self) -> &crate::BuySignalRecord {
        &self.0
    }
    pub(crate) fn into_record(self) -> crate::BuySignalRecord {
        self.0
    }
    #[must_use]
    pub fn id(&self) -> &crate::BuySignalOccurrenceId {
        self.0.id()
    }
    #[must_use]
    pub fn instrument_id(&self) -> &crate::InstrumentId {
        self.0.instrument_id()
    }
    #[must_use]
    pub fn interval(&self) -> crate::TimeInterval {
        self.0.interval()
    }
    #[must_use]
    pub fn raised_at(&self) -> Timestamp {
        self.0.raised_at()
    }
}

#[cfg(test)]
mod tests {
    use super::SignalDisplayFacts;
    use crate::{
        AdjustmentBasis, AiProbability, AiProbabilityInput, BuyingPower, Calibration,
        CalibrationEstimate, CalibrationEstimateId, CalibrationFact, CalibrationKey,
        CalibrationLedgerHead, CalibrationModel, CalibrationOutcome, CalibrationPosition,
        CalibrationSnapshot, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        CollectedItem, CommissionRateFact, CommissionValidity, CommonShare, CostEligibleTask,
        CostRate, Currency, DisplayInstrumentName, EstimateContext, EstimateCostPeriod,
        EstimateRestoreEvidence, EstimateTask, Evidence, EvidenceRef, EvidenceUrl,
        HistoricalClassifications, HistoricalPortfolio, HistoricalSignalContext, Horizon,
        HorizonSelection, HoursPermission, HoursPermissionState, InstrumentId, InvestorProfile,
        ListingStatus, Margin, Market, MarketCalendar, MarketSession, MasterCode, MasterInstrument,
        MasterInstrumentFacts, MasterInstrumentRow, MasterSecurityType, ModelIdentityPart, Money,
        Nanosecond, NormalizedUrl, NumericInput, OffsetSeconds, Ohlcv, OrderSide, Policy,
        Portfolio, Price, PriceAdjustment, Quantity, Ratio, RiskAppetite, SectorAssignments,
        SectorCatalog, SessionData, SessionInformation, SignalAssessmentFacts, SignalContext,
        SignalDecision, SignalEstimate, SignalOperation, SignalRestoreEvidence, SignalSourceLedger,
        SignalSubmission, SignalTaskKey, SignalTiming, SpanSeconds, StoredEstimateEvidence,
        StoredPriceObservation, Symbol, TimeInterval, TimeSpan, Timestamp, TradeCost, TradeTaxFact,
        UnixSeconds, UntrustedText, Volatility, calibrate_signal_submission,
        sector_assignment_from_compositions,
    };

    fn at(day: i64, offset: i32) -> Timestamp {
        let delta = day.checked_mul(86_400_i64).expect("일");
        Timestamp::from_parts(
            UnixSeconds::new(1_767_225_600_i64.checked_add(delta).expect("시각")),
            Nanosecond::new(0),
            OffsetSeconds::new(offset),
        )
        .expect("시각")
    }
    fn shifted(day: i64, seconds: u64) -> Timestamp {
        at(day, -18_000)
            .checked_add(
                TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0)).expect("기간"),
            )
            .expect("시각")
    }
    fn session(day: i64) -> MarketSession {
        MarketSession::try_new(
            at(day, -18_000).offset_calendar_date(),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(shifted(day, 21_600), shifted(day, 25_200))
                            .expect("구간"),
                        None,
                    )
                    .expect("장"),
                ),
                after_market: None,
            },
        )
        .expect("세션")
    }
    fn thirty_facts(key: &CalibrationKey) -> Vec<CalibrationFact> {
        let probability =
            AiProbability::parse(&AiProbabilityInput::try_new("70").expect("입력")).expect("확률");
        (0_i64..30_i64)
            .map(|index| {
                let start_day = index.checked_mul(10_i64).expect("시작일");
                let end_day = start_day.checked_add(5_i64).expect("종료일");
                CalibrationFact::new(
                    CalibrationEstimate::new(
                        CalibrationEstimateId::try_new(&format!("est-{index}")).expect("ID"),
                        InstrumentId::try_new(&format!("inst-{index}")).expect("종목"),
                        key.clone(),
                        TimeInterval::try_new(at(start_day, -18_000), at(end_day, -18_000))
                            .expect("구간"),
                        probability,
                    ),
                    CalibrationOutcome::Hit,
                )
            })
            .collect()
    }
    fn price(text: &str) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Usd).expect("가격")
    }
    fn master_row() -> MasterInstrumentRow {
        MasterInstrumentRow {
            listing_market: MasterCode::try_new("NASDAQ").expect("시장"),
            symbol: Symbol::try_new("ELIG").expect("심볼"),
            instrument_id: InstrumentId::try_new("elig-1").expect("식별자"),
            name: DisplayInstrumentName::try_new("적격 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("유형")),
            currency: Currency::Usd,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: None,
            observed_at: at(61, -18_000),
        }
    }
    fn candles() -> Vec<Candle> {
        (0_i64..61_i64)
            .map(|day| {
                let value = if day.checked_rem(2_i64) == Some(0_i64) {
                    price("100")
                } else {
                    price("103")
                };
                Candle::try_new(
                    InstrumentId::try_new("elig-1").expect("식별자"),
                    CandleTime::try_new(at(day, 0), CandlePeriod::Daily).expect("일봉"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(at(70, 0))),
                    Ohlcv::try_new(
                        value,
                        value,
                        value,
                        value,
                        Quantity::parse(&NumericInput::try_new("3").expect("입력")).expect("수량"),
                    )
                    .expect("봉"),
                    CandleObservation::new(at(70, 0), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect()
    }
    struct EligibleFixture {
        master: MasterInstrument,
        stored_estimate: StoredEstimateEvidence,
        eligible: CostEligibleTask,
        commission: CommissionRateFact,
        tax: TradeTaxFact,
        maturity: Timestamp,
    }
    fn eligible_task_fixture() -> EligibleFixture {
        let row = master_row();
        let master = MasterInstrument::try_new(row.clone()).expect("조회");
        let input = EstimateRestoreEvidence {
            master: row,
            side: OrderSide::Buy,
            candles: candles(),
            reference_price: price("104"),
            reference_at: at(61, -18_000),
            task_at: at(61, -18_000),
            horizon: Horizon::new(HorizonSelection::FewDays),
            permission: HoursPermissionState::Disabled,
            appetite: RiskAppetite::Neutral,
        };
        let task = EstimateTask::create(
            EstimateContext::new(
                master
                    .price_observation(input.reference_price, input.reference_at)
                    .expect("시세"),
                input.task_at,
            ),
            Volatility::calculate(&input.candles).expect("변동성"),
            input.horizon,
            HoursPermission::default(),
            input.appetite,
        )
        .expect("과제");
        let stored_estimate = input.capture(&task, &master).expect("과제 근거");
        let zero = CostRate::try_new(
            Ratio::parse(&NumericInput::try_new("0").expect("입력")).expect("비율"),
        )
        .expect("요율");
        let commission = CommissionRateFact::from_commissions_result(
            Market::Us,
            zero,
            CommissionValidity::try_new(None, None).expect("기간"),
        );
        let tax = TradeTaxFact::from_schedule_result(Market::Us, zero, zero);
        let maturity = shifted(67, 25_200);
        let cost = TradeCost::from_facts(
            Some(commission.clone()),
            Some(tax.clone()),
            EstimateCostPeriod::from_calendar_result(&task, maturity).expect("기간"),
        )
        .expect("비용");
        let eligible = CostEligibleTask::assess(task, &cost).expect("적격");
        EligibleFixture {
            master,
            stored_estimate,
            eligible,
            commission,
            tax,
            maturity,
        }
    }
    fn eligible_portfolio() -> Portfolio {
        Portfolio::new(
            vec![],
            BuyingPower::new(
                Money::parse(
                    &NumericInput::try_new("1000000").expect("입력"),
                    Currency::Krw,
                )
                .expect("원화"),
                at(61, -18_000),
            ),
            BuyingPower::new(
                Money::parse(&NumericInput::try_new("0").expect("입력"), Currency::Usd)
                    .expect("달러"),
                at(61, -18_000),
            ),
        )
        .expect("포트폴리오")
    }
    fn eligible_context(
        master: &MasterInstrument,
        item: CollectedItem,
        head: CalibrationLedgerHead,
    ) -> (
        Portfolio,
        SectorAssignments,
        crate::PolicySecurityFacts,
        Vec<MarketCalendar>,
        crate::PriceObservation,
        HistoricalSignalContext,
    ) {
        let portfolio = eligible_portfolio();
        let catalog = SectorCatalog::from_response(vec![], at(61, -18_000)).expect("목록");
        let assignments = SectorAssignments::try_new(vec![
            sector_assignment_from_compositions(master, &catalog, &[]).expect("분류"),
        ])
        .expect("분류표");
        let security = MasterInstrumentFacts::try_new(vec![master.clone()])
            .expect("마스터")
            .policy_security_facts();
        let calendars: Vec<_> = (60_i64..=68_i64)
            .map(|day| {
                MarketCalendar::try_new(
                    session(day.checked_sub(1_i64).expect("전일")),
                    session(day),
                    session(day.checked_add(1_i64).expect("익일")),
                )
                .expect("달력")
            })
            .collect();
        let target = master
            .price_observation(price("105"), at(61, -18_000))
            .expect("시세");
        let policy = Policy::new(InvestorProfile::default());
        let historical = HistoricalSignalContext {
            portfolio: Some(
                HistoricalPortfolio::capture(&portfolio, std::slice::from_ref(master))
                    .expect("포트폴리오 근거"),
            ),
            rate: None,
            policy,
            classifications: HistoricalClassifications::capture(&assignments, &security),
            items: vec![item],
            disclosures: vec![],
            tools: vec![],
            calendars: calendars.clone(),
            evaluated_at: at(61, -18_000),
            target: Some(StoredPriceObservation::from_price_observation(&target)),
            calibration_head: Some(head),
        };
        (
            portfolio,
            assignments,
            security,
            calendars,
            target,
            historical,
        )
    }
    struct EligibleModelFixture {
        key: CalibrationKey,
        head: CalibrationLedgerHead,
        snapshot: CalibrationSnapshot,
        probability: AiProbability,
        item: CollectedItem,
        source: Evidence,
        margin: Margin,
        estimate: SignalEstimate,
    }
    fn eligible_model_fixture(base: &EligibleFixture) -> EligibleModelFixture {
        let part = ModelIdentityPart::try_new("model").expect("모델");
        let key = CalibrationKey::new(
            CalibrationModel::new(part.clone(), part.clone(), part),
            Horizon::new(HorizonSelection::FewDays),
        );
        let head = CalibrationLedgerHead::new(key.clone(), CalibrationPosition::new(30));
        let calibration = Calibration::try_new(key.clone(), &thirty_facts(&key)).expect("보정");
        let snapshot = CalibrationSnapshot::try_new(calibration, head.clone()).expect("스냅숏");
        let probability =
            AiProbability::parse(&AiProbabilityInput::try_new("99").expect("입력")).expect("확률");
        let url = NormalizedUrl::try_new("https://example.com/eligible").expect("주소");
        let item = CollectedItem::from_web(
            url.clone(),
            UntrustedText::try_new("적격 출처").expect("제목"),
            None,
        );
        let source = Evidence::try_new(vec![EvidenceRef::Web(
            EvidenceUrl::try_new(url).expect("출처"),
        )])
        .expect("근거");
        let submission = SignalSubmission::create(
            SignalTaskKey::from_task(&base.eligible),
            key.clone(),
            probability,
            Some(source.clone()),
            at(61, -18_000),
        )
        .expect("제출");
        let calibrated =
            calibrate_signal_submission(Some(&snapshot), &head, submission).expect("보정");
        let margin = Margin::for_appetite(RiskAppetite::Neutral);
        let estimate = SignalEstimate::create(&base.eligible, &calibrated, margin).expect("판정");
        EligibleModelFixture {
            key,
            head,
            snapshot,
            probability,
            item,
            source,
            margin,
            estimate,
        }
    }
    #[test]
    fn eligible_calibration_signal_roundtrips_display_facts() {
        let base = eligible_task_fixture();
        let model = eligible_model_fixture(&base);
        let (portfolio, assignments, security, calendars, target, historical) =
            eligible_context(&base.master, model.item.clone(), model.head.clone());
        let valuation = portfolio.evaluate(None);
        let ledger = SignalSourceLedger::new(std::slice::from_ref(&model.item), &[], &[]);
        let facts = SignalAssessmentFacts::new(
            SignalTiming::new(at(61, -18_000), &calendars),
            Some(target),
            Some(model.head),
        );
        let context = SignalContext::new(
            Some(&valuation),
            &historical.policy,
            (&assignments, &security),
            &ledger,
            facts,
        );
        let signal = SignalDecision::assess(&model.estimate, &context)
            .into_signal()
            .expect("신호");
        let evidence = SignalRestoreEvidence {
            estimate: base.stored_estimate,
            commission: base.commission,
            tax: base.tax,
            maturity: base.maturity,
            calibration: model.snapshot,
            calibration_key: model.key,
            probability: model.probability,
            source: Some(model.source),
            submitted_at: at(61, -18_000),
            margin: model.margin,
            assessment: historical,
        };
        let saved = evidence.capture(&signal, &base.master).expect("수집");
        let restored = saved.restore_history(&base.master).expect("복원");
        assert_eq!(
            restored.display_facts().operation,
            SignalOperation::EligibleBeforeE91
        );
        assert_eq!(
            restored.display_facts(),
            SignalDisplayFacts::from_signal(&signal)
        );
    }
    fn held_valuation_for(
        master: &MasterInstrument,
        qty_str: &str,
        obs: Timestamp,
    ) -> crate::PortfolioValuation {
        let held = crate::Holding::new(
            master.order_instrument().expect("종목"),
            Quantity::parse(&NumericInput::try_new(qty_str).expect("입력")).expect("수량"),
            crate::AcquisitionPrice::try_new(
                Money::parse(&NumericInput::try_new("90").expect("입력"), Currency::Usd)
                    .expect("매입가"),
            )
            .expect("매입"),
            crate::HoldingObservation::new(Some(price("100")), None, obs),
        )
        .expect("보유");
        Portfolio::new(
            vec![held],
            BuyingPower::new(
                Money::parse(
                    &NumericInput::try_new("1000000").expect("입력"),
                    Currency::Krw,
                )
                .expect("원화"),
                obs,
            ),
            BuyingPower::new(
                Money::parse(&NumericInput::try_new("0").expect("입력"), Currency::Usd)
                    .expect("달러"),
                obs,
            ),
        )
        .expect("포트")
        .evaluate(None)
    }

    fn low_probability_estimate(
        base: &EligibleFixture,
        model: &EligibleModelFixture,
    ) -> SignalEstimate {
        let low_prob =
            AiProbability::parse(&AiProbabilityInput::try_new("1").expect("입력")).expect("확률");
        let low_sub = SignalSubmission::create(
            SignalTaskKey::from_task(&base.eligible),
            model.key.clone(),
            low_prob,
            Some(model.source.clone()),
            at(61, -18_000),
        )
        .expect("제출");
        let low_cal =
            calibrate_signal_submission(Some(&model.snapshot), &model.head, low_sub).expect("보정");
        SignalEstimate::create(&base.eligible, &low_cal, model.margin).expect("판정")
    }

    fn richer_valuation(at_time: Timestamp) -> crate::PortfolioValuation {
        Portfolio::new(
            vec![],
            BuyingPower::new(
                Money::parse(
                    &NumericInput::try_new("2000000").expect("입력"),
                    Currency::Krw,
                )
                .expect("원화"),
                at_time,
            ),
            BuyingPower::new(
                Money::parse(&NumericInput::try_new("0").expect("입력"), Currency::Usd)
                    .expect("달러"),
                at_time,
            ),
        )
        .expect("포트")
        .evaluate(None)
    }

    #[test]
    fn historical_signal_matches_live_checks_buy_sell_and_cross_direction() {
        let base = eligible_task_fixture();
        let model = eligible_model_fixture(&base);
        let (portfolio, assignments, security, calendars, target, historical) =
            eligible_context(&base.master, model.item.clone(), model.head.clone());
        let ledger = SignalSourceLedger::new(std::slice::from_ref(&model.item), &[], &[]);
        let t0 = at(61, -18_000);
        let t5 = shifted(61, 5);
        let t10 = shifted(61, 10);
        let assess = |est: &SignalEstimate, val: &crate::PortfolioValuation, now: Timestamp| {
            let facts = SignalAssessmentFacts::new(
                SignalTiming::new(now, &calendars),
                Some(target.clone()),
                Some(model.head.clone()),
            );
            let context = SignalContext::new(
                Some(val),
                &historical.policy,
                (&assignments, &security),
                &ledger,
                facts,
            );
            SignalDecision::assess(est, &context)
                .into_signal()
                .expect("신호")
        };
        let buy_one = assess(&model.estimate, &portfolio.evaluate(None), t0);
        let hist_buy = crate::HistoricalSignal::from_signal(&buy_one);
        assert_eq!(hist_buy.matches_live(&buy_one), crate::CheckResult::Passed);
        assert_eq!(
            hist_buy.matches_live(&assess(&model.estimate, &richer_valuation(t0), t0)),
            crate::CheckResult::Failed
        );

        let sell_est = low_probability_estimate(&base, &model);
        let sell_t5 = assess(&sell_est, &held_valuation_for(&base.master, "10", t5), t5);
        let sell_newer = assess(&sell_est, &held_valuation_for(&base.master, "10", t10), t10);
        let sell_older = assess(&sell_est, &held_valuation_for(&base.master, "10", t0), t10);
        let sell_qty = assess(&sell_est, &held_valuation_for(&base.master, "9", t10), t10);
        let hist_sell = crate::HistoricalSignal::from_signal(&sell_t5);
        assert_eq!(hist_sell.matches_live(&sell_t5), crate::CheckResult::Passed);
        assert_eq!(
            hist_sell.matches_live(&sell_newer),
            crate::CheckResult::Passed
        );
        assert_eq!(
            hist_sell.matches_live(&sell_older),
            crate::CheckResult::Failed
        );
        assert_eq!(
            hist_sell.matches_live(&sell_qty),
            crate::CheckResult::Failed
        );
        assert_eq!(hist_buy.matches_live(&sell_t5), crate::CheckResult::Failed);
        assert_eq!(hist_sell.matches_live(&buy_one), crate::CheckResult::Failed);
    }
}
