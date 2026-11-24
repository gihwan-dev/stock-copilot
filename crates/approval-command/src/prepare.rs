use crate::guard::{ApprovalGuard, DialogLease, ExpiryNotification, GuardAcquisitionError};
use crate::ports::{
    ExpiredProposalRedrafter, ProposalContextPort, RedraftOutcome, SymbolFreezePort,
    SymbolFreezeStatus,
};
use application::{
    AuditEntry, AuditKind, AuditLog, BrokerRead, CheckOrderRateLimit, Clock, ConsumptionStatus,
    DeliveryReport, EventBus, MarketInfo, OrderGateError, OrderGateOutcome, OrderGatePolicyContext,
    OrderRateRepository, PortError, ProposalConsumptionRepository, ProposalRepository,
    SettingsRepository, SystemPortError, ValidateOrderProposal, ValidationReportRepository,
};
use domain::{
    DomainEvent, Draft, MasterInstrument, OrderCount, OrderPermission, OrderRateDecision,
    OrderSide, OrderSummary, OrderSummaryError, PendingOrderConfirmation, PriceObservation,
    Proposal, ProposalExecutionError, ProposalId, Timestamp, ValidationReport,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareRejection {
    DialogInProgress,
    RejectionCooldown {
        retry_at: Timestamp,
    },
    ClockRollback,
    OrdersDisabled,
    HourlyLimitExceeded {
        limit: domain::HourlyOrderLimit,
        recent_count: OrderCount,
        retry_at: Timestamp,
    },
    AlreadyConsumed,
    ProposalNotFound,
    SymbolFrozen,
    ObservationInvalid,
    EstimatePriceOutOfRange,
    MarketClosed,
    ProposalExpired,
    GateRejected(Box<ValidationReport>),
    ExecutionCheckFailed(ProposalExecutionError),
    SummaryFailed(OrderSummaryError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareError {
    Broker(PortError),
    Storage(SystemPortError),
    Gate(OrderGateError),
}

#[derive(Debug)]
pub struct PreparedApproval<'a> {
    confirmation: PendingOrderConfirmation,
    policy_context: OrderGatePolicyContext,
    lease: DialogLease<'a>,
}

impl<'a> PreparedApproval<'a> {
    #[must_use]
    pub fn summary(&self) -> &OrderSummary {
        self.confirmation.summary()
    }

    #[must_use]
    pub fn proposal_id(&self) -> &ProposalId {
        self.confirmation.id()
    }

    #[must_use]
    pub fn policy_context(&self) -> &OrderGatePolicyContext {
        &self.policy_context
    }

    #[must_use]
    pub fn into_confirmation(self) -> (PendingOrderConfirmation, DialogLease<'a>) {
        (self.confirmation, self.lease)
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PendingOrderConfirmation,
        OrderGatePolicyContext,
        DialogLease<'a>,
    ) {
        (self.confirmation, self.policy_context, self.lease)
    }
}

#[derive(Debug)]
pub enum PrepareOutcome<'a> {
    Ready(Box<PreparedApproval<'a>>),
    Rejected(PrepareRejection),
}

#[must_use]
pub fn record_expired_proposal(
    guard: &ApprovalGuard,
    events: &EventBus,
    proposal_id: &ProposalId,
) -> Option<DeliveryReport> {
    match guard.mark_expired_once(proposal_id) {
        ExpiryNotification::FirstTime => {
            Some(events.publish(DomainEvent::ProposalExpired(proposal_id.clone())))
        }
        ExpiryNotification::AlreadyNotified => None,
    }
}

#[derive(Debug)]
pub struct ApprovalReadPorts<'a, Broker, MarketSource, Settings, Rates> {
    pub broker: &'a Broker,
    pub market: &'a MarketSource,
    pub settings: &'a Settings,
    pub rates: &'a Rates,
}

#[derive(Debug)]
pub struct ApprovalStoragePorts<'a, Proposals, Consumption, Reports, Audit> {
    pub proposals: &'a Proposals,
    pub consumption: &'a Consumption,
    pub reports: &'a Reports,
    pub audit: &'a Audit,
}

#[derive(Debug)]
pub struct ApprovalContextPorts<'a, Contexts, Redrafter, Freeze, Time> {
    pub contexts: &'a Contexts,
    pub redrafter: &'a Redrafter,
    pub freeze: &'a Freeze,
    pub clock: &'a Time,
    pub events: &'a EventBus,
}

#[derive(Debug)]
pub struct ApprovalPreparer<
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
> {
    guard: &'a ApprovalGuard,
    read: ApprovalReadPorts<'a, Broker, MarketSource, Settings, Rates>,
    storage: ApprovalStoragePorts<'a, Proposals, Consumption, Reports, Audit>,
    context: ApprovalContextPorts<'a, Contexts, Redrafter, Freeze, Time>,
}

