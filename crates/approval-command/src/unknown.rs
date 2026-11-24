use crate::execute::{
    SubmissionAuditStatus, UnknownOrderVault, map_rejection_reason, map_retry_eligibility,
    map_unknown_cause,
};
use crate::ports::{
    BrokerOrderRejectionReason, PreSubmitTransactionPort, SubmissionResponseRecord,
    SubmissionResponseStatus, SymbolFreezePort, SymbolFreezeStatus, UnknownOrderCause,
    UnknownOrderRetryStatus,
};
use application::{
    AuditEntry, AuditKind, AuditLog, BrokerRead, Clock, DeliveryReport, EventBus, PortError,
    SystemPortError,
};
use domain::{
    AccountId, BrokerOrder, BrokerOrderId, ClientOrderId, DomainEvent, ResolvedOrderInstrument,
    TimeInterval, UnknownOrderFacts, UnknownOrderResolution, resolve_unknown_order,
};
use order_execution::{BrokerTrade, OrderOutcome, RetryUnknownOutcome, retry_unknown_order};
use std::future::Future;
use std::sync::RwLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownRetryOutcome {
    NotRetryable,
    WindowExpired,
    ClockRollback,
    Resolved {
        broker_order_id: BrokerOrderId,
        audit_status: SubmissionAuditStatus,
        delivery: DeliveryReport,
    },
    Rejected {
        reason: BrokerOrderRejectionReason,
        audit_status: SubmissionAuditStatus,
        delivery: DeliveryReport,
    },
    StillUnknown {
        cause: UnknownOrderCause,
        retry: UnknownOrderRetryStatus,
        audit_status: SubmissionAuditStatus,
    },
}

#[derive(Debug)]
pub struct UnknownOrderRetrier<'a, TxPort, Audit, Time, Trade> {
    pub tx_port: &'a TxPort,
    pub audit: &'a Audit,
    pub clock: &'a Time,
    pub events: &'a EventBus,
    pub trade: &'a Trade,
    pub vault: &'a UnknownOrderVault,
}

async fn record<TxPort, Audit>(
    tx_port: &TxPort,
    audit: &Audit,
    record: SubmissionResponseRecord,
) -> SubmissionAuditStatus
where
    TxPort: PreSubmitTransactionPort,
    Audit: AuditLog,
{
    if let Err(error) = tx_port.record_submission_outcome(&record).await {
        return SubmissionAuditStatus::Failed(error);
    }
    match audit
        .append(&AuditEntry::new(
            record.recorded_at,
            AuditKind::SubmissionResponseRecorded(record.client_order_id.clone()),
        ))
        .await
    {
        Ok(_) => SubmissionAuditStatus::Recorded,
        Err(error) => SubmissionAuditStatus::Failed(error),
    }
}

impl<TxPort, Audit, Time, Trade> UnknownOrderRetrier<'_, TxPort, Audit, Time, Trade>
where
    TxPort: PreSubmitTransactionPort,
    Audit: AuditLog,
    Time: Clock,
    Trade: BrokerTrade,
{
    pub async fn retry(&self, client_order_id: &ClientOrderId) -> UnknownRetryOutcome {
        let Some(permit) = self.vault.take(client_order_id) else {
            return UnknownRetryOutcome::NotRetryable;
        };
        match retry_unknown_order(permit, self.clock.now(), self.trade).await {
            RetryUnknownOutcome::ClockRollback => UnknownRetryOutcome::ClockRollback,
            RetryUnknownOutcome::WindowExpired => UnknownRetryOutcome::WindowExpired,
            RetryUnknownOutcome::Retried(outcome) => self.finish(client_order_id, outcome).await,
        }
    }

    fn settle(&self, outcome: OrderOutcome) -> (SubmissionResponseStatus, Settled) {
        match outcome {
            OrderOutcome::Accepted(submitted) => {
                let broker_order_id = submitted.broker_order_id().clone();
                (
                    SubmissionResponseStatus::Accepted {
                        broker_order_id: broker_order_id.clone(),
                    },
                    Settled::Accepted(broker_order_id),
                )
            }
            OrderOutcome::Rejected(rejection) => {
                let reason = map_rejection_reason(rejection.reason());
                (
                    SubmissionResponseStatus::Rejected { reason },
                    Settled::Rejected(reason),
                )
            }
            OrderOutcome::Unknown(unknown) => {
                let cause = map_unknown_cause(unknown.cause());
                let retry = map_retry_eligibility(unknown.retry_eligibility());
                if let Some(permit) = unknown.into_retry() {
                    self.vault.store(permit);
                }
                (
                    SubmissionResponseStatus::Unknown { cause, retry },
                    Settled::Unknown(cause, retry),
                )
            }
        }
    }

    async fn finish(
        &self,
        client_order_id: &ClientOrderId,
        outcome: OrderOutcome,
    ) -> UnknownRetryOutcome {
        let recorded_at = self.clock.now();
        let (status, settled) = self.settle(outcome);
        let audit_status = record(
            self.tx_port,
            self.audit,
            SubmissionResponseRecord {
                client_order_id: client_order_id.clone(),
                recorded_at,
                status,
            },
        )
        .await;
        match settled {
            Settled::Accepted(broker_order_id) => UnknownRetryOutcome::Resolved {
                broker_order_id,
                audit_status,
                delivery: self
                    .events
                    .publish(DomainEvent::OrderOutcomeResolved(client_order_id.clone())),
            },
            Settled::Rejected(reason) => UnknownRetryOutcome::Rejected {
                reason,
                audit_status,
                delivery: self
                    .events
                    .publish(DomainEvent::OrderRejected(client_order_id.clone())),
            },
            Settled::Unknown(cause, retry) => UnknownRetryOutcome::StillUnknown {
                cause,
                retry,
                audit_status,
            },
        }
    }
}

