use crate::{CalibrationEstimateId, CostEligibleTask};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedEstimateTaskId(CalibrationEstimateId);
impl IssuedEstimateTaskId {
    #[must_use]
    pub const fn as_estimate_id(&self) -> &CalibrationEstimateId {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateConsumption {
    Available,
    Consumed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateAssignmentError {
    AlreadyIssued,
    UnknownTask,
    WrongRun,
    AlreadyConsumed,
    AssignmentExceeded,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedEstimateTask<Run> {
    id: IssuedEstimateTaskId,
    run: Run,
    task: CostEligibleTask,
    consumption: EstimateConsumption,
}
impl<Run> IssuedEstimateTask<Run> {
    #[must_use]
    pub const fn id(&self) -> &IssuedEstimateTaskId {
        &self.id
    }
    #[must_use]
    pub const fn run(&self) -> &Run {
        &self.run
    }
    #[must_use]
    pub const fn task(&self) -> &CostEligibleTask {
        &self.task
    }
    #[must_use]
    pub const fn consumption(&self) -> EstimateConsumption {
        self.consumption
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateAssignments<Run>(Vec<IssuedEstimateTask<Run>>);
impl<Run: Clone + Eq> Default for EstimateAssignments<Run> {
    fn default() -> Self {
        Self(Vec::new())
    }
}
impl<Run: Clone + Eq> EstimateAssignments<Run> {
    pub fn issue(
        &self,
        id: CalibrationEstimateId,
        run: Run,
        task: CostEligibleTask,
    ) -> Result<Self, EstimateAssignmentError> {
        if self.0.iter().any(|entry| entry.id.0 == id) {
            return Err(EstimateAssignmentError::AlreadyIssued);
        }
        let mut next = self.clone();
        next.0.push(IssuedEstimateTask {
            id: IssuedEstimateTaskId(id),
            run,
            task,
            consumption: EstimateConsumption::Available,
        });
        Ok(next)
    }
    pub fn lookup(
        &self,
        id: &CalibrationEstimateId,
    ) -> Result<&IssuedEstimateTask<Run>, EstimateAssignmentError> {
        self.0
            .iter()
            .find(|entry| &entry.id.0 == id)
            .ok_or(EstimateAssignmentError::UnknownTask)
    }
    #[must_use]
    pub fn assigned(&self, run: &Run) -> Vec<&IssuedEstimateTask<Run>> {
        self.0.iter().filter(|entry| &entry.run == run).collect()
    }
    pub fn consume(
        &self,
        run: &Run,
        requests: &[CalibrationEstimateId],
    ) -> Result<Self, EstimateAssignmentError> {
        let assigned = self.assigned(run);
        if requests.len() > assigned.len() {
            return Err(EstimateAssignmentError::AssignmentExceeded);
        }
        let mut next = self.clone();
        for id in requests {
            let entry = next
                .0
                .iter_mut()
                .find(|entry| &entry.id.0 == id)
                .ok_or(EstimateAssignmentError::UnknownTask)?;
            if &entry.run != run {
                return Err(EstimateAssignmentError::WrongRun);
            }
            if entry.consumption == EstimateConsumption::Consumed {
                return Err(EstimateAssignmentError::AlreadyConsumed);
            }
            entry.consumption = EstimateConsumption::Consumed;
        }
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AiProbability, AiProbabilityInput, CalibrationEstimateId, CalibrationKey, CalibrationModel,
        CommissionRateFact, CommissionValidity, CostEligibleTask, CostRate,
        EstimateAssignmentError, EstimateAssignments, EstimateConsumption, EstimateCostPeriod,
        EstimateMetadata, EstimateMetadataError, EstimateNarrative, EstimatePrompt,
        EstimateRecordError, EstimateSourceStatus, EstimateSubmissionRecord, Evidence,
        EvidenceBundleId, EvidenceRef, EvidenceUrl, Horizon, HorizonSelection, InstrumentId,
        Market, ModelIdentityPart, NormalizedUrl, PromptHash, Ratio, ScorecardPromptId,
        SignalSubmission, SignalTaskKey, ToolReference, TradeCost, TradeTaxFact, UntrustedText,
        candle, thresholds,
    };
    use proptest::test_runner::{Config, TestRunner};
    use proptest::{collection, prop_assert, prop_assert_eq};
    use rust_decimal::Decimal;

    fn id(value: &str) -> CalibrationEstimateId {
        CalibrationEstimateId::try_new(value).expect("ID")
    }
    fn eligible(horizon: HorizonSelection) -> CostEligibleTask {
        let task = thresholds::tests::task_for(Horizon::new(horizon));
        let rate = CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("비용률");
        let cost = TradeCost::from_facts(
            Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                rate,
                CommissionValidity::try_new(None, None).expect("유효기간"),
            )),
            Some(TradeTaxFact::from_schedule_result(Market::Kr, rate, rate)),
            EstimateCostPeriod::from_calendar_result(&task, candle::tests::at(250)).expect("기간"),
        )
        .expect("비용");
        CostEligibleTask::assess(task, &cost).expect("적격")
    }
    fn ledger() -> EstimateAssignments<InstrumentId> {
        let task = eligible(HorizonSelection::FewDays);
        EstimateAssignments::default()
            .issue(id("one"), run("alpha"), task.clone())
            .expect("발급")
            .issue(id("two"), run("alpha"), task.clone())
            .expect("발급")
            .issue(id("three"), run("beta"), task)
            .expect("발급")
    }
    fn run(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("실행 대역")
    }
    #[test]
    fn issuance_and_consumption_are_pure_and_errors_are_distinct() {
        let before = ledger();
        assert_eq!(before.assigned(&run("alpha")).len(), 2);
        assert_eq!(before.assigned(&run("beta")).len(), 1);
        assert_eq!(
            before.issue(id("one"), run("beta"), eligible(HorizonSelection::FewDays)),
            Err(EstimateAssignmentError::AlreadyIssued)
        );
        assert_eq!(
            before.consume(&run("alpha"), &[id("missing")]),
            Err(EstimateAssignmentError::UnknownTask)
        );
        assert_eq!(
            before.consume(&run("alpha"), &[id("three")]),
            Err(EstimateAssignmentError::WrongRun)
        );
        assert_eq!(
            before.consume(&run("alpha"), &[id("one"), id("two"), id("one")]),
            Err(EstimateAssignmentError::AssignmentExceeded)
        );
    }
    #[test]
    fn consumption_is_atomic_and_repeated_consumption_is_rejected() {
        let before = ledger();
        let after = before.consume(&run("alpha"), &[id("one")]).expect("소비");
        assert_eq!(
            before.lookup(&id("one")).expect("과제").consumption(),
            EstimateConsumption::Available
        );
        assert_eq!(
            after.lookup(&id("one")).expect("과제").consumption(),
            EstimateConsumption::Consumed
        );
        assert_eq!(
            after.lookup(&id("two")).expect("과제").consumption(),
            EstimateConsumption::Available
        );
        assert_eq!(
            after.consume(&run("alpha"), &[id("one")]),
            Err(EstimateAssignmentError::AlreadyConsumed)
        );
        assert_eq!(
            before.consume(&run("alpha"), &[id("one"), id("one")]),
            Err(EstimateAssignmentError::AlreadyConsumed)
        );
        assert_eq!(
            before.consume(&run("alpha"), &[id("one"), id("missing")]),
            Err(EstimateAssignmentError::UnknownTask)
        );
        assert_eq!(before.consume(&run("alpha"), &[]), Ok(before.clone()));
        let all = after.consume(&run("alpha"), &[id("two")]).expect("두번째");
        assert_eq!(
            all.assigned(&run("alpha"))
                .iter()
                .filter(|entry| entry.consumption() == EstimateConsumption::Consumed)
                .count(),
            2
        );
        assert_eq!(
            before
                .lookup(&id("one"))
                .expect("과제")
                .id()
                .as_estimate_id(),
            &id("one")
        );
        assert_eq!(
            before.lookup(&id("one")).expect("과제").run(),
            &run("alpha")
        );
    }
    #[test]
    fn arbitrary_consumption_sequences_never_repeat_or_exceed_assignments() {
        let original = ledger();
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(&collection::vec(0_usize..4, 0..32), |requests| {
                check_sequence(&original, requests)
            })
            .expect("일회 소비·상한");
    }
    fn check_sequence(
        original: &EstimateAssignments<InstrumentId>,
        requests: Vec<usize>,
    ) -> Result<(), proptest::test_runner::TestCaseError> {
        let mut state = original.clone();
        let mut seen = std::collections::BTreeSet::new();
        for index in requests {
            let key = ["one", "two", "three", "missing"][index];
            match state.consume(&run("alpha"), &[id(key)]) {
                Ok(next) => {
                    prop_assert!(index < 2);
                    prop_assert!(seen.insert(index));
                    state = next;
                }
                Err(error) => {
                    let expected = match index {
                        0 | 1 => EstimateAssignmentError::AlreadyConsumed,
                        2 => EstimateAssignmentError::WrongRun,
                        _ => EstimateAssignmentError::UnknownTask,
                    };
                    prop_assert_eq!(error, expected);
                }
            }
            let consumed = state
                .assigned(&run("alpha"))
                .iter()
                .filter(|entry| entry.consumption() == EstimateConsumption::Consumed)
                .count();
            prop_assert_eq!(consumed, seen.len());
            prop_assert!(consumed <= 2);
        }
        Ok(())
    }
    fn metadata() -> EstimateMetadata<HorizonSelection> {
        EstimateMetadata {
            effort: HorizonSelection::Hours,
            cli_version: ModelIdentityPart::try_new("cli-3").expect("버전"),
            prompt: EstimatePrompt {
                id: ScorecardPromptId::try_new("prompt-7").expect("프롬프트"),
                version: ModelIdentityPart::try_new("revision-11").expect("버전"),
                hash: PromptHash::try_new(&"ab".repeat(32)).expect("해시"),
            },
            evidence_bundle: EvidenceBundleId::new(id("bundle-13")),
        }
    }
    fn submission(task: &CostEligibleTask, evidence: Option<Evidence>) -> SignalSubmission {
        let model = CalibrationModel::new(
            ModelIdentityPart::try_new("provider-17").expect("제공자"),
            ModelIdentityPart::try_new("actual-19").expect("실제 모델"),
            ModelIdentityPart::try_new("model-23").expect("모델 버전"),
        );
        SignalSubmission::create(
            SignalTaskKey::from_task(task),
            CalibrationKey::new(model, Horizon::new(HorizonSelection::FewDays)),
            AiProbability::parse(&AiProbabilityInput::try_new("67").expect("입력")).expect("확률"),
            evidence,
            task.task().created_at(),
        )
        .expect("제출")
    }
    fn narrative() -> EstimateNarrative {
        EstimateNarrative {
            rationale: UntrustedText::try_new("근거-29").expect("근거"),
            counterargument: UntrustedText::try_new("반론-31").expect("반론"),
        }
    }
    #[test]
    fn complete_record_preserves_independent_fields_without_trade_input() {
        let assignments = ledger();
        let issued = assignments.lookup(&id("two")).expect("발급");
        let submitted = submission(issued.task(), None);
        let record =
            EstimateSubmissionRecord::create(issued, submitted.clone(), narrative(), metadata())
                .expect("기록");
        assert_eq!(record.task_id().as_estimate_id(), &id("two"));
        assert_eq!(record.run(), &run("alpha"));
        assert_eq!(record.submission(), &submitted);
        assert_eq!(
            record
                .submission()
                .calibration_key()
                .model()
                .name()
                .as_str(),
            "actual-19"
        );
        assert_eq!(record.narrative().rationale.as_untrusted_str(), "근거-29");
        assert_eq!(
            record.narrative().counterargument.as_untrusted_str(),
            "반론-31"
        );
        assert_eq!(record.metadata().effort, HorizonSelection::Hours);
        assert_eq!(record.metadata().cli_version.as_str(), "cli-3");
        assert_eq!(record.metadata().prompt.id.as_str(), "prompt-7");
        assert_eq!(record.metadata().prompt.version.as_str(), "revision-11");
        assert_eq!(record.metadata().prompt.hash.as_str(), "ab".repeat(32));
        assert_eq!(
            record.metadata().evidence_bundle.as_estimate_id(),
            &id("bundle-13")
        );
        assert_eq!(record.source_status(), EstimateSourceStatus::Missing);
        let other = eligible(HorizonSelection::WeeksToMonths);
        assert_eq!(
            EstimateSubmissionRecord::create(
                issued,
                submission(&other, None),
                narrative(),
                metadata()
            ),
            Err(EstimateRecordError::TaskMismatch)
        );
    }
    #[test]
    fn source_absence_includes_tool_only_and_preserves_all_references() {
        let assignments = ledger();
        let issued = assignments.lookup(&id("one")).expect("발급");
        let tool = EvidenceRef::Tool(ToolReference::try_new("tool:quote/a").expect("도구"));
        let url = EvidenceUrl::try_new(
            NormalizedUrl::try_new("https://example.com/research").expect("URL"),
        )
        .expect("출처");
        for (refs, status) in [
            (vec![tool.clone()], EstimateSourceStatus::Missing),
            (
                vec![EvidenceRef::Web(url.clone())],
                EstimateSourceStatus::External,
            ),
            (
                vec![tool, EvidenceRef::Web(url)],
                EstimateSourceStatus::External,
            ),
        ] {
            let evidence = Evidence::try_new(refs).expect("출처");
            let record = EstimateSubmissionRecord::create(
                issued,
                submission(issued.task(), Some(evidence.clone())),
                narrative(),
                metadata(),
            )
            .expect("기록");
            assert_eq!(record.source_status(), status);
            assert_eq!(record.submission().evidence(), Some(&evidence));
        }
    }
    #[test]
    fn prompt_hash_rejects_bad_length_and_non_hex() {
        for text in [
            String::new(),
            "a".repeat(63),
            "a".repeat(65),
            "z".repeat(64),
            "가".repeat(64),
        ] {
            assert_eq!(
                PromptHash::try_new(&text),
                Err(EstimateMetadataError::InvalidHash)
            );
        }
        assert_eq!(
            PromptHash::try_new(&"A0".repeat(32))
                .expect("SHA256")
                .as_str(),
            "A0".repeat(32)
        );
    }
}
