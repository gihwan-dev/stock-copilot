use crate::scorecard_arithmetic::{brier, reliability};
use crate::scorecard_deferred::{
    ScorecardDeferredBin, ScorecardDeferredBrier, defer_bin, defer_brier,
};
use crate::scorecard_input::{ScorecardCounts, ScorecardPromptId};
use crate::{
    Calibration, CalibrationBin, CalibrationCount, CalibrationKey, CalibrationModel,
    CalibrationReadiness,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScorecardError {
    Calculation,
    EmptyInput,
    TooManyObservations,
    InvalidPromptId,
    MismatchedCombination,
    ConflictingObservation,
    FutureOutcome,
    MissingFact,
    InvalidCalibration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScorecardTrial {
    model: CalibrationModel,
    prompt: ScorecardPromptId,
}
impl ScorecardTrial {
    #[must_use]
    pub(crate) const fn new(model: CalibrationModel, prompt: ScorecardPromptId) -> Self {
        Self { model, prompt }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardComparisonCount(usize);
impl ScorecardComparisonCount {
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScorecardComparisons(ScorecardComparisonCount);
impl ScorecardComparisons {
    pub(crate) const fn count(&self) -> &ScorecardComparisonCount {
        &self.0
    }
    #[must_use]
    pub(crate) fn from_trials(trials: &[ScorecardTrial]) -> Self {
        let unique: std::collections::BTreeSet<_> = trials
            .iter()
            .map(|trial| {
                (
                    trial.model.provider().as_str(),
                    trial.model.name().as_str(),
                    trial.model.version().as_str(),
                    trial.prompt.as_str(),
                )
            })
            .collect();
        Self(ScorecardComparisonCount(unique.len()))
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct ScorecardDisplay(String);
impl ScorecardDisplay {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for ScorecardDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ScorecardDisplay")
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scorecard {
    Pending(ScorecardPending),
    Ready(ScorecardMetrics),
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Metadata {
    key: CalibrationKey,
    count: CalibrationCount,
    comparisons: ScorecardComparisonCount,
    counts: ScorecardCounts,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardPending {
    metadata: Metadata,
    brier: ScorecardDeferredBrier,
    points: Vec<ScorecardDeferredBin>,
}
impl ScorecardPending {
    #[must_use]
    pub const fn counts(&self) -> &ScorecardCounts {
        &self.metadata.counts
    }
    #[must_use]
    pub const fn brier(&self) -> &ScorecardDeferredBrier {
        &self.brier
    }
    #[must_use]
    pub fn reliability(&self) -> &[ScorecardDeferredBin] {
        &self.points
    }
    #[must_use]
    pub const fn key(&self) -> &CalibrationKey {
        &self.metadata.key
    }
    #[must_use]
    pub const fn judgment_count(&self) -> &CalibrationCount {
        &self.metadata.count
    }
    #[must_use]
    pub const fn comparison_count(&self) -> &ScorecardComparisonCount {
        &self.metadata.comparisons
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardMetrics {
    metadata: Metadata,
    brier: ScorecardDisplay,
    reliability: Vec<ScorecardBin>,
}
impl ScorecardMetrics {
    #[must_use]
    pub const fn counts(&self) -> &ScorecardCounts {
        &self.metadata.counts
    }
    #[must_use]
    pub const fn key(&self) -> &CalibrationKey {
        &self.metadata.key
    }
    #[must_use]
    pub const fn judgment_count(&self) -> &CalibrationCount {
        &self.metadata.count
    }
    #[must_use]
    pub const fn comparison_count(&self) -> &ScorecardComparisonCount {
        &self.metadata.comparisons
    }
    #[must_use]
    pub const fn brier(&self) -> &ScorecardDisplay {
        &self.brier
    }
    #[must_use]
    pub fn reliability(&self) -> &[ScorecardBin] {
        &self.reliability
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScorecardBin {
    NoData(CalibrationBin),
    Observed(ScorecardPoint),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardPoint {
    bin: CalibrationBin,
    count: CalibrationCount,
    mean: ScorecardDisplay,
    hit_rate: ScorecardDisplay,
}
impl ScorecardPoint {
    pub(crate) fn new(
        bin: CalibrationBin,
        count: CalibrationCount,
        mean: ScorecardDisplay,
        hit_rate: ScorecardDisplay,
    ) -> Self {
        Self {
            bin,
            count,
            mean,
            hit_rate,
        }
    }
    #[must_use]
    pub const fn bin(&self) -> CalibrationBin {
        self.bin
    }
    #[must_use]
    pub const fn count(&self) -> &CalibrationCount {
        &self.count
    }
    #[must_use]
    pub const fn mean_prediction(&self) -> &ScorecardDisplay {
        &self.mean
    }
    #[must_use]
    pub const fn hit_rate(&self) -> &ScorecardDisplay {
        &self.hit_rate
    }
}
impl Scorecard {
    #[must_use]
    pub fn key(&self) -> &CalibrationKey {
        match self {
            Self::Pending(value) => value.key(),
            Self::Ready(value) => value.key(),
        }
    }
    #[must_use]
    pub fn counts(&self) -> &ScorecardCounts {
        match self {
            Self::Pending(value) => value.counts(),
            Self::Ready(value) => value.counts(),
        }
    }
    #[must_use]
    pub fn comparison_count(&self) -> &ScorecardComparisonCount {
        match self {
            Self::Pending(value) => value.comparison_count(),
            Self::Ready(value) => value.comparison_count(),
        }
    }
}
pub(crate) fn scorecard_calculate(
    calibration: &Calibration,
    comparisons: &ScorecardComparisons,
    counts: Result<ScorecardCounts, ScorecardError>,
) -> Result<Scorecard, ScorecardError> {
    let key = crate::calibration::scorecard_calibration_key(calibration);
    let table = calibration.for_combination(key);
    let metadata = Metadata {
        key: key.clone(),
        count: table.judgment_count(),
        comparisons: comparisons.0.clone(),
        counts: counts?,
    };
    match table.readiness() {
        CalibrationReadiness::Conservative => {
            let score = if table.judgment_count().as_usize() == 0 {
                Ok(None)
            } else {
                brier(&table).map(Some)
            };
            assemble_pending(metadata, score, reliability(&table))
        }
        CalibrationReadiness::MinimumReached => {
            assemble(metadata, brier(&table), reliability(&table))
        }
    }
}
fn assemble_pending(
    metadata: Metadata,
    score: Result<Option<ScorecardDisplay>, ScorecardError>,
    points: Result<Vec<ScorecardBin>, ScorecardError>,
) -> Result<Scorecard, ScorecardError> {
    Ok(Scorecard::Pending(ScorecardPending {
        metadata,
        brier: defer_brier(score?),
        points: points?.into_iter().map(defer_bin).collect(),
    }))
}

fn assemble(
    metadata: Metadata,
    score: Result<ScorecardDisplay, ScorecardError>,
    points: Result<Vec<ScorecardBin>, ScorecardError>,
) -> Result<Scorecard, ScorecardError> {
    Ok(Scorecard::Ready(ScorecardMetrics {
        metadata,
        brier: score?,
        reliability: points?,
    }))
}

#[cfg(test)]
mod tests {
    use super::{ScorecardComparisons, ScorecardTrial};
    use crate::{
        AiProbability, AiProbabilityInput, Calibration, CalibrationBin, CalibrationEstimate,
        CalibrationEstimateId, CalibrationFact, CalibrationKey, CalibrationModel,
        CalibrationOutcome, Horizon, HorizonSelection, InstrumentId, ModelIdentityPart, Nanosecond,
        OffsetSeconds, Scorecard, ScorecardBin, ScorecardMetrics, TimeInterval, Timestamp,
        UnixSeconds,
    };
    use proptest::test_runner::{Config, TestRunner};

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
    fn row(index: usize, percent: u8, outcome: CalibrationOutcome) -> CalibrationFact {
        let at = |seconds| {
            Timestamp::from_parts(
                UnixSeconds::new(seconds),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각")
        };
        CalibrationFact::new(
            CalibrationEstimate::new(
                CalibrationEstimateId::try_new(&index.to_string()).expect("식별"),
                InstrumentId::try_new(&format!("stock{index}")).expect("종목"),
                key("v1", HorizonSelection::FewDays),
                TimeInterval::try_new(at(0), at(1)).expect("기간"),
                AiProbability::parse(
                    &AiProbabilityInput::try_new(&percent.to_string()).expect("입력"),
                )
                .expect("확률"),
            ),
            outcome,
        )
    }
    fn calculate_table(
        calibration: &Calibration,
        comparisons: &ScorecardComparisons,
    ) -> Result<Scorecard, super::ScorecardError> {
        let table =
            calibration.for_combination(crate::calibration::scorecard_calibration_key(calibration));
        super::scorecard_calculate(
            calibration,
            comparisons,
            Ok(crate::scorecard_input::tests::counts_from_table(&table)),
        )
    }
    fn calculate(rows: &[CalibrationFact]) -> Scorecard {
        let calibration =
            Calibration::try_new(key("v1", HorizonSelection::FewDays), rows).expect("적중표");
        calculate_table(&calibration, &ScorecardComparisons::from_trials(&[])).expect("성적표")
    }
    #[test]
    fn pending_and_ready_assembly_propagate_errors() {
        let calibration =
            Calibration::try_new(key("v1", HorizonSelection::FewDays), &[]).expect("표");
        assert_eq!(
            super::scorecard_calculate(
                &calibration,
                &ScorecardComparisons::from_trials(&[]),
                Err(super::ScorecardError::Calculation)
            ),
            Err(super::ScorecardError::Calculation)
        );
        let pending = calculate(&[]);
        assert!(ready(&pending).is_none());
        let metadata = super::Metadata {
            key: key("v1", HorizonSelection::FewDays),
            count: Calibration::try_new(key("v1", HorizonSelection::FewDays), &[])
                .expect("빈 표")
                .for_combination(&key("v1", HorizonSelection::FewDays))
                .judgment_count(),
            comparisons: super::ScorecardComparisonCount(0),
            counts: crate::scorecard_input::tests::counts_from_table(
                &Calibration::try_new(key("v1", HorizonSelection::FewDays), &[])
                    .expect("표")
                    .for_combination(&key("v1", HorizonSelection::FewDays)),
            ),
        };
        assert_eq!(
            super::assemble_pending(
                metadata.clone(),
                Err(super::ScorecardError::Calculation),
                Ok(Vec::new())
            ),
            Err(super::ScorecardError::Calculation)
        );
        assert_eq!(
            super::assemble_pending(
                metadata.clone(),
                Ok(None),
                Err(super::ScorecardError::Calculation)
            ),
            Err(super::ScorecardError::Calculation)
        );
        assert_eq!(
            super::assemble(
                metadata.clone(),
                Err(super::ScorecardError::Calculation),
                Ok(Vec::new())
            ),
            Err(super::ScorecardError::Calculation)
        );
        assert_eq!(
            super::assemble(
                metadata,
                Ok(super::ScorecardDisplay::new("0.0000".into())),
                Err(super::ScorecardError::Calculation)
            ),
            Err(super::ScorecardError::Calculation)
        );
    }
    fn ready(result: &Scorecard) -> Option<&ScorecardMetrics> {
        match result {
            Scorecard::Ready(value) => Some(value),
            Scorecard::Pending(_) => None,
        }
    }
    #[test]
    fn sample_boundary_is_closed_and_model_horizon_are_separate() {
        for length in [0, 1, 29, 30, 31] {
            let rows: Vec<_> = (0..length)
                .map(|index| row(index, 60, CalibrationOutcome::Hit))
                .collect();
            let result = calculate(&rows);
            match result {
                Scorecard::Pending(value) => {
                    assert!(length < 30);
                    assert_eq!(value.judgment_count().as_usize(), length);
                    assert_eq!(value.comparison_count().as_usize(), 0);
                    assert_eq!(value.key(), &key("v1", HorizonSelection::FewDays));
                }
                Scorecard::Ready(value) => {
                    assert!(length >= 30);
                    assert_eq!(value.judgment_count().as_usize(), length);
                    assert_eq!(value.comparison_count().as_usize(), 0);
                    assert_eq!(value.key(), &key("v1", HorizonSelection::FewDays));
                }
            }
            for other in [
                key("v2", HorizonSelection::FewDays),
                key("v1", HorizonSelection::WeeksToMonths),
            ] {
                assert_eq!(Calibration::try_new(other, &rows).is_ok(), length == 0);
            }
        }
    }
    #[test]
    fn exact_examples_bins_empty_and_debug_are_redacted() {
        let rows: Vec<_> = (0..30)
            .map(|index| {
                row(
                    index,
                    60,
                    if index < 18 {
                        CalibrationOutcome::Hit
                    } else {
                        CalibrationOutcome::Miss
                    },
                )
            })
            .collect();
        let result = calculate(&rows);
        let metrics = ready(&result).expect("표시 가능");
        assert_eq!(comparison_count(&result).as_usize(), 0);
        assert_eq!(metrics.brier().as_str(), "0.2400");
        assert_eq!(
            metrics
                .reliability()
                .iter()
                .map(|point| match point {
                    ScorecardBin::NoData(bin) => *bin,
                    ScorecardBin::Observed(value) => value.bin(),
                })
                .collect::<Vec<_>>(),
            vec![
                CalibrationBin::Below50,
                CalibrationBin::From50To60,
                CalibrationBin::From60To70,
                CalibrationBin::AtLeast70
            ]
        );
        for point in metrics.reliability() {
            match point {
                ScorecardBin::NoData(bin) => assert_ne!(*bin, CalibrationBin::From60To70),
                ScorecardBin::Observed(value) => {
                    assert_eq!(value.bin(), CalibrationBin::From60To70);
                    assert_eq!(value.count().as_usize(), 30);
                    assert_eq!(value.mean_prediction().as_str(), "0.6000");
                    assert_eq!(value.hit_rate().as_str(), "0.6000");
                }
            }
        }
        assert!(!format!("{result:?}").contains("0.2400"));
        assert_eq!(format!("{:?}", metrics.brier()), "ScorecardDisplay");
    }
    #[test]
    fn constant_prediction_and_allowed_extremes_match_definitions() {
        let baseline: Vec<_> = (0..30)
            .map(|index| row(index, 50, CalibrationOutcome::Miss))
            .collect();
        assert_eq!(
            ready(&calculate(&baseline))
                .expect("표시 가능")
                .brier()
                .as_str(),
            "0.2500"
        );
        for (percent, outcome, expected) in [
            (99, CalibrationOutcome::Hit, "0.0001"),
            (1, CalibrationOutcome::Miss, "0.0001"),
            (1, CalibrationOutcome::Hit, "0.9801"),
            (99, CalibrationOutcome::Miss, "0.9801"),
        ] {
            let boundary: Vec<_> = (0..30).map(|index| row(index, percent, outcome)).collect();
            assert_eq!(
                ready(&calculate(&boundary))
                    .expect("표시 가능")
                    .brier()
                    .as_str(),
                expected
            );
        }
    }
    #[test]
    fn comparison_count_is_unique_model_prompt_pairs_including_pending() {
        let trial = |version, prompt| {
            ScorecardTrial::new(
                key(version, HorizonSelection::FewDays).model().clone(),
                crate::ScorecardPromptId::try_new(prompt).expect("프롬프트 해시"),
            )
        };
        let trials = [
            trial("v1", "a"),
            trial("v1", "a"),
            trial("v1", "b"),
            trial("v2", "a"),
        ];
        let comparisons = ScorecardComparisons::from_trials(&trials);
        let calibration =
            Calibration::try_new(key("v1", HorizonSelection::FewDays), &[]).expect("빈 표");
        let result = calculate_table(&calibration, &comparisons).expect("성적표");
        assert!(ready(&result).is_none());
        assert_eq!(comparison_count(&result).as_usize(), 3);
    }
    #[test]
    fn overlapping_and_repeated_facts_use_exactly_the_calibration_selection() {
        let mut facts = Vec::new();
        for index in 0..30 {
            let original = row(index, 60, CalibrationOutcome::Hit);
            let overlapping = CalibrationFact::new(
                CalibrationEstimate::new(
                    CalibrationEstimateId::try_new(&format!("overlap{index}")).expect("식별"),
                    original.estimate().instrument().clone(),
                    original.estimate().key().clone(),
                    TimeInterval::try_new(
                        Timestamp::from_parts(
                            UnixSeconds::new(0),
                            Nanosecond::new(0),
                            OffsetSeconds::new(0),
                        )
                        .expect("시각"),
                        Timestamp::from_parts(
                            UnixSeconds::new(1),
                            Nanosecond::new(0),
                            OffsetSeconds::new(0),
                        )
                        .expect("시각"),
                    )
                    .expect("구간"),
                    original.estimate().predicted_probability(),
                ),
                CalibrationOutcome::Miss,
            );
            facts.push(original.clone());
            facts.push(original);
            facts.push(overlapping);
        }
        let result = calculate(&facts);
        assert_eq!(
            ready(&result)
                .expect("표시 가능")
                .judgment_count()
                .as_usize(),
            30
        );
        assert_eq!(
            ready(&result).expect("표시 가능").brier().as_str(),
            "0.3600"
        );
    }

    #[test]
    fn thirty_boundary_and_best_worst_predictions_for_4096_cases() {
        TestRunner::new(Config::with_cases(4096))
            .run(&(0_usize..40, 0_u8..2), |(length, binary)| {
                let rows: Vec<_> = (0..length)
                    .map(|index| row(index, if binary == 1 { 99 } else { 1 }, outcome(binary)))
                    .collect();
                match calculate(&rows) {
                    Scorecard::Pending(value) => {
                        assert!(length < 30);
                        assert_eq!(value.judgment_count().as_usize(), length);
                    }
                    Scorecard::Ready(value) => {
                        assert!(length >= 30);
                        assert_eq!(value.brier().as_str(), "0.0001");
                    }
                }
                let opposite: Vec<_> = (0..30)
                    .map(|index| row(index, if binary == 1 { 1 } else { 99 }, outcome(binary)))
                    .collect();
                assert_eq!(
                    ready(&calculate(&opposite))
                        .expect("표시 가능")
                        .brier()
                        .as_str(),
                    "0.9801"
                );
                Ok(())
            })
            .expect("판정 경계와 허용 예측 끝점");
    }

    #[test]
    fn pending_numbers_require_judgment_pending_branch_for_29_and_empty() {
        let rows: Vec<_> = (0..29)
            .map(|index| row(index, 60, CalibrationOutcome::Hit))
            .collect();
        let complete: Vec<_> = (0..30)
            .map(|index| row(index, 60, CalibrationOutcome::Hit))
            .collect();
        assert!(deferred(&calculate(&complete)).is_none());
        for (result, expected) in [(calculate(&[]), None), (calculate(&rows), Some("0.1600"))] {
            let pending = deferred(&result).expect("보류");
            let actual = match pending.brier() {
                crate::ScorecardDeferredBrier::NoData => None,
                crate::ScorecardDeferredBrier::JudgmentPending(value) => {
                    assert_eq!(format!("{value:?}"), "ScorecardDeferredDisplay");
                    Some(value.as_str())
                }
            };
            assert_eq!(actual, expected);
            check_deferred_bins(pending.reliability(), expected);
        }
    }
    fn deferred(result: &Scorecard) -> Option<&crate::ScorecardPending> {
        match result {
            Scorecard::Pending(value) => Some(value),
            Scorecard::Ready(_) => None,
        }
    }
    fn check_deferred_bins(bins: &[crate::ScorecardDeferredBin], expected: Option<&str>) {
        for bin in bins {
            match bin {
                crate::ScorecardDeferredBin::NoData(kind) => {
                    assert!(expected.is_none() || *kind != CalibrationBin::From60To70);
                }
                crate::ScorecardDeferredBin::Observed(point) => {
                    assert_eq!(point.bin(), CalibrationBin::From60To70);
                    let crate::ScorecardDeferredCount::JudgmentPending(count) = point.count();
                    assert_eq!(count.as_usize(), 29);
                    let crate::ScorecardDeferredNumber::JudgmentPending(mean) =
                        point.mean_prediction();
                    assert_eq!(mean.as_str(), "0.6000");
                    let crate::ScorecardDeferredNumber::JudgmentPending(hit) = point.hit_rate();
                    assert_eq!(hit.as_str(), "1.0000");
                }
            }
        }
    }
    fn comparison_count(result: &Scorecard) -> &crate::ScorecardComparisonCount {
        match result {
            Scorecard::Pending(value) => value.comparison_count(),
            Scorecard::Ready(value) => value.comparison_count(),
        }
    }
    fn outcome(binary: u8) -> CalibrationOutcome {
        if binary == 1 {
            CalibrationOutcome::Hit
        } else {
            CalibrationOutcome::Miss
        }
    }
    fn check_points(metrics: &ScorecardMetrics, input: &[(u8, u8)]) {
        for point in metrics.reliability() {
            if let ScorecardBin::Observed(value) = point {
                let selected: Vec<_> = input
                    .iter()
                    .filter(|(percent, _)| match value.bin() {
                        CalibrationBin::Below50 => *percent < 50,
                        CalibrationBin::From50To60 => (50..60).contains(percent),
                        CalibrationBin::From60To70 => (60..70).contains(percent),
                        CalibrationBin::AtLeast70 => *percent >= 70,
                    })
                    .collect();
                let count = u64::try_from(selected.len()).expect("건수");
                let probability_sum: u64 = selected
                    .iter()
                    .map(|(percent, _)| u64::from(*percent))
                    .sum();
                let hits: u64 = selected.iter().map(|(_, binary)| u64::from(*binary)).sum();
                let expected_p = probability_sum
                    .checked_mul(200)
                    .expect("상한")
                    .checked_add(count)
                    .expect("상한")
                    .checked_div(count.checked_mul(2).expect("상한"))
                    .expect("표본");
                let expected_y = hits
                    .checked_mul(20000)
                    .expect("상한")
                    .checked_add(count)
                    .expect("상한")
                    .checked_div(count.checked_mul(2).expect("상한"))
                    .expect("표본");
                assert_eq!(value.count().as_usize(), selected.len());
                assert_eq!(
                    value
                        .mean_prediction()
                        .as_str()
                        .replace('.', "")
                        .parse::<u64>()
                        .expect("평균"),
                    expected_p
                );
                assert_eq!(
                    value
                        .hit_rate()
                        .as_str()
                        .replace('.', "")
                        .parse::<u64>()
                        .expect("적중"),
                    expected_y
                );
            }
        }
    }
    #[test]
    fn rational_reference_order_invariance_range_and_all_bins_for_4096_cases() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &proptest::collection::vec((1_u8..100, 0_u8..2), 30..70),
                |input| {
                    let rows: Vec<_> = input
                        .iter()
                        .enumerate()
                        .map(|(index, (percent, binary))| row(index, *percent, outcome(*binary)))
                        .collect();
                    let sum: i64 = input
                        .iter()
                        .map(|(percent, binary)| {
                            (i64::from(*percent) - 100 * i64::from(*binary)).pow(2)
                        })
                        .sum();
                    let length = i64::try_from(input.len()).expect("건수");
                    let expected = (sum * 2 + length).checked_div(2 * length).expect("표본");
                    let result = calculate(&rows);
                    let metrics = ready(&result).expect("표시 가능");
                    assert_eq!(comparison_count(&result).as_usize(), 0);
                    let actual: u64 = metrics
                        .brier()
                        .as_str()
                        .replace('.', "")
                        .parse()
                        .expect("표시");
                    assert_eq!(actual, u64::try_from(expected).expect("양수"));
                    assert!(actual <= 10000);
                    check_points(metrics, &input);
                    let mut reversed = rows;
                    reversed.reverse();
                    assert_eq!(calculate(&reversed), result);
                    Ok(())
                },
            )
            .expect("독립 유리수 기준");
    }
}
