use crate::{CitedEvidence, ClaimSupport, EvidenceBundle, EvidenceKind, Thesis, VerifiedClaim};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResearchCount(u16);

impl ResearchCount {
    #[must_use]
    pub fn of_support(verified: &[VerifiedClaim], wanted: ClaimSupport) -> Self {
        Self(
            verified
                .iter()
                .filter(|claim| claim.support() == wanted)
                .fold(0_u16, |total, _claim| total.saturating_add(1)),
        )
    }
    #[must_use]
    pub fn of_pending(thesis: &Thesis) -> Self {
        Self(
            thesis
                .pending()
                .iter()
                .fold(0_u16, |total, _entry| total.saturating_add(1)),
        )
    }
    #[must_use]
    pub const fn as_u16(&self) -> &u16 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResearchTally {
    pub supported: ResearchCount,
    pub unsupported: ResearchCount,
    pub contradicted: ResearchCount,
}

#[must_use]
pub fn tally_research_claims(verified: &[VerifiedClaim]) -> ResearchTally {
    let count = |wanted| ResearchCount::of_support(verified, wanted);
    ResearchTally {
        supported: count(ClaimSupport::Supported),
        unsupported: count(ClaimSupport::Unsupported),
        contradicted: count(ClaimSupport::Contradicted),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchSourceWarning {
    WebOnlySupport(VerifiedClaim),
}

#[must_use]
pub fn research_source_warnings(
    ledger: &EvidenceBundle,
    verified: &[VerifiedClaim],
) -> Vec<ResearchSourceWarning> {
    verified
        .iter()
        .filter(|claim| claim.support() == ClaimSupport::Supported)
        .filter(|claim| match claim.claim().cited_in(ledger) {
            CitedEvidence::Found(items) => {
                items.iter().all(|item| item.kind() == EvidenceKind::Web)
            }
            CitedEvidence::Missing => false,
        })
        .map(|claim| ResearchSourceWarning::WebOnlySupport(claim.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ResearchCount, ResearchSourceWarning, research_source_warnings, tally_research_claims,
    };
    use crate::{
        ClaimSupport, EvidenceBundle, EvidenceItem, EvidenceItemId, EvidenceKind, InstrumentId,
        Nanosecond, OffsetSeconds, ResearchClaim, ResearchClaimText, ResearchStance, Timestamp,
        UnixSeconds, UntrustedText, VerifiedClaim,
    };

    fn id(value: &str) -> EvidenceItemId {
        EvidenceItemId::try_new(value).expect("ID")
    }
    fn ledger() -> EvidenceBundle {
        let at = Timestamp::from_parts(
            UnixSeconds::new(1),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let item = |value: &str, kind| {
            EvidenceItem::new(
                id(value),
                kind,
                at,
                UntrustedText::try_new(value).expect("내용"),
            )
        };
        EvidenceBundle::try_new(
            InstrumentId::try_new("A005930").expect("종목"),
            vec![
                item("quote-1", EvidenceKind::Quote),
                item("web-1", EvidenceKind::Web),
                item("web-2", EvidenceKind::Web),
            ],
        )
        .expect("묶음")
    }
    fn verified(cites: &[&str], support: ClaimSupport) -> VerifiedClaim {
        VerifiedClaim::checked(
            ResearchStance::Bull,
            ResearchClaim::try_new(
                ResearchClaimText::try_new("주장").expect("문장"),
                cites.iter().map(|value| id(value)).collect(),
            )
            .expect("주장"),
            support,
        )
    }

    #[test]
    fn supported_claims_backed_only_by_web_get_a_source_warning() {
        let web_only = verified(&["web-1", "web-2"], ClaimSupport::Supported);
        let claims = vec![
            web_only.clone(),
            verified(&["web-1", "quote-1"], ClaimSupport::Supported),
            verified(&["web-1"], ClaimSupport::Contradicted),
            verified(&["web-1", "x-1"], ClaimSupport::Supported),
        ];
        assert_eq!(
            research_source_warnings(&ledger(), &claims),
            vec![ResearchSourceWarning::WebOnlySupport(web_only)]
        );
        let tally = tally_research_claims(&claims);
        assert_eq!(*tally.supported.as_u16(), 3);
        assert_eq!(*tally.unsupported.as_u16(), 0);
        assert_eq!(*tally.contradicted.as_u16(), 1);
        assert_eq!(
            ResearchCount::of_support(&claims, ClaimSupport::Contradicted),
            tally.contradicted
        );
        assert_eq!(
            *ResearchCount::of_pending(&crate::Thesis::new()).as_u16(),
            0
        );
    }
}
