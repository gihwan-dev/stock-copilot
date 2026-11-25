use std::sync::Arc;

use application::{ConsumptionOutcome, SystemPortError, WriteCompleted};
use approval_command::ports::{
    PreSubmitCommitOutcome, PreSubmitTransactionPort, PreparedSubmissionRecord,
    SubmissionResponseRecord,
};
use domain::{ClientOrderId, ProposalId, Timestamp};

use crate::ipc::state::BoxFuture;

pub trait SubmissionLedger: Send + Sync {
    fn commit_prepared<'a>(
        &'a self,
        proposal: &'a ProposalId,
        client_order_id: &'a ClientOrderId,
        prepared_at: Timestamp,
    ) -> BoxFuture<'a, Result<ConsumptionOutcome, SystemPortError>>;
}

pub struct LedgerTxPort(pub Arc<dyn SubmissionLedger>);

impl std::fmt::Debug for LedgerTxPort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("LedgerTxPort")
    }
}

impl PreSubmitTransactionPort for LedgerTxPort {
    async fn commit_consumption_and_submission(
        &self,
        record: &PreparedSubmissionRecord,
    ) -> Result<PreSubmitCommitOutcome, SystemPortError> {
        match self
            .0
            .commit_prepared(
                &record.proposal_id,
                &record.client_order_id,
                record.prepared_at,
            )
            .await?
        {
            ConsumptionOutcome::Recorded => Ok(PreSubmitCommitOutcome::Committed),
            ConsumptionOutcome::AlreadyConsumed => Ok(PreSubmitCommitOutcome::AlreadyConsumed),
        }
    }

    fn record_submission_outcome(
        &self,
        _record: &SubmissionResponseRecord,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        std::future::ready(Ok(WriteCompleted))
    }
}

const FORWARDED: [domain::DomainEventKind; 9] = [
    domain::DomainEventKind::ProposalExpired,
    domain::DomainEventKind::ProposalApproved,
    domain::DomainEventKind::ProposalRejected,
    domain::DomainEventKind::OrderSubmitted,
    domain::DomainEventKind::OrderOutcomeUnknown,
    domain::DomainEventKind::OrderRejected,
    domain::DomainEventKind::OrderOutcomeResolved,
    domain::DomainEventKind::SignalCleared,
    domain::DomainEventKind::ProposalCreated,
];

struct Forward(Arc<application::EventBus>);

impl application::EventSubscriber for Forward {
    fn on_event(&self, event: &domain::DomainEvent) -> Result<(), application::SubscriberFailure> {
        let _report = self.0.publish(event.clone());
        Ok(())
    }
}

#[must_use]
pub fn forwarding_bus(target: &Arc<application::EventBus>) -> application::EventBus {
    let mut builder = application::EventBusBuilder::default();
    builder.subscribe(FORWARDED.into_iter().collect(), Forward(Arc::clone(target)));
    builder.build()
}

#[cfg(test)]
mod tests;
