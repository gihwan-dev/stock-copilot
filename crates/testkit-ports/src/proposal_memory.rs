use crate::{Script, memory::MemoryReply, script::State};
use application::{
    ConsumptionOutcome, ConsumptionStatus, ProposalConsumptionRepository, SystemPortError,
    ValidationReportRepository, WriteCompleted,
};
use domain::{ProposalId, ValidationReport};

pub type ConsumeScript =
    Script<ProposalId, MemoryReply<Result<ConsumptionOutcome, SystemPortError>>>;
pub type StatusScript = Script<ProposalId, MemoryReply<Result<ConsumptionStatus, SystemPortError>>>;
pub type AppendReportsScript = Script<
    (ProposalId, Vec<ValidationReport>),
    MemoryReply<Result<WriteCompleted, SystemPortError>>,
>;
pub type ReportHistoryScript =
    Script<ProposalId, MemoryReply<Result<Vec<ValidationReport>, SystemPortError>>>;

#[derive(Debug, Default)]
pub struct MemoryProposalConsumptionRepository {
    consume: ConsumeScript,
    status: StatusScript,
    consumed: State<Vec<ProposalId>>,
}
impl MemoryProposalConsumptionRepository {
    #[must_use]
    pub const fn script_consume(&self) -> &ConsumeScript {
        &self.consume
    }
    #[must_use]
    pub const fn script_status(&self) -> &StatusScript {
        &self.status
    }
}
impl ProposalConsumptionRepository for MemoryProposalConsumptionRepository {
    async fn consume(&self, proposal: &ProposalId) -> Result<ConsumptionOutcome, SystemPortError> {
        match self.consume.run(proposal.clone()).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut consumed = self.consumed.write();
                if consumed.contains(proposal) {
                    return Ok(ConsumptionOutcome::AlreadyConsumed);
                }
                consumed.push(proposal.clone());
                Ok(ConsumptionOutcome::Recorded)
            }
        }
    }
    async fn status(&self, proposal: &ProposalId) -> Result<ConsumptionStatus, SystemPortError> {
        match self.status.run(proposal.clone()).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                if self.consumed.write().contains(proposal) {
                    Ok(ConsumptionStatus::Consumed)
                } else {
                    Ok(ConsumptionStatus::NotConsumed)
                }
            }
        }
    }
}
#[derive(Debug, Default)]
pub struct MemoryValidationReportRepository {
    append: AppendReportsScript,
    history: ReportHistoryScript,
    reports: State<Vec<(ProposalId, ValidationReport)>>,
}
impl MemoryValidationReportRepository {
    #[must_use]
    pub const fn script_append(&self) -> &AppendReportsScript {
        &self.append
    }
    #[must_use]
    pub const fn script_history(&self) -> &ReportHistoryScript {
        &self.history
    }
}
impl ValidationReportRepository for MemoryValidationReportRepository {
    async fn append(
        &self,
        proposal: &ProposalId,
        reports: &[ValidationReport],
    ) -> Result<WriteCompleted, SystemPortError> {
        match self.append.run((proposal.clone(), reports.to_vec())).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => {
                let mut state = self.reports.write();
                for report in reports {
                    state.push((proposal.clone(), report.clone()));
                }
                Ok(WriteCompleted)
            }
        }
    }
    async fn history(
        &self,
        proposal: &ProposalId,
    ) -> Result<Vec<ValidationReport>, SystemPortError> {
        match self.history.run(proposal.clone()).await {
            MemoryReply::Return(value) => value,
            MemoryReply::Memory => Ok(self
                .reports
                .write()
                .iter()
                .filter(|(id, _)| id == proposal)
                .map(|(_, report)| report.clone())
                .collect()),
        }
    }
}
