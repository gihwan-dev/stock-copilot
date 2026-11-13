mod evidence;
mod model;

pub use evidence::{Evidence, EvidenceRef, EvidenceUrl, ToolReference};
pub use model::{
    ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, ClaimVerdict, CriterionAssessment,
    CriterionComparison, CriterionKind, CriterionMetric, CriterionObservation, CriterionValue,
    CriterionVerdict, JudgedClaim, JudgedCriterion, KillCriterion, MachineCondition, UserReason,
};

const HISTORY_LIMIT: ThesisVersion = ThesisVersion(6144);
const MODEL_HISTORY_LIMIT: ThesisVersion = ThesisVersion(HISTORY_LIMIT.0.saturating_sub(8));
const WEB_HISTORY_LIMIT: ThesisVersion = ThesisVersion(HISTORY_LIMIT.0.saturating_sub(32));

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThesisVersion(usize);
impl ThesisVersion {
    pub fn try_new(value: usize) -> Result<Self, ThesisError> {
        if value > HISTORY_LIMIT.0 {
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
    CodeNarrativeJudgment,
    InvalidMetric,
    MetricMismatch,
    UserHistoryReserved,
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
#[derive(Clone, Copy)]
enum ValidatedOperation<'operation> {
    AddClaim(&'operation Claim),
    Judge(&'operation ClaimId, ClaimVerdict),
    AddKillCriterion(&'operation KillCriterion),
    JudgeCriterion(&'operation ClaimId, CriterionVerdict),
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
        if self.version() >= MODEL_HISTORY_LIMIT {
            return Err(ThesisError::UserHistoryReserved);
        }
        let validated = self.validate(&change.operation)?;
        let disposition = match &change.operation {
            ThesisOperation::JudgeCriterion(_, CriterionAssessment::Observed(_)) => {
                ChangeDisposition::Applied
            }
            ThesisOperation::AddClaim(_)
            | ThesisOperation::Judge(_, _)
            | ThesisOperation::AddKillCriterion(_)
            | ThesisOperation::JudgeCriterion(_, CriterionAssessment::Ai(_)) => {
                change.evidence.disposition()
            }
        };
        if disposition == ChangeDisposition::NeedsReview {
            self.check_pending_capacity(&change.operation)?;
        }
        if disposition == ChangeDisposition::Applied {
            self.apply(validated);
        }
        self.record(HistoryAction::Change(Box::new(change)), disposition);
        Ok(())
    }
    fn check_pending_capacity(&self, operation: &ThesisOperation) -> Result<(), ThesisError> {
        if self.version() >= WEB_HISTORY_LIMIT {
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
            let validated = self.validate(&operation)?;
            self.apply(validated);
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
        if self.version() >= HISTORY_LIMIT {
            return Err(ThesisError::HistoryFull);
        }
        Ok(())
    }
    fn validate<'operation>(
        &self,
        operation: &'operation ThesisOperation,
    ) -> Result<ValidatedOperation<'operation>, ThesisError> {
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
                Ok(ValidatedOperation::AddClaim(claim))
            }
            ThesisOperation::Judge(id, verdict) => {
                if !self.claims.iter().any(|entry| entry.claim.id() == id) {
                    return Err(ThesisError::UnknownTarget);
                }
                Ok(ValidatedOperation::Judge(id, *verdict))
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
                Ok(ValidatedOperation::AddKillCriterion(criterion))
            }
            ThesisOperation::JudgeCriterion(id, assessment) => {
                let entry = self
                    .criteria
                    .iter()
                    .find(|entry| entry.criterion.id() == id)
                    .ok_or(ThesisError::UnknownTarget)?;
                let verdict = entry.criterion.assess(assessment)?;
                Ok(ValidatedOperation::JudgeCriterion(id, verdict))
            }
        }
    }
    fn record(&mut self, action: HistoryAction, disposition: ChangeDisposition) {
        self.history.push(HistoryEntry {
            sequence: ThesisVersion(self.history.len().saturating_add(1)),
            action,
            disposition,
        });
    }
    fn apply(&mut self, operation: ValidatedOperation<'_>) {
        match operation {
            ValidatedOperation::AddClaim(claim) => self.claims.push(JudgedClaim {
                claim: claim.clone(),
                verdict: ClaimVerdict::Unverifiable,
            }),
            ValidatedOperation::Judge(id, verdict) => {
                for entry in self
                    .claims
                    .iter_mut()
                    .filter(|entry| entry.claim.id() == id)
                {
                    entry.verdict = verdict;
                }
            }
            ValidatedOperation::AddKillCriterion(criterion) => {
                self.criteria.push(JudgedCriterion {
                    criterion: criterion.clone(),
                    verdict: CriterionVerdict::Unverifiable,
                });
            }
            ValidatedOperation::JudgeCriterion(id, verdict) => {
                for entry in self
                    .criteria
                    .iter_mut()
                    .filter(|entry| entry.criterion.id() == id)
                {
                    entry.verdict = verdict;
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
        ThesisPatch, ThesisStatus, ThesisVersion, ToolReference, UserDecision, UserReason,
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
        for _ in 0_usize..12287 {
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
        assert_eq!(
            *ThesisVersion::try_new(6144).expect("상한").as_usize(),
            6144
        );
        assert_eq!(
            ThesisVersion::try_new(6145),
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
        let quote = EvidenceRef::Tool(ToolReference::try_new("tool:get_quote/1").expect("도구"));
        assert_eq!(
            Evidence::try_new(vec![web, quote.clone()])
                .expect("근거")
                .disposition(),
            ChangeDisposition::NeedsReview
        );
        assert_eq!(
            Evidence::try_new(vec![quote]).map(|value| value.disposition()),
            Ok(ChangeDisposition::NeedsReview)
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
                        ThesisPatch::try_new(vec![super::state_property_tests::rebuild_change(
                            change,
                        )])
                        .expect("차분"),
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

    fn flush_replay(restored: &mut Thesis, changes: &mut Vec<ProposedChange>) {
        if changes.is_empty() {
            return;
        }
        let patch = ThesisPatch::try_new(changes.clone()).expect("재생 묶음");
        changes.clear();
        restored
            .apply_batch(restored.version(), patch)
            .expect("묶음 재생");
    }
    pub(super) fn replay_batched(original: &Thesis) -> Thesis {
        let mut restored = Thesis::new();
        let mut changes = Vec::new();
        for entry in original.history() {
            match entry.action() {
                HistoryAction::Change(change) => {
                    changes.push(super::state_property_tests::rebuild_change(change));
                }
                HistoryAction::Decision(target, decision, reason) => {
                    flush_replay(&mut restored, &mut changes);
                    match decision {
                        UserDecision::Confirm => restored
                            .confirm_by_user(restored.version(), *target, reason.clone())
                            .expect("확인 재생"),
                        UserDecision::Reject => restored
                            .reject_by_user(restored.version(), *target, reason.clone())
                            .expect("거절 재생"),
                    }
                }
            }
            if changes.len() == 16 {
                flush_replay(&mut restored, &mut changes);
            }
        }
        flush_replay(&mut restored, &mut changes);
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
                version(6144),
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
        assert_eq!(&replay_batched(thesis), thesis);
    }

    #[test]
    fn mixed_history_prefix_failure_atomicity_replay_and_pending_isolation() {
        let strategy = proptest::collection::vec((0_u8..10, 0_u8..8), 0..64);
        TestRunner::new(Config::with_cases(256))
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
        TestSource, add, change, claim, criterion, evidence, id, reason, replay, replay_batched,
        text, user,
    };
    use super::{
        ChangeDisposition, ChangeReason, ClaimContent, ClaimId, ClaimRole, ClaimVerdict,
        CriterionAssessment, CriterionComparison, CriterionKind, CriterionMetric, CriterionValue,
        CriterionVerdict, Evidence, EvidenceRef, EvidenceUrl, HistoryAction, KillCriterion,
        MachineCondition, Thesis, ThesisError, ThesisOperation, ThesisPatch, ThesisStatus,
        ThesisVersion, ToolReference, UserReason,
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
            CriterionMetric::try_new("revenue").expect("지표"),
            comparison,
            CriterionValue::new(Decimal::from(10_i32)),
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
                    machine(comparison)
                        .evaluate(&super::final_review_tests::observation(observed, "revenue")),
                    Ok(verdict)
                );
            }
        }
    }

    #[test]
    fn machine_ai_rejection_and_narrative_bounds() {
        let condition = machine(CriterionComparison::AtLeast);
        assert_eq!(condition.metric().as_str(), "revenue");
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
                CriterionAssessment::Observed(super::final_review_tests::observation(
                    11, "revenue",
                )),
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
    fn tool() -> EvidenceRef {
        EvidenceRef::Tool(ToolReference::try_new("tool:source/1").expect("도구"))
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
    fn tool_references_never_promote_web_evidence_and_url_sources_do_not_conflict() {
        assert_eq!(
            Evidence::try_new(vec![tool()]).expect("기록").disposition(),
            ChangeDisposition::NeedsReview
        );
        assert_eq!(
            Evidence::try_new(vec![web_ref(0), tool()])
                .expect("웹과 도구")
                .disposition(),
            ChangeDisposition::NeedsReview
        );
        assert_eq!(
            Evidence::try_new(vec![tool(), tool()]),
            Err(ThesisError::DuplicateSource)
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
        assert_eq!(tool().disclosure_key(), None);
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
                    CriterionAssessment::Observed(super::final_review_tests::observation(
                        11, "revenue"
                    ))
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

    pub(super) fn fill_history(
        thesis: &mut Thesis,
        target: ThesisVersion,
        claim: &ClaimId,
        verdict: ClaimVerdict,
    ) {
        while thesis.version() < target {
            let before = thesis.version();
            let count = target
                .as_usize()
                .abs_diff(*thesis.version().as_usize())
                .min(16);
            let changes = (0..count)
                .map(|_| {
                    change(
                        ThesisOperation::Judge(claim.clone(), verdict),
                        TestSource::News,
                    )
                })
                .collect();
            thesis
                .apply_batch(
                    thesis.version(),
                    ThesisPatch::try_new(changes).expect("채움 묶음"),
                )
                .expect("이력 채움");
            assert_eq!(
                *thesis.version().as_usize(),
                before.as_usize().saturating_add(count)
            );
        }
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
        fill_history(
            &mut thesis,
            ThesisVersion::try_new(6135).expect("모델 상한 전"),
            &id("a"),
            ClaimVerdict::Intact,
        );
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
            Err(ThesisError::UserHistoryReserved)
        );
        assert_eq!(thesis, before);
    }

    fn resolve_all_at_model_limit(thesis: &mut Thesis) {
        let pending = thesis
            .pending()
            .into_iter()
            .map(super::HistoryEntry::sequence)
            .collect::<Vec<_>>();
        for target in pending {
            thesis
                .reject_by_user(thesis.version(), target, user())
                .expect("전용 공간 사용자 거절");
        }
        assert_eq!(
            thesis.version(),
            ThesisVersion::try_new(6144).expect("전체 상한")
        );
        assert_eq!(
            add(
                thesis,
                ThesisOperation::Judge(id("root"), ClaimVerdict::Broken),
                TestSource::News
            ),
            Err(ThesisError::HistoryFull)
        );
        assert_eq!(
            thesis.reject_by_user(
                thesis.version(),
                ThesisVersion::try_new(1).expect("순번"),
                user()
            ),
            Err(ThesisError::HistoryFull)
        );
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
        add(
            &mut thesis,
            ThesisOperation::AddClaim(claim("root", ClaimRole::Core)),
            TestSource::News,
        )
        .expect("현재 주장");
        fill_history(
            &mut thesis,
            ThesisVersion::try_new(6112).expect("웹 예약 경계"),
            &id("root"),
            ClaimVerdict::Intact,
        );
        let before = thesis.snapshot();
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::Judge(id("root"), ClaimVerdict::Intact),
                TestSource::Web
            ),
            Err(ThesisError::WebHistoryFull)
        );
        assert_eq!(thesis, before);
        add(
            &mut thesis,
            ThesisOperation::Judge(id("root"), ClaimVerdict::Broken),
            TestSource::News,
        )
        .expect("비웹 예약 공간");
        assert_eq!(thesis.status(), ThesisStatus::SuspectedBroken);
        fill_history(
            &mut thesis,
            ThesisVersion::try_new(6136).expect("사용자 예약 경계"),
            &id("root"),
            ClaimVerdict::Broken,
        );
        resolve_all_at_model_limit(&mut thesis);
        assert_eq!(replay_batched(&thesis), thesis);
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
#[cfg(test)]
mod final_review_tests {
    use super::tests::{TestSource, add, change, claim, id, reason, text, user};
    use super::{
        ChangeDisposition, ClaimRole, ClaimVerdict, CriterionAssessment, CriterionComparison,
        CriterionKind, CriterionMetric, CriterionObservation, CriterionValue, CriterionVerdict,
        Evidence, EvidenceRef, EvidenceUrl, HistoryAction, KillCriterion, MachineCondition, Thesis,
        ThesisError, ThesisOperation, ThesisPatch, ThesisVersion, ToolReference,
    };
    use crate::{NewsSource, NormalizedUrl};
    use rust_decimal::Decimal;

    fn metric(value: &str) -> CriterionMetric {
        CriterionMetric::try_new(value).expect("지표")
    }
    pub(super) fn observation(value: i32, name: &str) -> CriterionObservation {
        CriterionObservation::new(
            metric(name),
            CriterionValue::new(Decimal::from(value)),
            ToolReference::try_new("tool:fundamentals/call-1").expect("도구 참조"),
        )
    }
    pub(super) fn machine() -> MachineCondition {
        MachineCondition::new(
            metric("revenue"),
            CriterionComparison::AtLeast,
            CriterionValue::new(Decimal::from(10_i32)),
        )
    }
    fn tool_evidence() -> Evidence {
        Evidence::try_new(vec![EvidenceRef::Tool(
            ToolReference::try_new("tool:fundamentals/call-1").expect("도구"),
        )])
        .expect("도구 기록")
    }

    #[test]
    fn metric_ids_are_bounded_and_preserve_the_canonical_identity() {
        for value in ["", " ", "metric/name", "metric:call", "지표", "m\n"] {
            assert_eq!(
                CriterionMetric::try_new(value),
                Err(ThesisError::InvalidMetric)
            );
        }
        assert_eq!(
            CriterionMetric::try_new(&"a".repeat(64))
                .expect("상한")
                .as_str(),
            "a".repeat(64)
        );
        assert_eq!(
            CriterionMetric::try_new(&"a".repeat(65)),
            Err(ThesisError::InvalidMetric)
        );
        assert_eq!(
            CriterionMetric::try_new("revenue.growth_1-x")
                .expect("식별자")
                .as_str(),
            "revenue.growth_1-x"
        );
    }
    #[test]
    fn machine_parameters_and_observations_round_trip_through_public_accessors() {
        let condition = machine();
        assert_eq!(condition.comparison(), CriterionComparison::AtLeast);
        assert_eq!(condition.threshold().as_decimal(), Decimal::from(10_i32));
        assert_eq!(condition.metric().as_str(), "revenue");
        let restored = MachineCondition::new(
            CriterionMetric::try_new(condition.metric().as_str()).expect("지표"),
            condition.comparison(),
            CriterionValue::new(condition.threshold().as_decimal()),
        );
        assert_eq!(restored, condition);
        let observed = observation(11, "revenue");
        assert_eq!(observed.metric().as_str(), "revenue");
        assert_eq!(observed.value().as_decimal(), Decimal::from(11_i32));
        assert_eq!(observed.source().as_str(), "tool:fundamentals/call-1");
        let mut thesis = Thesis::new();
        add(
            &mut thesis,
            ThesisOperation::AddKillCriterion(
                KillCriterion::new(id("machine"), CriterionKind::Machine(restored)).expect("조건"),
            ),
            TestSource::News,
        )
        .expect("추가");
        let op =
            ThesisOperation::JudgeCriterion(id("machine"), CriterionAssessment::Observed(observed));
        thesis
            .apply_batch(
                thesis.version(),
                ThesisPatch::try_new(vec![super::ProposedChange::new(
                    op.clone(),
                    tool_evidence(),
                    reason(),
                )])
                .expect("차분"),
            )
            .expect("코드 관측");
        assert_eq!(thesis.kill_criteria()[0].verdict(), CriterionVerdict::Met);
        assert_eq!(
            thesis.history().last().expect("이력").action(),
            &HistoryAction::Change(Box::new(super::ProposedChange::new(
                op,
                tool_evidence(),
                reason()
            )))
        );
        assert_eq!(super::tests::replay(&thesis), thesis);
    }

    #[test]
    fn metric_mismatch_and_both_wrong_judgment_kinds_are_rejected_atomically() {
        let mut thesis = Thesis::new();
        add(
            &mut thesis,
            ThesisOperation::AddKillCriterion(
                KillCriterion::new(id("m"), CriterionKind::Machine(machine())).expect("조건"),
            ),
            TestSource::News,
        )
        .expect("추가");
        add(
            &mut thesis,
            ThesisOperation::AddKillCriterion(
                KillCriterion::new(id("n"), CriterionKind::Narrative(text("인허가 취소")))
                    .expect("조건"),
            ),
            TestSource::News,
        )
        .expect("추가");
        let before = thesis.snapshot();
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::JudgeCriterion(
                    id("m"),
                    CriterionAssessment::Observed(observation(-999, "other"))
                ),
                TestSource::News
            ),
            Err(ThesisError::MetricMismatch)
        );
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::JudgeCriterion(
                    id("m"),
                    CriterionAssessment::Ai(CriterionVerdict::Met)
                ),
                TestSource::News
            ),
            Err(ThesisError::AiMachineJudgment)
        );
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::JudgeCriterion(
                    id("n"),
                    CriterionAssessment::Observed(observation(11, "revenue"))
                ),
                TestSource::News
            ),
            Err(ThesisError::CodeNarrativeJudgment)
        );
        assert_eq!(thesis, before);
    }

    #[test]
    fn tool_labels_cannot_promote_web_evidence_and_code_observations_are_separate() {
        let url =
            EvidenceUrl::try_new(NormalizedUrl::try_new("https://example.com/a").expect("URL"))
                .expect("길이");
        let tool = EvidenceRef::Tool(ToolReference::try_new("tool:get_quote/1").expect("도구"));
        for refs in [
            vec![tool.clone()],
            vec![EvidenceRef::Web(url.clone()), tool.clone()],
        ] {
            assert_eq!(
                Evidence::try_new(refs).expect("기록").disposition(),
                ChangeDisposition::NeedsReview
            );
        }
        assert_eq!(
            Evidence::try_new(vec![tool, EvidenceRef::News(NewsSource::Hankyung, url)])
                .expect("뉴스 근거")
                .disposition(),
            ChangeDisposition::Applied
        );
        assert_eq!(Evidence::try_new(vec![]), Err(ThesisError::EmptyEvidence));
    }

    fn model_limit_with_pending() -> Thesis {
        let mut thesis = Thesis::new();
        for index in 0_usize..8 {
            add(
                &mut thesis,
                ThesisOperation::AddClaim(claim(&format!("p{index}"), ClaimRole::Core)),
                TestSource::Web,
            )
            .expect("보류");
        }
        add(
            &mut thesis,
            ThesisOperation::AddClaim(claim("root", ClaimRole::Core)),
            TestSource::News,
        )
        .expect("기준 주장");
        while *thesis.version().as_usize() < 6136 {
            let before = thesis.version();
            let count = 6136_usize.abs_diff(*thesis.version().as_usize()).min(16);
            let changes = (0..count)
                .map(|_| {
                    change(
                        ThesisOperation::Judge(id("root"), ClaimVerdict::Intact),
                        TestSource::News,
                    )
                })
                .collect();
            thesis
                .apply_batch(
                    thesis.version(),
                    ThesisPatch::try_new(changes).expect("차분"),
                )
                .expect("모델 영역");
            assert_eq!(
                *thesis.version().as_usize(),
                before.as_usize().saturating_add(count)
            );
        }
        thesis
    }
    #[test]
    fn model_changes_cannot_consume_the_eight_user_only_slots() {
        let mut thesis = model_limit_with_pending();
        let before = thesis.snapshot();
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::Judge(id("root"), ClaimVerdict::Broken),
                TestSource::News
            ),
            Err(ThesisError::UserHistoryReserved)
        );
        assert_eq!(thesis, before);
        let pending = thesis
            .pending()
            .into_iter()
            .map(super::HistoryEntry::sequence)
            .collect::<Vec<_>>();
        for (index, target) in pending.into_iter().enumerate() {
            if index == 0 {
                thesis
                    .confirm_by_user(thesis.version(), target, user())
                    .expect("사용자 확인");
            } else {
                thesis
                    .reject_by_user(thesis.version(), target, user())
                    .expect("사용자 거절");
            }
        }
        assert_eq!(
            thesis.version(),
            ThesisVersion::try_new(6144).expect("전체 상한")
        );
        assert_eq!(thesis.pending().len(), 0);
        assert_eq!(
            add(
                &mut thesis,
                ThesisOperation::Judge(id("root"), ClaimVerdict::Broken),
                TestSource::News
            ),
            Err(ThesisError::HistoryFull)
        );
    }
}
#[cfg(test)]
mod state_property_tests {
    use super::tests::{TestSource, add, change, claim, id, text, user};
    use super::{
        ClaimRole, ClaimVerdict, CriterionAssessment, CriterionComparison, CriterionKind,
        CriterionMetric, CriterionObservation, CriterionValue, CriterionVerdict, KillCriterion,
        MachineCondition, ProposedChange, Thesis, ThesisError, ThesisOperation, ThesisPatch,
        ThesisVersion, UserDecision,
    };
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    fn observed(value: i32, metric: &str) -> CriterionObservation {
        CriterionObservation::new(
            CriterionMetric::try_new(metric).expect("지표"),
            CriterionValue::new(Decimal::from(value)),
            super::ToolReference::try_new("tool:fundamentals/observation-2").expect("도구"),
        )
    }
    pub(super) fn rebuild_change(change: &ProposedChange) -> ProposedChange {
        let operation = match change.operation() {
            ThesisOperation::AddClaim(original) => ThesisOperation::AddClaim(super::Claim::new(
                id(original.id().as_str()),
                original.role(),
                super::ClaimContent::try_new(
                    original.statement().clone(),
                    original.metric().clone(),
                    original.expected().clone(),
                    original.observe_via().clone(),
                    original.refutation().clone(),
                )
                .expect("주장 복원"),
            )),
            ThesisOperation::Judge(target, verdict) => {
                ThesisOperation::Judge(id(target.as_str()), *verdict)
            }
            ThesisOperation::AddKillCriterion(original) => ThesisOperation::AddKillCriterion(
                KillCriterion::new(id(original.id().as_str()), rebuild_kind(original.kind()))
                    .expect("조건 복원"),
            ),
            ThesisOperation::JudgeCriterion(target, assessment) => {
                ThesisOperation::JudgeCriterion(id(target.as_str()), rebuild_assessment(assessment))
            }
        };
        let original = change.reason();
        let reason = super::ChangeReason::try_new(
            original.reason().clone(),
            original.model().clone(),
            original.prompt_version().clone(),
        )
        .expect("메타데이터 복원");
        ProposedChange::new(
            operation,
            super::Evidence::try_new(change.evidence().as_items().to_vec()).expect("근거 키"),
            reason,
        )
    }
    fn rebuild_kind(original: &CriterionKind) -> CriterionKind {
        match original {
            CriterionKind::Narrative(value) => CriterionKind::Narrative(value.clone()),
            CriterionKind::Machine(condition) => CriterionKind::Machine(MachineCondition::new(
                CriterionMetric::try_new(condition.metric().as_str()).expect("지표 복원"),
                condition.comparison(),
                CriterionValue::new(condition.threshold().as_decimal()),
            )),
        }
    }
    fn rebuild_assessment(original: &CriterionAssessment) -> CriterionAssessment {
        match original {
            CriterionAssessment::Ai(verdict) => CriterionAssessment::Ai(*verdict),
            CriterionAssessment::Observed(value) => {
                CriterionAssessment::Observed(CriterionObservation::new(
                    CriterionMetric::try_new(value.metric().as_str()).expect("지표 복원"),
                    CriterionValue::new(value.value().as_decimal()),
                    super::ToolReference::try_new(value.source().as_str()).expect("출처 복원"),
                ))
            }
        }
    }
    fn initial() -> Thesis {
        let mut thesis = Thesis::new();
        let condition = MachineCondition::new(
            CriterionMetric::try_new("revenue").expect("지표"),
            CriterionComparison::AtLeast,
            CriterionValue::new(Decimal::from(10_i32)),
        );
        let operations = vec![
            ThesisOperation::AddClaim(claim("root", ClaimRole::Core)),
            ThesisOperation::AddKillCriterion(
                KillCriterion::new(id("machine"), CriterionKind::Machine(condition))
                    .expect("코드형"),
            ),
            ThesisOperation::AddKillCriterion(
                KillCriterion::new(
                    id("narrative"),
                    CriterionKind::Narrative(text("인허가 취소")),
                )
                .expect("서술형"),
            ),
        ];
        let changes = operations
            .into_iter()
            .map(|op| change(op, TestSource::News))
            .collect();
        thesis
            .apply_batch(
                thesis.version(),
                ThesisPatch::try_new(changes).expect("초기 묶음"),
            )
            .expect("초기 상태");
        thesis
    }
    fn claim_verdict(value: u8) -> ClaimVerdict {
        match value % 4 {
            0 => ClaimVerdict::Intact,
            1 => ClaimVerdict::Weakened,
            2 => ClaimVerdict::Broken,
            _ => ClaimVerdict::Unverifiable,
        }
    }
    fn condition_verdict(value: u8) -> CriterionVerdict {
        match value % 3 {
            0 => CriterionVerdict::Met,
            1 => CriterionVerdict::NotMet,
            _ => CriterionVerdict::Unverifiable,
        }
    }
    fn make_change(seed: (u8, u8, i16, u8)) -> ProposedChange {
        let (kind, key, value, flag) = seed;
        let target = id(&format!("c{key}"));
        let operation = match kind % 7 {
            0 => ThesisOperation::AddClaim(claim(
                target.as_str(),
                if flag.is_multiple_of(2) {
                    ClaimRole::Core
                } else {
                    ClaimRole::Supporting
                },
            )),
            1 => ThesisOperation::AddKillCriterion(
                KillCriterion::new(
                    target,
                    CriterionKind::Machine(super::final_review_tests::machine()),
                )
                .expect("코드형 조건"),
            ),
            2 => ThesisOperation::AddKillCriterion(
                KillCriterion::new(target, CriterionKind::Narrative(text("새 공시 위험")))
                    .expect("서술형 조건"),
            ),
            3 => ThesisOperation::Judge(
                if flag.is_multiple_of(2) {
                    id("root")
                } else {
                    target
                },
                claim_verdict(flag),
            ),
            4 => ThesisOperation::JudgeCriterion(
                if flag.is_multiple_of(2) {
                    id("narrative")
                } else {
                    target
                },
                CriterionAssessment::Ai(condition_verdict(flag)),
            ),
            5 => ThesisOperation::JudgeCriterion(
                if flag.is_multiple_of(2) {
                    id("machine")
                } else {
                    target
                },
                CriterionAssessment::Observed(observed(i32::from(value), "revenue")),
            ),
            _ => ThesisOperation::JudgeCriterion(
                id("machine"),
                CriterionAssessment::Observed(observed(i32::from(value), "wrong_metric")),
            ),
        };
        change(
            operation,
            if flag.is_multiple_of(2) {
                TestSource::Web
            } else {
                TestSource::News
            },
        )
    }
    fn check_invariants(before: &Thesis, after: &Thesis, result: &Result<(), ThesisError>) {
        assert_eq!(&after.history()[..before.history().len()], before.history());
        if result.is_err() {
            assert_eq!(after, before);
        }
        let restored = super::tests::replay(after);
        assert_eq!(&restored, after);
        assert_eq!(restored.pending(), after.pending());
        assert_eq!(&super::tests::replay_batched(after), after);
        let applied = after
            .history()
            .iter()
            .skip(before.history().len())
            .any(|entry| entry.disposition() == super::ChangeDisposition::Applied);
        if !applied {
            assert_eq!(after.status(), before.status());
        }
    }
    fn batch(thesis: &mut Thesis, seeds: Vec<(u8, u8, i16, u8)>, version_choice: u8) {
        let before = thesis.snapshot();
        let base = if version_choice.is_multiple_of(2) {
            thesis.version()
        } else {
            ThesisVersion::try_new(thesis.version().as_usize().saturating_add(1))
                .expect("틀린 버전")
        };
        let changes = seeds.into_iter().map(make_change).collect();
        let result = thesis.apply_batch(base, ThesisPatch::try_new(changes).expect("1~16개 차분"));
        if !version_choice.is_multiple_of(2) {
            assert_eq!(result, Err(ThesisError::VersionConflict));
        }
        check_invariants(&before, thesis, &result);
    }
    fn injected_failure(thesis: &mut Thesis, length: usize, failure: usize, value: u8) {
        let before = thesis.snapshot();
        let changes = (0..length)
            .map(|index| {
                change(
                    ThesisOperation::Judge(
                        id(if index == failure { "missing" } else { "root" }),
                        claim_verdict(value),
                    ),
                    TestSource::News,
                )
            })
            .collect();
        let result = thesis.apply_batch(
            thesis.version(),
            ThesisPatch::try_new(changes).expect("실패 위치 차분"),
        );
        assert_eq!(result, Err(ThesisError::UnknownTarget));
        check_invariants(&before, thesis, &result);
    }
    fn actual_pending(thesis: &Thesis, selector: u8) -> ThesisVersion {
        let pending = thesis.pending();
        if pending.is_empty() {
            return ThesisVersion::try_new(0).expect("없는 순번");
        }
        pending[usize::from(selector)
            .checked_rem(pending.len())
            .expect("보류 존재")]
        .sequence()
    }
    fn decide(thesis: &mut Thesis, selector: u8, decision: UserDecision, version_choice: u8) {
        let before = thesis.snapshot();
        let target = actual_pending(thesis, selector);
        let base = if version_choice.is_multiple_of(2) {
            thesis.version()
        } else {
            ThesisVersion::try_new(thesis.version().as_usize().saturating_add(1))
                .expect("틀린 버전")
        };
        let result = match decision {
            UserDecision::Confirm => thesis.confirm_by_user(base, target, user()),
            UserDecision::Reject => thesis.reject_by_user(base, target, user()),
        };
        if !version_choice.is_multiple_of(2) {
            assert_eq!(result, Err(ThesisError::VersionConflict));
        }
        check_invariants(&before, thesis, &result);
    }
    fn superseded(thesis: &mut Thesis, value: u8) {
        let targets = thesis
            .pending()
            .into_iter()
            .map(super::HistoryEntry::sequence)
            .collect::<Vec<_>>();
        for target in targets {
            thesis
                .reject_by_user(thesis.version(), target, user())
                .expect("기존 보류 정리");
        }
        add(
            thesis,
            ThesisOperation::Judge(id("root"), claim_verdict(value)),
            TestSource::Web,
        )
        .expect("새 보류");
        let target = thesis.version();
        add(
            thesis,
            ThesisOperation::Judge(id("root"), ClaimVerdict::Intact),
            TestSource::News,
        )
        .expect("보류 대체");
        let before = thesis.snapshot();
        let result = thesis.confirm_by_user(thesis.version(), target, user());
        assert_eq!(result, Err(ThesisError::NotPending));
        check_invariants(&before, thesis, &result);
    }
    #[test]
    fn generated_batches_observations_decisions_rollback_and_supersession() {
        let seed = (0_u8..7, 0_u8..8, -20_i16..21, 0_u8..8);
        let step = (0_u8..6, 0_u8..16, proptest::collection::vec(seed, 1..17));
        let strategy = (
            2_usize..17,
            0_usize..16,
            0_u8..8,
            proptest::collection::vec(step, 1..25),
        );
        TestRunner::new(Config::with_cases(128))
            .run(&strategy, |(length, index, value, steps)| {
                let mut thesis = initial();
                injected_failure(
                    &mut thesis,
                    length,
                    1_usize.saturating_add(
                        index
                            .checked_rem(length.saturating_sub(1))
                            .expect("길이 2 이상"),
                    ),
                    value,
                );
                superseded(&mut thesis, value);
                for (mode, selector, seeds) in steps {
                    run_step(&mut thesis, mode, selector, seeds);
                }
                Ok(())
            })
            .expect("혼합 묶음·관측·결정 속성");
    }
    fn run_step(thesis: &mut Thesis, mode: u8, selector: u8, seeds: Vec<(u8, u8, i16, u8)>) {
        match mode {
            0 | 1 => batch(thesis, seeds, mode),
            2 => injected_failure(
                thesis,
                seeds.len(),
                usize::from(selector)
                    .checked_rem(seeds.len())
                    .expect("묶음 존재"),
                selector,
            ),
            3 => decide(thesis, selector, UserDecision::Confirm, selector),
            4 => decide(thesis, selector, UserDecision::Reject, selector),
            _ => superseded(thesis, selector),
        }
    }
}
