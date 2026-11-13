use crate::ClientOrderId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProposalId(ClientOrderId);
impl ProposalId {
    #[must_use]
    pub fn new(value: ClientOrderId) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_client_order_id(&self) -> &ClientOrderId {
        &self.0
    }
}
