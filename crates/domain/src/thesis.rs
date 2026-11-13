mod evidence;
mod model;

pub use evidence::{Evidence, EvidenceRef, EvidenceUrl, ToolPurpose, ToolReference};
pub use model::{
    ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, ClaimVerdict, CriterionAssessment,
    CriterionComparison, CriterionKind, CriterionValue, CriterionVerdict, JudgedClaim,
    JudgedCriterion, KillCriterion, MachineCondition, UserReason,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThesisVersion(usize);
impl ThesisVersion {
    pub fn try_new(value: usize) -> Result<Self, ThesisError> {
        if value > 256 {
            return Err(ThesisError::InvalidVersion);
        }
        Ok(Self(value))
    }
    #[must_use]
    pub const fn as_usize(&self) -> &usize {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThesisStatus {
    SuspectedBroken,
    NoBreakSignal,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeDisposition {
    Applied,
    NeedsReview,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThesisError {
    InvalidId,
    InvalidVersion,
    InvalidValue,
    InvalidToolReference,
    EmptyText,
    TextTooLong,
    ReferenceTooLong,
    EmptyEvidence,
    TooManySources,
    DuplicateSource,
    ConflictingSource,
    NoSubstantiveEvidence,
    VersionConflict,
    HistoryFull,
    WebHistoryFull,
    TooManyClaims,
    TooManyCriteria,
    DuplicateTarget,
    UnknownTarget,
    NotPending,
    PendingTarget,
    TooManyPending,
    EmptyPatch,
    TooManyOperations,
    AiMachineJudgment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThesisOperation {
    AddClaim(Claim),
    Judge(ClaimId, ClaimVerdict),
    AddKillCriterion(KillCriterion),
    JudgeCriterion(ClaimId, CriterionAssessment),
}
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    Claim(ClaimId),
    Criterion(ClaimId),
}
impl ThesisOperation {
    fn target(&self) -> Target {
        match self {
            Self::AddClaim(value) => Target::Claim(value.id().clone()),
            Self::Judge(id, _) => Target::Claim(id.clone()),
            Self::AddKillCriterion(value) => Target::Criterion(value.id().clone()),
            Self::JudgeCriterion(id, _) => Target::Criterion(id.clone()),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposedChange {
    operation: ThesisOperation,
    evidence: Evidence,
    reason: ChangeReason,
}
impl ProposedChange {
    #[must_use]
    pub const fn new(operation: ThesisOperation, evidence: Evidence, reason: ChangeReason) -> Self {
        Self {
            operation,
            evidence,
            reason,
        }
    }
    #[must_use]
    pub const fn operation(&self) -> &ThesisOperation {
        &self.operation
    }
    #[must_use]
    pub const fn evidence(&self) -> &Evidence {
        &self.evidence
    }
    #[must_use]
    pub const fn reason(&self) -> &ChangeReason {
        &self.reason
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThesisPatch(Box<[ProposedChange]>);
impl ThesisPatch {
    pub fn try_new(changes: Vec<ProposedChange>) -> Result<Self, ThesisError> {
        if changes.is_empty() {
            return Err(ThesisError::EmptyPatch);
        }
        if changes.len() > 16 {
            return Err(ThesisError::TooManyOperations);
        }
        Ok(Self(changes.into_boxed_slice()))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserDecision {
    Confirm,
    Reject,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryAction {
    Change(Box<ProposedChange>),
    Decision(ThesisVersion, UserDecision, UserReason),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    sequence: ThesisVersion,
    action: HistoryAction,
    disposition: ChangeDisposition,
}
impl HistoryEntry {
    #[must_use]
    pub const fn sequence(&self) -> ThesisVersion {
        self.sequence
    }
    #[must_use]
    pub const fn action(&self) -> &HistoryAction {
        &self.action
    }
    #[must_use]
    pub const fn disposition(&self) -> ChangeDisposition {
        self.disposition
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct Thesis {
    history: Vec<HistoryEntry>,
    claims: Vec<JudgedClaim>,
    criteria: Vec<JudgedCriterion>,
}
impl Default for Thesis {
    fn default() -> Self {
        Self::new()
    }
}
impl Thesis {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            history: Vec::new(),
            claims: Vec::new(),
            criteria: Vec::new(),
        }
    }
    #[must_use]
    pub fn version(&self) -> ThesisVersion {
        ThesisVersion(self.history.len())
    }
    #[must_use]
    pub fn history(&self) -> &[HistoryEntry] {
        &self.history
    }
    #[must_use]
    pub fn claims(&self) -> &[JudgedClaim] {
        &self.claims
    }
    #[must_use]
    pub fn kill_criteria(&self) -> &[JudgedCriterion] {
        &self.criteria
    }
    #[must_use]
    pub fn status(&self) -> ThesisStatus {
        if self.claims.iter().any(|entry| {
            entry.verdict == ClaimVerdict::Broken && entry.claim.role() == ClaimRole::Core
        }) || self
            .criteria
            .iter()
            .any(|entry| entry.verdict == CriterionVerdict::Met)
        {
            ThesisStatus::SuspectedBroken
        } else {
            ThesisStatus::NoBreakSignal
        }
    }
    #[must_use]
    pub fn pending(&self) -> Vec<&HistoryEntry> {
        self.history
            .iter()
            .filter(|entry| self.pending_change(entry).is_some())
            .collect()
    }
    fn pending_change<'entry>(
        &self,
        entry: &'entry HistoryEntry,
    ) -> Option<&'entry ProposedChange> {
        let HistoryAction::Change(change) = &entry.action else {
            return None;
        };
        if entry.disposition != ChangeDisposition::NeedsReview {
            return None;
        }
        if self.history.iter().any(|later| match &later.action {
            HistoryAction::Decision(target, _, _) => *target == entry.sequence,
            HistoryAction::Change(candidate) => {
                later.sequence > entry.sequence
                    && candidate.operation.target() == change.operation.target()
            }
        }) {
            return None;
        }
        Some(change)
    }
    fn snapshot(&self) -> Self {
        Self {
            history: self.history.clone(),
            claims: self.claims.clone(),
            criteria: self.criteria.clone(),
        }
    }
    pub fn apply_batch(
        &mut self,
        base: ThesisVersion,
        patch: ThesisPatch,
    ) -> Result<(), ThesisError> {
        self.check_base(base)?;
        let mut staged = self.snapshot();
        for change in patch.0 {
            staged.append_change(change)?;
        }
        *self = staged;
        Ok(())
    }
    fn append_change(&mut self, change: ProposedChange) -> Result<(), ThesisError> {
        self.check_base(self.version())?;
        self.validate(&change.operation)?;
        let disposition = change.evidence.disposition();
        if disposition == ChangeDisposition::NeedsReview {
            self.check_pending_capacity(&change.operation)?;
        }
        if disposition == ChangeDisposition::Applied {
            self.apply(&change.operation);
        }
        self.record(HistoryAction::Change(Box::new(change)), disposition);
        Ok(())
    }
    fn check_pending_capacity(&self, operation: &ThesisOperation) -> Result<(), ThesisError> {
        if self.history.len() >= 224 {
            return Err(ThesisError::WebHistoryFull);
        }
        let pending = self
            .history
            .iter()
            .filter_map(|entry| self.pending_change(entry))
            .collect::<Vec<_>>();
        if pending
            .iter()
            .any(|change| change.operation.target() == operation.target())
        {
            return Err(ThesisError::PendingTarget);
        }
        if pending.len() >= 8 {
            return Err(ThesisError::TooManyPending);
        }
        Ok(())
    }
    pub fn confirm_by_user(
        &mut self,
        base: ThesisVersion,
        pending: ThesisVersion,
        reason: UserReason,
    ) -> Result<(), ThesisError> {
        self.decide(base, pending, UserDecision::Confirm, reason)
    }
    pub fn reject_by_user(
        &mut self,
        base: ThesisVersion,
        pending: ThesisVersion,
        reason: UserReason,
    ) -> Result<(), ThesisError> {
        self.decide(base, pending, UserDecision::Reject, reason)
    }
    fn decide(
        &mut self,
        base: ThesisVersion,
        pending: ThesisVersion,
        decision: UserDecision,
        reason: UserReason,
    ) -> Result<(), ThesisError> {
        self.check_base(base)?;
        let original = self
            .history
            .iter()
            .find(|entry| entry.sequence == pending)
            .and_then(|entry| self.pending_change(entry))
            .ok_or(ThesisError::NotPending)?;
        let operation = original.operation.clone();
        if decision == UserDecision::Confirm {
            self.validate(&operation)?;
            self.apply(&operation);
        }
        self.record(
            HistoryAction::Decision(pending, decision, reason),
            ChangeDisposition::Applied,
        );
        Ok(())
    }
    fn check_base(&self, base: ThesisVersion) -> Result<(), ThesisError> {
        if self.version() != base {
            return Err(ThesisError::VersionConflict);
        }
        if self.history.len() >= 256 {
            return Err(ThesisError::HistoryFull);
        }
        Ok(())
    }
    fn validate(&self, operation: &ThesisOperation) -> Result<(), ThesisError> {
        match operation {
            ThesisOperation::AddClaim(claim) => {
                if self
                    .claims
                    .iter()
                    .any(|entry| entry.claim.id() == claim.id())
                {
                    return Err(ThesisError::DuplicateTarget);
                }
                if self.claims.len() >= 32 {
                    return Err(ThesisError::TooManyClaims);
                }
            }
            ThesisOperation::Judge(id, _) => {
                if !self.claims.iter().any(|entry| entry.claim.id() == id) {
                    return Err(ThesisError::UnknownTarget);
                }
            }
            ThesisOperation::AddKillCriterion(criterion) => {
                if self
                    .criteria
                    .iter()
                    .any(|entry| entry.criterion.id() == criterion.id())
                {
                    return Err(ThesisError::DuplicateTarget);
                }
                if self.criteria.len() >= 16 {
                    return Err(ThesisError::TooManyCriteria);
                }
            }
            ThesisOperation::JudgeCriterion(id, assessment) => {
                let entry = self
                    .criteria
                    .iter()
                    .find(|entry| entry.criterion.id() == id)
                    .ok_or(ThesisError::UnknownTarget)?;
                entry.criterion.validate_assessment(*assessment)?;
            }
        }
        Ok(())
    }
    fn record(&mut self, action: HistoryAction, disposition: ChangeDisposition) {
        self.history.push(HistoryEntry {
            sequence: ThesisVersion(self.history.len().saturating_add(1)),
            action,
            disposition,
        });
    }
    fn apply(&mut self, operation: &ThesisOperation) {
        match operation {
            ThesisOperation::AddClaim(claim) => self.claims.push(JudgedClaim {
                claim: claim.clone(),
                verdict: ClaimVerdict::Unverifiable,
            }),
            ThesisOperation::Judge(id, verdict) => {
                for entry in self
                    .claims
                    .iter_mut()
                    .filter(|entry| entry.claim.id() == id)
                {
                    entry.verdict = *verdict;
                }
            }
            ThesisOperation::AddKillCriterion(criterion) => self.criteria.push(JudgedCriterion {
                criterion: criterion.clone(),
                verdict: CriterionVerdict::Unverifiable,
            }),
            ThesisOperation::JudgeCriterion(id, assessment) => {
                for entry in self
                    .criteria
                    .iter_mut()
                    .filter(|entry| entry.criterion.id() == id)
                {
                    entry.verdict = assessment.verdict();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChangeDisposition, ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, ClaimVerdict,
        CriterionAssessment, CriterionKind, CriterionVerdict, Evidence, EvidenceRef, EvidenceUrl,
        HistoryAction, KillCriterion, ProposedChange, Thesis, ThesisError, ThesisOperation,
        ThesisPatch, ThesisStatus, ThesisVersion, ToolPurpose, ToolReference, UserDecision,
        UserReason,
    };
    use crate::{DartReceiptNo, Disclosure, NewsSource, NormalizedUrl, ReceiptNo, UntrustedText};
    use proptest::test_runner::Config;
    use proptest::test_runner::TestRunner;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) enum TestSource {
        Web,
        News,
    }

    pub(super) fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("문구")
    }
    pub(super) fn id(value: &str) -> ClaimId {
        ClaimId::try_new(value).expect("ID")
    }
    pub(super) fn reason() -> ChangeReason {
        ChangeReason::try_new(text("모델 근거"), text("model"), text("v1")).expect("사유")
    }
    pub(super) fn user() -> UserReason {
        UserReason::try_new(text("사용자가 원문을 읽고 판단함")).expect("사용자 사유")
    }
    pub(super) fn claim(value: &str, role: ClaimRole) -> Claim {
        Claim::new(
            id(value),
            role,
            ClaimContent::try_new(
                text("매출 성장"),
                text("매출 성장률"),
                text("10% 이상"),
                text("공시"),
                text("10% 미만이면 반증"),
            )
            .expect("내용"),
        )
    }
    pub(super) fn evidence(web: TestSource) -> Evidence {
        let url =
            EvidenceUrl::try_new(NormalizedUrl::try_new("https://example.com/a").expect("주소"))
                .expect("상한");
        Evidence::try_new(vec![if web == TestSource::Web {
            EvidenceRef::Web(url)
        } else {
            EvidenceRef::News(NewsSource::Hankyung, url)
        }])
        .expect("근거")
    }
    pub(super) fn change(operation: ThesisOperation, web: TestSource) -> ProposedChange {
        ProposedChange::new(operation, evidence(web), reason())
    }
    fn single_patch(change: ProposedChange) -> ThesisPatch {
        ThesisPatch::try_new(vec![change]).expect("단일 차분")
    }
    fn version(value: usize) -> ThesisVersion {
        ThesisVersion::try_new(value).expect("범위 안 버전")
    }
    pub(super) fn add(
        thesis: &mut Thesis,
        operation: ThesisOperation,
        web: TestSource,
    ) -> Result<(), ThesisError> {
        thesis.apply_batch(thesis.version(), single_patch(change(operation, web)))
    }
    pub(super) fn criterion(value: &str) -> KillCriterion {
        KillCriterion::new(id(value), CriterionKind::Narrative(text("인허가 취소"))).expect("조건")
    }

    fn check_table_cell(
        verdict: ClaimVerdict,
        condition: CriterionVerdict,
        expected: ThesisStatus,
    ) {
        let mut thesis = Thesis::new();
        add(
            &mut thesis,
            ThesisOperation::AddClaim(claim("a", ClaimRole::Core)),
            TestSource::News,
        )
        .expect("주장");
        add(
            &mut thesis,
            ThesisOperation::AddKillCriterion(criterion("a")),
            TestSource::News,
        )
        .expect("조건");
        add(
            &mut thesis,
            ThesisOperation::Judge(id("a"), verdict),
            TestSource::News,
        )
        .expect("판정");
        add(
            &mut thesis,
            ThesisOperation::JudgeCriterion(id("a"), CriterionAssessment::Ai(condition)),
            TestSource::News,
        )
        .expect("판정");
        assert_eq!(thesis.status(), expected);
        let before = thesis.status();
        add(
            &mut thesis,
            ThesisOperation::Judge(id("a"), ClaimVerdict::Broken),
            TestSource::Web,
        )
        .expect("웹 보류");
        assert_eq!(thesis.status(), before);
        assert_eq!(thesis.pending().len(), 1);
    }

    #[test]
    fn r12_claim_and_kill_criterion_table_and_pending_are_independent() {
        let expected = [
            [
                ThesisStatus::NoBreakSignal,
                ThesisStatus::SuspectedBroken,
                ThesisStatus::NoBreakSignal,
            ],
            [
                ThesisStatus::NoBreakSignal,
                ThesisStatus::SuspectedBroken,
                ThesisStatus::NoBreakSignal,
            ],
            [ThesisStatus::SuspectedBroken; 3],
            [
                ThesisStatus::NoBreakSignal,
                ThesisStatus::SuspectedBroken,
                ThesisStatus::NoBreakSignal,
            ],
        ];
        for (row, verdict) in [
            ClaimVerdict::Intact,
            ClaimVerdict::Weakened,
            ClaimVerdict::Broken,
            ClaimVerdict::Unverifiable,
        ]
        .into_iter()
        .enumerate()
        {
            for (column, condition) in [
                CriterionVerdict::NotMet,
                CriterionVerdict::Met,
                CriterionVerdict::Unverifiable,
            ]
            .into_iter()
            .enumerate()
            {
                check_table_cell(verdict, condition, expected[row][column]);
            }
        }
    }

    #[test]
    fn web_flood_m11_cannot_block_nonweb_break_signal_and_decisions_keep_user_reason_m17() {
        let mut thesis = Thesis::new();
        add(
            &mut thesis,
            ThesisOperation::AddClaim(claim("a", ClaimRole::Core)),
            TestSource::News,
        )
        .expect("추가");
        add(
            &mut thesis,
            ThesisOperation::Judge(id("a"), ClaimVerdict::Intact),
            TestSource::Web,
        )
        .expect("보류");
        let pending = thesis.version();
        for _ in 0_usize..4095 {
            assert_eq!(
                add(
                    &mut thesis,
                    ThesisOperation::Judge(id("a"), ClaimVerdict::Intact),
                    TestSource::Web
                ),
                Err(ThesisError::PendingTarget)
            );
        }
        thesis
            .reject_by_user(thesis.version(), pending, user())
            .expect("거절");
        assert_eq!(thesis.pending().len(), 0);
        assert_eq!(
            thesis.history().last().expect("이력").action(),
            &HistoryAction::Decision(pending, UserDecision::Reject, user())
        );
        add(
            &mut thesis,
            ThesisOperation::Judge(id("a"), ClaimVerdict::Broken),
            TestSource::Web,
        )
        .expect("새 보류");
        let confirmed_target = thesis.version();
        thesis
            .confirm_by_user(thesis.version(), confirmed_target, user())
            .expect("확인");
        assert_eq!(
            thesis.history().last().expect("이력").action(),
            &HistoryAction::Decision(confirmed_target, UserDecision::Confirm, user())
        );
        assert_eq!(thesis.status(), ThesisStatus::SuspectedBroken);
        add(
            &mut thesis,
            ThesisOperation::Judge(id("a"), ClaimVerdict::Intact),
            TestSource::Web,
        )
        .expect("보류");
        add(
            &mut thesis,
            ThesisOperation::Judge(id("a"), ClaimVerdict::Broken),
            TestSource::News,
        )
        .expect("실제 신호");
        assert_eq!(thesis.pending().len(), 0);
    }

    #[test]
    fn batch_is_atomic_and_versions_round_trip_from_external_integers() {
        let mut thesis = Thesis::new();
        let patch = ThesisPatch::try_new(vec![
            change(
                ThesisOperation::AddClaim(claim("a", ClaimRole::Core)),
                TestSource::News,
            ),
            change(
                ThesisOperation::Judge(id("missing"), ClaimVerdict::Broken),
                TestSource::News,
            ),
        ])
        .expect("차분");
        assert_eq!(
            thesis.apply_batch(ThesisVersion::try_new(0).expect("버전"), patch),
            Err(ThesisError::UnknownTarget)
        );
        assert_eq!(thesis.history(), []);
        assert_eq!(thesis.claims(), []);
        assert_eq!(*ThesisVersion::try_new(256).expect("상한").as_usize(), 256);
        assert_eq!(
            ThesisVersion::try_new(257),
            Err(ThesisError::InvalidVersion)
        );
    }

    #[test]
    fn evidence_preserves_receipts_rejects_cross_source_urls_and_ignores_quote_tools() {
        let url = NormalizedUrl::try_new("https://example.com/a").expect("주소");
        let disclosure = Disclosure::new(
            ReceiptNo::Dart(DartReceiptNo::try_new("20261113000001").expect("번호")),
            url.clone(),
            text("제목"),
            None,
        );
        let reference = EvidenceRef::from_disclosure(&disclosure).expect("공시 참조");
        assert_eq!(reference.disclosure_key(), Some(disclosure.key()));
        let web = EvidenceRef::Web(EvidenceUrl::try_new(url).expect("주소"));
        assert_eq!(
            Evidence::try_new(vec![reference, web.clone()]),
            Err(ThesisError::ConflictingSource)
        );
        let quote = EvidenceRef::Tool(
            ToolReference::try_new("tool:get_quote/1").expect("도구"),
            ToolPurpose::Quote,
        );
        assert_eq!(
            Evidence::try_new(vec![web, quote.clone()])
                .expect("근거")
                .disposition(),
            ChangeDisposition::NeedsReview
        );
        assert_eq!(
            Evidence::try_new(vec![quote]),
            Err(ThesisError::NoSubstantiveEvidence)
        );
    }

    pub(super) fn replay(original: &Thesis) -> Thesis {
        let mut restored = Thesis::new();
        for entry in original.history() {
            let base = ThesisVersion::try_new(*restored.version().as_usize()).expect("버전");
            match entry.action() {
                HistoryAction::Change(change) => restored
                    .apply_batch(
                        base,
                        ThesisPatch::try_new(vec![change.as_ref().clone()]).expect("차분"),
                    )
                    .expect("재생"),
                HistoryAction::Decision(target, UserDecision::Confirm, reason) => restored
                    .confirm_by_user(base, *target, reason.clone())
                    .expect("확인 재생"),
                HistoryAction::Decision(target, UserDecision::Reject, reason) => restored
                    .reject_by_user(base, *target, reason.clone())
                    .expect("거절 재생"),
            }
        }
        restored
    }

    fn generated_step(thesis: &mut Thesis, operation: u8, key: u8) -> Result<(), ThesisError> {
        let target = id(&format!("c{key}"));
        match operation {
            0 => add(
                thesis,
                ThesisOperation::AddClaim(claim(target.as_str(), ClaimRole::Core)),
                TestSource::News,
            ),
            1 => add(
                thesis,
                ThesisOperation::AddKillCriterion(criterion(target.as_str())),
                TestSource::News,
            ),
            2 => add(
                thesis,
                ThesisOperation::Judge(target, ClaimVerdict::Broken),
                TestSource::Web,
            ),
            3 => add(
                thesis,
                ThesisOperation::Judge(target, ClaimVerdict::Intact),
                TestSource::News,
            ),
            4 => add(
                thesis,
                ThesisOperation::JudgeCriterion(
                    target,
                    CriterionAssessment::Ai(CriterionVerdict::Met),
                ),
                TestSource::Web,
            ),
            5 => add(
                thesis,
                ThesisOperation::JudgeCriterion(
                    target,
                    CriterionAssessment::Ai(CriterionVerdict::Met),
                ),
                TestSource::News,
            ),
            6 => thesis.confirm_by_user(thesis.version(), version(usize::from(key)), user()),
            7 => thesis.reject_by_user(thesis.version(), version(usize::from(key)), user()),
            8 => thesis.apply_batch(
                version(256),
                single_patch(change(
                    ThesisOperation::AddClaim(claim(target.as_str(), ClaimRole::Core)),
                    TestSource::News,
                )),
            ),
            _ => add(
                thesis,
                ThesisOperation::AddClaim(claim(target.as_str(), ClaimRole::Supporting)),
                TestSource::Web,
            ),
        }
    }

    fn check_generated_step(thesis: &mut Thesis, operation: u8, key: u8) {
        let before = thesis.snapshot();
        let result = generated_step(thesis, operation, key);
        assert_eq!(
            &thesis.history()[..before.history().len()],
            before.history()
        );
        if result.is_err() {
            assert_eq!(&*thesis, &before);
        }
        if result.is_ok()
            && thesis
                .history()
                .last()
                .is_some_and(|entry| entry.disposition() == ChangeDisposition::NeedsReview)
        {
            assert_eq!(thesis.status(), before.status());
        }
        let restored = replay(thesis);
        assert_eq!(restored.pending(), thesis.pending());
        assert_eq!(&restored, thesis);
    }

    #[test]
    fn mixed_history_prefix_failure_atomicity_replay_and_pending_isolation() {
        let strategy = proptest::collection::vec((0_u8..10, 0_u8..8), 0..64);
        TestRunner::new(Config::with_cases(64))
            .run(&strategy, |steps| {
                let mut thesis = Thesis::new();
                for (operation, key) in steps {
                    check_generated_step(&mut thesis, operation, key);
                }
                Ok(())
            })
            .expect("혼합 이력 속성");
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::tests::{
        TestSource, add, change, claim, criterion, evidence, id, reason, replay, text, user,
    };
    use super::{
        ChangeDisposition, ChangeReason, ClaimContent, ClaimId, ClaimRole, ClaimVerdict,
        CriterionAssessment, CriterionComparison, CriterionKind, CriterionValue, CriterionVerdict,
        Evidence, EvidenceRef, EvidenceUrl, HistoryAction, KillCriterion, MachineCondition, Thesis,
        ThesisError, ThesisOperation, ThesisPatch, ThesisStatus, ThesisVersion, ToolPurpose,
        ToolReference, UserReason,
    };
    use crate::{DartReceiptNo, Disclosure, NewsSource, NormalizedUrl, ReceiptNo};
    use rust_decimal::Decimal;

    #[test]
    fn claim_and_metadata_bounds_getters_and_debug_are_preserved() {
        for value in ["", " ", "a.", "가", "a\n"] {
            assert_eq!(ClaimId::try_new(value), Err(ThesisError::InvalidId));
        }
        assert_eq!(id(&"a".repeat(64)).as_str(), "a".repeat(64));
        assert_eq!(
            ClaimId::try_new(&"a".repeat(65)),
            Err(ThesisError::InvalidId)
        );
        let limits = [1024, 128, 256, 256, 512];
        for (index, limit) in limits.into_iter().enumerate() {
            let mut fields = [text("s"), text("m"), text("e"), text("o"), text("r")];
            fields[index] = text(&"a".repeat(limit));
            let [statement, metric, expected, observe, refutation] = fields.clone();
            ClaimContent::try_new(statement, metric, expected, observe, refutation)
                .expect("경계값 통과");
            fields[index] = text(&"a".repeat(limit + 1));
            let [
                bad_statement,
                bad_metric,
                bad_expected,
                bad_observe,
                bad_refutation,
            ] = fields;
            assert_eq!(
                ClaimContent::try_new(
                    bad_statement,
                    bad_metric,
                    bad_expected,
                    bad_observe,
                    bad_refutation
                ),
                Err(ThesisError::TextTooLong)
            );
        }
        assert_eq!(
            ClaimContent::try_new(text(""), text("m"), text("e"), text("o"), text("r")),
            Err(ThesisError::EmptyText)
        );
        let created = claim("A_1-z", ClaimRole::Supporting);
        assert_eq!(created.statement(), &text("매출 성장"));
        assert_eq!(created.metric(), &text("매출 성장률"));
        assert_eq!(created.expected(), &text("10% 이상"));
        assert_eq!(created.observe_via(), &text("공시"));
        assert_eq!(created.refutation(), &text("10% 미만이면 반증"));
        assert!(!format!("{created:?}").contains("매출 성장"));
    }

    #[test]
    fn model_and_user_reasons_have_distinct_bounded_metadata() {
        let created = reason();
        assert_eq!(created.reason(), &text("모델 근거"));
        assert_eq!(created.model(), &text("model"));
        assert_eq!(created.prompt_version(), &text("v1"));
        assert_eq!(
            user().as_untrusted_text(),
            &text("사용자가 원문을 읽고 판단함")
        );
        for values in [["", "m", "p"], ["r", "", "p"], ["r", "m", ""]] {
            assert_eq!(
                ChangeReason::try_new(text(values[0]), text(values[1]), text(values[2])),
                Err(ThesisError::EmptyText)
            );
        }
        for (index, limit) in [512, 64, 64].into_iter().enumerate() {
            let mut fields = [text("r"), text("m"), text("p")];
            fields[index] = text(&"a".repeat(limit));
            let [detail, model, prompt] = fields.clone();
            ChangeReason::try_new(detail, model, prompt).expect("경계값 통과");
            fields[index] = text(&"a".repeat(limit + 1));
            let [bad_detail, bad_model, bad_prompt] = fields;
            assert_eq!(
                ChangeReason::try_new(bad_detail, bad_model, bad_prompt),
                Err(ThesisError::TextTooLong)
            );
        }
        UserReason::try_new(text(&"a".repeat(512))).expect("경계값 통과");
        assert_eq!(
            UserReason::try_new(text(&"a".repeat(513))),
            Err(ThesisError::TextTooLong)
        );
        assert_eq!(
            UserReason::try_new(text(" \n")),
            Err(ThesisError::EmptyText)
        );
    }

    fn machine(comparison: CriterionComparison) -> MachineCondition {
        MachineCondition::new(
            ToolReference::try_new("tool:revenue/1").expect("도구"),
            comparison,
            CriterionValue::new(Decimal::from(10)),
        )
    }
    #[test]
    fn machine_conditions_are_evaluated_in_code_and_ai_cannot_judge_them() {
        for (comparison, expected) in [
            (
                CriterionComparison::AtLeast,
                [
                    CriterionVerdict::NotMet,
                    CriterionVerdict::Met,
                    CriterionVerdict::Met,
                ],
            ),
            (
                CriterionComparison::AtMost,
                [
                    CriterionVerdict::Met,
                    CriterionVerdict::Met,
                    CriterionVerdict::NotMet,
                ],
            ),
            (
                CriterionComparison::Equal,
                [
                    CriterionVerdict::NotMet,
                    CriterionVerdict::Met,
                    CriterionVerdict::NotMet,
                ],
            ),
        ] {
            for (observed, verdict) in [9_i32, 10_i32, 11_i32].into_iter().zip(expected) {
                assert_eq!(
                    machine(comparison).evaluate(CriterionValue::new(Decimal::from(observed))),
                    CriterionAssessment::Code(verdict)
                );
            }
        }
    }

    #[test]
    fn machine_ai_rejection_and_narrative_bounds() {
        let condition = machine(CriterionComparison::AtLeast);
        assert_eq!(condition.metric().as_str(), "tool:revenue/1");
        let created =
            KillCriterion::new(id("k"), CriterionKind::Machine(condition.clone())).expect("조건");
        assert_eq!(created.kind(), &CriterionKind::Machine(condition));
        let mut thesis = Thesis::new();
        add(
            &mut thesis,
            ThesisOperation::AddKillCriterion(created),
            TestSource::News,
        )
        .expect("추가");
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::JudgeCriterion(
                    id("k"),
                    CriterionAssessment::Ai(CriterionVerdict::Met)
                ),
                TestSource::News
            ),
            Err(ThesisError::AiMachineJudgment)
        );
        add(
            &mut thesis,
            ThesisOperation::JudgeCriterion(
                id("k"),
                machine(CriterionComparison::AtLeast)
                    .evaluate(CriterionValue::new(Decimal::from(11_i32))),
            ),
            TestSource::News,
        )
        .expect("코드 판정");
        assert_eq!(thesis.status(), ThesisStatus::SuspectedBroken);
        assert_eq!(thesis.kill_criteria()[0].criterion().id(), &id("k"));
        assert_eq!(thesis.kill_criteria()[0].verdict(), CriterionVerdict::Met);
        assert_eq!(
            KillCriterion::new(id("x"), CriterionKind::Narrative(text(""))),
            Err(ThesisError::EmptyText)
        );
        assert_eq!(
            KillCriterion::new(id("x"), CriterionKind::Narrative(text(&"a".repeat(513)))),
            Err(ThesisError::TextTooLong)
        );
        KillCriterion::new(id("x"), CriterionKind::Narrative(text(&"a".repeat(512))))
            .expect("경계값 통과");
    }

    fn web_ref(index: usize) -> EvidenceRef {
        EvidenceRef::Web(
            EvidenceUrl::try_new(
                NormalizedUrl::try_new(&format!("https://example.com/{index}")).expect("주소"),
            )
            .expect("길이"),
        )
    }
    fn tool(purpose: ToolPurpose) -> EvidenceRef {
        EvidenceRef::Tool(
            ToolReference::try_new("tool:source/1").expect("도구"),
            purpose,
        )
    }
    #[test]
    fn evidence_key_boundaries_and_all_source_combinations_are_explicit() {
        assert_eq!(Evidence::try_new(vec![]), Err(ThesisError::EmptyEvidence));
        assert_eq!(
            Evidence::try_new(vec![web_ref(0), web_ref(0)]),
            Err(ThesisError::DuplicateSource)
        );
        let items = (0_usize..8).map(web_ref).collect::<Vec<_>>();
        assert_eq!(
            Evidence::try_new(items.clone()).expect("상한").as_items(),
            items
        );
        assert_eq!(
            Evidence::try_new((0_usize..9).map(web_ref).collect()),
            Err(ThesisError::TooManySources)
        );
    }

    #[test]
    fn tool_reference_syntax_is_bounded() {
        for invalid in [
            "",
            "tool:",
            "news:1",
            "tool:가",
            "tool:space id",
            "tool:a?b",
        ] {
            assert_eq!(
                ToolReference::try_new(invalid),
                Err(ThesisError::InvalidToolReference)
            );
        }
        assert_eq!(
            ToolReference::try_new(&format!("tool:{}", "a".repeat(124))),
            Err(ThesisError::InvalidToolReference)
        );
        assert_eq!(
            ToolReference::try_new(&format!("tool:{}", "a".repeat(123)))
                .expect("상한")
                .as_str()
                .len(),
            128
        );
    }

    #[test]
    fn tool_purposes_and_url_source_collisions() {
        for purpose in [ToolPurpose::Quote, ToolPurpose::Portfolio] {
            assert_eq!(
                Evidence::try_new(vec![tool(purpose)]),
                Err(ThesisError::NoSubstantiveEvidence)
            );
            assert_eq!(
                Evidence::try_new(vec![web_ref(0), tool(purpose)])
                    .expect("웹과 보조 도구")
                    .disposition(),
                ChangeDisposition::NeedsReview
            );
        }
        assert_eq!(
            Evidence::try_new(vec![web_ref(0), tool(ToolPurpose::Research)])
                .expect("실질 도구")
                .disposition(),
            ChangeDisposition::Applied
        );
        assert_eq!(
            Evidence::try_new(vec![tool(ToolPurpose::Research), tool(ToolPurpose::Quote)]),
            Err(ThesisError::ConflictingSource)
        );
        let ref_url =
            EvidenceUrl::try_new(NormalizedUrl::try_new("https://example.com/0").expect("주소"))
                .expect("길이");
        assert_eq!(ref_url.as_url().as_str(), "https://example.com/0");
        assert_eq!(
            Evidence::try_new(vec![
                web_ref(0),
                EvidenceRef::News(NewsSource::Hankyung, ref_url)
            ]),
            Err(ThesisError::ConflictingSource)
        );
        assert_eq!(web_ref(0).disclosure_key(), None);
        assert_eq!(tool(ToolPurpose::Research).disclosure_key(), None);
        assert_eq!(
            evidence(TestSource::News).as_items()[0].disclosure_key(),
            None
        );
    }

    #[test]
    fn disclosure_identity_and_url_bounds_survive_without_body_copying() {
        let disclosure = Disclosure::new(
            ReceiptNo::Dart(DartReceiptNo::try_new("20261113000001").expect("번호")),
            NormalizedUrl::try_new("https://example.com/0").expect("주소"),
            text("본문 복사 금지"),
            Some(text("본문")),
        );
        let first = EvidenceRef::from_disclosure(&disclosure).expect("참조");
        let second = EvidenceRef::Disclosure(
            disclosure.key().clone(),
            EvidenceUrl::try_new(NormalizedUrl::try_new("https://example.com/1").expect("주소"))
                .expect("길이"),
        );
        assert_eq!(
            Evidence::try_new(vec![first.clone(), second]),
            Err(ThesisError::DuplicateSource)
        );
        assert!(!format!("{first:?}").contains("본문"));
        let prefix = "https://example.com/";
        let url = NormalizedUrl::try_new(&format!("{prefix}{}", "a".repeat(2048 - prefix.len())))
            .expect("주소");
        EvidenceUrl::try_new(url).expect("경계값 통과");
        let overlong_url =
            NormalizedUrl::try_new(&format!("{prefix}{}", "a".repeat(2049 - prefix.len())))
                .expect("주소");
        assert_eq!(
            EvidenceUrl::try_new(overlong_url.clone()),
            Err(ThesisError::ReferenceTooLong)
        );
        let long = Disclosure::new(
            disclosure.key().receipt_no().clone(),
            overlong_url,
            text("공시"),
            None,
        );
        assert_eq!(
            EvidenceRef::from_disclosure(&long),
            Err(ThesisError::ReferenceTooLong)
        );
    }

    #[test]
    fn patch_and_state_limits_are_atomic_including_confirmations() {
        assert_eq!(ThesisPatch::try_new(vec![]), Err(ThesisError::EmptyPatch));
        let operation = change(
            ThesisOperation::AddClaim(claim("a", ClaimRole::Core)),
            TestSource::News,
        );
        ThesisPatch::try_new(vec![operation.clone(); 16]).expect("경계값 통과");
        assert_eq!(
            ThesisPatch::try_new(vec![operation; 17]),
            Err(ThesisError::TooManyOperations)
        );
    }

    #[test]
    fn claim_limit_also_applies_when_confirming() {
        let mut thesis = Thesis::default();
        add(
            &mut thesis,
            ThesisOperation::AddClaim(claim("pending", ClaimRole::Core)),
            TestSource::Web,
        )
        .expect("보류");
        let pending = thesis.version();
        for index in 0_usize..32 {
            add(
                &mut thesis,
                ThesisOperation::AddClaim(claim(&format!("c{index}"), ClaimRole::Supporting)),
                TestSource::News,
            )
            .expect("상한");
        }
        let before = thesis.snapshot();
        assert_eq!(
            thesis.confirm_by_user(thesis.version(), pending, user()),
            Err(ThesisError::TooManyClaims)
        );
        assert_eq!(thesis, before);
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::AddClaim(claim("overflow", ClaimRole::Core)),
                TestSource::News
            ),
            Err(ThesisError::TooManyClaims)
        );
        assert_eq!(thesis.claims()[0].claim().id(), &id("c0"));
        assert_eq!(thesis.claims()[0].verdict(), ClaimVerdict::Unverifiable);
        assert_eq!(thesis.status(), ThesisStatus::NoBreakSignal);
    }

    #[test]
    fn criterion_limits_and_unknown_targets() {
        let mut thesis = Thesis::new();
        for index in 0_usize..16 {
            add(
                &mut thesis,
                ThesisOperation::AddKillCriterion(criterion(&format!("k{index}"))),
                TestSource::News,
            )
            .expect("조건 상한");
        }
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::AddKillCriterion(criterion("overflow")),
                TestSource::News
            ),
            Err(ThesisError::TooManyCriteria)
        );
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::AddKillCriterion(criterion("k0")),
                TestSource::News
            ),
            Err(ThesisError::DuplicateTarget)
        );
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::JudgeCriterion(
                    id("missing"),
                    CriterionAssessment::Code(CriterionVerdict::Met)
                ),
                TestSource::News
            ),
            Err(ThesisError::UnknownTarget)
        );
    }

    #[test]
    fn pending_limits_leave_reserved_history_for_nonweb_signals() {
        let mut thesis = Thesis::new();
        for index in 0_usize..8 {
            add(
                &mut thesis,
                ThesisOperation::AddClaim(claim(&format!("p{index}"), ClaimRole::Core)),
                TestSource::Web,
            )
            .expect("보류 상한");
        }
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::AddClaim(claim("overflow", ClaimRole::Core)),
                TestSource::Web
            ),
            Err(ThesisError::TooManyPending)
        );
    }

    #[test]
    fn batch_crossing_history_cap_rolls_back_all_intermediate_entries() {
        let mut thesis = Thesis::new();
        add(
            &mut thesis,
            ThesisOperation::AddClaim(claim("a", ClaimRole::Core)),
            TestSource::News,
        )
        .expect("추가");
        for _ in 1_usize..255 {
            add(
                &mut thesis,
                ThesisOperation::Judge(id("a"), ClaimVerdict::Intact),
                TestSource::News,
            )
            .expect("상한 전");
        }
        let before = thesis.snapshot();
        let change = change(
            ThesisOperation::Judge(id("a"), ClaimVerdict::Broken),
            TestSource::News,
        );
        assert_eq!(
            thesis.apply_batch(
                thesis.version(),
                ThesisPatch::try_new(vec![change.clone(), change]).expect("차분")
            ),
            Err(ThesisError::HistoryFull)
        );
        assert_eq!(thesis, before);
    }

    #[test]
    fn reserved_history_and_full_history_are_distinct() {
        let mut thesis = Thesis::new();
        for index in 0_usize..8 {
            add(
                &mut thesis,
                ThesisOperation::AddClaim(claim(&format!("p{index}"), ClaimRole::Core)),
                TestSource::Web,
            )
            .expect("보류");
        }
        let pending = thesis.pending()[0].sequence();
        thesis
            .confirm_by_user(thesis.version(), pending, user())
            .expect("확인");
        while *thesis.version().as_usize() < 224 {
            add(
                &mut thesis,
                ThesisOperation::Judge(id("p0"), ClaimVerdict::Intact),
                TestSource::News,
            )
            .expect("공간 사용");
        }
        let before = thesis.snapshot();
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::Judge(id("p0"), ClaimVerdict::Intact),
                TestSource::Web
            ),
            Err(ThesisError::WebHistoryFull)
        );
        assert_eq!(thesis, before);
        add(
            &mut thesis,
            ThesisOperation::Judge(id("p0"), ClaimVerdict::Broken),
            TestSource::News,
        )
        .expect("예약 공간 실제 신호");
        assert_eq!(thesis.status(), ThesisStatus::SuspectedBroken);
        while *thesis.version().as_usize() < 256 {
            add(
                &mut thesis,
                ThesisOperation::Judge(id("p0"), ClaimVerdict::Broken),
                TestSource::News,
            )
            .expect("이력 상한");
        }
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::Judge(id("p0"), ClaimVerdict::Broken),
                TestSource::News
            ),
            Err(ThesisError::HistoryFull)
        );
        assert_eq!(
            thesis.reject_by_user(thesis.version(), pending, user()),
            Err(ThesisError::HistoryFull)
        );
        assert_eq!(replay(&thesis), thesis);
    }

    #[test]
    fn stale_decisions_and_batch_success_preserve_public_actions() {
        let mut thesis = Thesis::new();
        let changes = vec![
            change(
                ThesisOperation::AddClaim(claim("a", ClaimRole::Core)),
                TestSource::News,
            ),
            change(
                ThesisOperation::Judge(id("a"), ClaimVerdict::Broken),
                TestSource::News,
            ),
        ];
        assert_eq!(
            changes[0].operation(),
            &ThesisOperation::AddClaim(claim("a", ClaimRole::Core))
        );
        assert_eq!(changes[0].evidence(), &evidence(TestSource::News));
        assert_eq!(changes[0].reason(), &reason());
        thesis
            .apply_batch(
                thesis.version(),
                ThesisPatch::try_new(changes).expect("차분"),
            )
            .expect("묶음 성공");
        assert_eq!(thesis.version(), ThesisVersion::try_new(2).expect("순번"));
    }

    #[test]
    fn stale_user_decisions_are_rejected() {
        let mut thesis = Thesis::new();
        add(
            &mut thesis,
            ThesisOperation::AddClaim(claim("a", ClaimRole::Core)),
            TestSource::News,
        )
        .expect("주장");
        assert_eq!(
            thesis.confirm_by_user(thesis.version(), thesis.version(), user()),
            Err(ThesisError::NotPending)
        );
        add(
            &mut thesis,
            ThesisOperation::Judge(id("a"), ClaimVerdict::Intact),
            TestSource::Web,
        )
        .expect("보류");
        let pending = thesis.version();
        thesis
            .reject_by_user(thesis.version(), pending, user())
            .expect("거절");
        assert_eq!(
            thesis.confirm_by_user(thesis.version(), pending, user()),
            Err(ThesisError::NotPending)
        );
        assert_eq!(
            thesis.confirm_by_user(ThesisVersion::try_new(0).expect("순번"), pending, user()),
            Err(ThesisError::VersionConflict)
        );
        assert_eq!(
            thesis.reject_by_user(
                thesis.version(),
                ThesisVersion::try_new(0).expect("순번"),
                user()
            ),
            Err(ThesisError::NotPending)
        );
        assert_eq!(
            thesis.history()[0].disposition(),
            ChangeDisposition::Applied
        );
        assert_eq!(
            thesis.history().last().expect("이력").action(),
            &HistoryAction::Decision(pending, super::UserDecision::Reject, user())
        );
        assert_eq!(replay(&thesis), thesis);
    }
}
