use crate::{ApprovalSignatureSlot, BrokerTrade, OrderOutcome, UnknownOrderRetry, UserApproval};
use application::{OrderConfirmation, user_presence::UserPresence};
use domain::{
    Nanosecond, PendingOrderConfirmation, PriceObservation, ProposalExecutionError, SpanSeconds,
    TimeSpan, Timestamp,
};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostConfirmationRecheck {
    pub now: Timestamp,
    pub observation: PriceObservation,
}

pub trait PostApprovalGate: Send + Sync {
    type Error: Send;

    fn verify_after_presence(
        &self,
        approval: &UserApproval,
    ) -> impl Future<Output = Result<PostConfirmationRecheck, Self::Error>> + Send;

    fn commit_before_submit(
        &self,
        approval: &UserApproval,
    ) -> impl Future<Output = Result<Timestamp, Self::Error>> + Send;
}

#[derive(Debug)]
pub enum ApproveAndSubmitOutcome<Error> {
    PresenceDenied,
    PresenceUnavailable,
    PostPresenceRejected(Error),
    RecheckFailed(ProposalExecutionError),
    CommitFailed(Error),
    ClockRollbackBeforeSubmit,
    Submitted(OrderOutcome),
}

#[derive(Debug)]
pub enum RetryUnknownOutcome {
    ClockRollback,
    WindowExpired,
    Retried(OrderOutcome),
}

pub async fn approve_and_submit<Presence, Gate, Broker>(
    pending: PendingOrderConfirmation,
    presence: &Presence,
    gate: &Gate,
    broker: &Broker,
) -> ApproveAndSubmitOutcome<Gate::Error>
where
    Presence: UserPresence,
    Gate: PostApprovalGate,
    Broker: BrokerTrade,
{
    match presence.confirm_order(pending.summary()).await {
        OrderConfirmation::Denied => return ApproveAndSubmitOutcome::PresenceDenied,
        OrderConfirmation::Unavailable => return ApproveAndSubmitOutcome::PresenceUnavailable,
        OrderConfirmation::Confirmed => (),
    }
    let approval =
        UserApproval::from_confirmed(pending, ApprovalSignatureSlot::PendingSecureEnclave);
    let recheck_input = match gate.verify_after_presence(&approval).await {
        Ok(input) => input,
        Err(error) => return ApproveAndSubmitOutcome::PostPresenceRejected(error),
    };
    let checked = match approval.recheck(recheck_input.now, recheck_input.observation) {
        Ok(valid) => valid,
        Err(error) => return ApproveAndSubmitOutcome::RecheckFailed(error),
    };
    let submitted_at = match gate.commit_before_submit(&checked).await {
        Ok(timestamp) => timestamp,
        Err(error) => return ApproveAndSubmitOutcome::CommitFailed(error),
    };
    if submitted_at < checked.checked_at() {
        return ApproveAndSubmitOutcome::ClockRollbackBeforeSubmit;
    }
    let approved = checked.into_order();
    let outcome = broker
        .submit(approved)
        .await
        .stamp_first_sent_at(submitted_at);
    ApproveAndSubmitOutcome::Submitted(outcome)
}

pub async fn retry_unknown_order<Broker>(
    permit: UnknownOrderRetry,
    now: Timestamp,
    broker: &Broker,
) -> RetryUnknownOutcome
where
    Broker: BrokerTrade,
{
    let (order, first_sent_at) = permit.into_parts();
    if now < first_sent_at {
        return RetryUnknownOutcome::ClockRollback;
    }
    let Ok(window) = TimeSpan::try_new(SpanSeconds::new(600), Nanosecond::new(0)) else {
        return RetryUnknownOutcome::WindowExpired;
    };
    let Ok(deadline) = first_sent_at.checked_add(window) else {
        return RetryUnknownOutcome::WindowExpired;
    };
    if now >= deadline {
        return RetryUnknownOutcome::WindowExpired;
    }
    let outcome = broker
        .submit(order)
        .await
        .stamp_first_sent_at(first_sent_at);
    RetryUnknownOutcome::Retried(outcome)
}
