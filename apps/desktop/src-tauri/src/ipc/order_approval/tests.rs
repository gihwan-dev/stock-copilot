use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

use approval_command::execute::ApprovalCommandError;
use approval_command::execute::{ApprovalCommandOutcome, ApprovalCommandRejection};
use approval_command::prepare::PrepareRejection;
use approval_command::service::OrderApprovalHandler;
use domain::ProposalId;
use std::future::Future;
use std::pin::Pin;
use tauri::test::{mock_builder, mock_context, noop_assets};

use serde_json::json;

use super::{
    OrderApprovalArgsError, OrderApprovalReason, OrderApprovalRequest, OrderApprovalResponse,
    OrderApprovalRuntime, OrderApprovalStatus, parse_order_approval_args, request_order_approval,
};

struct RecordingHandler {
    calls: RwLock<Vec<ProposalId>>,
    outcome: ApprovalCommandOutcome,
}

impl RecordingHandler {
    fn new(outcome: ApprovalCommandOutcome) -> Self {
        Self {
            calls: RwLock::new(Vec::new()),
            outcome,
        }
    }

    fn calls(&self) -> Vec<ProposalId> {
        match self.calls.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

impl OrderApprovalHandler for RecordingHandler {
    fn request_order_approval<'a>(
        &'a self,
        proposal_id: &'a ProposalId,
    ) -> Pin<
        Box<dyn Future<Output = Result<ApprovalCommandOutcome, ApprovalCommandError>> + Send + 'a>,
    > {
        match self.calls.write() {
            Ok(mut guard) => guard.push(proposal_id.clone()),
            Err(poisoned) => poisoned.into_inner().push(proposal_id.clone()),
        }
        let outcome = self.outcome.clone();
        Box::pin(async move { Ok(outcome) })
    }
}

fn request(id: &str) -> OrderApprovalRequest {
    OrderApprovalRequest {
        proposal_id: String::from(id),
    }
}

#[test]
fn request_accepts_only_proposal_id() {
    assert_eq!(
        serde_json::from_str::<OrderApprovalRequest>(r#"{"proposal_id":"prop-0001"}"#)
            .expect("정상 요청"),
        request("prop-0001")
    );
    for input in [
        r#"{"proposal_id":"prop-0001","quantity":"10"}"#,
        r#"{"proposal_id":"prop-0001","price":"70000"}"#,
        r#"{"proposal_id":"prop-0001","approved":true}"#,
        r#"{"proposal_id":"prop-0001","side":"buy"}"#,
        r#"{"proposalId":"prop-0001"}"#,
        "{}",
    ] {
        assert!(
            serde_json::from_str::<OrderApprovalRequest>(input).is_err(),
            "거부돼야 할 입력: {input}"
        );
    }
}

#[test]
fn command_without_runtime_reports_unavailable() {
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let response = tauri::async_runtime::block_on(request_order_approval(
        app.handle().clone(),
        request("prop-0001"),
    ))
    .expect("응답");
    assert_eq!(
        response,
        OrderApprovalResponse {
            status: OrderApprovalStatus::Unavailable,
            reason: OrderApprovalReason::SwitchOff,
        }
    );
}

#[test]
fn command_passes_only_the_proposal_id_to_the_handler() {
    let handler = Arc::new(RecordingHandler::new(ApprovalCommandOutcome::Rejected(
        ApprovalCommandRejection::PresenceDenied,
    )));
    let shared: Arc<dyn OrderApprovalHandler> = Arc::<RecordingHandler>::clone(&handler);
    let app = mock_builder()
        .manage(OrderApprovalRuntime::new(shared))
        .build(mock_context(noop_assets()))
        .expect("mock app");
    let response = tauri::async_runtime::block_on(request_order_approval(
        app.handle().clone(),
        request("prop-0001"),
    ))
    .expect("응답");
    assert_eq!(
        response,
        OrderApprovalResponse {
            status: OrderApprovalStatus::Rejected,
            reason: OrderApprovalReason::PresenceDenied,
        }
    );
    assert_eq!(handler.calls().len(), 1);
    let path = Path::new("ipc/responses/request_order_approval.json");
    let mut actual = serde_json::to_string_pretty(&response).expect("직렬화");
    actual.push('\n');
    let recorded = fs::read_to_string(path).expect("기록된 응답");
    assert_eq!(recorded, actual, "응답 스냅샷 불일치");
}

#[test]
fn invalid_proposal_id_is_rejected_without_calling_the_handler() {
    let handler = Arc::new(RecordingHandler::new(ApprovalCommandOutcome::Rejected(
        ApprovalCommandRejection::Prepare(PrepareRejection::ProposalNotFound),
    )));
    let shared: Arc<dyn OrderApprovalHandler> = Arc::<RecordingHandler>::clone(&handler);
    let runtime = OrderApprovalRuntime::new(shared);
    let response = tauri::async_runtime::block_on(runtime.request(&request(""))).expect("응답");
    assert_eq!(response.status, OrderApprovalStatus::Rejected);
    assert_eq!(response.reason, OrderApprovalReason::ProposalUnavailable);
    assert_eq!(handler.calls(), Vec::<ProposalId>::new());
}

#[test]
fn rejections_map_to_reasons() {
    let cases = [
        (
            ApprovalCommandRejection::OrdersDisabled,
            OrderApprovalReason::SwitchOff,
        ),
        (
            ApprovalCommandRejection::Prepare(PrepareRejection::OrdersDisabled),
            OrderApprovalReason::SwitchOff,
        ),
        (
            ApprovalCommandRejection::Prepare(PrepareRejection::DialogInProgress),
            OrderApprovalReason::DialogInProgress,
        ),
        (
            ApprovalCommandRejection::AlreadyConsumed,
            OrderApprovalReason::AlreadyConsumed,
        ),
        (
            ApprovalCommandRejection::Prepare(PrepareRejection::MarketClosed),
            OrderApprovalReason::MarketClosed,
        ),
    ];
    for (rejection, reason) in cases {
        let mapped = super::map_outcome(&ApprovalCommandOutcome::Rejected(rejection));
        assert_eq!(mapped.status, OrderApprovalStatus::Rejected);
        assert_eq!(mapped.reason, reason);
    }
}

#[test]
fn only_the_proposal_id_is_accepted() {
    let parsed = parse_order_approval_args(json!({"proposalId": "proposal-1"})).expect("제안 ID");
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
