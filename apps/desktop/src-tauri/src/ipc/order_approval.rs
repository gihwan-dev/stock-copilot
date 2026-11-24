use domain::{ClientOrderId, ProposalId};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RequestOrderApprovalArgs {
    pub proposal_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderApprovalArgsError {
    Malformed,
    InvalidProposalId,
}

pub fn parse_order_approval_args(
    raw: serde_json::Value,
) -> Result<ProposalId, OrderApprovalArgsError> {
    let args: RequestOrderApprovalArgs =
        serde_json::from_value(raw).map_err(|_error| OrderApprovalArgsError::Malformed)?;
    ClientOrderId::try_new(&args.proposal_id)
        .map(ProposalId::new)
        .map_err(|_error| OrderApprovalArgsError::InvalidProposalId)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{OrderApprovalArgsError, parse_order_approval_args};

    #[test]
    fn only_the_proposal_id_is_accepted() {
        let parsed =
            parse_order_approval_args(json!({"proposalId": "proposal-1"})).expect("제안 ID");
        assert_eq!(parsed.as_client_order_id().as_str(), "proposal-1");
    }

    #[test]
    fn any_other_field_is_rejected_at_deserialization() {
        for extra in [
            json!({"proposalId": "proposal-1", "quantity": "10"}),
            json!({"proposalId": "proposal-1", "price": "70000"}),
            json!({"proposalId": "proposal-1", "side": "buy"}),
            json!({"proposalId": "proposal-1", "approved": true}),
            json!({"quantity": "10"}),
            json!({}),
            json!("proposal-1"),
        ] {
            assert_eq!(
                parse_order_approval_args(extra),
                Err(OrderApprovalArgsError::Malformed)
            );
        }
    }

    #[test]
    fn malformed_proposal_ids_are_rejected() {
        for bad in ["", "has space", "../../etc", &"a".repeat(37)] {
            assert_eq!(
                parse_order_approval_args(json!({"proposalId": bad})),
                Err(OrderApprovalArgsError::InvalidProposalId)
            );
        }
    }
}
