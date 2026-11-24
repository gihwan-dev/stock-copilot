use crate::execute::{
    ApprovalCommand, ApprovalCommandError, ApprovalCommandOutcome, ApprovalExecutionPorts,
    UnknownOrderVault,
};
use crate::guard::ApprovalGuard;
use crate::ports::{
    ExpiredProposalRedrafter, PreSubmitTransactionPort, ProposalContextPort, SymbolFreezePort,
};
use crate::prepare::{ApprovalContextPorts, ApprovalReadPorts, ApprovalStoragePorts};
use application::{
    AuditLog, BrokerRead, Clock, EventBus, MarketInfo, OrderRateRepository,
    ProposalConsumptionRepository, ProposalRepository, SettingsRepository,
    ValidationReportRepository, user_presence::UserPresence,
};
use domain::ProposalId;
use order_execution::BrokerTrade;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub trait ApprovalServiceTypes: Send + Sync + 'static {
    type Broker: BrokerRead + 'static;
    type Market: MarketInfo + 'static;
    type Settings: SettingsRepository + 'static;
    type Rates: OrderRateRepository + 'static;
    type Proposals: ProposalRepository + 'static;
    type Consumption: ProposalConsumptionRepository + 'static;
    type Reports: ValidationReportRepository + 'static;
    type Audit: AuditLog + 'static;
    type Contexts: ProposalContextPort + 'static;
    type Redrafter: ExpiredProposalRedrafter + 'static;
    type Freeze: SymbolFreezePort + 'static;
    type Clock: Clock + 'static;
    type Presence: UserPresence + 'static;
    type TxPort: PreSubmitTransactionPort + 'static;
    type Trade: BrokerTrade + 'static;
}

pub struct ApprovalServiceParts<T: ApprovalServiceTypes> {
    pub broker: Arc<T::Broker>,
    pub market: Arc<T::Market>,
    pub settings: Arc<T::Settings>,
    pub rates: Arc<T::Rates>,
    pub proposals: Arc<T::Proposals>,
    pub consumption: Arc<T::Consumption>,
    pub reports: Arc<T::Reports>,
    pub audit: Arc<T::Audit>,
    pub contexts: Arc<T::Contexts>,
    pub redrafter: Arc<T::Redrafter>,
    pub freeze: Arc<T::Freeze>,
    pub clock: Arc<T::Clock>,
    pub presence: Arc<T::Presence>,
    pub tx_port: Arc<T::TxPort>,
    pub trade: Arc<T::Trade>,
    pub events: EventBus,
}

impl<T: ApprovalServiceTypes> fmt::Debug for ApprovalServiceParts<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApprovalServiceParts(<redacted>)")
    }
}

pub trait OrderApprovalHandler: Send + Sync {
    fn request_order_approval<'a>(
        &'a self,
        proposal_id: &'a ProposalId,
    ) -> Pin<
        Box<dyn Future<Output = Result<ApprovalCommandOutcome, ApprovalCommandError>> + Send + 'a>,
    >;
}

pub struct ApprovalService<T: ApprovalServiceTypes> {
    parts: ApprovalServiceParts<T>,
    guard: ApprovalGuard,
    vault: UnknownOrderVault,
}

impl<T: ApprovalServiceTypes> fmt::Debug for ApprovalService<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApprovalService(<redacted>)")
    }
}

impl<T: ApprovalServiceTypes> ApprovalService<T> {
    #[must_use]
    pub fn new(parts: ApprovalServiceParts<T>) -> Self {
        Self {
            parts,
            guard: ApprovalGuard::default(),
            vault: UnknownOrderVault::new(),
        }
    }

    #[must_use]
    pub fn with_guard(parts: ApprovalServiceParts<T>, guard: ApprovalGuard) -> Self {
        Self {
            parts,
            guard,
            vault: UnknownOrderVault::new(),
        }
    }

    pub async fn request(
        &self,
        proposal_id: &ProposalId,
    ) -> Result<ApprovalCommandOutcome, ApprovalCommandError> {
        let parts = &self.parts;
        let command = ApprovalCommand::new(
            &self.guard,
            ApprovalReadPorts {
                broker: parts.broker.as_ref(),
                market: parts.market.as_ref(),
                settings: parts.settings.as_ref(),
                rates: parts.rates.as_ref(),
            },
            ApprovalStoragePorts {
                proposals: parts.proposals.as_ref(),
                consumption: parts.consumption.as_ref(),
                reports: parts.reports.as_ref(),
                audit: parts.audit.as_ref(),
            },
            ApprovalContextPorts {
                contexts: parts.contexts.as_ref(),
                redrafter: parts.redrafter.as_ref(),
                freeze: parts.freeze.as_ref(),
                clock: parts.clock.as_ref(),
                events: &parts.events,
            },
            ApprovalExecutionPorts {
                presence: parts.presence.as_ref(),
                tx_port: parts.tx_port.as_ref(),
                trade: parts.trade.as_ref(),
                vault: &self.vault,
            },
        );
        command.request_order_approval(proposal_id).await
    }
}

impl<T: ApprovalServiceTypes> OrderApprovalHandler for ApprovalService<T> {
    fn request_order_approval<'a>(
        &'a self,
        proposal_id: &'a ProposalId,
    ) -> Pin<
        Box<dyn Future<Output = Result<ApprovalCommandOutcome, ApprovalCommandError>> + Send + 'a>,
    > {
        Box::pin(self.request(proposal_id))
    }
}
