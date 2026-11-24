use crate::guard::ApprovalGuard;
use crate::ports::{
    BrokerOrderRejectionReason, ExpiredProposalRedrafter, PreSubmitCommitOutcome,
    PreSubmitTransactionPort, PreparedSubmissionRecord, ProposalContextPort,
    SubmissionResponseRecord, SubmissionResponseStatus, SymbolFreezePort, SymbolFreezeStatus,
    UnknownOrderCause, UnknownOrderRetryStatus,
};
use crate::prepare::{
    ApprovalContextPorts, ApprovalPreparer, ApprovalReadPorts, ApprovalStoragePorts, PrepareError,
    PrepareOutcome, PrepareRejection,
};
use application::{
    AuditEntry, AuditKind, AuditLog, BrokerRead, CheckOrderRateLimit, Clock, DeliveryReport,
    IntentRecheckOutcome, MarketInfo, OrderConfirmation, OrderGateError, OrderGatePolicyContext,
    OrderRateRepository, PortError, ProposalConsumptionRepository, ProposalRepository,
    SettingsRepository, SystemPortError, ValidateOrderProposal, ValidationReportRepository,
    user_presence::UserPresence,
};
use domain::{
    BrokerOrderId, ClientOrderId, DomainEvent, HourlyOrderLimit, OrderCount, OrderPermission,
    OrderRateDecision, ProposalExecutionError, ProposalId, Timestamp, ValidationReport,
};
use order_execution::{
    ApproveAndSubmitOutcome, BrokerTrade, OrderOutcome, OrderRejectionReason, OrderUnknownCause,
    PostApprovalGate, PostConfirmationRecheck, RetryEligibility, UnknownOrderRetry, UserApproval,
    approve_and_submit,
};
use std::collections::BTreeMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug, Default)]
pub struct UnknownOrderVault {
    permits: RwLock<BTreeMap<ClientOrderId, UnknownOrderRetry>>,
}

impl UnknownOrderVault {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn retry_status(&self, client_order_id: &ClientOrderId) -> UnknownOrderRetryStatus {
        let state = read_vault(&self.permits);
        if state.contains_key(client_order_id) {
            UnknownOrderRetryStatus::Eligible
        } else {
            UnknownOrderRetryStatus::Forbidden
        }
    }

    pub fn discard(&self, client_order_id: &ClientOrderId) {
        let mut state = write_vault(&self.permits);
        let _removed = state.remove(client_order_id);
    }

