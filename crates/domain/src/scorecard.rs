use crate::scorecard_arithmetic::{brier, reliability};
use crate::{
    Calibration, CalibrationBin, CalibrationCount, CalibrationKey, CalibrationModel,
    CalibrationReadiness, ModelIdentityPart,
};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScorecardError {
    Calculation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardTrial {
    model: CalibrationModel,
    prompt: ModelIdentityPart,
}
impl ScorecardTrial {
    #[must_use]
    pub const fn new(model: CalibrationModel, prompt: ModelIdentityPart) -> Self {
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
pub struct ScorecardComparisons(ScorecardComparisonCount);
impl ScorecardComparisons {
    #[must_use]
    pub fn from_trials(trials: &[ScorecardTrial]) -> Self {
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
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardPending(Metadata);
impl ScorecardPending {
    #[must_use]
    pub const fn key(&self) -> &CalibrationKey {
        &self.0.key
    }
    #[must_use]
    pub const fn judgment_count(&self) -> &CalibrationCount {
        &self.0.count
    }
    #[must_use]
    pub const fn comparison_count(&self) -> &ScorecardComparisonCount {
        &self.0.comparisons
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
    pub fn calculate(
        calibration: &Calibration,
        comparisons: &ScorecardComparisons,
    ) -> Result<Self, ScorecardError> {
        let key = crate::calibration::scorecard_calibration_key(calibration);
        let table = calibration.for_combination(key);
        let metadata = Metadata {
            key: key.clone(),
            count: table.judgment_count(),
            comparisons: comparisons.0.clone(),
        };
        match table.readiness() {
            CalibrationReadiness::Conservative => Ok(Self::Pending(ScorecardPending(metadata))),
            CalibrationReadiness::MinimumReached => {
                assemble(metadata, brier(&table), reliability(&table))
            }
        }
    }
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
    use crate::{
        AiProbability, AiProbabilityInput, Calibration, CalibrationBin, CalibrationEstimate,
        CalibrationEstimateId, CalibrationFact, CalibrationKey, CalibrationModel,
        CalibrationOutcome, Horizon, HorizonSelection, InstrumentId, ModelIdentityPart, Nanosecond,
        OffsetSeconds, Scorecard, ScorecardBin, ScorecardComparisons, ScorecardMetrics,
        ScorecardTrial, TimeInterval, Timestamp, UnixSeconds,
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
    fn calculate(rows: &[CalibrationFact]) -> Scorecard {
        let calibration =
            Calibration::try_new(key("v1", HorizonSelection::FewDays), rows).expect("적중표");
        Scorecard::calculate(&calibration, &ScorecardComparisons::from_trials(&[])).expect("성적표")
    }
    #[test]
    fn pending_exposes_no_metric_and_assembly_propagates_errors() {
        let pending = calculate(&[]);
        assert!(ready(&pending).is_none());
        let metadata = super::Metadata {
            key: key("v1", HorizonSelection::FewDays),
            count: Calibration::try_new(key("v1", HorizonSelection::FewDays), &[])
                .expect("빈 표")
                .for_combination(&key("v1", HorizonSelection::FewDays))
                .judgment_count(),
            comparisons: super::ScorecardComparisonCount(0),
        };
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
                ModelIdentityPart::try_new(prompt).expect("프롬프트 해시"),
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
        let result = Scorecard::calculate(&calibration, &comparisons).expect("성적표");
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