enum DraftResolution {
    Active(Box<Proposal<Draft>>),
    Rejected(PrepareRejection),
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
>
    ApprovalPreparer<
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
{
    #[must_use]
    pub const fn new(
        guard: &'a ApprovalGuard,
        read: ApprovalReadPorts<'a, Broker, MarketSource, Settings, Rates>,
        storage: ApprovalStoragePorts<'a, Proposals, Consumption, Reports, Audit>,
        context: ApprovalContextPorts<'a, Contexts, Redrafter, Freeze, Time>,
    ) -> Self {
        Self {
            guard,
            read,
            storage,
            context,
        }
    }

    pub async fn prepare(
        &self,
        proposal_id: &ProposalId,
    ) -> Result<PrepareOutcome<'a>, PrepareError> {
        let now = self.context.clock.now();
        let lease = match self.guard.try_acquire(proposal_id, now) {
            Ok(lease) => lease,
            Err(error) => return Ok(PrepareOutcome::Rejected(map_guard_error(error))),
        };
        let _audit_outcome = self
            .storage
            .audit
            .append(&AuditEntry::new(
                now,
                AuditKind::ApprovalRequested(proposal_id.clone()),
            ))
            .await
            .map_err(PrepareError::Storage)?;
        if let Some(rejection) = self.check_preconditions(proposal_id).await? {
            return Ok(PrepareOutcome::Rejected(rejection));
        }
        let Some(policy_context) = self
            .context
            .contexts
            .policy_context(proposal_id)
            .await
            .map_err(PrepareError::Storage)?
        else {
            return Ok(PrepareOutcome::Rejected(PrepareRejection::ProposalNotFound));
        };
        let active_draft = match self
            .load_and_refresh_draft(proposal_id, &policy_context)
            .await?
        {
            DraftResolution::Active(draft) => *draft,
            DraftResolution::Rejected(rejection) => return Ok(PrepareOutcome::Rejected(rejection)),
        };
        self.validate_and_summarize(active_draft, policy_context, lease)
            .await
    }

    async fn check_preconditions(
        &self,
        proposal_id: &ProposalId,
    ) -> Result<Option<PrepareRejection>, PrepareError> {
        let risk = self
            .read
            .settings
            .risk_settings()
            .await
            .map_err(PrepareError::Storage)?;
        let enabled = match risk {
            Some(saved) => saved.orders() == OrderPermission::Enabled,
            None => false,
        };
        if !enabled {
            return Ok(Some(PrepareRejection::OrdersDisabled));
        }
        let rate_decision = CheckOrderRateLimit::new(self.read.rates, self.context.clock)
            .evaluate()
            .await
            .map_err(PrepareError::Storage)?;
        match rate_decision {
            OrderRateDecision::Allowed { .. } => (),
            OrderRateDecision::Exceeded {
                limit,
                recent_count,
                retry_at,
            } => {
                return Ok(Some(PrepareRejection::HourlyLimitExceeded {
                    limit,
                    recent_count,
                    retry_at,
                }));
            }
            OrderRateDecision::ClockRollback => {
                return Ok(Some(PrepareRejection::ClockRollback));
            }
        }
        let consumption = self
            .storage
            .consumption
            .status(proposal_id)
            .await
            .map_err(PrepareError::Storage)?;
        match consumption {
            ConsumptionStatus::NotConsumed => Ok(None),
            ConsumptionStatus::Consumed => Ok(Some(PrepareRejection::AlreadyConsumed)),
        }
    }

    async fn load_and_refresh_draft(
        &self,
        proposal_id: &ProposalId,
        policy_context: &OrderGatePolicyContext,
    ) -> Result<DraftResolution, PrepareError> {
        let Some(draft) = self
            .storage
            .proposals
            .draft(proposal_id, &policy_context.master)
            .await
            .map_err(PrepareError::Storage)?
        else {
            return Ok(DraftResolution::Rejected(
                PrepareRejection::ProposalNotFound,
            ));
        };
        let intent = draft.intent();
        let freeze = self
            .context
            .freeze
            .freeze_status(intent.account_id(), intent.instrument())
            .await
            .map_err(PrepareError::Storage)?;
        if freeze == SymbolFreezeStatus::Frozen {
            return Ok(DraftResolution::Rejected(PrepareRejection::SymbolFrozen));
        }
        let now = self.context.clock.now();
        let observation = self
            .fetch_observation(intent.side(), &policy_context.master)
            .await?;
        match draft.check_current(now, &observation) {
            Ok(()) => Ok(DraftResolution::Active(Box::new(draft))),
            Err(ProposalExecutionError::ClockRollback) => {
                Ok(DraftResolution::Rejected(PrepareRejection::ClockRollback))
            }
            Err(ProposalExecutionError::ObservationInvalid) => Ok(DraftResolution::Rejected(
                PrepareRejection::ObservationInvalid,
            )),
            Err(ProposalExecutionError::Expired | ProposalExecutionError::PriceOutsideRange) => {
                self.redraft_expired(&draft, now).await
            }
        }
    }

