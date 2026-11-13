use crate::{CollectedItem, InformationSource, UntrustedText};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClaimId(String);

impl ClaimId {
    pub fn try_new(value: &str) -> Result<Self, ThesisError> {
        if value.is_empty()
            || value.len() > 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        {
            return Err(ThesisError::InvalidId);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ThesisVersion(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimRole {
    Core,
    Supporting,
    KillCriterion,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimVerdict {
    Intact,
    Weakened,
    Broken,
    Unverifiable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThesisStatus {
    SuspectedBroken,
    NeedsReview,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeDisposition {
    Applied,
    NeedsReview,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThesisError {
    InvalidId,
    EmptyText,
    EmptyEvidence,
    TooManySources,
    DuplicateSource,
    VersionConflict,
    HistoryFull,
    TooManyClaims,
    DuplicateClaim,
    UnknownClaim,
    NotPending,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    id: ClaimId,
    role: ClaimRole,
    statement: UntrustedText,
    metric: UntrustedText,
    expected: UntrustedText,
    observe_via: UntrustedText,
    refutation: UntrustedText,
}

impl Claim {
    #[must_use]
    pub fn new(id: ClaimId, role: ClaimRole, content: ClaimContent) -> Self {
        let ClaimContent {
            statement,
            metric,
            expected,
            observe_via,
            refutation,
        } = content;
        Self {
            id,
            role,
            statement,
            metric,
            expected,
            observe_via,
            refutation,
        }
    }
    #[must_use]
    pub const fn id(&self) -> &ClaimId {
        &self.id
    }
    #[must_use]
    pub const fn role(&self) -> ClaimRole {
        self.role
    }
    #[must_use]
    pub const fn statement(&self) -> &UntrustedText {
        &self.statement
    }
    #[must_use]
    pub const fn metric(&self) -> &UntrustedText {
        &self.metric
    }
    #[must_use]
    pub const fn expected(&self) -> &UntrustedText {
        &self.expected
    }
    #[must_use]
    pub const fn observe_via(&self) -> &UntrustedText {
        &self.observe_via
    }
    #[must_use]
    pub const fn refutation(&self) -> &UntrustedText {
        &self.refutation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimContent {
    statement: UntrustedText,
    metric: UntrustedText,
    expected: UntrustedText,
    observe_via: UntrustedText,
    refutation: UntrustedText,
}
impl ClaimContent {
    pub fn try_new(
        statement: UntrustedText,
        metric: UntrustedText,
        expected: UntrustedText,
        observe_via: UntrustedText,
        refutation: UntrustedText,
    ) -> Result<Self, ThesisError> {
        for value in [&statement, &metric, &expected, &observe_via, &refutation] {
            require_text(value)?;
        }
        Ok(Self {
            statement,
            metric,
            expected,
            observe_via,
            refutation,
        })
    }
}

fn require_text(value: &UntrustedText) -> Result<(), ThesisError> {
    if value.as_untrusted_str().trim().is_empty() {
        return Err(ThesisError::EmptyText);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence(Vec<CollectedItem>);
impl Evidence {
    pub fn try_new(items: Vec<CollectedItem>) -> Result<Self, ThesisError> {
        if items.is_empty() {
            return Err(ThesisError::EmptyEvidence);
        }
        if items.len() > 32 {
            return Err(ThesisError::TooManySources);
        }
        for (index, item) in items.iter().enumerate() {
            if items
                .iter()
                .take(index)
                .any(|previous| previous.source() == item.source() && previous.url() == item.url())
            {
                return Err(ThesisError::DuplicateSource);
            }
        }
        Ok(Self(items))
    }
    #[must_use]
    pub fn as_items(&self) -> &[CollectedItem] {
        &self.0
    }
    fn disposition(&self) -> ChangeDisposition {
        if self
            .0
            .iter()
            .all(|item| item.source() == &InformationSource::Web)
        {
            ChangeDisposition::NeedsReview
        } else {
            ChangeDisposition::Applied
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeReason {
    reason: UntrustedText,
    model: UntrustedText,
    prompt_version: UntrustedText,
}
impl ChangeReason {
    pub fn try_new(
        reason: UntrustedText,
        model: UntrustedText,
        prompt_version: UntrustedText,
    ) -> Result<Self, ThesisError> {
        for value in [&reason, &model, &prompt_version] {
            require_text(value)?;
        }
        Ok(Self {
            reason,
            model,
            prompt_version,
        })
    }
    #[must_use]
    pub const fn reason(&self) -> &UntrustedText {
        &self.reason
    }
    #[must_use]
    pub const fn model(&self) -> &UntrustedText {
        &self.model
    }
    #[must_use]
    pub const fn prompt_version(&self) -> &UntrustedText {
        &self.prompt_version
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThesisOperation {
    AddClaim(Claim),
    Judge(ClaimId, ClaimVerdict),
}
impl ThesisOperation {
    fn target(&self) -> &ClaimId {
        match self {
            Self::AddClaim(claim) => claim.id(),
            Self::Judge(id, _) => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    sequence: ThesisVersion,
    operation: ThesisOperation,
    evidence: Evidence,
    reason: ChangeReason,
    disposition: ChangeDisposition,
    confirms: Option<ThesisVersion>,
}
impl HistoryEntry {
    #[must_use]
    pub const fn sequence(&self) -> ThesisVersion {
        self.sequence
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
    #[must_use]
    pub const fn disposition(&self) -> ChangeDisposition {
        self.disposition
    }
    #[must_use]
    pub const fn confirms(&self) -> Option<ThesisVersion> {
        self.confirms
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgedClaim {
    claim: Claim,
    verdict: ClaimVerdict,
}
impl JudgedClaim {
    #[must_use]
    pub const fn claim(&self) -> &Claim {
        &self.claim
    }
    #[must_use]
    pub const fn verdict(&self) -> ClaimVerdict {
        self.verdict
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Thesis {
    history: Vec<HistoryEntry>,
    claims: Vec<JudgedClaim>,
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
    pub fn status(&self) -> ThesisStatus {
        if self.claims.iter().any(|entry| {
            entry.verdict == ClaimVerdict::Broken && entry.claim.role != ClaimRole::Supporting
        }) {
            ThesisStatus::SuspectedBroken
        } else {
            ThesisStatus::NeedsReview
        }
    }
    pub fn append(
        &mut self,
        base: ThesisVersion,
        operation: ThesisOperation,
        evidence: Evidence,
        reason: ChangeReason,
    ) -> Result<(), ThesisError> {
        self.check_base(base)?;
        self.validate(&operation)?;
        let disposition = evidence.disposition();
        self.record(HistoryEntry {
            sequence: ThesisVersion(self.history.len().saturating_add(1)),
            operation,
            evidence,
            reason,
            disposition,
            confirms: None,
        });
        Ok(())
    }
    pub fn confirm_by_user(
        &mut self,
        base: ThesisVersion,
        pending: ThesisVersion,
        reason: ChangeReason,
    ) -> Result<(), ThesisError> {
        self.check_base(base)?;
        let original = self
            .history
            .iter()
            .find(|entry| entry.sequence == pending)
            .ok_or(ThesisError::NotPending)?;
        if original.disposition != ChangeDisposition::NeedsReview
            || self
                .history
                .iter()
                .any(|entry| entry.confirms == Some(pending))
        {
            return Err(ThesisError::NotPending);
        }
        if self.history.iter().any(|entry| {
            entry.sequence > pending && entry.operation.target() == original.operation.target()
        }) {
            return Err(ThesisError::Superseded);
        }
        let operation = original.operation.clone();
        let evidence = original.evidence.clone();
        self.validate(&operation)?;
        self.record(HistoryEntry {
            sequence: ThesisVersion(self.history.len().saturating_add(1)),
            operation,
            evidence,
            reason,
            disposition: ChangeDisposition::Applied,
            confirms: Some(pending),
        });
        Ok(())
    }
    fn check_base(&self, base: ThesisVersion) -> Result<(), ThesisError> {
        if self.version() != base {
            return Err(ThesisError::VersionConflict);
        }
        if self.history.len() >= 4096 {
            return Err(ThesisError::HistoryFull);
        }
        Ok(())
    }
    fn validate(&self, operation: &ThesisOperation) -> Result<(), ThesisError> {
        let existing = self
            .claims
            .iter()
            .find(|entry| entry.claim.id() == operation.target());
        match operation {
            ThesisOperation::AddClaim(_) => {
                if existing.is_some() {
                    return Err(ThesisError::DuplicateClaim);
                }
                if self.claims.len() >= 128 {
                    return Err(ThesisError::TooManyClaims);
                }
            }
            ThesisOperation::Judge(_, _) => {
                if existing.is_none() {
                    return Err(ThesisError::UnknownClaim);
                }
            }
        }
        Ok(())
    }
    fn record(&mut self, entry: HistoryEntry) {
        if entry.disposition == ChangeDisposition::Applied {
            self.apply(&entry.operation);
        }
        self.history.push(entry);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ChangeDisposition, ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, ClaimVerdict,
        Evidence, Thesis, ThesisError, ThesisOperation, ThesisStatus,
    };
    use crate::{
        CollectedItem, InformationSource, NewsItem, NewsSource, NormalizedUrl, UntrustedText,
    };

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("비신뢰 문구")
    }

    fn claim(id: &str, role: ClaimRole) -> Claim {
        Claim::new(
            ClaimId::try_new(id).expect("ID"),
            role,
            ClaimContent::try_new(
                text("매출 성장"),
                text("매출 성장률"),
                text("10% 이상"),
                text("DART 실적"),
                text("10% 미만이면 반증"),
            )
            .expect("내용"),
        )
    }

    fn evidence(source: InformationSource) -> Evidence {
        let url = NormalizedUrl::try_new("https://example.com/a").expect("주소");
        let item = match source {
            InformationSource::Web => CollectedItem::from_web(url, text("자료"), None),
            InformationSource::News(news) => NewsItem::new(news, url, text("자료"), None)
                .collected()
                .clone(),
            InformationSource::Disclosure(disclosure) => CollectedItem::from_source(
                InformationSource::Disclosure(disclosure),
                url,
                text("자료"),
                None,
            ),
        };
        Evidence::try_new(vec![item]).expect("출처")
    }

    fn audit() -> ChangeReason {
        ChangeReason::try_new(text("새 증거"), text("model"), text("prompt-v1")).expect("변경 사유")
    }

    fn append(thesis: &mut Thesis, operation: ThesisOperation, source: InformationSource) {
        thesis
            .append(thesis.version(), operation, evidence(source), audit())
            .expect("추가");
    }

    fn check_pair(
        roles: [ClaimRole; 2],
        first: ClaimVerdict,
        second: ClaimVerdict,
        expected: ThesisStatus,
    ) {
        let mut thesis = Thesis::new();
        for (id, role, verdict) in [("a", roles[0], first), ("b", roles[1], second)] {
            append(
                &mut thesis,
                ThesisOperation::AddClaim(claim(id, role)),
                InformationSource::Disclosure(crate::DisclosureSource::Dart),
            );
            append(
                &mut thesis,
                ThesisOperation::Judge(ClaimId::try_new(id).expect("ID"), verdict),
                InformationSource::News(NewsSource::Hankyung),
            );
        }
        assert_eq!(thesis.status(), expected);
        append(
            &mut thesis,
            ThesisOperation::AddClaim(claim("support", ClaimRole::Supporting)),
            InformationSource::News(NewsSource::Hankyung),
        );
        for verdict in [
            ClaimVerdict::Intact,
            ClaimVerdict::Weakened,
            ClaimVerdict::Broken,
            ClaimVerdict::Unverifiable,
        ] {
            append(
                &mut thesis,
                ThesisOperation::Judge(ClaimId::try_new("support").expect("ID"), verdict),
                InformationSource::News(NewsSource::Hankyung),
            );
            assert_eq!(thesis.status(), expected);
        }
    }

    #[test]
    fn r12_aggregation_table_covers_every_verdict_combination_and_order() {
        let verdicts = [
            ClaimVerdict::Intact,
            ClaimVerdict::Weakened,
            ClaimVerdict::Broken,
            ClaimVerdict::Unverifiable,
        ];
        let expected = [
            [
                ThesisStatus::NeedsReview,
                ThesisStatus::NeedsReview,
                ThesisStatus::SuspectedBroken,
                ThesisStatus::NeedsReview,
            ],
            [
                ThesisStatus::NeedsReview,
                ThesisStatus::NeedsReview,
                ThesisStatus::SuspectedBroken,
                ThesisStatus::NeedsReview,
            ],
            [ThesisStatus::SuspectedBroken; 4],
            [
                ThesisStatus::NeedsReview,
                ThesisStatus::NeedsReview,
                ThesisStatus::SuspectedBroken,
                ThesisStatus::NeedsReview,
            ],
        ];
        for (row, first) in verdicts.iter().enumerate() {
            for (column, second) in verdicts.iter().enumerate() {
                check_pair(
                    [ClaimRole::Core, ClaimRole::Core],
                    *first,
                    *second,
                    expected[row][column],
                );
                check_pair(
                    [ClaimRole::Core, ClaimRole::KillCriterion],
                    *first,
                    *second,
                    expected[row][column],
                );
                check_pair(
                    [ClaimRole::KillCriterion, ClaimRole::Core],
                    *second,
                    *first,
                    expected[row][column],
                );
            }
        }
    }

    #[test]
    fn empty_and_supporting_only_theses_need_review_for_every_verdict() {
        let verdicts = [
            ClaimVerdict::Intact,
            ClaimVerdict::Weakened,
            ClaimVerdict::Broken,
            ClaimVerdict::Unverifiable,
        ];
        let mut thesis = Thesis::new();
        assert_eq!(thesis.status(), ThesisStatus::NeedsReview);
        append(
            &mut thesis,
            ThesisOperation::AddClaim(claim("s", ClaimRole::Supporting)),
            InformationSource::News(NewsSource::Hankyung),
        );
        for verdict in verdicts {
            append(
                &mut thesis,
                ThesisOperation::Judge(ClaimId::try_new("s").expect("ID"), verdict),
                InformationSource::News(NewsSource::Hankyung),
            );
            assert_eq!(thesis.status(), ThesisStatus::NeedsReview);
        }
    }

    #[test]
    fn web_only_changes_are_pending_and_never_enter_analysis_or_aggregation() {
        let mut thesis = Thesis::new();
        append(
            &mut thesis,
            ThesisOperation::AddClaim(claim("a", ClaimRole::Core)),
            InformationSource::Web,
        );
        assert_eq!(thesis.claims(), []);
        assert_eq!(
            thesis.history()[0].disposition(),
            ChangeDisposition::NeedsReview
        );
        let pending = thesis.version();
        let prefix = thesis.history().to_vec();
        thesis
            .confirm_by_user(thesis.version(), pending, audit())
            .expect("사용자 확인");
        assert_eq!(&thesis.history()[..prefix.len()], prefix);
        assert_eq!(
            thesis.history().last().expect("확인 이력").confirms(),
            Some(pending)
        );
        assert_eq!(thesis.claims().len(), 1);
        assert_eq!(thesis.claims()[0].verdict(), ClaimVerdict::Unverifiable);
        append(
            &mut thesis,
            ThesisOperation::Judge(ClaimId::try_new("a").expect("ID"), ClaimVerdict::Broken),
            InformationSource::Web,
        );
        assert_eq!(thesis.status(), ThesisStatus::NeedsReview);
        let pending_verdict = thesis.version();
        thesis
            .confirm_by_user(thesis.version(), pending_verdict, audit())
            .expect("사용자 확인");
        assert_eq!(thesis.status(), ThesisStatus::SuspectedBroken);
        append(
            &mut thesis,
            ThesisOperation::Judge(ClaimId::try_new("a").expect("ID"), ClaimVerdict::Intact),
            InformationSource::Web,
        );
        assert_eq!(thesis.status(), ThesisStatus::SuspectedBroken);
        assert_eq!(
            thesis.confirm_by_user(thesis.version(), pending_verdict, audit()),
            Err(ThesisError::NotPending)
        );
    }

    #[test]
    fn append_preserves_every_history_prefix_for_generated_operation_sequences() {
        for length in 0..64 {
            let mut thesis = Thesis::new();
            append(
                &mut thesis,
                ThesisOperation::AddClaim(claim("a", ClaimRole::Core)),
                InformationSource::News(NewsSource::Hankyung),
            );
            for step in 0..length {
                let before = thesis.history().to_vec();
                let verdict = [
                    ClaimVerdict::Intact,
                    ClaimVerdict::Broken,
                    ClaimVerdict::Weakened,
                    ClaimVerdict::Unverifiable,
                ][step % 4];
                append(
                    &mut thesis,
                    ThesisOperation::Judge(ClaimId::try_new("a").expect("ID"), verdict),
                    InformationSource::News(NewsSource::Hankyung),
                );
                assert_eq!(&thesis.history()[..before.len()], before);
                assert_eq!(thesis.history().len(), before.len() + 1);
                assert_eq!(
                    thesis.history().last().expect("이력").sequence(),
                    thesis.version()
                );
            }
        }
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::{
        ChangeDisposition, ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, ClaimVerdict,
        Evidence, Thesis, ThesisError, ThesisOperation, ThesisStatus,
    };
    use crate::{
        CollectedItem, DisclosureSource, InformationSource, NewsSource, NormalizedUrl,
        UntrustedText,
    };

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("문구")
    }
    fn claim(id: &str) -> Claim {
        Claim::new(
            ClaimId::try_new(id).expect("ID"),
            ClaimRole::Core,
            ClaimContent::try_new(
                text("statement"),
                text("metric"),
                text("expected"),
                text("observe"),
                text("refutation"),
            )
            .expect("내용"),
        )
    }
    fn reason() -> ChangeReason {
        ChangeReason::try_new(text("reason"), text("model"), text("prompt")).expect("사유")
    }
    fn item(index: usize, source: InformationSource) -> CollectedItem {
        CollectedItem::from_source(
            source,
            NormalizedUrl::try_new(&format!("https://example.com/{index}")).expect("주소"),
            text("자료"),
            None,
        )
    }
    fn evidence() -> Evidence {
        Evidence::try_new(vec![item(0, InformationSource::News(NewsSource::Hankyung))])
            .expect("근거")
    }

    #[test]
    fn constructors_bound_ids_texts_and_evidence_without_input_in_errors_or_debug() {
        for id in ["", " a", "a.", "한글", "a\n"] {
            assert_eq!(ClaimId::try_new(id), Err(ThesisError::InvalidId));
        }
        assert_eq!(
            ClaimId::try_new(&"a".repeat(65)),
            Err(ThesisError::InvalidId)
        );
        assert_eq!(
            ClaimId::try_new(&"a".repeat(64)).expect("상한").as_str(),
            "a".repeat(64)
        );
        assert_eq!(ClaimId::try_new("A_1-z").expect("문법").as_str(), "A_1-z");
        for field in 0..5 {
            let mut fields = (0_usize..5).map(|_| text("secret")).collect::<Vec<_>>();
            fields[field] = text(" \n");
            assert_eq!(
                ClaimContent::try_new(
                    fields[0].clone(),
                    fields[1].clone(),
                    fields[2].clone(),
                    fields[3].clone(),
                    fields[4].clone()
                ),
                Err(ThesisError::EmptyText)
            );
        }
        for fields in [
            ["", "model", "prompt"],
            ["reason", "", "prompt"],
            ["reason", "model", ""],
        ] {
            assert_eq!(
                ChangeReason::try_new(text(fields[0]), text(fields[1]), text(fields[2])),
                Err(ThesisError::EmptyText)
            );
        }
    }

    #[test]
    fn content_accessors_preserve_untrusted_values_and_redact_debug() {
        let created_claim = claim("a");
        assert_eq!(created_claim.statement(), &text("statement"));
        assert_eq!(created_claim.metric(), &text("metric"));
        assert_eq!(created_claim.expected(), &text("expected"));
        assert_eq!(created_claim.observe_via(), &text("observe"));
        assert_eq!(created_claim.refutation(), &text("refutation"));
        assert_eq!(created_claim.role(), ClaimRole::Core);
        let created_reason = reason();
        assert_eq!(created_reason.reason(), &text("reason"));
        assert_eq!(created_reason.model(), &text("model"));
        assert_eq!(created_reason.prompt_version(), &text("prompt"));
        assert!(!format!("{created_claim:?} {created_reason:?}").contains("\"statement\""));
        assert!(!format!("{created_claim:?} {created_reason:?}").contains("\"reason\""));
    }

    #[test]
    fn evidence_is_nonempty_bounded_and_classifies_mixed_sources() {
        assert_eq!(Evidence::try_new(vec![]), Err(ThesisError::EmptyEvidence));
        let sources = (0..32)
            .map(|index| item(index, InformationSource::Web))
            .collect::<Vec<_>>();
        assert_eq!(
            Evidence::try_new(sources.clone()).expect("상한").as_items(),
            sources
        );
        let mut overflow = sources;
        overflow.push(item(32, InformationSource::Web));
        assert_eq!(
            Evidence::try_new(overflow),
            Err(ThesisError::TooManySources)
        );
        assert_eq!(
            Evidence::try_new(vec![
                item(0, InformationSource::Web),
                item(0, InformationSource::Web)
            ]),
            Err(ThesisError::DuplicateSource)
        );
        for source in [
            InformationSource::News(NewsSource::Hankyung),
            InformationSource::Disclosure(DisclosureSource::Dart),
        ] {
            let mixed = Evidence::try_new(vec![item(0, InformationSource::Web), item(0, source)])
                .expect("혼합 출처");
            assert_eq!(mixed.disposition(), ChangeDisposition::Applied);
        }
    }

    #[test]
    fn failures_are_atomic_stale_versions_and_superseded_confirmation_are_rejected() {
        let mut thesis = Thesis::default();
        let zero = thesis.version();
        assert_eq!(zero.0, 0);
        assert_eq!(
            thesis.confirm_by_user(zero, zero, reason()),
            Err(ThesisError::NotPending)
        );
        let id = ClaimId::try_new("a").expect("ID");
        assert_eq!(
            thesis.append(
                zero,
                ThesisOperation::Judge(id, ClaimVerdict::Broken),
                evidence(),
                reason()
            ),
            Err(ThesisError::UnknownClaim)
        );
        thesis
            .append(
                zero,
                ThesisOperation::AddClaim(claim("a")),
                evidence(),
                reason(),
            )
            .expect("추가");
        assert_eq!(
            thesis.append(
                zero,
                ThesisOperation::AddClaim(claim("b")),
                evidence(),
                reason()
            ),
            Err(ThesisError::VersionConflict)
        );
        assert_eq!(
            thesis.confirm_by_user(zero, zero, reason()),
            Err(ThesisError::VersionConflict)
        );
        assert_eq!(
            thesis.append(
                thesis.version(),
                ThesisOperation::AddClaim(claim("a")),
                evidence(),
                reason()
            ),
            Err(ThesisError::DuplicateClaim)
        );
    }
    #[test]
    fn superseded_confirmation_is_atomic_and_preserves_audit_metadata() {
        let mut thesis = Thesis::new();
        let id = ClaimId::try_new("a").expect("ID");
        thesis
            .append(
                thesis.version(),
                ThesisOperation::AddClaim(claim("a")),
                evidence(),
                reason(),
            )
            .expect("추가");
        let pending_evidence =
            Evidence::try_new(vec![item(0, InformationSource::Web)]).expect("웹");
        thesis
            .append(
                thesis.version(),
                ThesisOperation::Judge(id.clone(), ClaimVerdict::Broken),
                pending_evidence,
                reason(),
            )
            .expect("보류");
        let pending = thesis.version();
        thesis
            .append(
                thesis.version(),
                ThesisOperation::Judge(id, ClaimVerdict::Intact),
                evidence(),
                reason(),
            )
            .expect("최신 판정");
        let before = thesis.history().to_vec();
        assert_eq!(
            thesis.confirm_by_user(thesis.version(), pending, reason()),
            Err(ThesisError::Superseded)
        );
        assert_eq!(thesis.history(), before);
        assert_eq!(thesis.status(), ThesisStatus::NeedsReview);
        assert_eq!(thesis.claims()[0].claim().id().as_str(), "a");
        let first = &thesis.history()[0];
        assert_eq!(first.operation(), &ThesisOperation::AddClaim(claim("a")));
        assert_eq!(first.evidence(), &evidence());
        assert_eq!(first.reason(), &reason());
        assert_eq!(first.confirms(), None);
        assert_eq!(
            thesis.confirm_by_user(thesis.version(), first.sequence(), reason()),
            Err(ThesisError::NotPending)
        );
    }

    #[test]
    fn pending_addition_cannot_exceed_claim_cap_when_confirmed() {
        let mut thesis = Thesis::new();
        thesis
            .append(
                thesis.version(),
                ThesisOperation::AddClaim(claim("pending")),
                Evidence::try_new(vec![item(0, InformationSource::Web)]).expect("근거"),
                reason(),
            )
            .expect("보류");
        let pending = thesis.version();
        for index in 0_usize..128 {
            thesis
                .append(
                    thesis.version(),
                    ThesisOperation::AddClaim(claim(&format!("c{index}"))),
                    evidence(),
                    reason(),
                )
                .expect("추가");
        }
        let before = thesis.history().to_vec();
        assert_eq!(
            thesis.confirm_by_user(thesis.version(), pending, reason()),
            Err(ThesisError::TooManyClaims)
        );
        assert_eq!(thesis.history(), before);
    }

    #[test]
    fn list_and_history_caps_allow_the_boundary_and_reject_without_mutation() {
        let mut thesis = Thesis::new();
        for index in 0_usize..128 {
            thesis
                .append(
                    thesis.version(),
                    ThesisOperation::AddClaim(claim(&format!("c{index}"))),
                    evidence(),
                    reason(),
                )
                .expect("주장 상한");
        }
        assert_eq!(thesis.claims().len(), 128);
        assert_eq!(
            thesis.append(
                thesis.version(),
                ThesisOperation::AddClaim(claim("overflow")),
                evidence(),
                reason()
            ),
            Err(ThesisError::TooManyClaims)
        );
        for _ in 128_usize..4096 {
            thesis
                .append(
                    thesis.version(),
                    ThesisOperation::Judge(
                        ClaimId::try_new("c0").expect("ID"),
                        ClaimVerdict::Intact,
                    ),
                    evidence(),
                    reason(),
                )
                .expect("이력 상한");
        }
        assert_eq!(thesis.version().0, 4096);
        let before = thesis.history().to_vec();
        assert_eq!(
            thesis.append(
                thesis.version(),
                ThesisOperation::Judge(ClaimId::try_new("c0").expect("ID"), ClaimVerdict::Broken),
                evidence(),
                reason()
            ),
            Err(ThesisError::HistoryFull)
        );
        assert_eq!(
            thesis.confirm_by_user(thesis.version(), thesis.version(), reason()),
            Err(ThesisError::HistoryFull)
        );
        assert_eq!(thesis.history(), before);
    }
}
