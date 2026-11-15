use domain::{Draft, OrderIntent, Proposal, ProposalId, Timestamp};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftRecord {
    pub id: ProposalId,
    pub intent: OrderIntent,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
}
impl DraftRecord {
    #[must_use]
    pub fn from_draft(proposal: &Proposal<Draft>) -> Self {
        Self {
            id: proposal.id().clone(),
            intent: proposal.intent().clone(),
            created_at: proposal.created_at(),
            expires_at: proposal.expires_at(),
        }
    }
}
