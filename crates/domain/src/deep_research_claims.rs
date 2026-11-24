use crate::{EvidenceBundle, EvidenceItem, EvidenceItemId};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResearchStance {
    Bull,
    Bear,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchClaimText(String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearchClaimError {
    EmptyText,
    TextTooLong,
    NoCitation,
    TooManyCitations,
    DuplicateCitation,
    NoClaims,
    TooManyClaims,
}

impl ResearchClaimText {
    pub fn try_new(value: &str) -> Result<Self, ResearchClaimError> {
        if value.trim().is_empty() {
            return Err(ResearchClaimError::EmptyText);
        }
        if value.chars().count() > 1000 {
            return Err(ResearchClaimError::TextTooLong);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchClaim {
    text: ResearchClaimText,
    cites: Vec<EvidenceItemId>,
}

impl ResearchClaim {
    pub fn try_new(
        text: ResearchClaimText,
        cites: Vec<EvidenceItemId>,
    ) -> Result<Self, ResearchClaimError> {
        if cites.is_empty() {
            return Err(ResearchClaimError::NoCitation);
        }
        if cites.len() > 8 {
            return Err(ResearchClaimError::TooManyCitations);
        }
        let mut seen = BTreeSet::new();
        if !cites.iter().all(|id| seen.insert(id)) {
            return Err(ResearchClaimError::DuplicateCitation);
        }
        Ok(Self { text, cites })
    }
    #[must_use]
    pub const fn text(&self) -> &ResearchClaimText {
        &self.text
    }
    #[must_use]
    pub fn cites(&self) -> &[EvidenceItemId] {
        &self.cites
    }
    #[must_use]
    pub fn cited_in(&self, ledger: &EvidenceBundle) -> CitedEvidence {
        let mut found = Vec::with_capacity(self.cites.len());
        for id in &self.cites {
            match ledger.get(id) {
                Some(item) => found.push(item.clone()),
                None => return CitedEvidence::Missing,
            }
        }
        CitedEvidence::Found(found)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchDraft {
    stance: ResearchStance,
    claims: Vec<ResearchClaim>,
}

impl ResearchDraft {
    pub fn try_new(
        stance: ResearchStance,
        claims: Vec<ResearchClaim>,
    ) -> Result<Self, ResearchClaimError> {
        if claims.is_empty() {
            return Err(ResearchClaimError::NoClaims);
        }
        if claims.len() > 12 {
            return Err(ResearchClaimError::TooManyClaims);
        }
        Ok(Self { stance, claims })
    }
    #[must_use]
    pub const fn stance(&self) -> ResearchStance {
        self.stance
    }
    #[must_use]
    pub fn claims(&self) -> &[ResearchClaim] {
        &self.claims
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimSupport {
    Supported,
    Unsupported,
    Contradicted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimCheckBasis {
    Verifier,
    MissingEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedClaim {
    stance: ResearchStance,
    claim: ResearchClaim,
    support: ClaimSupport,
    basis: ClaimCheckBasis,
}

impl VerifiedClaim {
    #[must_use]
    pub const fn missing_evidence(stance: ResearchStance, claim: ResearchClaim) -> Self {
        Self {
            stance,
            claim,
            support: ClaimSupport::Unsupported,
            basis: ClaimCheckBasis::MissingEvidence,
        }
    }
    #[must_use]
    pub const fn checked(
        stance: ResearchStance,
        claim: ResearchClaim,
        support: ClaimSupport,
    ) -> Self {
        Self {
            stance,
            claim,
            support,
            basis: ClaimCheckBasis::Verifier,
        }
    }
    #[must_use]
    pub const fn stance(&self) -> ResearchStance {
        self.stance
    }
    #[must_use]
    pub const fn claim(&self) -> &ResearchClaim {
        &self.claim
    }
    #[must_use]
    pub const fn support(&self) -> ClaimSupport {
        self.support
    }
    #[must_use]
    pub const fn basis(&self) -> ClaimCheckBasis {
        self.basis
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitedEvidence {
    Found(Vec<EvidenceItem>),
    Missing,
}

#[cfg(test)]
mod tests {
    use super::{
        CitedEvidence, ClaimCheckBasis, ClaimSupport, ResearchClaim, ResearchClaimError,
        ResearchClaimText, ResearchDraft, ResearchStance, VerifiedClaim,
    };
    use crate::{
        EvidenceBundle, EvidenceItem, EvidenceItemId, EvidenceKind, InstrumentId, Nanosecond,
        OffsetSeconds, Timestamp, UnixSeconds, UntrustedText,
    };

    fn id(value: &str) -> EvidenceItemId {
        EvidenceItemId::try_new(value).expect("ID")
    }
    fn claim(cites: &[&str]) -> ResearchClaim {
        ResearchClaim::try_new(
            ResearchClaimText::try_new("주장").expect("문장"),
            cites.iter().map(|value| id(value)).collect(),
        )
        .expect("주장")
    }
    fn bundle() -> EvidenceBundle {
        let at = Timestamp::from_parts(
            UnixSeconds::new(1),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        EvidenceBundle::try_new(
            InstrumentId::try_new("A005930").expect("종목"),
            ["q1", "web-1"]
                .iter()
                .map(|value| {
                    EvidenceItem::new(
                        id(value),
                        EvidenceKind::Quote,
                        at,
                        UntrustedText::try_new(value).expect("내용"),
                    )
                })
                .collect(),
        )
        .expect("묶음")
    }

    #[test]
    fn claims_need_text_and_one_to_eight_distinct_citations() {
        assert_eq!(
            ResearchClaimText::try_new("  "),
            Err(ResearchClaimError::EmptyText)
        );
        assert_eq!(
            ResearchClaimText::try_new(&"가".repeat(1001)),
            Err(ResearchClaimError::TextTooLong)
        );
        let text = ResearchClaimText::try_new("가".repeat(1000).as_str()).expect("상한");
        assert_eq!(
            ResearchClaim::try_new(text.clone(), Vec::new()),
            Err(ResearchClaimError::NoCitation)
        );
        assert_eq!(
            ResearchClaim::try_new(text.clone(), vec![id("q1"), id("q1")]),
            Err(ResearchClaimError::DuplicateCitation)
        );
        assert_eq!(
            ResearchClaim::try_new(
                text,
                (0_u8..9).map(|index| id(&format!("q{index}"))).collect()
            ),
            Err(ResearchClaimError::TooManyCitations)
        );
    }

    #[test]
    fn drafts_hold_one_to_twelve_claims_of_one_stance() {
        assert_eq!(
            ResearchDraft::try_new(ResearchStance::Bull, Vec::new()),
            Err(ResearchClaimError::NoClaims)
        );
        assert_eq!(
            ResearchDraft::try_new(ResearchStance::Bear, vec![claim(&["q1"]); 13]),
            Err(ResearchClaimError::TooManyClaims)
        );
        let draft =
            ResearchDraft::try_new(ResearchStance::Bear, vec![claim(&["q1"]); 12]).expect("초안");
        assert_eq!(draft.stance(), ResearchStance::Bear);
        assert_eq!(draft.claims().len(), 12);
    }

    #[test]
    fn citations_are_resolved_against_the_ledger_only() {
        let ledger = bundle();
        let CitedEvidence::Found(items) = claim(&["web-1", "q1"]).cited_in(&ledger) else {
            panic!("찾음");
        };
        assert_eq!(
            items
                .iter()
                .map(|item| item.id().as_str())
                .collect::<Vec<_>>(),
            vec!["web-1", "q1"]
        );
        assert_eq!(
            claim(&["q1", "x9"]).cited_in(&ledger),
            CitedEvidence::Missing
        );
        let missing = VerifiedClaim::missing_evidence(ResearchStance::Bull, claim(&["x9"]));
        assert_eq!(missing.support(), ClaimSupport::Unsupported);
        assert_eq!(missing.basis(), ClaimCheckBasis::MissingEvidence);
        let checked = VerifiedClaim::checked(
            ResearchStance::Bear,
            claim(&["q1"]),
            ClaimSupport::Contradicted,
        );
        assert_eq!(checked.stance(), ResearchStance::Bear);
        assert_eq!(checked.basis(), ClaimCheckBasis::Verifier);
        assert_eq!(checked.claim().cites(), &[id("q1")]);
    }
}
