use crate::scorecard::{
    Scorecard, ScorecardComparisonCount, ScorecardComparisons, ScorecardError, ScorecardTrial,
    scorecard_calculate,
};
use crate::{
    AiProbability, Calibration, CalibrationCount, CalibrationError, CalibrationEstimateId,
    CalibrationFact, CalibrationKey, EstimateOutcome, HorizonSelection, OutcomeState, Timestamp,
};
use std::{cmp::Ordering, collections::BTreeMap};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardPromptId(String);
impl ScorecardPromptId {
    pub fn try_new(value: &str) -> Result<Self, ScorecardError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(ScorecardError::InvalidPromptId);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardObservation {
    id: CalibrationEstimateId,
    key: CalibrationKey,
    prompt: ScorecardPromptId,
    probability: AiProbability,
    outcome: EstimateOutcome,
}
impl ScorecardObservation {
    pub fn try_new(
        id: CalibrationEstimateId,
        key: CalibrationKey,
        prompt: ScorecardPromptId,
        probability: AiProbability,
        outcome: EstimateOutcome,
    ) -> Result<Self, ScorecardError> {
        if crate::estimate_outcome::scorecard_outcome_horizon(&outcome)
            .is_some_and(|horizon| horizon != key.horizon())
        {
            return Err(ScorecardError::MismatchedCombination);
        }
        Ok(Self {
            id,
            key,
            prompt,
            probability,
            outcome,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardCount(usize);
impl ScorecardCount {
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
    fn difference(self, other: &Self) -> Result<Self, ScorecardError> {
        self.0
            .checked_sub(other.0)
            .map(Self)
            .ok_or(ScorecardError::Calculation)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardCounts {
    total: ScorecardCount,
    ongoing: ScorecardCount,
    unavailable: ScorecardCount,
    before_maturity: ScorecardCount,
    overlapping: ScorecardCount,
    judged: CalibrationCount,
}
impl ScorecardCounts {
    #[must_use]
    pub const fn total(&self) -> &ScorecardCount {
        &self.total
    }
    #[must_use]
    pub const fn ongoing(&self) -> &ScorecardCount {
        &self.ongoing
    }
    #[must_use]
    pub const fn unavailable(&self) -> &ScorecardCount {
        &self.unavailable
    }
    #[must_use]
    pub const fn before_maturity(&self) -> &ScorecardCount {
        &self.before_maturity
    }
    #[must_use]
    pub const fn overlapping(&self) -> &ScorecardCount {
        &self.overlapping
    }
    #[must_use]
    pub const fn judged(&self) -> &CalibrationCount {
        &self.judged
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scorecards {
    cards: Vec<Scorecard>,
    comparisons: ScorecardComparisonCount,
}
impl Scorecards {
    pub fn calculate(
        input: &[ScorecardObservation],
        as_of: Timestamp,
    ) -> Result<Self, ScorecardError> {
        if input.is_empty() {
            return Err(ScorecardError::EmptyInput);
        }
        if input.len() > 65536 {
            return Err(ScorecardError::TooManyObservations);
        }
        let mut groups: BTreeMap<
            CombinationOrder,
            BTreeMap<CalibrationEstimateId, &ScorecardObservation>,
        > = BTreeMap::new();
        for observation in input {
            let group = groups
                .entry(CombinationOrder(observation.key.clone()))
                .or_default();
            if let Some(prior) = group.insert(observation.id.clone(), observation)
                && prior != observation
            {
                return Err(ScorecardError::ConflictingObservation);
            }
        }
        let trials: Vec<_> = input
            .iter()
            .map(|observation| {
                ScorecardTrial::new(observation.key.model().clone(), observation.prompt.clone())
            })
            .collect();
        let comparisons = ScorecardComparisons::from_trials(&trials);
        let cards = groups
            .into_iter()
            .map(|(key, records)| {
                single(
                    &key.0,
                    &records.values().copied().collect::<Vec<_>>(),
                    as_of,
                    &comparisons,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            cards,
            comparisons: comparisons.count().clone(),
        })
    }
    #[must_use]
    pub fn cards(&self) -> &[Scorecard] {
        &self.cards
    }
    #[must_use]
    pub const fn comparison_count(&self) -> &ScorecardComparisonCount {
        &self.comparisons
    }
    #[must_use]
    pub fn select(&self, key: &CalibrationKey) -> Option<&Scorecard> {
        self.cards.iter().find(|card| card.key() == key)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct CombinationOrder(CalibrationKey);
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HorizonOrder {
    Hours,
    FewDays,
    WeeksToMonths,
    HalfYearOrMore,
}
fn horizon_order(selection: HorizonSelection) -> HorizonOrder {
    match selection {
        HorizonSelection::Hours => HorizonOrder::Hours,
        HorizonSelection::FewDays => HorizonOrder::FewDays,
        HorizonSelection::WeeksToMonths => HorizonOrder::WeeksToMonths,
        HorizonSelection::HalfYearOrMore => HorizonOrder::HalfYearOrMore,
    }
}
impl Ord for CombinationOrder {
    fn cmp(&self, other: &Self) -> Ordering {
        let left = self.0.model();
        let right = other.0.model();
        (
            left.provider().as_str(),
            left.name().as_str(),
            left.version().as_str(),
            horizon_order(self.0.horizon().selection()),
        )
            .cmp(&(
                right.provider().as_str(),
                right.name().as_str(),
                right.version().as_str(),
                horizon_order(other.0.horizon().selection()),
            ))
    }
}
impl PartialOrd for CombinationOrder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum Classified {
    BeforeMaturity,
    Ongoing,
    Unavailable,
    Judged(CalibrationFact),
}
fn classify(
    observation: &ScorecardObservation,
    as_of: Timestamp,
) -> Result<Classified, ScorecardError> {
    if let Some(evidence) = observation.outcome.evidence() {
        if evidence.at() > as_of {
            return Err(ScorecardError::FutureOutcome);
        }
        if evidence.interval().end() > as_of {
            return Ok(Classified::BeforeMaturity);
        }
    }
    match observation.outcome.state() {
        OutcomeState::Ongoing => Ok(Classified::Ongoing),
        OutcomeState::Unavailable(_reason) => Ok(Classified::Unavailable),
        OutcomeState::Success | OutcomeState::Failure(_) => observation
            .outcome
            .calibration_fact(
                observation.id.clone(),
                observation.key.model().clone(),
                observation.probability,
            )
            .map(Classified::Judged)
            .ok_or(ScorecardError::MissingFact),
    }
}
fn single(
    key: &CalibrationKey,
    records: &[&ScorecardObservation],
    as_of: Timestamp,
    comparisons: &ScorecardComparisons,
) -> Result<Scorecard, ScorecardError> {
    let classified = records
        .iter()
        .map(|record| classify(record, as_of))
        .collect::<Result<Vec<_>, _>>()?;
    let facts: Vec<_> = classified
        .iter()
        .filter_map(|status| match status {
            Classified::Judged(fact) => Some(fact.clone()),
            Classified::BeforeMaturity | Classified::Ongoing | Classified::Unavailable => None,
        })
        .collect();
    let calibration = Calibration::try_new(key.clone(), &facts).map_err(calibration_error)?;
    let table = calibration.for_combination(key);
    let counts = ScorecardCount(facts.len())
        .difference(&ScorecardCount(table.judgment_count().as_usize()))
        .map(|overlapping| ScorecardCounts {
            total: ScorecardCount(records.len()),
            ongoing: ScorecardCount(
                classified
                    .iter()
                    .filter(|status| **status == Classified::Ongoing)
                    .count(),
            ),
            unavailable: ScorecardCount(
                classified
                    .iter()
                    .filter(|status| **status == Classified::Unavailable)
                    .count(),
            ),
            before_maturity: ScorecardCount(
                classified
                    .iter()
                    .filter(|status| **status == Classified::BeforeMaturity)
                    .count(),
            ),
            overlapping,
            judged: table.judgment_count(),
        });
    scorecard_calculate(&calibration, comparisons, counts)
}
fn calibration_error(_error: CalibrationError) -> ScorecardError {
    ScorecardError::InvalidCalibration
}

#[cfg(test)]
pub(crate) mod tests {
    pub(crate) fn counts_from_table(table: &crate::CalibrationTable) -> super::ScorecardCounts {
        super::ScorecardCounts {
            total: super::ScorecardCount(table.judgment_count().as_usize()),
            ongoing: super::ScorecardCount(0),
            unavailable: super::ScorecardCount(0),
            before_maturity: super::ScorecardCount(0),
            overlapping: super::ScorecardCount(0),
            judged: table.judgment_count(),
        }
    }
    use super::{ScorecardObservation, ScorecardPromptId, Scorecards};
    use crate::candle::tests::{at, price, series};
    use crate::{
        AdjustmentBasis, AiProbability, AiProbabilityInput, CalibrationEstimateId, CalibrationKey,
        CalibrationModel, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        Currency, EstimateContext, EstimateOutcome, EstimateTask, Horizon, HorizonSelection,
        HoursPermission, InstrumentId, Market, MarketCalendar, MarketSession, ModelIdentityPart,
        Nanosecond, NumericInput, OffsetSeconds, Ohlcv, OutcomeAdjustment, OutcomeState,
        OutcomeUnavailable, PriceAdjustment, PriceObservation, Quantity, ResolvedOrderInstrument,
        RiskAppetite, Scorecard, ScorecardError, SessionData, SessionInformation, Symbol,
        TimeInterval, Timestamp, UnixSeconds, Volatility,
    };
    use proptest::test_runner::{Config, TestRunner};
    fn instant(day: i64, minute: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                at(day)
                    .utc_seconds()
                    .as_i64()
                    .checked_add(minute.checked_mul(60).expect("분"))
                    .expect("초"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(-18000),
        )
        .expect("시각")
    }
    fn key(provider: &str, name: &str, version: &str, horizon: HorizonSelection) -> CalibrationKey {
        CalibrationKey::new(
            CalibrationModel::new(
                ModelIdentityPart::try_new(provider).expect("제공자"),
                ModelIdentityPart::try_new(name).expect("이름"),
                ModelIdentityPart::try_new(version).expect("버전"),
            ),
            Horizon::new(horizon),
        )
    }
    fn main_key() -> CalibrationKey {
        key("provider", "model", "v1", HorizonSelection::FewDays)
    }
    fn observation(
        index: usize,
        probability: u8,
        outcome: &EstimateOutcome,
    ) -> ScorecardObservation {
        ScorecardObservation::try_new(
            CalibrationEstimateId::try_new(&index.to_string()).expect("식별"),
            main_key(),
            ScorecardPromptId::try_new("prompt").expect("프롬프트"),
            AiProbability::parse(
                &AiProbabilityInput::try_new(&probability.to_string()).expect("입력"),
            )
            .expect("확률"),
            outcome.clone(),
        )
        .expect("판정 입력")
    }
    fn session(day: i64) -> MarketSession {
        MarketSession::try_new(
            instant(day, 900).offset_calendar_date(),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(instant(day, 900), instant(day, 904)).expect("장"),
                        None,
                    )
                    .expect("구간"),
                ),
                after_market: None,
            },
        )
        .expect("거래일")
    }
    fn outcome_task(index: usize, start: i64) -> EstimateTask {
        let instrument = InstrumentId::try_new(&format!("stock{index}")).expect("종목");
        let prices: Vec<_> = (0_u32..61)
            .map(|position| {
                if position.wrapping_rem(2) == 0 {
                    "100"
                } else {
                    "102"
                }
            })
            .collect();
        let history: Vec<_> = series(&prices)
            .iter()
            .map(|bar| {
                Candle::try_new(
                    instrument.clone(),
                    bar.time(),
                    bar.adjustment(),
                    bar.values(),
                    CandleObservation::new(bar.collected_at(), bar.status()),
                )
                .expect("이력")
            })
            .collect();
        EstimateTask::create(
            EstimateContext::new(
                PriceObservation::from_identified_quote_parts(
                    instrument,
                    ResolvedOrderInstrument::from_master_lookup_result(
                        Symbol::try_new("ABC").expect("심볼"),
                        Market::Us,
                    )
                    .expect("시장"),
                    price("100", Currency::Usd),
                    instant(start, 900),
                )
                .expect("관측"),
                instant(start, 900),
            ),
            Volatility::calculate(&history).expect("변동성"),
            Horizon::new(HorizonSelection::FewDays),
            HoursPermission::default(),
            RiskAppetite::Neutral,
        )
        .expect("과제")
    }
    fn outcome_candle(
        instrument: &InstrumentId,
        adjustment: &OutcomeAdjustment,
        position: (i64, i64, CandlePeriod),
        prices: (&str, &str),
    ) -> Candle {
        Candle::try_new(
            instrument.clone(),
            CandleTime::try_new(instant(position.0, position.1), position.2).expect("봉 시각"),
            PriceAdjustment::Adjusted(adjustment.basis()),
            Ohlcv::try_new(
                price(prices.1, Currency::Usd),
                price(prices.0, Currency::Usd),
                price(prices.1, Currency::Usd),
                price(prices.1, Currency::Usd),
                Quantity::parse(&NumericInput::try_new("0").expect("수량")).expect("수량"),
            )
            .expect("가격"),
            CandleObservation::new(instant(70, 904), CandleStatus::Finalized),
        )
        .expect("봉")
    }
    fn outcomes(index: usize, start: i64) -> [EstimateOutcome; 4] {
        let task = outcome_task(index, start);
        let instrument = task.volatility().instrument().clone();
        let calendars: Vec<_> = (start..=start.checked_add(5).expect("만기"))
            .map(|day| {
                MarketCalendar::try_new(
                    session(day.checked_sub(1).expect("전일")),
                    session(day),
                    session(day.checked_add(1).expect("익일")),
                )
                .expect("달력")
            })
            .collect();
        let adjustment =
            OutcomeAdjustment::from_actions(&task, AdjustmentBasis::new(instant(70, 904)), &[])
                .expect("수정");
        let candle = |day, minute, period, high, low| {
            outcome_candle(&instrument, &adjustment, (day, minute, period), (high, low))
        };
        let mut flat: Vec<_> = (901..=904)
            .map(|minute| candle(start, minute, CandlePeriod::Minute, "100", "100"))
            .collect();
        flat.extend(
            (start.checked_add(1).expect("익일")..=start.checked_add(5).expect("만기"))
                .map(|day| candle(day, 300, CandlePeriod::Daily, "100", "100")),
        );
        let failure =
            EstimateOutcome::evaluate(&task, &calendars, &adjustment, &flat, instant(70, 904));
        let mut success_bars = flat.clone();
        success_bars.retain(|bar| {
            bar.time().date()
                != instant(start.checked_add(1).expect("익일"), 300).offset_calendar_date()
        });
        success_bars.push(candle(
            start.checked_add(1).expect("익일"),
            300,
            CandlePeriod::Daily,
            "200",
            "100",
        ));
        let success = EstimateOutcome::evaluate(
            &task,
            &calendars,
            &adjustment,
            &success_bars,
            instant(70, 904),
        );
        let ongoing =
            EstimateOutcome::evaluate(&task, &calendars, &adjustment, &[], instant(start, 901));
        let unavailable = EstimateOutcome::evaluate(&task, &[], &adjustment, &[], instant(70, 904));
        assert_eq!(success.state(), OutcomeState::Success);
        assert_eq!(
            unavailable.state(),
            OutcomeState::Unavailable(OutcomeUnavailable::Calendar)
        );
        [success, failure, ongoing, unavailable]
    }
    fn ready(card: &Scorecard) -> Option<&crate::ScorecardMetrics> {
        match card {
            Scorecard::Ready(value) => Some(value),
            Scorecard::Pending(_) => None,
        }
    }
    #[test]
    fn all_outcomes_exclusions_duplicates_overlap_and_maturity_are_counted() {
        let source = outcomes(0, 61);
        let late = outcomes(1, 62);
        let input = vec![
            observation(0, 60, &source[0]),
            observation(1, 60, &source[1]),
            observation(2, 60, &source[2]),
            observation(3, 60, &source[3]),
            observation(4, 60, &late[0]),
            observation(0, 60, &source[0]),
        ];
        let result = Scorecards::calculate(&input, instant(66, 904)).expect("전체 성적표");
        assert_eq!(result.comparison_count().as_usize(), 1);
        assert_eq!(result.cards().len(), 1);
        let counts = result.select(&main_key()).expect("조합").counts();
        assert_eq!(counts.total().as_usize(), 5);
        assert_eq!(counts.ongoing().as_usize(), 1);
        assert_eq!(counts.unavailable().as_usize(), 1);
        assert_eq!(counts.before_maturity().as_usize(), 1);
        assert_eq!(counts.overlapping().as_usize(), 1);
        assert_eq!(counts.judged().as_usize(), 1);
        assert!(
            result
                .select(&key("other", "model", "v1", HorizonSelection::FewDays))
                .is_none()
        );
    }
    #[test]
    fn comparison_identity_includes_provider_name_version_prompt_and_all_groups() {
        let source = outcomes(0, 61);
        let mut input = Vec::new();
        for (index, (provider, name, version, prompt, horizon)) in [
            ("provider", "model", "v1", "a", HorizonSelection::FewDays),
            ("other", "model", "v1", "a", HorizonSelection::FewDays),
            ("provider", "other", "v1", "a", HorizonSelection::FewDays),
            ("provider", "model", "v2", "a", HorizonSelection::FewDays),
            ("provider", "model", "v1", "b", HorizonSelection::FewDays),
            (
                "provider",
                "model",
                "v1",
                "a",
                HorizonSelection::WeeksToMonths,
            ),
            ("provider", "model", "v1", "a", HorizonSelection::Hours),
            (
                "provider",
                "model",
                "v1",
                "a",
                HorizonSelection::HalfYearOrMore,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            input.push(
                ScorecardObservation::try_new(
                    CalibrationEstimateId::try_new(&index.to_string()).expect("식별"),
                    key(provider, name, version, horizon),
                    ScorecardPromptId::try_new(prompt).expect("프롬프트"),
                    AiProbability::parse(&AiProbabilityInput::try_new("60").expect("확률"))
                        .expect("확률"),
                    source[3].clone(),
                )
                .expect("판정"),
            );
        }
        input.push(input[0].clone());
        let result = Scorecards::calculate(&input, instant(70, 904)).expect("전체");
        assert_eq!(result.comparison_count().as_usize(), 5);
        assert_eq!(result.cards().len(), 7);
        for card in result.cards() {
            assert_eq!(card.comparison_count().as_usize(), 5);
            assert!(ready(card).is_none());
        }
        input.reverse();
        assert_eq!(
            Scorecards::calculate(&input, instant(70, 904)).expect("역순"),
            result
        );
    }
    #[test]
    fn empty_conflicting_future_and_wrong_horizon_inputs_are_rejected() {
        assert_eq!(
            Scorecards::calculate(&[], instant(70, 904)),
            Err(ScorecardError::EmptyInput)
        );
        let source = outcomes(0, 61);
        let original = observation(0, 60, &source[0]);
        let conflict = observation(0, 60, &source[1]);
        assert_eq!(
            Scorecards::calculate(&[original.clone(), conflict], instant(70, 904)),
            Err(ScorecardError::ConflictingObservation)
        );
        assert_eq!(
            Scorecards::calculate(&[original], instant(61, 900)),
            Err(ScorecardError::FutureOutcome)
        );
        assert_eq!(
            ScorecardObservation::try_new(
                CalibrationEstimateId::try_new("id").expect("식별"),
                key("provider", "model", "v1", HorizonSelection::WeeksToMonths),
                ScorecardPromptId::try_new("prompt").expect("프롬프트"),
                AiProbability::parse(&AiProbabilityInput::try_new("60").expect("확률"))
                    .expect("확률"),
                source[2].clone()
            ),
            Err(ScorecardError::MismatchedCombination)
        );
        for text in ["", "a b", "한글", &"x".repeat(65)] {
            assert_eq!(
                ScorecardPromptId::try_new(text),
                Err(ScorecardError::InvalidPromptId)
            );
        }
        let prompt = ScorecardPromptId::try_new(&"x".repeat(64)).expect("64자");
        assert_eq!(prompt.as_str().len(), 64);
    }
    #[test]
    fn input_bounds_and_internal_validation_errors_are_closed() {
        let source = outcomes(0, 61);
        let record = observation(0, 60, &source[3]);
        assert_eq!(
            Scorecards::calculate(&vec![record; 65537], instant(70, 904)),
            Err(ScorecardError::TooManyObservations)
        );
        let mut invalid = observation(0, 60, &source[0]);
        invalid.key = key("provider", "model", "v1", HorizonSelection::WeeksToMonths);
        assert_eq!(
            Scorecards::calculate(&[invalid], instant(70, 904)),
            Err(ScorecardError::InvalidCalibration)
        );
        assert_eq!(
            super::ScorecardCount(0).difference(&super::ScorecardCount(1)),
            Err(ScorecardError::Calculation)
        );
        let left = super::CombinationOrder(main_key());
        let right = super::CombinationOrder(main_key());
        assert_eq!(left.partial_cmp(&right), Some(std::cmp::Ordering::Equal));
        assert_eq!(
            super::horizon_order(HorizonSelection::Hours),
            super::HorizonOrder::Hours
        );
        assert_eq!(
            super::horizon_order(HorizonSelection::HalfYearOrMore),
            super::HorizonOrder::HalfYearOrMore
        );
    }
    #[test]
    fn early_success_does_not_change_scores_before_maturity_for_4096_cases() {
        let sources: Vec<_> = (0..30).map(|index| outcomes(index, 61)).collect();
        let late = outcomes(31, 62);
        let baseline: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(index, outcomes)| observation(index, 60, &outcomes[1]))
            .collect();
        let expected = Scorecards::calculate(&baseline, instant(66, 904)).expect("만기");
        assert_eq!(
            expected
                .select(&main_key())
                .expect("조합")
                .comparison_count()
                .as_usize(),
            1
        );
        assert_eq!(
            ready(&expected.cards()[0]).expect("30건").brier().as_str(),
            "0.3600"
        );
        TestRunner::new(Config::with_cases(4096))
            .run(&(1_u8..100, 0_u8..2), |(percent, reverse)| {
                let mut input = baseline.clone();
                input.push(observation(31, percent, &late[0]));
                if reverse == 1 {
                    input.reverse();
                }
                let result = Scorecards::calculate(&input, instant(66, 904)).expect("만기 전");
                let metrics = ready(&result.cards()[0]).expect("30건");
                assert_eq!(
                    metrics.brier(),
                    ready(&expected.cards()[0]).expect("기준").brier()
                );
                assert_eq!(
                    metrics.reliability(),
                    ready(&expected.cards()[0]).expect("기준").reliability()
                );
                assert_eq!(metrics.counts().before_maturity().as_usize(), 1);
                let after = Scorecards::calculate(&input, instant(67, 904)).expect("만기 경계");
                assert_eq!(after.cards()[0].counts().judged().as_usize(), 31);
                assert_eq!(after.cards()[0].counts().before_maturity().as_usize(), 0);
                Ok(())
            })
            .expect("조기 성공 편향 방지");
    }
}
