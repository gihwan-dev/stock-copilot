use crate::calibration_arithmetic::{
    CalibrationNumber, CalibrationSample, average, recalibrated, squared_error, sum,
};
use crate::{
    AiProbability, Amount, BaseProbability, Horizon, InstrumentId, MoneyError, Nanosecond,
    Probability, Ratio, RatioQuotient, TimeInterval, Timestamp, UnixSeconds,
};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationError {
    InvalidModelIdentity,
    TooManyFacts,
    InvalidEstimateId,
    ConflictingEstimate,
    MismatchedCombination,
    MissingCalibration,
    StaleCalibration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelIdentityPart(String);
impl ModelIdentityPart {
    pub fn try_new(value: &str) -> Result<Self, CalibrationError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(CalibrationError::InvalidModelIdentity);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationModel {
    provider: ModelIdentityPart,
    name: ModelIdentityPart,
    version: ModelIdentityPart,
}
impl CalibrationModel {
    #[must_use]
    pub fn new(
        provider: ModelIdentityPart,
        name: ModelIdentityPart,
        version: ModelIdentityPart,
    ) -> Self {
        Self {
            provider,
            name,
            version,
        }
    }
    #[must_use]
    pub const fn provider(&self) -> &ModelIdentityPart {
        &self.provider
    }
    #[must_use]
    pub const fn name(&self) -> &ModelIdentityPart {
        &self.name
    }
    #[must_use]
    pub const fn version(&self) -> &ModelIdentityPart {
        &self.version
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationKey {
    model: CalibrationModel,
    horizon: Horizon,
}
impl CalibrationKey {
    #[must_use]
    pub fn new(model: CalibrationModel, horizon: Horizon) -> Self {
        Self { model, horizon }
    }
    #[must_use]
    pub const fn model(&self) -> &CalibrationModel {
        &self.model
    }
    #[must_use]
    pub const fn horizon(&self) -> Horizon {
        self.horizon
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CalibrationOutcome {
    Miss,
    Hit,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalibrationEstimateId(String);
impl CalibrationEstimateId {
    pub fn try_new(value: &str) -> Result<Self, CalibrationError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(CalibrationError::InvalidEstimateId);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalibrationInstant {
    seconds: UnixSeconds,
    nanosecond: Nanosecond,
}
impl CalibrationInstant {
    pub fn from_parts(
        seconds: UnixSeconds,
        nanosecond: Nanosecond,
    ) -> Result<Self, crate::TimeError> {
        let timestamp = Timestamp::from_parts(seconds, nanosecond, crate::OffsetSeconds::new(0))?;
        Ok(Self::from_timestamp(timestamp))
    }
    #[must_use]
    pub fn from_timestamp(value: Timestamp) -> Self {
        Self {
            seconds: value.utc_seconds(),
            nanosecond: value.nanosecond(),
        }
    }
    #[must_use]
    pub const fn utc_seconds(self) -> UnixSeconds {
        self.seconds
    }
    #[must_use]
    pub const fn nanosecond(self) -> Nanosecond {
        self.nanosecond
    }
}
impl Ord for CalibrationInstant {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.seconds.as_i64(), self.nanosecond.as_u32())
            .cmp(&(other.seconds.as_i64(), other.nanosecond.as_u32()))
    }
}
impl PartialOrd for CalibrationInstant {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalibrationInterval {
    start: CalibrationInstant,
    end: CalibrationInstant,
}
impl CalibrationInterval {
    pub fn try_new(
        start: CalibrationInstant,
        end: CalibrationInstant,
    ) -> Result<Self, crate::TimeIntervalError> {
        if start >= end {
            return Err(crate::TimeIntervalError::NonIncreasing);
        }
        Ok(Self { start, end })
    }
    #[must_use]
    pub fn from_interval(value: TimeInterval) -> Self {
        Self {
            start: CalibrationInstant::from_timestamp(value.start()),
            end: CalibrationInstant::from_timestamp(value.end()),
        }
    }
    #[must_use]
    pub const fn start(self) -> CalibrationInstant {
        self.start
    }
    #[must_use]
    pub const fn end(self) -> CalibrationInstant {
        self.end
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationEstimate {
    id: CalibrationEstimateId,
    instrument: InstrumentId,
    key: CalibrationKey,
    interval: CalibrationInterval,
    predicted_probability: AiProbability,
}
impl CalibrationEstimate {
    #[must_use]
    pub(crate) fn new(
        id: CalibrationEstimateId,
        instrument: InstrumentId,
        key: CalibrationKey,
        interval: TimeInterval,
        predicted_probability: AiProbability,
    ) -> Self {
        Self {
            id,
            instrument,
            key,
            interval: CalibrationInterval::from_interval(interval),
            predicted_probability,
        }
    }
    #[must_use]
    pub const fn id(&self) -> &CalibrationEstimateId {
        &self.id
    }
    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }
    #[must_use]
    pub const fn key(&self) -> &CalibrationKey {
        &self.key
    }
    #[must_use]
    pub const fn estimated_at(&self) -> CalibrationInstant {
        self.interval.start()
    }
    #[must_use]
    pub const fn interval(&self) -> CalibrationInterval {
        self.interval
    }
    #[must_use]
    pub const fn predicted_probability(&self) -> AiProbability {
        self.predicted_probability
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedCalibrationFact {
    pub id: CalibrationEstimateId,
    pub instrument: InstrumentId,
    pub key: CalibrationKey,
    pub interval: CalibrationInterval,
    pub predicted_probability: AiProbability,
    pub outcome: CalibrationOutcome,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationFact {
    estimate: CalibrationEstimate,
    outcome: CalibrationOutcome,
}
impl CalibrationFact {
    #[must_use]
    pub(crate) fn new(estimate: CalibrationEstimate, outcome: CalibrationOutcome) -> Self {
        Self { estimate, outcome }
    }
    #[must_use]
    pub fn restore_from_saved_state(saved: SavedCalibrationFact) -> Self {
        Self {
            estimate: CalibrationEstimate {
                id: saved.id,
                instrument: saved.instrument,
                key: saved.key,
                interval: saved.interval,
                predicted_probability: saved.predicted_probability,
            },
            outcome: saved.outcome,
        }
    }
    #[must_use]
    pub const fn estimate(&self) -> &CalibrationEstimate {
        &self.estimate
    }
    #[must_use]
    pub const fn outcome(&self) -> CalibrationOutcome {
        self.outcome
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CalibrationCount(usize);
impl CalibrationCount {
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationReadiness {
    Conservative,
    MinimumReached,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationCapacity {
    Available,
    Exhausted,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalibratedProbability {
    InsufficientSamples(ConservativeCalibration),
    EligibleBeforeE91(EligibleCalibration),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConservativeCalibration(CalibrationEvidence);
impl ConservativeCalibration {
    #[must_use]
    pub const fn evidence(&self) -> &CalibrationEvidence {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EligibleCalibration(CalibrationEvidence);
impl EligibleCalibration {
    #[must_use]
    pub const fn evidence(&self) -> &CalibrationEvidence {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationEvidence {
    probability: Probability,
    original: Probability,
    key: CalibrationKey,
    judgment_count: CalibrationCount,
}
impl CalibrationEvidence {
    #[must_use]
    pub const fn probability(&self) -> Probability {
        self.probability
    }
    #[must_use]
    pub const fn original_probability(&self) -> Probability {
        self.original
    }
    #[must_use]
    pub const fn key(&self) -> &CalibrationKey {
        &self.key
    }
    #[must_use]
    pub const fn judgment_count(&self) -> &CalibrationCount {
        &self.judgment_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationBin {
    Below50,
    From50To60,
    From60To70,
    AtLeast70,
}
impl CalibrationBin {
    #[must_use]
    pub fn for_probability(probability: Probability) -> Self {
        let value = probability.as_decimal();
        if value < Decimal::new(50, 2) {
            Self::Below50
        } else if value < Decimal::new(60, 2) {
            Self::From50To60
        } else if value < Decimal::new(70, 2) {
            Self::From60To70
        } else {
            Self::AtLeast70
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationBinStats {
    judgment_count: CalibrationCount,
    hits: CalibrationCount,
}
impl CalibrationBinStats {
    #[must_use]
    pub const fn judgment_count(&self) -> &CalibrationCount {
        &self.judgment_count
    }
    #[must_use]
    pub const fn hits(&self) -> &CalibrationCount {
        &self.hits
    }
    pub fn hit_rate(self) -> Result<Option<RatioQuotient>, MoneyError> {
        if self.judgment_count.0 == 0 {
            return Ok(None);
        }
        crate::exact_arithmetic::divide(
            Amount::new(Decimal::from(self.hits.0)),
            Amount::new(Decimal::from(self.judgment_count.0)),
        )
        .map(Some)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationTable {
    selected: Vec<CalibrationFact>,
}
impl CalibrationTable {
    #[must_use]
    pub fn selected_facts(&self) -> &[CalibrationFact] {
        &self.selected
    }
    #[must_use]
    pub fn judgment_count(&self) -> CalibrationCount {
        CalibrationCount(self.selected.len())
    }
    #[must_use]
    pub fn readiness(&self) -> CalibrationReadiness {
        if self.judgment_count().0 < 30 {
            CalibrationReadiness::Conservative
        } else {
            CalibrationReadiness::MinimumReached
        }
    }
    #[must_use]
    pub fn bin(&self, bin: CalibrationBin) -> CalibrationBinStats {
        let rows: Vec<_> = self
            .selected
            .iter()
            .filter(|fact| {
                CalibrationBin::for_probability(
                    fact.estimate.predicted_probability.into_probability(),
                ) == bin
            })
            .collect();
        CalibrationBinStats {
            judgment_count: CalibrationCount(rows.len()),
            hits: CalibrationCount(
                rows.iter()
                    .filter(|fact| fact.outcome == CalibrationOutcome::Hit)
                    .count(),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Calibration {
    key: CalibrationKey,
    facts: Vec<CalibrationFact>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalibrationPosition(u64);
impl CalibrationPosition {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u64(&self) -> &u64 {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationLedgerHead {
    key: CalibrationKey,
    position: CalibrationPosition,
}
impl CalibrationLedgerHead {
    #[must_use]
    pub const fn new(key: CalibrationKey, position: CalibrationPosition) -> Self {
        Self { key, position }
    }
    #[must_use]
    pub const fn key(&self) -> &CalibrationKey {
        &self.key
    }
    #[must_use]
    pub const fn position(&self) -> CalibrationPosition {
        self.position
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationSnapshot {
    calibration: Calibration,
    through: CalibrationLedgerHead,
}
impl CalibrationSnapshot {
    pub fn try_new(
        calibration: Calibration,
        through: CalibrationLedgerHead,
    ) -> Result<Self, CalibrationError> {
        if calibration.key != through.key {
            return Err(CalibrationError::MismatchedCombination);
        }
        Ok(Self {
            calibration,
            through,
        })
    }
    #[must_use]
    pub const fn calibration(&self) -> &Calibration {
        &self.calibration
    }
    #[must_use]
    pub const fn through(&self) -> &CalibrationLedgerHead {
        &self.through
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FactRepetition {
    New,
    Repeated,
}
fn repetition(
    prior: Option<&CalibrationFact>,
    current: &CalibrationFact,
) -> Result<FactRepetition, CalibrationError> {
    if let Some(previous) = prior
        && previous.estimate.id == current.estimate.id
    {
        if previous != current {
            return Err(CalibrationError::ConflictingEstimate);
        }
        return Ok(FactRepetition::Repeated);
    }
    Ok(FactRepetition::New)
}
impl Calibration {
    #[must_use]
    pub const fn key(&self) -> &CalibrationKey {
        &self.key
    }
    #[must_use]
    pub fn capacity(&self) -> CalibrationCapacity {
        if self.facts.len() >= 65_536 {
            CalibrationCapacity::Exhausted
        } else {
            CalibrationCapacity::Available
        }
    }
    pub fn with_fact(&self, fact: CalibrationFact) -> Result<Self, CalibrationError> {
        match self.capacity() {
            CalibrationCapacity::Exhausted => return Err(CalibrationError::TooManyFacts),
            CalibrationCapacity::Available => (),
        }
        let mut combined = self.facts.clone();
        combined.push(fact);
        Self::try_new(self.key.clone(), &combined)
    }
    #[must_use]
    pub fn facts(&self) -> &[CalibrationFact] {
        &self.facts
    }
    pub fn try_new(
        key: CalibrationKey,
        facts: &[CalibrationFact],
    ) -> Result<Self, CalibrationError> {
        if facts.len() > 65_536 {
            return Err(CalibrationError::TooManyFacts);
        }
        if facts.iter().any(|fact| fact.estimate.key != key) {
            return Err(CalibrationError::MismatchedCombination);
        }
        Self::from_validated_facts(key, facts.to_vec())
    }
    fn from_validated_facts(
        key: CalibrationKey,
        mut ordered: Vec<CalibrationFact>,
    ) -> Result<Self, CalibrationError> {
        ordered.sort_by(|left, right| left.estimate.id.cmp(&right.estimate.id));
        let mut unique = Vec::new();
        for fact in ordered {
            match repetition(unique.last(), &fact)? {
                FactRepetition::New => unique.push(fact),
                FactRepetition::Repeated => {}
            }
        }
        Ok(Self { key, facts: unique })
    }
    pub fn for_combination(
        &self,
        key: &CalibrationKey,
    ) -> Result<CalibrationTable, CalibrationError> {
        if &self.key != key {
            return Err(CalibrationError::MismatchedCombination);
        }
        Ok(self.table())
    }
    pub(crate) fn table(&self) -> CalibrationTable {
        let mut facts = self.facts.clone();
        facts.sort_by(selection_order);
        let mut selected: Vec<CalibrationFact> = Vec::new();
        for fact in facts {
            if let Some(previous) = selected.last()
                && previous.estimate.instrument == fact.estimate.instrument
                && fact.estimate.interval.start() <= previous.estimate.interval.end()
            {
                continue;
            }
            selected.push(fact);
        }
        CalibrationTable { selected }
    }
    pub fn calibrate(
        &self,
        key: &CalibrationKey,
        probability: AiProbability,
        base: BaseProbability,
    ) -> Result<CalibratedProbability, CalibrationError> {
        let table = self.for_combination(key)?;
        Ok(self.calibrate_table(&table, probability, base))
    }
    pub(crate) fn calibrate_bound(
        &self,
        probability: AiProbability,
        base: BaseProbability,
    ) -> CalibratedProbability {
        self.calibrate_table(&self.table(), probability, base)
    }
    fn calibrate_table(
        &self,
        table: &CalibrationTable,
        probability: AiProbability,
        base: BaseProbability,
    ) -> CalibratedProbability {
        let original = probability.into_probability();
        let evidence = |used| CalibrationEvidence {
            probability: used,
            original,
            key: self.key.clone(),
            judgment_count: table.judgment_count(),
        };
        match table.readiness() {
            CalibrationReadiness::Conservative => CalibratedProbability::InsufficientSamples(
                ConservativeCalibration(evidence(original)),
            ),
            CalibrationReadiness::MinimumReached => {
                match shrunk_toward_base(
                    table.selected_facts(),
                    original,
                    base,
                    CalibrationPriorStrength::DEFAULT,
                ) {
                    Some(calibrated) => CalibratedProbability::EligibleBeforeE91(
                        EligibleCalibration(evidence(calibrated)),
                    ),
                    None => CalibratedProbability::InsufficientSamples(ConservativeCalibration(
                        evidence(original),
                    )),
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalibrationPriorStrength(u32);
impl CalibrationPriorStrength {
    pub const DEFAULT: Self = Self(30);
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u32(&self) -> &u32 {
        &self.0
    }
}

const fn observed(outcome: CalibrationOutcome) -> CalibrationNumber {
    match outcome {
        CalibrationOutcome::Hit => CalibrationNumber::ONE,
        CalibrationOutcome::Miss => CalibrationNumber::ZERO,
    }
}

fn number(probability: Probability) -> CalibrationNumber {
    CalibrationNumber::new(probability.as_decimal())
}

#[must_use]
pub fn shrunk_toward_base(
    facts: &[CalibrationFact],
    probability: Probability,
    base: BaseProbability,
    strength: CalibrationPriorStrength,
) -> Option<Probability> {
    let samples: Vec<CalibrationSample> = facts
        .iter()
        .map(|fact| CalibrationSample {
            predicted: number(fact.estimate.predicted_probability.into_probability()),
            observed: observed(fact.outcome),
        })
        .collect();
    let value = recalibrated(
        &samples,
        number(probability),
        CalibrationNumber::new(base.as_decimal()),
        CalibrationNumber::new(Decimal::from(strength.0)),
    )?;
    Probability::try_new(value.as_decimal()).ok()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrierScore(CalibrationNumber);
impl BrierScore {
    #[must_use]
    pub fn as_ratio(&self) -> Ratio {
        Ratio::new(self.0.as_decimal())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrierComparison {
    pub before: BrierScore,
    pub after: BrierScore,
}

#[must_use]
pub fn brier_comparison(
    calibrated: &CalibratedProbability,
    outcome: CalibrationOutcome,
) -> Option<BrierComparison> {
    let evidence = match calibrated {
        CalibratedProbability::InsufficientSamples(value) => value.evidence(),
        CalibratedProbability::EligibleBeforeE91(value) => value.evidence(),
    };
    let hit = observed(outcome);
    Some(BrierComparison {
        before: BrierScore(squared_error(number(evidence.original), hit)?),
        after: BrierScore(squared_error(number(evidence.probability), hit)?),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrierTally {
    count: CalibrationNumber,
    before: CalibrationNumber,
    after: CalibrationNumber,
    judged: CalibrationCount,
}
impl BrierTally {
    #[must_use]
    pub fn with(self, comparison: BrierComparison) -> Option<Self> {
        Some(Self {
            count: sum(self.count, CalibrationNumber::ONE)?,
            before: sum(self.before, comparison.before.0)?,
            after: sum(self.after, comparison.after.0)?,
            judged: CalibrationCount(self.judged.0.checked_add(1)?),
        })
    }
    #[must_use]
    pub const fn judgment_count(&self) -> &CalibrationCount {
        &self.judged
    }
    #[must_use]
    pub fn mean(&self) -> Option<BrierComparison> {
        if self.judged.0 == 0 {
            return None;
        }
        Some(BrierComparison {
            before: BrierScore(average(self.before, self.count)?),
            after: BrierScore(average(self.after, self.count)?),
        })
    }
}
fn selection_order(left: &CalibrationFact, right: &CalibrationFact) -> std::cmp::Ordering {
    left.estimate
        .instrument
        .cmp(&right.estimate.instrument)
        .then(
            left.estimate
                .interval
                .start()
                .cmp(&right.estimate.interval.start()),
        )
        .then_with(|| {
            right
                .estimate
                .interval
                .end()
                .cmp(&left.estimate.interval.end())
        })
        .then(left.outcome.cmp(&right.outcome))
        .then(
            left.estimate
                .predicted_probability
                .into_probability()
                .cmp(&right.estimate.predicted_probability.into_probability()),
        )
        .then(left.estimate.id.cmp(&right.estimate.id))
}

pub(crate) const fn scorecard_calibration_key(value: &Calibration) -> &CalibrationKey {
    &value.key
}

#[cfg(test)]
mod tests {
    use super::{
        CalibratedProbability, Calibration, CalibrationBin, CalibrationError, CalibrationEstimate,
        CalibrationEstimateId, CalibrationEvidence, CalibrationFact, CalibrationInstant,
        CalibrationKey, CalibrationModel, CalibrationOutcome, ModelIdentityPart,
    };
    use crate::{
        AiProbability, AiProbabilityInput, Horizon, HorizonSelection, InstrumentId, Nanosecond,
        OffsetSeconds, TimeInterval, Timestamp, UnixSeconds,
    };
    use proptest::strategy::Strategy;
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn at(seconds: i64, offset: i32) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(offset),
        )
        .expect("시각")
    }
    fn key(version: &str) -> CalibrationKey {
        CalibrationKey::new(
            CalibrationModel::new(
                ModelIdentityPart::try_new("provider").expect("제공자"),
                ModelIdentityPart::try_new("model").expect("모델"),
                ModelIdentityPart::try_new(version).expect("버전"),
            ),
            Horizon::new(HorizonSelection::FewDays),
        )
    }
    fn ai(percent: u8) -> AiProbability {
        AiProbability::parse(&AiProbabilityInput::try_new(&percent.to_string()).expect("정수 입력"))
            .expect("AI 확률")
    }
    fn row(
        id: &str,
        symbol: &str,
        bounds: (i64, i64, i32),
        percent: u8,
        outcome: CalibrationOutcome,
    ) -> CalibrationFact {
        CalibrationFact::new(
            CalibrationEstimate::new(
                CalibrationEstimateId::try_new(id).expect("추정 식별"),
                InstrumentId::try_new(symbol).expect("종목"),
                key("v1"),
                TimeInterval::try_new(at(bounds.0, bounds.2), at(bounds.1, bounds.2))
                    .expect("구간"),
                ai(percent),
            ),
            outcome,
        )
    }
    fn snapshot(rows: &[CalibrationFact]) -> Calibration {
        Calibration::try_new(key("v1"), rows).expect("조합 입력")
    }
    fn selected(rows: &[CalibrationFact]) -> Vec<CalibrationFact> {
        snapshot(rows)
            .for_combination(&key("v1"))
            .expect("같은 조합")
            .selected_facts()
            .to_vec()
    }
    fn evidence(result: &CalibratedProbability) -> &CalibrationEvidence {
        match result {
            CalibratedProbability::InsufficientSamples(value) => value.evidence(),
            CalibratedProbability::EligibleBeforeE91(value) => value.evidence(),
        }
    }
    #[test]
    fn reviewer_weekly_28_day_year_counts_330_and_bridge_does_not_erase_history() {
        let rows: Vec<_> = (0_u32..30)
            .flat_map(|symbol| {
                (0_i64..52).map(move |week| {
                    row(
                        &format!("{symbol}-{week}"),
                        &symbol.to_string(),
                        (
                            week.saturating_mul(604_800),
                            week.saturating_mul(604_800).saturating_add(2_419_200),
                            0_i32,
                        ),
                        60,
                        CalibrationOutcome::Hit,
                    )
                })
            })
            .collect();
        assert_eq!(selected(&rows).len(), 330);
        let first = row("first", "A", (0, 2, 0_i32), 60, CalibrationOutcome::Hit);
        let second = row("second", "A", (4, 6, 0_i32), 70, CalibrationOutcome::Miss);
        let bridge = row("bridge", "A", (2, 4, 0_i32), 50, CalibrationOutcome::Hit);
        assert_eq!(
            selected(&[first.clone(), second.clone(), bridge]),
            vec![first, second]
        );
    }
    #[test]
    fn closed_endpoint_ties_and_late_historical_outcomes_are_explicit() {
        let first = row("z", "A", (0, 3, 32400_i32), 80, CalibrationOutcome::Hit);
        let longer = row("z2", "A", (0, 4, 0_i32), 80, CalibrationOutcome::Hit);
        let miss = row("z3", "A", (0, 4, -18000_i32), 80, CalibrationOutcome::Miss);
        let lower = row("z4", "A", (0, 4, 0_i32), 60, CalibrationOutcome::Miss);
        let identifier = row("a", "A", (0, 4, 32400_i32), 60, CalibrationOutcome::Miss);
        assert_eq!(
            selected(&[first.clone(), longer.clone()]),
            vec![longer.clone()]
        );
        assert_eq!(
            selected(&[longer.clone(), miss.clone()]),
            vec![miss.clone()]
        );
        assert_eq!(
            selected(&[miss.clone(), lower.clone()]),
            vec![lower.clone()]
        );
        assert_eq!(
            selected(&[lower.clone(), identifier.clone()]),
            vec![identifier.clone()]
        );
        let touching = row("touching", "A", (4, 6, 0_i32), 90, CalibrationOutcome::Hit);
        let later = row("later", "A", (5, 7, 0_i32), 60, CalibrationOutcome::Hit);
        assert_eq!(
            selected(&[
                first,
                longer,
                miss,
                lower,
                identifier.clone(),
                touching,
                later.clone()
            ]),
            vec![identifier.clone(), later.clone()]
        );
        let historical = row(
            "historical",
            "A",
            (-1, 8, 0_i32),
            40,
            CalibrationOutcome::Hit,
        );
        assert_eq!(
            selected(&[identifier, later, historical.clone()]),
            vec![historical]
        );
    }
    #[test]
    fn insufficient_samples_and_eligible_identity_require_distinct_matches() {
        for count in [0_usize, 29, 30] {
            let rows: Vec<_> = (0..count)
                .map(|index| {
                    row(
                        &index.to_string(),
                        &index.to_string(),
                        (0, 1, 0_i32),
                        60,
                        CalibrationOutcome::Hit,
                    )
                })
                .collect();
            let calibration = snapshot(&rows);
            let result = calibration
                .calibrate(&key("v1"), ai(37), base(37))
                .expect("같은 조합");
            assert_eq!(evidence(&result).key(), &key("v1"));
            assert_eq!(evidence(&result).judgment_count().as_usize(), count);
            assert_eq!(
                evidence(&result).original_probability().as_decimal(),
                Decimal::new(37, 2)
            );
            assert_eq!(
                evidence(&result).probability().as_decimal(),
                if count < 30 {
                    Decimal::new(37, 2)
                } else {
                    Decimal::new(685, 3)
                }
            );
            assert_eq!(matches_insufficient(&result), count < 30);
            match result {
                CalibratedProbability::InsufficientSamples(_) => assert!(count < 30),
                CalibratedProbability::EligibleBeforeE91(_) => assert_eq!(count, 30),
            }
            assert_eq!(
                calibration.calibrate(&key("v2"), ai(37), base(37)),
                Err(CalibrationError::MismatchedCombination)
            );
        }
        let empty = snapshot(&[]);
        assert_eq!(
            empty.calibrate(&key("arbitrary"), ai(60), base(40)),
            Err(CalibrationError::MismatchedCombination)
        );
    }
    fn base(percent: i64) -> crate::BaseProbability {
        crate::base_probability::tests::base_value(Decimal::new(percent, 2))
    }
    fn matches_insufficient(value: &CalibratedProbability) -> bool {
        match value {
            CalibratedProbability::InsufficientSamples(_) => true,
            CalibratedProbability::EligibleBeforeE91(_) => false,
        }
    }
    #[test]
    fn identical_estimates_deduplicate_and_conflicting_estimates_are_rejected() {
        let first = row("same", "A", (0, 5, 0_i32), 60, CalibrationOutcome::Hit);
        let alternate_offset = row("same", "A", (0, 5, 32400_i32), 60, CalibrationOutcome::Hit);
        assert_eq!(
            selected(&[first.clone(), alternate_offset.clone()]),
            vec![first.clone()]
        );
        assert_eq!(
            format!("{:?}", selected(&[alternate_offset.clone(), first.clone()])),
            format!("{:?}", selected(&[first.clone(), alternate_offset]))
        );
        assert_eq!(
            snapshot(&[first.clone(), first.clone()]),
            snapshot(std::slice::from_ref(&first))
        );
        let conflict = CalibrationFact::new(first.estimate().clone(), CalibrationOutcome::Miss);
        assert_eq!(
            Calibration::try_new(key("v1"), &[first.clone(), conflict]),
            Err(CalibrationError::ConflictingEstimate)
        );
        let changed = row("same", "A", (0, 6, 0_i32), 60, CalibrationOutcome::Hit);
        assert_eq!(
            Calibration::try_new(key("v1"), &[first, changed]),
            Err(CalibrationError::ConflictingEstimate)
        );
    }
    #[test]
    fn per_combination_capacity_accepts_200_stocks_weekly_for_five_years() {
        let rows: Vec<_> = (0_u32..200)
            .flat_map(|symbol| {
                (0_i64..260).map(move |week| {
                    row(
                        &format!("{symbol}-{week}"),
                        &symbol.to_string(),
                        (
                            week.saturating_mul(604_800),
                            week.saturating_mul(604_800).saturating_add(2_419_200),
                            0_i32,
                        ),
                        60,
                        CalibrationOutcome::Hit,
                    )
                })
            })
            .collect();
        assert_eq!(
            snapshot(&rows)
                .for_combination(&key("v1"))
                .expect("같은 조합")
                .judgment_count()
                .as_usize(),
            10400
        );
        let duplicate = row("same", "A", (0, 1, 0_i32), 60, CalibrationOutcome::Hit);
        assert_eq!(selected(&vec![duplicate.clone(); 65536]).len(), 1);
        assert_eq!(
            Calibration::try_new(key("v1"), &vec![duplicate; 65537]),
            Err(CalibrationError::TooManyFacts)
        );
        assert_eq!(
            Calibration::try_new(key("v2"), &rows),
            Err(CalibrationError::MismatchedCombination)
        );
        assert_eq!(
            snapshot(&rows).calibrate(&key("v2"), ai(60), base(40)),
            Err(CalibrationError::MismatchedCombination)
        );
    }
    #[test]
    fn d51_matching_empty_table_is_conservative_and_mismatch_is_error() {
        let calibration = snapshot(&[]);
        let matching = calibration
            .calibrate(&key("v1"), ai(63), base(40))
            .expect("같은 조합");
        assert!(matches_insufficient(&matching));
        assert_eq!(evidence(&matching).judgment_count().as_usize(), 0);
        assert_eq!(
            evidence(&matching).probability().as_decimal(),
            Decimal::new(63, 2)
        );
        assert_eq!(
            calibration.for_combination(&key("v2")),
            Err(CalibrationError::MismatchedCombination)
        );
    }
    #[test]
    fn bins_rates_accessors_and_ai_canonical_scale_preserve_contracts() {
        for (percent, bin) in [
            (1, CalibrationBin::Below50),
            (49, CalibrationBin::Below50),
            (50, CalibrationBin::From50To60),
            (59, CalibrationBin::From50To60),
            (60, CalibrationBin::From60To70),
            (69, CalibrationBin::From60To70),
            (70, CalibrationBin::AtLeast70),
            (99, CalibrationBin::AtLeast70),
        ] {
            let table = snapshot(&[row(
                "id",
                "A",
                (0, 1, 32400_i32),
                percent,
                CalibrationOutcome::Hit,
            )])
            .for_combination(&key("v1"))
            .expect("같은 조합");
            assert_eq!(table.bin(bin).judgment_count().as_usize(), 1);
            assert_eq!(table.bin(bin).hits().as_usize(), 1);
            assert_eq!(
                format!("{:?}", table.bin(bin).hit_rate()),
                "Ok(Some(Exact(Ratio(1))))"
            );
        }
        let rows = [
            row("a", "A", (0, 1, 0_i32), 60, CalibrationOutcome::Hit),
            row("b", "B", (0, 1, 0_i32), 60, CalibrationOutcome::Miss),
            row("c", "C", (0, 1, 0_i32), 60, CalibrationOutcome::Miss),
        ];
        let table = snapshot(&rows)
            .for_combination(&key("v1"))
            .expect("같은 조합");
        assert_eq!(
            format!("{:?}", table.bin(CalibrationBin::From60To70).hit_rate()),
            "Ok(Some(Approximate(ApproximateRatio(0.3333333333333333333333333333))))"
        );
        assert_eq!(table.bin(CalibrationBin::Below50).hit_rate(), Ok(None));
    }
    #[test]
    fn estimate_accessors_and_ai_canonical_scale_preserve_contracts() {
        let first = row("a", "A", (0, 1, 0_i32), 60, CalibrationOutcome::Hit);
        let estimate = first.estimate();
        assert_eq!(estimate.id().as_str(), "a");
        assert_eq!(estimate.instrument().as_str(), "A");
        assert_eq!(estimate.key().model().provider().as_str(), "provider");
        assert_eq!(estimate.key().model().name().as_str(), "model");
        assert_eq!(estimate.key().model().version().as_str(), "v1");
        assert_eq!(
            estimate.key().horizon(),
            Horizon::new(HorizonSelection::FewDays)
        );
        assert_eq!(estimate.estimated_at().utc_seconds(), UnixSeconds::new(0));
        assert_eq!(estimate.estimated_at().nanosecond(), Nanosecond::new(0));
        assert_eq!(
            estimate.interval().end(),
            CalibrationInstant::from_timestamp(at(1, 0_i32))
        );
        assert_eq!(
            estimate
                .predicted_probability()
                .into_probability()
                .as_decimal(),
            Decimal::new(60, 2)
        );
        assert_eq!(first.outcome(), CalibrationOutcome::Hit);
        assert_eq!(ai(60).into_probability().as_decimal().scale(), 2);
        for spelling in ["0.6", "0.600", "60.0"] {
            assert_eq!(
                AiProbabilityInput::try_new(spelling),
                Err(crate::AiProbabilityError::InvalidLength)
            );
        }
    }
    #[test]
    fn identifiers_reject_unsafe_inputs_and_preserve_bounds() {
        assert_eq!(
            CalibrationBin::for_probability(crate::Probability::try_new(Decimal::ZERO).expect("0")),
            CalibrationBin::Below50
        );
        assert_eq!(
            CalibrationBin::for_probability(crate::Probability::try_new(Decimal::ONE).expect("1")),
            CalibrationBin::AtLeast70
        );
        let nanos = Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(1),
            OffsetSeconds::new(0),
        )
        .expect("나노초");
        assert!(
            CalibrationInstant::from_timestamp(nanos)
                > CalibrationInstant::from_timestamp(at(0, 0_i32))
        );

        for invalid in ["", "a b", "한글", &"a".repeat(65)] {
            assert_eq!(
                CalibrationEstimateId::try_new(invalid),
                Err(CalibrationError::InvalidEstimateId)
            );
            assert_eq!(
                ModelIdentityPart::try_new(invalid),
                Err(CalibrationError::InvalidModelIdentity)
            );
        }
        assert_eq!(
            CalibrationEstimateId::try_new(&"a".repeat(64))
                .expect("상한")
                .as_str()
                .len(),
            64
        );
        assert_eq!(
            ModelIdentityPart::try_new(&"a".repeat(64))
                .expect("상한")
                .as_str()
                .len(),
            64
        );
    }

    fn test_offset(enabled: u8) -> i32 {
        if enabled == 1 { 32400 } else { 0 }
    }
    fn test_outcome(hit: u8) -> CalibrationOutcome {
        if hit == 1 {
            CalibrationOutcome::Hit
        } else {
            CalibrationOutcome::Miss
        }
    }
    fn generated_rows() -> impl Strategy<Value = Vec<CalibrationFact>> {
        proptest::collection::vec(
            (0_i64..60, 1_i64..12, 0_u8..4, 1_u8..100, 0_u8..2, 0_u8..2),
            0..45,
        )
        .prop_map(|values| {
            values
                .iter()
                .enumerate()
                .map(|(index, (start, length, symbol, percent, hit, offset))| {
                    row(
                        &index.to_string(),
                        &symbol.to_string(),
                        (*start, start.saturating_add(*length), test_offset(*offset)),
                        *percent,
                        test_outcome(*hit),
                    )
                })
                .collect()
        })
    }
    fn reference(rows: &[CalibrationFact]) -> Vec<CalibrationFact> {
        let mut remaining = rows.to_vec();
        let mut selected = Vec::new();
        while !remaining.is_empty() {
            let next = reference_first(&remaining);
            remaining.retain(|candidate| {
                candidate.estimate().instrument() != next.estimate().instrument()
                    || candidate.estimate().estimated_at() > next.estimate().interval().end()
            });
            selected.push(next);
        }
        selected
    }
    fn reference_first(rows: &[CalibrationFact]) -> CalibrationFact {
        rows.iter()
            .min_by_key(|fact| {
                let estimate = fact.estimate();
                (
                    estimate.instrument().clone(),
                    estimate.estimated_at(),
                    std::cmp::Reverse(estimate.interval().end()),
                    fact.outcome(),
                    estimate.predicted_probability().into_probability(),
                    estimate.id().clone(),
                )
            })
            .expect("대표")
            .clone()
    }
    #[test]
    fn appended_later_judgments_preserve_selected_prefix_and_count() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&generated_rows(), |rows| {
                let before = selected(&rows);
                let latest = rows
                    .iter()
                    .map(|fact| *fact.estimate().estimated_at().utc_seconds().as_i64())
                    .max()
                    .map_or(0_i64, |seconds| seconds.saturating_add(1));
                let mut extended = rows;
                extended.push(row(
                    "later",
                    "0",
                    (latest, latest.saturating_add(10), 0_i32),
                    37,
                    CalibrationOutcome::Miss,
                ));
                let after = selected(&extended);
                proptest::prop_assert!(after.len() >= before.len());
                proptest::prop_assert!(before.iter().all(|previous| after.contains(previous)));
                Ok(())
            })
            .expect("뒤에 추가한 판정의 단조성");
    }
    #[test]
    fn selected_estimates_are_pairwise_non_overlapping() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&generated_rows(), |rows| {
                let counted = selected(&rows);
                proptest::prop_assert!(
                    counted.iter().enumerate().all(|(index, left)| {
                        counted
                            .iter()
                            .skip(index.saturating_add(1))
                            .all(|right| pair_is_separate(left, right))
                    }),
                    "선택된 쌍은 겹치지 않는다"
                );
                Ok(())
            })
            .expect("선택 집합은 비겹침");
    }
    fn pair_is_separate(left: &CalibrationFact, right: &CalibrationFact) -> bool {
        left.estimate().instrument() != right.estimate().instrument()
            || left.estimate().interval().end() < right.estimate().estimated_at()
    }
    #[test]
    fn input_permutations_and_offset_spellings_preserve_canonical_results() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&generated_rows(), |rows| {
                let expected = selected(&rows);
                let mut reversed = rows;
                reversed.reverse();
                proptest::prop_assert_eq!(
                    format!("{:?}", selected(&reversed)),
                    format!("{expected:?}")
                );
                if !reversed.is_empty() {
                    reversed.rotate_left(1);
                }
                proptest::prop_assert_eq!(selected(&reversed), expected);
                Ok(())
            })
            .expect("입력 순서와 시각 표현에 무관");
    }
    #[test]
    fn selection_matches_independent_elimination_reference() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&generated_rows(), |rows| {
                proptest::prop_assert_eq!(selected(&rows), reference(&rows));
                Ok(())
            })
            .expect("독립 제거 기준과 일치");
    }
}

#[cfg(test)]
mod e91_tests {
    use super::{
        BrierTally, CalibratedProbability, Calibration, CalibrationEstimate, CalibrationEstimateId,
        CalibrationFact, CalibrationKey, CalibrationModel, CalibrationOutcome,
        CalibrationPriorStrength, ModelIdentityPart, brier_comparison, shrunk_toward_base,
    };
    use crate::{
        AiProbability, AiProbabilityInput, BaseProbability, Horizon, HorizonSelection,
        InstrumentId, Nanosecond, OffsetSeconds, TimeInterval, Timestamp, UnixSeconds,
    };
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn key() -> CalibrationKey {
        CalibrationKey::new(
            CalibrationModel::new(
                ModelIdentityPart::try_new("provider").expect("제공자"),
                ModelIdentityPart::try_new("model").expect("모델"),
                ModelIdentityPart::try_new("v1").expect("버전"),
            ),
            Horizon::new(HorizonSelection::FewDays),
        )
    }
    fn ai(percent: u8) -> AiProbability {
        AiProbability::parse(&AiProbabilityInput::try_new(&percent.to_string()).expect("입력"))
            .expect("AI 확률")
    }
    fn base(percent: i64) -> BaseProbability {
        crate::base_probability::tests::base_value(Decimal::new(percent, 2))
    }
    fn fact(id: &str, percent: u8, outcome: CalibrationOutcome) -> CalibrationFact {
        CalibrationFact::new(
            CalibrationEstimate::new(
                CalibrationEstimateId::try_new(id).expect("ID"),
                InstrumentId::try_new(id).expect("종목"),
                key(),
                TimeInterval::try_new(at(0), at(10)).expect("구간"),
                ai(percent),
            ),
            outcome,
        )
    }
    fn identity_facts(per_percent: u8) -> Vec<CalibrationFact> {
        (1_u8..=99_u8)
            .flat_map(|percent| (0_u8..per_percent).map(move |index| (percent, index)))
            .map(|(percent, index)| {
                let hit = u16::from(index).saturating_mul(100)
                    < u16::from(percent).saturating_mul(u16::from(per_percent));
                fact(
                    &format!("e{percent}-{index}"),
                    percent,
                    if hit {
                        CalibrationOutcome::Hit
                    } else {
                        CalibrationOutcome::Miss
                    },
                )
            })
            .collect()
    }
    fn shrunk(facts: &[CalibrationFact], percent: u8, prior: i64) -> Decimal {
        shrunk_toward_base(
            facts,
            ai(percent).into_probability(),
            base(prior),
            CalibrationPriorStrength::DEFAULT,
        )
        .expect("보정")
        .as_decimal()
    }

    #[test]
    fn a_single_sample_leaves_the_result_next_to_the_base_probability() {
        let one = [fact("only", 90, CalibrationOutcome::Hit)];
        let value = shrunk(&one, 90, 30);
        assert_eq!(value, Decimal::new(32_2581, 6));
        assert!((value - Decimal::new(30, 2)).abs() < Decimal::new(3, 2));
        assert_eq!(shrunk(&[], 90, 30), Decimal::new(30, 2));
    }

    #[test]
    fn many_well_calibrated_samples_move_the_result_toward_the_ai_estimate() {
        let facts = identity_facts(100);
        let value = shrunk(&facts, 80, 30);
        assert!(value > Decimal::new(79, 2), "{value}");
        assert!(value <= Decimal::new(80, 2), "{value}");
        let heavier_prior = shrunk_toward_base(
            &facts,
            ai(80).into_probability(),
            base(30),
            CalibrationPriorStrength::new(9_900),
        )
        .expect("보정")
        .as_decimal();
        assert!(heavier_prior < value, "{heavier_prior} < {value}");
        assert!(heavier_prior > Decimal::new(50, 2), "{heavier_prior}");
        assert!(heavier_prior < Decimal::new(60, 2), "{heavier_prior}");
    }

    #[test]
    fn overconfident_history_pulls_high_estimates_down() {
        let facts: Vec<_> = (0_u8..60_u8)
            .map(|index| {
                fact(
                    &format!("o{index}"),
                    if index % 2 == 0 { 90 } else { 10 },
                    if index % 3 == 0 {
                        CalibrationOutcome::Hit
                    } else {
                        CalibrationOutcome::Miss
                    },
                )
            })
            .collect();
        assert!(shrunk(&facts, 90, 40) < Decimal::new(90, 2));
    }

    #[test]
    fn conservative_combinations_keep_the_original_and_eligible_ones_record_both() {
        let few: Vec<_> = (0_u8..29_u8)
            .map(|index| fact(&format!("f{index}"), 70, CalibrationOutcome::Hit))
            .collect();
        let conservative = Calibration::try_new(key(), &few)
            .expect("표")
            .calibrate(&key(), ai(70), base(30))
            .expect("조합");
        match &conservative {
            CalibratedProbability::InsufficientSamples(value) => {
                assert_eq!(value.evidence().probability(), ai(70).into_probability());
                assert_eq!(
                    value.evidence().original_probability(),
                    ai(70).into_probability()
                );
            }
            CalibratedProbability::EligibleBeforeE91(_) => panic!("29건은 보수 운용"),
        }
        let eligible = Calibration::try_new(key(), &identity_facts(10))
            .expect("표")
            .calibrate(&key(), ai(70), base(30))
            .expect("조합");
        match &eligible {
            CalibratedProbability::EligibleBeforeE91(value) => {
                assert_eq!(
                    value.evidence().original_probability(),
                    ai(70).into_probability()
                );
                assert_ne!(
                    value.evidence().probability(),
                    value.evidence().original_probability()
                );
            }
            CalibratedProbability::InsufficientSamples(_) => panic!("99건은 보정 대상"),
        }
    }

    #[test]
    fn brier_scores_before_and_after_are_recorded_and_averaged() {
        let eligible = Calibration::try_new(key(), &identity_facts(10))
            .expect("표")
            .calibrate(&key(), ai(90), base(30))
            .expect("조합");
        let hit = brier_comparison(&eligible, CalibrationOutcome::Hit).expect("Brier");
        assert_eq!(hit.before.as_ratio().as_decimal(), Decimal::new(1, 2));
        let calibrated = match &eligible {
            CalibratedProbability::EligibleBeforeE91(value) => value.evidence().probability(),
            CalibratedProbability::InsufficientSamples(_) => panic!("보정 대상"),
        }
        .as_decimal();
        assert_eq!(
            hit.after.as_ratio().as_decimal(),
            (Decimal::ONE - calibrated) * (Decimal::ONE - calibrated)
        );
        let miss = brier_comparison(&eligible, CalibrationOutcome::Miss).expect("Brier");
        assert_eq!(miss.before.as_ratio().as_decimal(), Decimal::new(81, 2));
        let tally = BrierTally::default()
            .with(hit)
            .and_then(|next| next.with(miss))
            .expect("합계");
        assert_eq!(tally.judgment_count().as_usize(), 2);
        let mean = tally.mean().expect("평균");
        assert_eq!(mean.before.as_ratio().as_decimal(), Decimal::new(41, 2));
        assert_eq!(BrierTally::default().mean(), None);
    }

    fn outcome_of(hit: u8) -> CalibrationOutcome {
        if hit == 1 {
            CalibrationOutcome::Hit
        } else {
            CalibrationOutcome::Miss
        }
    }
    #[test]
    fn calibrated_values_stay_between_zero_and_one_for_any_history() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(
                    proptest::collection::vec((1_u8..=99_u8, 0_u8..2_u8), 0..80),
                    1_u8..=99_u8,
                    1_i64..=99_i64,
                ),
                |(rows, percent, prior)| {
                    let facts: Vec<_> = rows
                        .iter()
                        .enumerate()
                        .map(|(index, (predicted, hit))| {
                            fact(&format!("p{index}"), *predicted, outcome_of(*hit))
                        })
                        .collect();
                    let value = shrunk(&facts, percent, prior);
                    proptest::prop_assert!(value >= Decimal::ZERO && value <= Decimal::ONE);
                    Ok(())
                },
            )
            .expect("범위");
    }
}
