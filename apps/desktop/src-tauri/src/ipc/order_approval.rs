use std::fmt;
use std::sync::Arc;

use application::ClassifiedError;
use approval_command::execute::{
    ApprovalCommandError, ApprovalCommandOutcome, ApprovalCommandRejection,
};
use approval_command::prepare::{PrepareError, PrepareRejection};
use approval_command::service::OrderApprovalHandler;
use domain::{ClientOrderId, ProposalId};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct OrderApprovalRequest {
    pub proposal_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum OrderApprovalStatus {
    #[serde(rename = "submitted")]
    Submitted,
    #[serde(rename = "rejected")]
    Rejected,
    #[serde(rename = "broker-rejected")]
    BrokerRejected,
    #[serde(rename = "outcome-unknown")]
    OutcomeUnknown,
    #[serde(rename = "unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum OrderApprovalReason {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "presence-denied")]
    PresenceDenied,
    #[serde(rename = "switch-off")]
    SwitchOff,
    #[serde(rename = "hourly-limit")]
    HourlyLimit,
    #[serde(rename = "dialog-in-progress")]
    DialogInProgress,
    #[serde(rename = "cooldown")]
    Cooldown,
    #[serde(rename = "already-consumed")]
    AlreadyConsumed,
    #[serde(rename = "proposal-unavailable")]
    ProposalUnavailable,
    #[serde(rename = "market-closed")]
    MarketClosed,
    #[serde(rename = "conditions-changed")]
    ConditionsChanged,
    #[serde(rename = "other")]
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub struct OrderApprovalResponse {
    pub status: OrderApprovalStatus,
    pub reason: OrderApprovalReason,
}

#[derive(Clone)]
pub struct OrderApprovalRuntime {
    handler: Arc<dyn OrderApprovalHandler>,
}

impl fmt::Debug for OrderApprovalRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("OrderApprovalRuntime(<redacted>)")
    }
}

impl OrderApprovalRuntime {
    #[must_use]
    pub fn new(handler: Arc<dyn OrderApprovalHandler>) -> Self {
        Self { handler }
    }

    pub async fn request(
        &self,
        request: &OrderApprovalRequest,
    ) -> Result<OrderApprovalResponse, IpcError> {
        let Ok(client_order_id) = ClientOrderId::try_new(&request.proposal_id) else {
            return Ok(response(
                OrderApprovalStatus::Rejected,
                OrderApprovalReason::ProposalUnavailable,
            ));
        };
        let proposal_id = ProposalId::new(client_order_id);
        match self.handler.request_order_approval(&proposal_id).await {
            Ok(outcome) => Ok(map_outcome(&outcome)),
            Err(error) => Err(map_error(&error)),
        }
    }
}

const fn response(
    status: OrderApprovalStatus,
    reason: OrderApprovalReason,
) -> OrderApprovalResponse {
    OrderApprovalResponse { status, reason }
}

fn map_outcome(outcome: &ApprovalCommandOutcome) -> OrderApprovalResponse {
    match outcome {
        ApprovalCommandOutcome::Rejected(rejection) => {
            response(OrderApprovalStatus::Rejected, map_rejection(rejection))
        }
        ApprovalCommandOutcome::Submitted { .. } => {
            response(OrderApprovalStatus::Submitted, OrderApprovalReason::None)
        }
        ApprovalCommandOutcome::BrokerRejected { .. } => response(
            OrderApprovalStatus::BrokerRejected,
            OrderApprovalReason::Other,
        ),
        ApprovalCommandOutcome::OutcomeUnknown { .. } => response(
            OrderApprovalStatus::OutcomeUnknown,
            OrderApprovalReason::None,
        ),
    }
}

const fn map_rejection(rejection: &ApprovalCommandRejection) -> OrderApprovalReason {
    match rejection {
        ApprovalCommandRejection::Prepare(prepare) => map_prepare_rejection(prepare),
        ApprovalCommandRejection::PresenceDenied
        | ApprovalCommandRejection::PresenceUnavailable => OrderApprovalReason::PresenceDenied,
        ApprovalCommandRejection::OrdersDisabled => OrderApprovalReason::SwitchOff,
        ApprovalCommandRejection::HourlyLimitExceeded { .. } => OrderApprovalReason::HourlyLimit,
        ApprovalCommandRejection::AlreadyConsumed => OrderApprovalReason::AlreadyConsumed,
        ApprovalCommandRejection::PostPresenceConditionsChanged(_)
        | ApprovalCommandRejection::PostPresenceRecheckFailed(_) => {
            OrderApprovalReason::ConditionsChanged
        }
        ApprovalCommandRejection::SymbolFrozen
        | ApprovalCommandRejection::ClockRollback
        | ApprovalCommandRejection::OrderKeyMismatch => OrderApprovalReason::Other,
    }
}

const fn map_prepare_rejection(rejection: &PrepareRejection) -> OrderApprovalReason {
    match rejection {
        PrepareRejection::DialogInProgress => OrderApprovalReason::DialogInProgress,
        PrepareRejection::RejectionCooldown { .. } => OrderApprovalReason::Cooldown,
        PrepareRejection::OrdersDisabled => OrderApprovalReason::SwitchOff,
        PrepareRejection::HourlyLimitExceeded { .. } => OrderApprovalReason::HourlyLimit,
        PrepareRejection::AlreadyConsumed => OrderApprovalReason::AlreadyConsumed,
        PrepareRejection::ProposalNotFound
        | PrepareRejection::ProposalExpired
        | PrepareRejection::EstimatePriceOutOfRange => OrderApprovalReason::ProposalUnavailable,
        PrepareRejection::MarketClosed => OrderApprovalReason::MarketClosed,
        PrepareRejection::GateRejected(_)
        | PrepareRejection::ExecutionCheckFailed(_)
        | PrepareRejection::ObservationInvalid => OrderApprovalReason::ConditionsChanged,
        PrepareRejection::ClockRollback
        | PrepareRejection::SymbolFrozen
        | PrepareRejection::SummaryFailed(_) => OrderApprovalReason::Other,
    }
}

fn map_error(error: &ApprovalCommandError) -> IpcError {
    match error {
        ApprovalCommandError::Broker(port)
        | ApprovalCommandError::Prepare(PrepareError::Broker(port)) => IpcError::from(port.class()),
        ApprovalCommandError::Storage(system)
        | ApprovalCommandError::Prepare(PrepareError::Storage(system)) => {
            IpcError::from(system.class())
        }
        ApprovalCommandError::Gate(_) | ApprovalCommandError::Prepare(PrepareError::Gate(_)) => {
            IpcError {
                kind: IpcErrorKind::Retryable,
            }
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn request_order_approval<R: Runtime>(
    app: AppHandle<R>,
    request: OrderApprovalRequest,
) -> Result<OrderApprovalResponse, IpcError> {
    let managed = app
        .try_state::<OrderApprovalRuntime>()
        .map(|state| state.inner().clone());
    match managed {
        Some(runtime) => runtime.request(&request).await,
        None => Ok(response(
            OrderApprovalStatus::Unavailable,
            OrderApprovalReason::SwitchOff,
        )),
    }
}

#[cfg(test)]
mod tests;