    async fn redraft_expired(
        &self,
        expired: &Proposal<Draft>,
        now: Timestamp,
    ) -> Result<DraftResolution, PrepareError> {
        let _delivery = record_expired_proposal(self.guard, self.context.events, expired.id());
        let outcome = self
            .context
            .redrafter
            .redraft(expired, now)
            .await
            .map_err(PrepareError::Storage)?;
        match outcome {
            RedraftOutcome::OutOfEstimateRange => Ok(DraftResolution::Rejected(
                PrepareRejection::EstimatePriceOutOfRange,
            )),
            RedraftOutcome::MarketClosed => {
                Ok(DraftResolution::Rejected(PrepareRejection::MarketClosed))
            }
            RedraftOutcome::NoSignal => {
                Ok(DraftResolution::Rejected(PrepareRejection::ProposalExpired))
            }
            RedraftOutcome::Redrafted(fresh) => self.verify_redrafted(expired, *fresh).await,
        }
    }

    async fn verify_redrafted(
        &self,
        expired: &Proposal<Draft>,
        fresh: Proposal<Draft>,
    ) -> Result<DraftResolution, PrepareError> {
        let old_intent = expired.intent();
        let new_intent = fresh.intent();
        if old_intent.account_id() != new_intent.account_id()
            || old_intent.instrument() != new_intent.instrument()
        {
            return Ok(DraftResolution::Rejected(PrepareRejection::ProposalExpired));
        }
        let status = self
            .storage
            .consumption
            .status(fresh.id())
            .await
            .map_err(PrepareError::Storage)?;
        match status {
            ConsumptionStatus::NotConsumed => Ok(DraftResolution::Active(Box::new(fresh))),
            ConsumptionStatus::Consumed => {
                Ok(DraftResolution::Rejected(PrepareRejection::AlreadyConsumed))
            }
        }
    }

    async fn validate_and_summarize(
        &self,
        draft: Proposal<Draft>,
        policy_context: OrderGatePolicyContext,
        lease: DialogLease<'a>,
    ) -> Result<PrepareOutcome<'a>, PrepareError> {
        let validator = ValidateOrderProposal::new(
            self.read.broker,
            self.read.market,
            self.storage.reports,
            self.storage.audit,
            self.context.clock,
        );
        let outcome = validator
            .execute(draft, &policy_context)
            .await
            .map_err(PrepareError::Gate)?;
        match outcome {
            OrderGateOutcome::Rejected { report } => Ok(PrepareOutcome::Rejected(
                PrepareRejection::GateRejected(Box::new(report)),
            )),
            OrderGateOutcome::Passed {
                validated,
                collected,
                ..
            } => {
                let token =
                    match validated.into_execution(collected.validated_at, collected.observation) {
                        Ok(token) => token,
                        Err(error) => {
                            return Ok(PrepareOutcome::Rejected(
                                PrepareRejection::ExecutionCheckFailed(error),
                            ));
                        }
                    };
                let confirmation = match PendingOrderConfirmation::from_execution_with_commission(
                    token,
                    collected.instrument_name,
                    collected.account_display,
                    collected.commission_rate.as_ref(),
                    collected.order_fx_rates,
                ) {
                    Ok(confirmation) => confirmation,
                    Err(error) => {
                        return Ok(PrepareOutcome::Rejected(PrepareRejection::SummaryFailed(
                            error,
                        )));
                    }
                };
                Ok(PrepareOutcome::Ready(Box::new(PreparedApproval {
                    confirmation,
                    policy_context,
                    lease,
                })))
            }
        }
    }

    async fn fetch_observation(
        &self,
        side: OrderSide,
        master: &MasterInstrument,
    ) -> Result<PriceObservation, PrepareError> {
        let quote_instrument = master
            .quote_instrument()
            .map_err(|error| PrepareError::Gate(OrderGateError::Master(error)))?;
        let quotes = self
            .read
            .broker
            .quotes(std::slice::from_ref(&quote_instrument))
            .await
            .map_err(PrepareError::Broker)?;
        let matched = quotes
            .into_iter()
            .find(|entry| entry.instrument == quote_instrument)
            .ok_or(PrepareError::Gate(OrderGateError::MissingQuote))?;
        let quote = matched.result.map_err(PrepareError::Broker)?;
        let price = quote
            .last_price()
            .ok_or(PrepareError::Gate(OrderGateError::MissingQuote))?;
        let observed_at = quote.timestamp().unwrap_or_else(|| quote.collected_at());
        master
            .price_observation_for(side, price, observed_at)
            .map_err(|_error| PrepareError::Gate(OrderGateError::InvalidObservation))
    }
}

const fn map_guard_error(error: GuardAcquisitionError) -> PrepareRejection {
    match error {
        GuardAcquisitionError::DialogInProgress => PrepareRejection::DialogInProgress,
        GuardAcquisitionError::RejectionCooldown { retry_at } => {
            PrepareRejection::RejectionCooldown { retry_at }
        }
        GuardAcquisitionError::ClockRollback => PrepareRejection::ClockRollback,
    }
}
