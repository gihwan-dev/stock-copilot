use crate::{
    AiRunId, DeliveryReport, EventBus, PortError, ReasoningEffort, SystemFailure, SystemPort,
    SystemPortError,
};
use domain::{
    Calibration, CalibrationError, CalibrationEstimateId, CalibrationKey, CalibrationLedgerHead,
    CalibrationSnapshot, Candle, DomainEvent, EstimateOutcome, EstimateSubmissionRecord,
    MarketCalendar, OutcomeAdjustment, OutcomeState, OutcomeUnavailable, Timestamp,
};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionInputs {
    pub calendars: Vec<MarketCalendar>,
    pub adjustment: OutcomeAdjustment,
    pub candles: Vec<Candle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionDataIssue {
    Incomplete(OutcomeUnavailable),
    Read(PortError),
    Storage(SystemPortError),
    Calibration(CalibrationError),
}

pub trait ResolutionMarketData: Send + Sync {
    fn inputs(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        now: Timestamp,
    ) -> impl Future<Output = Result<ResolutionInputs, ResolutionDataIssue>> + Send;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionCommitOutcome {
    Committed,
    AlreadyResolved,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionCalibration {
    pub snapshot: CalibrationSnapshot,
    pub head: CalibrationLedgerHead,
}

pub trait EstimateResolutionRepository: Send + Sync {
    fn pending_submissions(
        &self,
    ) -> impl Future<
        Output = Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError>,
    > + Send;
    fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> impl Future<Output = Result<Option<ResolutionCalibration>, SystemPortError>> + Send;
    fn resolution(
        &self,
        id: &CalibrationEstimateId,
    ) -> impl Future<Output = Result<Option<EstimateOutcome>, SystemPortError>> + Send;
    fn record_unavailable(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        reason: ResolutionDataIssue,
    ) -> impl Future<Output = Result<ResolutionCommitOutcome, SystemPortError>> + Send;
    fn commit_resolution(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        outcome: &EstimateOutcome,
        previous: &ResolutionCalibration,
        next: &Calibration,
    ) -> impl Future<Output = Result<ResolutionCommitOutcome, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionStatus {
    Resolved {
        outcome: EstimateOutcome,
        delivery: DeliveryReport,
    },
    Ongoing,
    Unavailable(ResolutionDataIssue),
    Storage(SystemPortError),
    Calibration(CalibrationError),
    MissingFact,
    AlreadyResolved,
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionReport {
    pub id: CalibrationEstimateId,
    pub status: ResolutionStatus,
}

#[derive(Debug)]
pub struct ResolveEstimates<'a, Repository, Data> {
    repository: &'a Repository,
    data: &'a Data,
    events: &'a EventBus,
}

impl<'a, Repository: EstimateResolutionRepository, Data: ResolutionMarketData>
    ResolveEstimates<'a, Repository, Data>
{
    #[must_use]
    pub const fn new(repository: &'a Repository, data: &'a Data, events: &'a EventBus) -> Self {
        Self {
            repository,
            data,
            events,
        }
    }

    pub async fn execute(&self, now: Timestamp) -> Result<Vec<ResolutionReport>, SystemPortError> {
        let records = self.repository.pending_submissions().await?;
        let mut reports = Vec::with_capacity(records.len());
        for record in records {
            let id = record.task_id().as_estimate_id().clone();
            let status = self.resolve(&record, now).await;
            reports.push(ResolutionReport { id, status });
        }
        Ok(reports)
    }

    async fn resolve(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        now: Timestamp,
    ) -> ResolutionStatus {
        let input = match self.data.inputs(record, now).await {
            Ok(value) => value,
            Err(reason) => return self.unavailable(record, reason).await,
        };
        let submission = record.submission();
        let outcome = EstimateOutcome::evaluate_with_complete_data(
            submission.task_key().task(),
            &input.calendars,
            &input.adjustment,
            &input.candles,
            now,
        );
        match outcome.state() {
            OutcomeState::Ongoing => return ResolutionStatus::Ongoing,
            OutcomeState::Unavailable(reason) => {
                return self
                    .unavailable(record, ResolutionDataIssue::Incomplete(reason))
                    .await;
            }
            OutcomeState::Success | OutcomeState::Failure(_) => {}
        }
        let Some(fact) = outcome.calibration_fact(
            record.task_id().as_estimate_id().clone(),
            submission.calibration_key().model().clone(),
            submission.probability(),
        ) else {
            return ResolutionStatus::MissingFact;
        };
        let key = submission.calibration_key();
        let previous = match self.repository.calibration(key).await {
            Ok(Some(value)) => value,
            Ok(None) => return ResolutionStatus::Calibration(CalibrationError::MissingCalibration),
            Err(error) => return ResolutionStatus::Storage(error),
        };
        if previous.head.key() != key {
            return ResolutionStatus::Calibration(CalibrationError::MismatchedCombination);
        }
        if previous.snapshot.through() != &previous.head {
            return ResolutionStatus::Calibration(CalibrationError::StaleCalibration);
        }
        let next = match previous.snapshot.calibration().with_fact(fact) {
            Ok(value) => value,
            Err(error) => {
                return self
                    .calibration_error(record, previous.snapshot.calibration(), error)
                    .await;
            }
        };
        match self
            .repository
            .commit_resolution(record, &outcome, &previous, &next)
            .await
        {
            Ok(ResolutionCommitOutcome::Committed) => self.resolved(record, outcome),
            Ok(ResolutionCommitOutcome::AlreadyResolved) => ResolutionStatus::AlreadyResolved,
            Ok(ResolutionCommitOutcome::Conflict) => ResolutionStatus::Conflict,
            Err(error) => ResolutionStatus::Storage(error),
        }
    }

    async fn calibration_error(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        previous: &Calibration,
        error: CalibrationError,
    ) -> ResolutionStatus {
        match error {
            CalibrationError::TooManyFacts => {
                self.unavailable(record, ResolutionDataIssue::Calibration(error))
                    .await
            }
            CalibrationError::ConflictingEstimate => {
                match self
                    .repository
                    .resolution(record.task_id().as_estimate_id())
                    .await
                {
                    Ok(Some(outcome)) => Self::existing_resolution(record, previous, &outcome),
                    Ok(None) => ResolutionStatus::Calibration(error),
                    Err(failure) => ResolutionStatus::Storage(failure),
                }
            }
            CalibrationError::InvalidModelIdentity
            | CalibrationError::InvalidEstimateId
            | CalibrationError::MismatchedCombination
            | CalibrationError::MissingCalibration
            | CalibrationError::StaleCalibration => ResolutionStatus::Calibration(error),
        }
    }

    fn existing_resolution(
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        previous: &Calibration,
        outcome: &EstimateOutcome,
    ) -> ResolutionStatus {
        let id = record.task_id().as_estimate_id();
        let expected = outcome.calibration_fact(
            id.clone(),
            record.submission().calibration_key().model().clone(),
            record.submission().probability(),
        );
        let stored = previous
            .facts()
            .iter()
            .find(|fact| fact.estimate().id() == id);
        match (expected.as_ref(), stored) {
            (Some(fact), Some(prior)) if fact == prior => ResolutionStatus::AlreadyResolved,
            _ => ResolutionStatus::Storage(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::RepositoryIntegrity,
            )),
        }
    }

    fn resolved(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        outcome: EstimateOutcome,
    ) -> ResolutionStatus {
        let delivery = self.events.publish(DomainEvent::EstimateResolved(
            record
                .submission()
                .task_key()
                .task()
                .volatility()
                .instrument()
                .clone(),
        ));
        ResolutionStatus::Resolved { outcome, delivery }
    }

    async fn unavailable(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        reason: ResolutionDataIssue,
    ) -> ResolutionStatus {
        match self.repository.record_unavailable(record, reason).await {
            Ok(ResolutionCommitOutcome::Committed) => ResolutionStatus::Unavailable(reason),
            Ok(ResolutionCommitOutcome::AlreadyResolved) => ResolutionStatus::AlreadyResolved,
            Ok(ResolutionCommitOutcome::Conflict) => ResolutionStatus::Conflict,
            Err(error) => ResolutionStatus::Storage(error),
        }
    }
}
