use domain::{Draft, DraftOrderIntent, Proposal, ProposalId, ProposalPriceRange, Timestamp};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftRecord {
    pub id: ProposalId,
    pub intent: DraftOrderIntent,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub range: ProposalPriceRange,
}
impl DraftRecord {
    #[must_use]
    pub fn from_draft(proposal: &Proposal<Draft>) -> Self {
        Self {
            id: proposal.id().clone(),
            intent: proposal.intent(),
            created_at: proposal.created_at(),
            expires_at: proposal.expires_at(),
            range: proposal.price_range().clone(),
        }
    }
}
