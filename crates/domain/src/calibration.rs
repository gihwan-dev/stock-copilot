use crate::{
    Amount, Horizon, InstrumentId, MoneyError, Probability, RatioQuotient, TimeInterval, Timestamp,
};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationError {
    InvalidModelIdentity,
    TooManyFacts,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibrationFact {
    instrument: InstrumentId,
    key: CalibrationKey,
    interval: TimeInterval,
    predicted_probability: Probability,
    outcome: CalibrationOutcome,
}
impl CalibrationFact {
    #[must_use]
    pub fn new(
        instrument: InstrumentId,
        key: CalibrationKey,
        interval: TimeInterval,
        predicted_probability: Probability,
        outcome: CalibrationOutcome,
    ) -> Self {
        Self {
            instrument,
            key,
            interval,
            predicted_probability,
            outcome,
        }
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
    pub const fn estimated_at(&self) -> Timestamp {
        self.interval.start()
    }
    #[must_use]
    pub const fn interval(&self) -> TimeInterval {
        self.interval
    }
    #[must_use]
    pub const fn predicted_probability(&self) -> Probability {
        self.predicted_probability
    }
    #[must_use]
    pub const fn outcome(&self) -> CalibrationOutcome {
        self.outcome
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
pub enum CalibrationStatus {
    NotCalibratedInsufficientSamples,
    IdentityBeforeE91,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalibratedProbability {
    probability: Probability,
    key: CalibrationKey,
    judgment_count: CalibrationCount,
    status: CalibrationStatus,
}
impl CalibratedProbability {
    #[must_use]
    pub const fn probability(&self) -> Probability {
        self.probability
    }
    #[must_use]
    pub const fn key(&self) -> &CalibrationKey {
        &self.key
    }
    #[must_use]
    pub const fn judgment_count(&self) -> &CalibrationCount {
        &self.judgment_count
    }
    #[must_use]
    pub const fn status(&self) -> CalibrationStatus {
        self.status
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
            .filter(|fact| CalibrationBin::for_probability(fact.predicted_probability) == bin)
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
    facts: Vec<CalibrationFact>,
}
impl Calibration {
    pub fn try_new(facts: &[CalibrationFact]) -> Result<Self, CalibrationError> {
        if facts.len() > 4096 {
            return Err(CalibrationError::TooManyFacts);
        }
        Ok(Self {
            facts: facts.to_vec(),
        })
    }
    #[must_use]
    pub fn for_combination(&self, key: &CalibrationKey) -> CalibrationTable {
        let mut facts: Vec<_> = self
            .facts
            .iter()
            .filter(|fact| &fact.key == key)
            .cloned()
            .collect();
        facts.sort_by(|left, right| {
            left.instrument
                .cmp(&right.instrument)
                .then(left.interval.start().cmp(&right.interval.start()))
                .then_with(|| right.interval.end().cmp(&left.interval.end()))
                .then(left.outcome.cmp(&right.outcome))
                .then(left.predicted_probability.cmp(&right.predicted_probability))
        });
        let mut selected = Vec::new();
        let mut component: Option<(InstrumentId, Timestamp)> = None;
        for fact in facts {
            if let Some((instrument, end)) = &mut component
                && instrument == &fact.instrument
                && fact.interval.start() <= *end
            {
                *end = (*end).max(fact.interval.end());
                continue;
            }
            component = Some((fact.instrument.clone(), fact.interval.end()));
            selected.push(fact);
        }
        CalibrationTable { selected }
    }
    #[must_use]
    pub fn calibrate(
        &self,
        key: &CalibrationKey,
        probability: Probability,
    ) -> CalibratedProbability {
        let table = self.for_combination(key);
        let status = match table.readiness() {
            CalibrationReadiness::Conservative => {
                CalibrationStatus::NotCalibratedInsufficientSamples
            }
            CalibrationReadiness::MinimumReached => CalibrationStatus::IdentityBeforeE91,
        };
        CalibratedProbability {
            probability,
            key: key.clone(),
            judgment_count: table.judgment_count(),
            status,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{
        Calibration, CalibrationBin, CalibrationError, CalibrationFact, CalibrationKey,
        CalibrationModel, CalibrationOutcome, CalibrationReadiness, CalibrationStatus,
        CalibrationTable, ModelIdentityPart,
    };
    use crate::{Horizon, InstrumentId, Probability, RatioQuotient, TimeInterval, Timestamp};
    use crate::{HorizonSelection, Nanosecond, OffsetSeconds, UnixSeconds};
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시점")
    }
    fn key(version: &str, horizon: HorizonSelection) -> CalibrationKey {
        CalibrationKey::new(
            CalibrationModel::new(
                ModelIdentityPart::try_new("provider").expect("제공자"),
                ModelIdentityPart::try_new("model").expect("모델"),
                ModelIdentityPart::try_new(version).expect("버전"),
            ),
            Horizon::new(horizon),
        )
    }
    fn fact(
        symbol: &str,
        start: i64,
        end: i64,
        percent: i64,
        outcome: CalibrationOutcome,
    ) -> CalibrationFact {
        CalibrationFact::new(
            InstrumentId::try_new(symbol).expect("종목"),
            key("v1", HorizonSelection::FewDays),
            TimeInterval::try_new(at(start), at(end)).expect("기간"),
            Probability::try_new(Decimal::new(percent, 2)).expect("확률"),
            outcome,
        )
    }
    fn table(facts: &[CalibrationFact]) -> CalibrationTable {
        Calibration::try_new(facts)
            .expect("입력")
            .for_combination(&key("v1", HorizonSelection::FewDays))
    }
    #[test]
    fn threshold_29_30_and_missing_combination_preserve_identity() {
        for size in [0_usize, 29, 30] {
            let facts: Vec<_> = (0..size)
                .map(|index| fact(&format!("s{index}"), 0, 1, 60, CalibrationOutcome::Hit))
                .collect();
            let calibration = Calibration::try_new(&facts).expect("입력");
            let current_key = key("v1", HorizonSelection::FewDays);
            let result = calibration.calibrate(
                &current_key,
                Probability::try_new(Decimal::new(2475, 4)).expect("원래 확률"),
            );
            assert_eq!(result.probability().as_decimal(), Decimal::new(2475, 4));
            assert_eq!(result.key(), &current_key);
            assert_eq!(result.judgment_count().as_usize(), size);
            assert_eq!(
                result.status(),
                if size < 30 {
                    CalibrationStatus::NotCalibratedInsufficientSamples
                } else {
                    CalibrationStatus::IdentityBeforeE91
                }
            );
            assert_eq!(
                calibration.for_combination(&current_key).readiness(),
                if size < 30 {
                    CalibrationReadiness::Conservative
                } else {
                    CalibrationReadiness::MinimumReached
                }
            );
            let missing = calibration.for_combination(&key("v2", HorizonSelection::FewDays));
            assert_eq!(missing.judgment_count().as_usize(), 0);
        }
    }
    #[test]
    fn closed_overlap_components_count_once_and_choose_earliest_longest_failure() {
        let facts = [
            fact("A", 0, 2, 55, CalibrationOutcome::Hit),
            fact("A", 2, 4, 65, CalibrationOutcome::Hit),
            fact("A", 4, 6, 75, CalibrationOutcome::Hit),
            fact("A", 0, 3, 80, CalibrationOutcome::Hit),
            fact("A", 0, 3, 90, CalibrationOutcome::Miss),
            fact("A", 0, 3, 70, CalibrationOutcome::Miss),
            fact("A", 7, 8, 60, CalibrationOutcome::Hit),
            fact("B", 0, 8, 60, CalibrationOutcome::Hit),
        ];
        let result = table(&facts);
        let mut reversed = facts.clone();
        reversed.reverse();
        assert_eq!(table(&reversed), result);
        assert_eq!(result.judgment_count().as_usize(), 3);
        assert_eq!(result.selected_facts().first(), facts.get(5));
        assert_eq!(result.bin(CalibrationBin::AtLeast70).hits().as_usize(), 0);
        assert_eq!(result.bin(CalibrationBin::From60To70).hits().as_usize(), 2);
        assert_eq!(
            result
                .selected_facts()
                .first()
                .expect("대표")
                .estimated_at(),
            at(0)
        );
    }
    #[test]
    fn bins_cover_endpoints_and_rates_keep_approximation_and_empty_state() {
        let cases = [
            (0, CalibrationBin::Below50),
            (49, CalibrationBin::Below50),
            (50, CalibrationBin::From50To60),
            (59, CalibrationBin::From50To60),
            (60, CalibrationBin::From60To70),
            (69, CalibrationBin::From60To70),
            (70, CalibrationBin::AtLeast70),
            (100, CalibrationBin::AtLeast70),
        ];
        for (percent, bin) in cases {
            let result = table(&[fact("A", 0, 1, percent, CalibrationOutcome::Hit)]);
            let stats = result.bin(bin);
            assert_eq!(stats.judgment_count().as_usize(), 1);
            assert_eq!(
                stats.hit_rate().expect("割算"),
                Some(RatioQuotient::Exact(crate::Ratio::new(Decimal::ONE)))
            );
        }
        let result = table(&[
            fact("A", 0, 1, 60, CalibrationOutcome::Hit),
            fact("B", 0, 1, 60, CalibrationOutcome::Miss),
            fact("C", 0, 1, 60, CalibrationOutcome::Miss),
        ]);
        let stats = result.bin(CalibrationBin::From60To70);
        assert_eq!(stats.hits().as_usize(), 1);
        assert_eq!(stats.judgment_count().as_usize(), 3);
        assert_eq!(
            format!("{:?}", stats.hit_rate()),
            "Ok(Some(Approximate(ApproximateRatio(0.3333333333333333333333333333))))"
        );
        assert_eq!(result.bin(CalibrationBin::Below50).hit_rate(), Ok(None));
    }
    #[test]
    fn model_provider_name_version_and_horizon_never_pool_samples() {
        let base = key("v1", HorizonSelection::FewDays);
        assert_eq!(base.model().provider().as_str(), "provider");
        assert_eq!(base.model().name().as_str(), "model");
        assert_eq!(base.model().version().as_str(), "v1");
        assert_eq!(base.horizon(), Horizon::new(HorizonSelection::FewDays));
        let mut facts = Vec::new();
        for current in [
            base.clone(),
            key("v2", HorizonSelection::FewDays),
            key("v1", HorizonSelection::Hours),
            CalibrationKey::new(
                CalibrationModel::new(
                    ModelIdentityPart::try_new("other").expect("제공자"),
                    base.model().name().clone(),
                    base.model().version().clone(),
                ),
                base.horizon(),
            ),
            CalibrationKey::new(
                CalibrationModel::new(
                    base.model().provider().clone(),
                    ModelIdentityPart::try_new("other").expect("모델"),
                    base.model().version().clone(),
                ),
                base.horizon(),
            ),
        ] {
            let mut row = fact("A", 0, 1, 60, CalibrationOutcome::Hit);
            row.key = current;
            facts.push(row);
        }
        let calibration = Calibration::try_new(&facts).expect("입력");
        for row in &facts {
            assert_eq!(
                calibration
                    .for_combination(row.key())
                    .judgment_count()
                    .as_usize(),
                1
            );
        }
    }
    #[test]
    fn input_limits_identifiers_and_estimate_interval_are_checked() {
        for invalid in ["", "left right", "한글", "left\n", &"a".repeat(65)] {
            assert_eq!(
                ModelIdentityPart::try_new(invalid),
                Err(CalibrationError::InvalidModelIdentity)
            );
        }
        assert_eq!(
            ModelIdentityPart::try_new(&"a".repeat(64))
                .expect("상한")
                .as_str()
                .len(),
            64
        );
        assert_ne!(
            ModelIdentityPart::try_new("A"),
            ModelIdentityPart::try_new("left")
        );
        let mut row = fact("A", 0, 1, 60, CalibrationOutcome::Hit);
        assert_eq!(
            row.interval(),
            TimeInterval::try_new(at(0), at(1)).expect("구간")
        );
        assert_eq!(
            row.predicted_probability().as_decimal(),
            Decimal::new(60, 2)
        );
        assert_eq!(row.outcome(), CalibrationOutcome::Hit);
        let maximum: Vec<_> = (0_usize..4096)
            .map(|index| fact(&format!("s{index}"), 0, 1, 60, CalibrationOutcome::Hit))
            .collect();
        assert_eq!(table(&maximum).judgment_count().as_usize(), 4096);
        let rows = vec![row.clone(); 4096];
        assert_eq!(table(&rows).judgment_count().as_usize(), 1);
        assert_eq!(
            Calibration::try_new(&vec![row.clone(); 4097]),
            Err(CalibrationError::TooManyFacts)
        );
        row.instrument = InstrumentId::try_new("B").expect("종목");
        assert_eq!(row.instrument().as_str(), "B");
    }
    fn graph_touches(raw: &[(i64, i64, u8)], first: usize, second: usize) -> bool {
        let left = raw.get(first).expect("정점");
        let right = raw.get(second).expect("정점");
        left.2 == right.2
            && left.0 <= right.0.saturating_add(right.1)
            && right.0 <= left.0.saturating_add(left.1)
    }
    fn merge_graph_component(
        raw: &[(i64, i64, u8)],
        index: usize,
        components: Vec<Vec<usize>>,
    ) -> Vec<Vec<usize>> {
        let mut connected = vec![index];
        let mut remaining = Vec::new();
        for group in components {
            if group.iter().any(|other| graph_touches(raw, index, *other)) {
                connected.extend(group);
            } else {
                remaining.push(group);
            }
        }
        remaining.push(connected);
        remaining
    }
    fn graph_reference(raw: &[(i64, i64, u8)], facts: &[CalibrationFact]) -> Vec<CalibrationFact> {
        let mut components: Vec<Vec<usize>> = Vec::new();
        for index in 0..raw.len() {
            components = merge_graph_component(raw, index, components);
        }
        let mut expected: Vec<_> = components
            .iter()
            .map(|group| {
                group
                    .iter()
                    .filter_map(|index| facts.get(*index))
                    .min_by(|left, right| {
                        left.estimated_at()
                            .cmp(&right.estimated_at())
                            .then_with(|| right.interval.end().cmp(&left.interval.end()))
                    })
                    .expect("대표")
                    .clone()
            })
            .collect();
        expected.sort_by(|left, right| {
            left.instrument
                .cmp(&right.instrument)
                .then(left.interval.start().cmp(&right.interval.start()))
        });

        expected
    }
    #[test]
    fn interval_components_match_independent_graph_reference_under_permutations() {
        let strategy = proptest::collection::vec((0_i64..20, 1_i64..8, 0_u8..3), 0..35);
        TestRunner::new(Config::with_cases(4096))
            .run(&strategy, |raw| {
                let facts: Vec<_> = raw
                    .iter()
                    .map(|(start, length, symbol)| {
                        fact(
                            &symbol.to_string(),
                            *start,
                            start.saturating_add(*length),
                            60,
                            CalibrationOutcome::Hit,
                        )
                    })
                    .collect();
                let expected = graph_reference(&raw, &facts);
                let actual = table(&facts);
                proptest::prop_assert_eq!(actual.selected_facts(), expected.as_slice());
                let mut reversed = facts;
                reversed.reverse();
                proptest::prop_assert_eq!(table(&reversed), actual);
                Ok(())
            })
            .expect("독립 그래프와 순서 불변성");
    }
}