enum Settled {
    Accepted(BrokerOrderId),
    Rejected(BrokerOrderRejectionReason),
    Unknown(UnknownOrderCause, UnknownOrderRetryStatus),
}

#[derive(Debug, Default)]
pub struct UnconfirmedOrderFreeze {
    frozen: RwLock<Vec<(AccountId, ResolvedOrderInstrument, ClientOrderId)>>,
}

impl UnconfirmedOrderFreeze {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn freeze(
        &self,
        account: AccountId,
        instrument: ResolvedOrderInstrument,
        client_order_id: ClientOrderId,
    ) {
        let mut state = match self.frozen.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !state.iter().any(|(_, _, key)| key == &client_order_id) {
            state.push((account, instrument, client_order_id));
        }
    }

    pub fn release(&self, client_order_id: &ClientOrderId) {
        let mut state = match self.frozen.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.retain(|(_, _, key)| key != client_order_id);
    }

    fn status(
        &self,
        account: &AccountId,
        instrument: &ResolvedOrderInstrument,
    ) -> SymbolFreezeStatus {
        let state = match self.frozen.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if state
            .iter()
            .any(|(frozen_account, frozen, _)| frozen_account == account && frozen == instrument)
        {
            SymbolFreezeStatus::Frozen
        } else {
            SymbolFreezeStatus::Unfrozen
        }
    }
}

impl SymbolFreezePort for UnconfirmedOrderFreeze {
    fn freeze_status(
        &self,
        account: &AccountId,
        instrument: &ResolvedOrderInstrument,
    ) -> impl Future<Output = Result<SymbolFreezeStatus, SystemPortError>> + Send {
        std::future::ready(Ok(self.status(account, instrument)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnknownReconcileOutcome {
    Resolved {
        broker_order_id: BrokerOrderId,
        audit_status: SubmissionAuditStatus,
        delivery: DeliveryReport,
    },
    Unconfirmed,
}

#[derive(Debug)]
pub struct UnknownOrderReconciler<'a, Broker, TxPort, Audit, Time> {
    pub broker: &'a Broker,
    pub tx_port: &'a TxPort,
    pub audit: &'a Audit,
    pub clock: &'a Time,
    pub events: &'a EventBus,
    pub freeze: &'a UnconfirmedOrderFreeze,
}

impl<Broker, TxPort, Audit, Time> UnknownOrderReconciler<'_, Broker, TxPort, Audit, Time>
where
    Broker: BrokerRead,
    TxPort: PreSubmitTransactionPort,
    Audit: AuditLog,
    Time: Clock,
{
    pub async fn reconcile(
        &self,
        account: &AccountId,
        client_order_id: &ClientOrderId,
        facts: &UnknownOrderFacts,
        window: TimeInterval,
    ) -> Result<UnknownReconcileOutcome, PortError> {
        let open_result = self.broker.open_orders(account).await;
        let closed_result = self.broker.closed_orders(account, window).await;
        let (open, closed) = match (open_result, closed_result) {
            (Ok(open), Ok(closed)) => (open, closed),
            (Err(error), _) | (_, Err(error)) => {
                self.freeze_unconfirmed(account, facts, client_order_id);
                return Err(error);
            }
        };
        let orders: Vec<BrokerOrder> = open
            .orders()
            .values()
            .chain(closed.orders().orders().values())
            .cloned()
            .collect();
        match resolve_unknown_order(facts, &orders) {
            UnknownOrderResolution::Matched(broker_order_id) => {
                let audit_status = record(
                    self.tx_port,
                    self.audit,
                    SubmissionResponseRecord {
                        client_order_id: client_order_id.clone(),
                        recorded_at: self.clock.now(),
                        status: SubmissionResponseStatus::Accepted {
                            broker_order_id: broker_order_id.clone(),
                        },
                    },
                )
                .await;
                self.freeze.release(client_order_id);
                Ok(UnknownReconcileOutcome::Resolved {
                    broker_order_id,
                    audit_status,
                    delivery: self
                        .events
                        .publish(DomainEvent::OrderOutcomeResolved(client_order_id.clone())),
                })
            }
            UnknownOrderResolution::Unconfirmed => {
                self.freeze_unconfirmed(account, facts, client_order_id);
                Ok(UnknownReconcileOutcome::Unconfirmed)
            }
        }
    }

    fn freeze_unconfirmed(
        &self,
        account: &AccountId,
        facts: &UnknownOrderFacts,
        client_order_id: &ClientOrderId,
    ) {
        self.freeze.freeze(
            account.clone(),
            facts.instrument.clone(),
            client_order_id.clone(),
        );
    }
}