    pub(crate) fn store(&self, permit: UnknownOrderRetry) {
        let mut state = write_vault(&self.permits);
        let key = permit.client_order_id().clone();
        let _previous = state.insert(key, permit);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubmissionAuditStatus {
    Recorded,
    Failed(SystemPortError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalCommandRejection {
    Prepare(PrepareRejection),
    PresenceDenied,
    PresenceUnavailable,
    OrdersDisabled,
    HourlyLimitExceeded {
        limit: HourlyOrderLimit,
        recent_count: OrderCount,
        retry_at: Timestamp,
    },
    SymbolFrozen,
    ClockRollback,
    AlreadyConsumed,
    OrderKeyMismatch,
    PostPresenceConditionsChanged(Box<ValidationReport>),
    PostPresenceRecheckFailed(ProposalExecutionError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalCommandOutcome {
    Rejected(ApprovalCommandRejection),
    Submitted {
        client_order_id: ClientOrderId,
        broker_order_id: BrokerOrderId,
        audit_status: SubmissionAuditStatus,
        delivery: DeliveryReport,
    },
    BrokerRejected {
        client_order_id: ClientOrderId,
        reason: BrokerOrderRejectionReason,
        audit_status: SubmissionAuditStatus,
        delivery: DeliveryReport,
    },
    OutcomeUnknown {
        client_order_id: ClientOrderId,
        cause: UnknownOrderCause,
        retry: UnknownOrderRetryStatus,
        audit_status: SubmissionAuditStatus,
        delivery: DeliveryReport,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalCommandError {
    Prepare(PrepareError),
    Broker(PortError),
    Storage(SystemPortError),
    Gate(OrderGateError),
}

#[derive(Debug)]
pub struct ApprovalExecutionPorts<'a, Presence, TxPort, Trade> {
    pub presence: &'a Presence,
    pub tx_port: &'a TxPort,
    pub trade: &'a Trade,
    pub vault: &'a UnknownOrderVault,
}

#[derive(Debug)]
pub struct ApprovalCommand<
    'a,
    Broker,
    MarketSource,
    Settings,
    Rates,
    Proposals,
    Consumption,
    Reports,
    Audit,
    Contexts,
    Redrafter,
    Freeze,
    Time,
    Presence,
    TxPort,
    Trade,
> {
    guard: &'a ApprovalGuard,
    read: ApprovalReadPorts<'a, Broker, MarketSource, Settings, Rates>,
    storage: ApprovalStoragePorts<'a, Proposals, Consumption, Reports, Audit>,
    context: ApprovalContextPorts<'a, Contexts, Redrafter, Freeze, Time>,
    execution: ApprovalExecutionPorts<'a, Presence, TxPort, Trade>,
}

enum PostGateError {
    OrdersDisabled,
    HourlyLimitExceeded {
        limit: HourlyOrderLimit,
        recent_count: OrderCount,
        retry_at: Timestamp,
    },
    SymbolFrozen,
    ClockRollback,
    AlreadyConsumed,
    OrderKeyMismatch,
    ConditionsChanged(Box<ValidationReport>),
    Storage(SystemPortError),
    Gate(OrderGateError),
}

struct LivePostApprovalGate<
    'a,
    Broker,
    MarketSource,
    Settings,
    Rates,
    Reports,
    Audit,
    Freeze,
    Time,
    TxPort,
> {
    broker: &'a Broker,
    market: &'a MarketSource,
    settings: &'a Settings,
    rates: &'a Rates,
    reports: &'a Reports,
    audit: &'a Audit,
    freeze: &'a Freeze,
    clock: &'a Time,
    events: &'a application::EventBus,
    tx_port: &'a TxPort,
    policy_context: OrderGatePolicyContext,
}

impl<Broker, MarketSource, Settings, Rates, Reports, Audit, Freeze, Time, TxPort> PostApprovalGate
    for LivePostApprovalGate<
        '_,
        Broker,
        MarketSource,
        Settings,
        Rates,
        Reports,
        Audit,
        Freeze,
        Time,
        TxPort,
    >
where
    Broker: BrokerRead,
    MarketSource: MarketInfo,
    Settings: SettingsRepository,
    Rates: OrderRateRepository,
    Reports: ValidationReportRepository,
    Audit: AuditLog,
    Freeze: SymbolFreezePort,
    Time: Clock,
    TxPort: PreSubmitTransactionPort,
{
    type Error = PostGateError;

    async fn verify_after_presence(
        &self,
        approval: &UserApproval,
    ) -> Result<PostConfirmationRecheck, Self::Error> {
        let now = self.clock.now();
        let _audit = self
            .audit
            .append(&AuditEntry::new(
                now,
                AuditKind::PresenceRecorded {
                    proposal: approval.id().clone(),
                    outcome: OrderConfirmation::Confirmed,
                },
            ))
            .await
            .map_err(PostGateError::Storage)?;
        let _delivery = self
            .events
            .publish(DomainEvent::ProposalApproved(approval.id().clone()));
        self.recheck_runtime_guards(approval).await?;
        let validator = ValidateOrderProposal::new(
            self.broker,
            self.market,
            self.reports,
            self.audit,
            self.clock,
        );
        match validator
            .recheck_intent(approval.intent(), &self.policy_context)
            .await
            .map_err(PostGateError::Gate)?
        {
            IntentRecheckOutcome::Passed {
                checked_at,
                observation,
            } => Ok(PostConfirmationRecheck {
                now: checked_at,
                observation,
            }),
            IntentRecheckOutcome::Rejected { report } => {
                Err(PostGateError::ConditionsChanged(Box::new(report)))
            }
        }
    }

    async fn commit_before_submit(
        &self,
        approval: &UserApproval,
    ) -> Result<Timestamp, Self::Error> {
        let intent = approval.intent();
        if intent.client_order_id() != approval.id().as_client_order_id() {
            return Err(PostGateError::OrderKeyMismatch);
        }
        let prepared_at = self.clock.now();
        if prepared_at < approval.checked_at() {
            return Err(PostGateError::ClockRollback);
        }
        let record = PreparedSubmissionRecord {
            proposal_id: approval.id().clone(),
            client_order_id: intent.client_order_id().clone(),
            account_id: intent.account_id().clone(),
            instrument: intent.instrument().clone(),
            terms: *intent.terms(),
            large_order: approval.large_order_confirmation(),
            prepared_at,
        };
        match self
            .tx_port
            .commit_consumption_and_submission(&record)
            .await
            .map_err(PostGateError::Storage)?
        {
            PreSubmitCommitOutcome::Committed => Ok(prepared_at),
            PreSubmitCommitOutcome::AlreadyConsumed => Err(PostGateError::AlreadyConsumed),
        }
    }
}

impl<Broker, MarketSource, Settings, Rates, Reports, Audit, Freeze, Time, TxPort>
    LivePostApprovalGate<
        '_,
        Broker,
        MarketSource,
        Settings,
        Rates,
        Reports,
        Audit,
        Freeze,
        Time,
        TxPort,
    >
where
    Broker: Sync,
    MarketSource: Sync,
    Settings: SettingsRepository,
    Rates: OrderRateRepository,
    Reports: Sync,
    Audit: Sync,
    Freeze: SymbolFreezePort,
    Time: Clock,
    TxPort: Sync,
{
    async fn recheck_runtime_guards(&self, approval: &UserApproval) -> Result<(), PostGateError> {
        let risk = self
            .settings
            .risk_settings()
            .await
            .map_err(PostGateError::Storage)?;
        let enabled = match risk {
            Some(saved) => saved.orders() == OrderPermission::Enabled,
            None => false,
        };
        if !enabled {
            return Err(PostGateError::OrdersDisabled);
        }
        match CheckOrderRateLimit::new(self.rates, self.clock)
            .evaluate()
            .await
            .map_err(PostGateError::Storage)?
        {
            OrderRateDecision::Allowed { .. } => (),
            OrderRateDecision::Exceeded {
                limit,
                recent_count,
                retry_at,
            } => {
                return Err(PostGateError::HourlyLimitExceeded {
                    limit,
                    recent_count,
                    retry_at,
                });
            }
            OrderRateDecision::ClockRollback => return Err(PostGateError::ClockRollback),
        }
        let intent = approval.intent();
        let freeze = self
            .freeze
            .freeze_status(intent.account_id(), intent.instrument())
            .await
            .map_err(PostGateError::Storage)?;
        if freeze == SymbolFreezeStatus::Frozen {
            return Err(PostGateError::SymbolFrozen);
        }
        Ok(())
    }
}

impl<
    'a,
    Broker,
    MarketSource,
    Settings,
    Rates,
    Proposals,
    Consumption,
    Reports,
    Audit,
    Contexts,
    Redrafter,
    Freeze,
    Time,
    Presence,
    TxPort,
    Trade,
>
    ApprovalCommand<
        'a,
        Broker,
        MarketSource,
        Settings,
        Rates,
        Proposals,
        Consumption,
        Reports,
        Audit,
        Contexts,
        Redrafter,
        Freeze,
        Time,
        Presence,
        TxPort,
        Trade,
    >
where
    Broker: BrokerRead,
    MarketSource: MarketInfo,
    Settings: SettingsRepository,
    Rates: OrderRateRepository,
    Proposals: ProposalRepository,
    Consumption: ProposalConsumptionRepository,
    Reports: ValidationReportRepository,
    Audit: AuditLog,
    Contexts: ProposalContextPort,
    Redrafter: ExpiredProposalRedrafter,
    Freeze: SymbolFreezePort,
    Time: Clock,
    Presence: UserPresence,
    TxPort: PreSubmitTransactionPort,
    Trade: BrokerTrade,
{
    #[must_use]
    pub const fn new(
        guard: &'a ApprovalGuard,
        read: ApprovalReadPorts<'a, Broker, MarketSource, Settings, Rates>,
        storage: ApprovalStoragePorts<'a, Proposals, Consumption, Reports, Audit>,
        context: ApprovalContextPorts<'a, Contexts, Redrafter, Freeze, Time>,
        execution: ApprovalExecutionPorts<'a, Presence, TxPort, Trade>,
    ) -> Self {
        Self {
            guard,
            read,
            storage,
            context,
            execution,
        }
    }

    pub async fn request_order_approval(
        &self,
        proposal_id: &ProposalId,
    ) -> Result<ApprovalCommandOutcome, ApprovalCommandError> {
        let preparer = ApprovalPreparer::new(
            self.guard,
            ApprovalReadPorts {
                broker: self.read.broker,
                market: self.read.market,
                settings: self.read.settings,
                rates: self.read.rates,
            },
            ApprovalStoragePorts {
                proposals: self.storage.proposals,
                consumption: self.storage.consumption,
                reports: self.storage.reports,
                audit: self.storage.audit,
            },
            ApprovalContextPorts {
                contexts: self.context.contexts,
                redrafter: self.context.redrafter,
                freeze: self.context.freeze,
                clock: self.context.clock,
                events: self.context.events,
            },
        );
        let ready = match preparer
            .prepare(proposal_id)
            .await
            .map_err(ApprovalCommandError::Prepare)?
        {
            PrepareOutcome::Ready(ready) => *ready,
            PrepareOutcome::Rejected(rejection) => {
                return Ok(ApprovalCommandOutcome::Rejected(
                    ApprovalCommandRejection::Prepare(rejection),
                ));
            }
        };
        let (confirmation, policy_context, _lease) = ready.into_parts();
        let active_proposal_id = confirmation.id().clone();
        let gate = LivePostApprovalGate {
            broker: self.read.broker,
            market: self.read.market,
            settings: self.read.settings,
            rates: self.read.rates,
            reports: self.storage.reports,
            audit: self.storage.audit,
            freeze: self.context.freeze,
            clock: self.context.clock,
            events: self.context.events,
            tx_port: self.execution.tx_port,
            policy_context,
        };
        let result = approve_and_submit(
            confirmation,
            self.execution.presence,
            &gate,
            self.execution.trade,
        )
        .await;
        self.handle_submit_outcome(proposal_id, &active_proposal_id, result)
            .await
    }

    async fn handle_submit_outcome(
        &self,
        requested_id: &ProposalId,
        active_id: &ProposalId,
        result: ApproveAndSubmitOutcome<PostGateError>,
    ) -> Result<ApprovalCommandOutcome, ApprovalCommandError> {
        match result {
            ApproveAndSubmitOutcome::PresenceDenied => {
                self.record_non_confirmed_presence(
                    requested_id,
                    active_id,
                    OrderConfirmation::Denied,
                )
                .await?;
                Ok(ApprovalCommandOutcome::Rejected(
                    ApprovalCommandRejection::PresenceDenied,
                ))
            }
            ApproveAndSubmitOutcome::PresenceUnavailable => {
                self.record_non_confirmed_presence(
                    requested_id,
                    active_id,
                    OrderConfirmation::Unavailable,
                )
                .await?;
                Ok(ApprovalCommandOutcome::Rejected(
                    ApprovalCommandRejection::PresenceUnavailable,
                ))
            }
            ApproveAndSubmitOutcome::PostPresenceRejected(error)
            | ApproveAndSubmitOutcome::CommitFailed(error) => map_post_gate_error(error),
            ApproveAndSubmitOutcome::RecheckFailed(error) => Ok(ApprovalCommandOutcome::Rejected(
                ApprovalCommandRejection::PostPresenceRecheckFailed(error),
            )),
            ApproveAndSubmitOutcome::ClockRollbackBeforeSubmit => Ok(
                ApprovalCommandOutcome::Rejected(ApprovalCommandRejection::ClockRollback),
            ),
            ApproveAndSubmitOutcome::Submitted(outcome) => Ok(self.finalize_outcome(outcome).await),
        }
    }

    async fn record_non_confirmed_presence(
        &self,
        requested_id: &ProposalId,
        active_id: &ProposalId,
        outcome: OrderConfirmation,
    ) -> Result<(), ApprovalCommandError> {
        let now = self.context.clock.now();
        if outcome == OrderConfirmation::Denied {
            self.guard.record_rejection(requested_id.clone(), now);
            if active_id != requested_id {
                self.guard.record_rejection(active_id.clone(), now);
            }
        }
        let _audit = self
            .storage
            .audit
            .append(&AuditEntry::new(
                now,
                AuditKind::PresenceRecorded {
                    proposal: active_id.clone(),
                    outcome,
                },
            ))
            .await
            .map_err(ApprovalCommandError::Storage)?;
        let _delivery = self
            .context
            .events
            .publish(DomainEvent::ProposalRejected(active_id.clone()));
        Ok(())
    }

    async fn finalize_outcome(&self, outcome: OrderOutcome) -> ApprovalCommandOutcome {
        let recorded_at = self.context.clock.now();
        match outcome {
            OrderOutcome::Accepted(submitted) => {
                self.finalize_accepted(submitted, recorded_at).await
            }
            OrderOutcome::Rejected(rejection) => {
                self.finalize_rejected(rejection, recorded_at).await
            }
            OrderOutcome::Unknown(unknown) => self.finalize_unknown(unknown, recorded_at).await,
        }
    }

    async fn finalize_accepted(
        &self,
        submitted: order_execution::SubmittedOrder,
        recorded_at: Timestamp,
    ) -> ApprovalCommandOutcome {
        let client_order_id = submitted.client_order_id().clone();
        let broker_order_id = submitted.broker_order_id().clone();
        let status = SubmissionResponseStatus::Accepted {
            broker_order_id: broker_order_id.clone(),
        };
        let audit_status = self
            .record_response(&client_order_id, recorded_at, status)
            .await;
        let delivery = self
            .context
            .events
            .publish(DomainEvent::OrderSubmitted(client_order_id.clone()));
        ApprovalCommandOutcome::Submitted {
            client_order_id,
            broker_order_id,
            audit_status,
            delivery,
        }
    }

    async fn finalize_rejected(
        &self,
        rejection: order_execution::OrderRejection,
        recorded_at: Timestamp,
    ) -> ApprovalCommandOutcome {
        let client_order_id = rejection.client_order_id().clone();
        let reason = map_rejection_reason(rejection.reason());
        let status = SubmissionResponseStatus::Rejected { reason };
        let audit_status = self
            .record_response(&client_order_id, recorded_at, status)
            .await;
        let delivery = self
            .context
            .events
            .publish(DomainEvent::OrderRejected(client_order_id.clone()));
        ApprovalCommandOutcome::BrokerRejected {
            client_order_id,
            reason,
            audit_status,
            delivery,
        }
    }

    async fn finalize_unknown(
        &self,
        unknown: order_execution::UnknownOrderOutcome,
        recorded_at: Timestamp,
    ) -> ApprovalCommandOutcome {
        let client_order_id = unknown.client_order_id().clone();
        let cause = map_unknown_cause(unknown.cause());
        let retry = map_retry_eligibility(unknown.retry_eligibility());
        if let Some(permit) = unknown.into_retry() {
            self.execution.vault.store(permit);
        }
        let status = SubmissionResponseStatus::Unknown { cause, retry };
        let audit_status = self
            .record_response(&client_order_id, recorded_at, status)
            .await;
        let delivery = self
            .context
            .events
            .publish(DomainEvent::OrderOutcomeUnknown(client_order_id.clone()));
        ApprovalCommandOutcome::OutcomeUnknown {
            client_order_id,
            cause,
            retry,
            audit_status,
            delivery,
        }
    }

    async fn record_response(
        &self,
        client_order_id: &ClientOrderId,
        recorded_at: Timestamp,
        status: SubmissionResponseStatus,
    ) -> SubmissionAuditStatus {
        let record = SubmissionResponseRecord {
            client_order_id: client_order_id.clone(),
            recorded_at,
            status,
        };
        if let Err(error) = self
            .execution
            .tx_port
            .record_submission_outcome(&record)
            .await
        {
            return SubmissionAuditStatus::Failed(error);
        }
        match self
            .storage
            .audit
            .append(&AuditEntry::new(
                recorded_at,
                AuditKind::SubmissionResponseRecorded(client_order_id.clone()),
            ))
            .await
        {
            Ok(_) => SubmissionAuditStatus::Recorded,
            Err(error) => SubmissionAuditStatus::Failed(error),
        }
    }
}

fn map_post_gate_error(
    error: PostGateError,
) -> Result<ApprovalCommandOutcome, ApprovalCommandError> {
    match error {
        PostGateError::OrdersDisabled => Ok(ApprovalCommandOutcome::Rejected(
            ApprovalCommandRejection::OrdersDisabled,
        )),
        PostGateError::HourlyLimitExceeded {
            limit,
            recent_count,
            retry_at,
        } => Ok(ApprovalCommandOutcome::Rejected(
            ApprovalCommandRejection::HourlyLimitExceeded {
                limit,
                recent_count,
                retry_at,
            },
        )),
        PostGateError::SymbolFrozen => Ok(ApprovalCommandOutcome::Rejected(
            ApprovalCommandRejection::SymbolFrozen,
        )),
        PostGateError::ClockRollback => Ok(ApprovalCommandOutcome::Rejected(
            ApprovalCommandRejection::ClockRollback,
        )),
        PostGateError::AlreadyConsumed => Ok(ApprovalCommandOutcome::Rejected(
            ApprovalCommandRejection::AlreadyConsumed,
        )),
        PostGateError::OrderKeyMismatch => Ok(ApprovalCommandOutcome::Rejected(
            ApprovalCommandRejection::OrderKeyMismatch,
        )),
        PostGateError::ConditionsChanged(report) => Ok(ApprovalCommandOutcome::Rejected(
            ApprovalCommandRejection::PostPresenceConditionsChanged(report),
        )),
        PostGateError::Storage(err) => Err(ApprovalCommandError::Storage(err)),
        PostGateError::Gate(err) => Err(ApprovalCommandError::Gate(err)),
    }
}

const fn map_rejection_reason(reason: OrderRejectionReason) -> BrokerOrderRejectionReason {
    match reason {
        OrderRejectionReason::InvalidRequest => BrokerOrderRejectionReason::InvalidRequest,
        OrderRejectionReason::ConfirmHighValueRequired => {
            BrokerOrderRejectionReason::ConfirmHighValueRequired
        }
        OrderRejectionReason::AccountHeaderRequired => {
            BrokerOrderRejectionReason::AccountHeaderRequired
        }
        OrderRejectionReason::AuthenticationRequired => {
            BrokerOrderRejectionReason::AuthenticationRequired
        }
        OrderRejectionReason::AccessForbidden => BrokerOrderRejectionReason::AccessForbidden,
        OrderRejectionReason::OppositePendingOrderExists => {
            BrokerOrderRejectionReason::OppositePendingOrderExists
        }
        OrderRejectionReason::InsufficientBuyingPower => {
            BrokerOrderRejectionReason::InsufficientBuyingPower
        }
        OrderRejectionReason::InsufficientSellableQuantity => {
            BrokerOrderRejectionReason::InsufficientSellableQuantity
        }
        OrderRejectionReason::OrderHoursClosed => BrokerOrderRejectionReason::OrderHoursClosed,
        OrderRejectionReason::StockRestricted => BrokerOrderRejectionReason::StockRestricted,
        OrderRejectionReason::PriceOutOfRange => BrokerOrderRejectionReason::PriceOutOfRange,
        OrderRejectionReason::OrderTypeNotAllowed => {
            BrokerOrderRejectionReason::OrderTypeNotAllowed
        }
        OrderRejectionReason::PrerequisiteRequired => {
            BrokerOrderRejectionReason::PrerequisiteRequired
        }
        OrderRejectionReason::MarketNotSupportedForStock => {
            BrokerOrderRejectionReason::MarketNotSupportedForStock
        }
        OrderRejectionReason::InvestorExchangeNotIntegrated => {
            BrokerOrderRejectionReason::InvestorExchangeNotIntegrated
        }
        OrderRejectionReason::AmountOrderOutsideRegularHours => {
            BrokerOrderRejectionReason::AmountOrderOutsideRegularHours
        }
        OrderRejectionReason::FractionalQuantityOutsideRegularHours => {
            BrokerOrderRejectionReason::FractionalQuantityOutsideRegularHours
        }
        OrderRejectionReason::AccountRestricted => BrokerOrderRejectionReason::AccountRestricted,
        OrderRejectionReason::MaxOrderAmountExceeded => {
            BrokerOrderRejectionReason::MaxOrderAmountExceeded
        }
        OrderRejectionReason::IdempotencyKeyConflict => {
            BrokerOrderRejectionReason::IdempotencyKeyConflict
        }
        OrderRejectionReason::RateLimited => BrokerOrderRejectionReason::RateLimited,
        OrderRejectionReason::OtherRejected => BrokerOrderRejectionReason::OtherRejected,
    }
}

const fn map_unknown_cause(cause: OrderUnknownCause) -> UnknownOrderCause {
    match cause {
        OrderUnknownCause::Maintenance => UnknownOrderCause::Maintenance,
        OrderUnknownCause::ServerError => UnknownOrderCause::ServerError,
        OrderUnknownCause::RequestInProgress => UnknownOrderCause::RequestInProgress,
        OrderUnknownCause::Transport => UnknownOrderCause::Transport,
        OrderUnknownCause::InvalidResponse => UnknownOrderCause::InvalidResponse,
    }
}

const fn map_retry_eligibility(eligibility: RetryEligibility) -> UnknownOrderRetryStatus {
    match eligibility {
        RetryEligibility::Eligible => UnknownOrderRetryStatus::Eligible,
        RetryEligibility::Forbidden => UnknownOrderRetryStatus::Forbidden,
    }
}

fn read_vault(
    lock: &RwLock<BTreeMap<ClientOrderId, UnknownOrderRetry>>,
) -> RwLockReadGuard<'_, BTreeMap<ClientOrderId, UnknownOrderRetry>> {
    match lock.read() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn write_vault(
    lock: &RwLock<BTreeMap<ClientOrderId, UnknownOrderRetry>>,
) -> RwLockWriteGuard<'_, BTreeMap<ClientOrderId, UnknownOrderRetry>> {
    match lock.write() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}
