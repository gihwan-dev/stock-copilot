use super::ThesisError;
use crate::UntrustedText;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimRole {
    Core,
    Supporting,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimVerdict {
    Intact,
    Weakened,
    Broken,
    Unverifiable,
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
        require_text(&statement, TextField::Statement)?;
        require_text(&metric, TextField::Metric)?;
        require_text(&expected, TextField::Detail)?;
        require_text(&observe_via, TextField::Detail)?;
        require_text(&refutation, TextField::Reason)?;
        Ok(Self {
            statement,
            metric,
            expected,
            observe_via,
            refutation,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum TextField {
    Statement,
    Metric,
    Detail,
    Reason,
    Metadata,
}
pub(super) fn require_text(value: &UntrustedText, field: TextField) -> Result<(), ThesisError> {
    if value.as_untrusted_str().trim().is_empty() {
        return Err(ThesisError::EmptyText);
    }
    let limit = match field {
        TextField::Statement => 1024,
        TextField::Metric => 128,
        TextField::Detail => 256,
        TextField::Reason => 512,
        TextField::Metadata => 64,
    };
    if value.as_untrusted_str().len() > limit {
        return Err(ThesisError::TextTooLong);
    }
    Ok(())
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
        require_text(&reason, TextField::Reason)?;
        require_text(&model, TextField::Metadata)?;
        require_text(&prompt_version, TextField::Metadata)?;
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
pub struct UserReason(UntrustedText);
impl UserReason {
    pub fn try_new(value: UntrustedText) -> Result<Self, ThesisError> {
        require_text(&value, TextField::Reason)?;
        Ok(Self(value))
    }
    #[must_use]
    pub const fn as_untrusted_text(&self) -> &UntrustedText {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgedClaim {
    pub(super) claim: Claim,
    pub(super) verdict: ClaimVerdict,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CriterionKind {
    Machine(MachineCondition),
    Narrative(UntrustedText),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriterionVerdict {
    Met,
    NotMet,
    Unverifiable,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriterionAssessment {
    Code(CriterionVerdict),
    Ai(CriterionVerdict),
}
impl CriterionAssessment {
    pub(super) const fn verdict(self) -> CriterionVerdict {
        match self {
            Self::Code(value) | Self::Ai(value) => value,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillCriterion {
    id: ClaimId,
    kind: CriterionKind,
}
impl KillCriterion {
    pub fn new(id: ClaimId, kind: CriterionKind) -> Result<Self, ThesisError> {
        if let CriterionKind::Narrative(value) = &kind {
            require_text(value, TextField::Reason)?;
        }
        Ok(Self { id, kind })
    }
    #[must_use]
    pub const fn id(&self) -> &ClaimId {
        &self.id
    }
    #[must_use]
    pub const fn kind(&self) -> &CriterionKind {
        &self.kind
    }
    pub(super) fn validate_assessment(
        &self,
        assessment: CriterionAssessment,
    ) -> Result<(), ThesisError> {
        if let (CriterionKind::Machine(_), CriterionAssessment::Ai(_)) = (&self.kind, assessment) {
            return Err(ThesisError::AiMachineJudgment);
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JudgedCriterion {
    pub(super) criterion: KillCriterion,
    pub(super) verdict: CriterionVerdict,
}
impl JudgedCriterion {
    #[must_use]
    pub const fn criterion(&self) -> &KillCriterion {
        &self.criterion
    }
    #[must_use]
    pub const fn verdict(&self) -> CriterionVerdict {
        self.verdict
    }
}

use super::ToolReference;
use rust_decimal::Decimal;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CriterionValue(Decimal);
impl CriterionValue {
    #[must_use]
    pub fn new(value: Decimal) -> Self {
        Self(value.normalize())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CriterionComparison {
    AtLeast,
    AtMost,
    Equal,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCondition {
    metric: ToolReference,
    comparison: CriterionComparison,
    threshold: CriterionValue,
}
impl MachineCondition {
    #[must_use]
    pub const fn new(
        metric: ToolReference,
        comparison: CriterionComparison,
        threshold: CriterionValue,
    ) -> Self {
        Self {
            metric,
            comparison,
            threshold,
        }
    }
    #[must_use]
    pub const fn metric(&self) -> &ToolReference {
        &self.metric
    }
    #[must_use]
    pub fn evaluate(&self, observed: CriterionValue) -> CriterionAssessment {
        let met = match self.comparison {
            CriterionComparison::AtLeast => observed >= self.threshold,
            CriterionComparison::AtMost => observed <= self.threshold,
            CriterionComparison::Equal => observed == self.threshold,
        };
        CriterionAssessment::Code(if met {
            CriterionVerdict::Met
        } else {
            CriterionVerdict::NotMet
        })
    }
}
