use crate::{SystemPortError, WriteCompleted};
use domain::{ProposalId, ValidationReport};
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumptionStatus {
    NotConsumed,
    Consumed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsumptionOutcome {
    Recorded,
    AlreadyConsumed,
}

pub trait ProposalConsumptionRepository: Send + Sync {
    fn consume(
        &self,
        proposal: &ProposalId,
    ) -> impl Future<Output = Result<ConsumptionOutcome, SystemPortError>> + Send;
    fn status(
        &self,
        proposal: &ProposalId,
    ) -> impl Future<Output = Result<ConsumptionStatus, SystemPortError>> + Send;
}

pub trait ValidationReportRepository: Send + Sync {
    fn append(
        &self,
        proposal: &ProposalId,
        reports: &[ValidationReport],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
    fn history(
        &self,
        proposal: &ProposalId,
    ) -> impl Future<Output = Result<Vec<ValidationReport>, SystemPortError>> + Send;
}
