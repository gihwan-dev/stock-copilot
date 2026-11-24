use crate::script::State;
use application::resolve_estimates::{
    EstimateResolutionRepository, ResolutionCalibration, ResolutionCommitOutcome,
    ResolutionDataIssue,
};
use application::{
    AiRunId, EstimateCommitOutcome, EstimateRepository, EstimateSubmissionRepository,
    EstimateTaskRepository, ReasoningEffort, RecordOutcome, SystemFailure, SystemPort,
    SystemPortError, ValuationDayRepository,
};
use domain::{
    AccountId, CalendarDate, Calibration, CalibrationEstimateId, CalibrationKey,
    CalibrationLedgerHead, CalibrationPosition, CalibrationSnapshot, CostEligibleTask,
    EstimateAssignments, EstimateOutcome, EstimateSubmissionRecord, InstrumentId, SignalSubmission,
    SignalTaskKey, ValuationDayResult,
};
use std::future::{Future, ready};

fn integrity_error() -> SystemPortError {
    SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity)
}

#[derive(Debug, Default)]
struct EstimateLedgerState {
    assignments: EstimateAssignments<AiRunId>,
    submission_records: Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>,
    submissions: Vec<SignalSubmission>,
    resolutions: Vec<(SignalTaskKey, EstimateOutcome)>,
    task_resolutions: Vec<(CalibrationEstimateId, EstimateOutcome)>,
    unavailable: Vec<(CalibrationEstimateId, ResolutionDataIssue)>,
    calibrations: Vec<(CalibrationKey, u64, Calibration)>,
}

impl EstimateLedgerState {
    fn issue_task(
        &mut self,
        id: &CalibrationEstimateId,
        run: &AiRunId,
        task: &CostEligibleTask,
    ) -> Result<RecordOutcome, SystemPortError> {
        if let Ok(existing) = self.assignments.lookup(id) {
            if existing.run() == run && existing.task() == task {
                return Ok(RecordOutcome::AlreadyRecorded);
            }
            return Err(integrity_error());
        }
        self.assignments = self
            .assignments
            .issue(id.clone(), run.clone(), task.clone())
            .map_err(|_error| integrity_error())?;
        Ok(RecordOutcome::Recorded)
    }

    fn append_calibration(
        &mut self,
        key: &CalibrationKey,
        calibration: &Calibration,
    ) -> Result<RecordOutcome, SystemPortError> {
        if calibration.key() != key {
            return Err(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::InvalidData,
            ));
        }
        if self
            .calibrations
            .iter()
            .any(|(stored_key, _, stored)| stored_key == key && stored == calibration)
        {
            return Ok(RecordOutcome::AlreadyRecorded);
        }
        let next_position = self
            .calibrations
            .iter()
            .filter(|(stored_key, _, _)| stored_key == key)
            .map(|(_, pos, _)| *pos)
            .max()
            .map_or(Some(0_u64), |last| last.checked_add(1))
            .ok_or_else(integrity_error)?;
        self.calibrations
            .push((key.clone(), next_position, calibration.clone()));
        Ok(RecordOutcome::Recorded)
    }

    fn resolution_calibration(
        &self,
        key: &CalibrationKey,
    ) -> Result<Option<ResolutionCalibration>, SystemPortError> {
        self.calibrations
            .iter()
            .rev()
            .find(|(stored_key, _, _)| stored_key == key)
            .map(|(_, pos, cal)| {
                let head = CalibrationLedgerHead::new(key.clone(), CalibrationPosition::new(*pos));
                CalibrationSnapshot::try_new(cal.clone(), head.clone())
                    .map(|snapshot| ResolutionCalibration { snapshot, head })
                    .map_err(|_error| integrity_error())
            })
            .transpose()
    }

    fn commit_resolution(
        &mut self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        outcome: &EstimateOutcome,
        previous: &ResolutionCalibration,
        next: &Calibration,
    ) -> Result<ResolutionCommitOutcome, SystemPortError> {
        let id = record.task_id().as_estimate_id();
        let key = record.submission().calibration_key();
        if self
            .task_resolutions
            .iter()
            .any(|(candidate, _)| candidate == id)
        {
            return Ok(ResolutionCommitOutcome::AlreadyResolved);
        }
        let current = self.resolution_calibration(key)?;
        if current.as_ref() != Some(previous) {
            return Ok(ResolutionCommitOutcome::Conflict);
        }
        let next_position = previous
            .head
            .position()
            .as_u64()
            .checked_add(1)
            .ok_or_else(integrity_error)?;
        let next_head =
            CalibrationLedgerHead::new(key.clone(), CalibrationPosition::new(next_position));
        let _verified = CalibrationSnapshot::try_new(next.clone(), next_head)
            .map_err(|_error| integrity_error())?;
        self.task_resolutions.push((id.clone(), outcome.clone()));
        self.calibrations
            .push((key.clone(), next_position, next.clone()));
        Ok(ResolutionCommitOutcome::Committed)
    }
}

#[derive(Debug, Default)]
pub struct MemoryEstimateLedger {
    state: State<EstimateLedgerState>,
}

impl MemoryEstimateLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl EstimateTaskRepository for MemoryEstimateLedger {
    fn issue_task(
        &self,
        id: &CalibrationEstimateId,
        run: &AiRunId,
        task: &CostEligibleTask,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        ready(self.state.write().issue_task(id, run, task))
    }
}

impl EstimateRepository for MemoryEstimateLedger {
    fn append_submission(
        &self,
        submission: &SignalSubmission,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        let mut state = self.state.write();
        let outcome = if state.submissions.contains(submission) {
            RecordOutcome::AlreadyRecorded
        } else {
            state.submissions.push(submission.clone());
            RecordOutcome::Recorded
        };
        ready(Ok(outcome))
    }

    fn submissions(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<SignalSubmission>, SystemPortError>> + Send {
        let state = self.state.write();
        let items = state
            .submissions
            .iter()
            .filter(|item| item.task_key().task().volatility().instrument() == instrument)
            .cloned()
            .collect();
        ready(Ok(items))
    }

    fn append_resolution(
        &self,
        task: &SignalTaskKey,
        outcome: &EstimateOutcome,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        let mut state = self.state.write();
        let result = if state.resolutions.iter().any(|(key, _)| key == task) {
            RecordOutcome::AlreadyRecorded
        } else {
            state.resolutions.push((task.clone(), outcome.clone()));
            RecordOutcome::Recorded
        };
        ready(Ok(result))
    }

    fn resolution(
        &self,
        task: &SignalTaskKey,
    ) -> impl Future<Output = Result<Option<EstimateOutcome>, SystemPortError>> + Send {
        let state = self.state.write();
        let found = state
            .resolutions
            .iter()
            .find(|(key, _)| key == task)
            .map(|(_, value)| value.clone());
        ready(Ok(found))
    }

    fn append_calibration(
        &self,
        key: &CalibrationKey,
        calibration: &Calibration,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        ready(self.state.write().append_calibration(key, calibration))
    }

    fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> impl Future<Output = Result<Option<Calibration>, SystemPortError>> + Send {
        let state = self.state.write();
        let found = state
            .calibrations
            .iter()
            .rev()
            .find(|(stored_key, _, _)| stored_key == key)
            .map(|(_, _, value)| value.clone());
        ready(Ok(found))
    }
}

impl EstimateSubmissionRepository for MemoryEstimateLedger {
    fn submission_records(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<
        Output = Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError>,
    > + Send {
        let state = self.state.write();
        let items = state
            .submission_records
            .iter()
            .filter(|record| {
                record
                    .submission()
                    .task_key()
                    .task()
                    .volatility()
                    .instrument()
                    == instrument
            })
            .cloned()
            .collect();
        ready(Ok(items))
    }

    fn assignments(
        &self,
    ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send {
        ready(Ok(self.state.write().assignments.clone()))
    }

    fn commit_submission(
        &self,
        previous: &EstimateAssignments<AiRunId>,
        next: &EstimateAssignments<AiRunId>,
        records: &[EstimateSubmissionRecord<AiRunId, ReasoningEffort>],
    ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send {
        let mut state = self.state.write();
        let outcome = if &state.assignments == previous {
            state.assignments = next.clone();
            state.submission_records.extend_from_slice(records);
            EstimateCommitOutcome::Committed
        } else {
            EstimateCommitOutcome::Conflict
        };
        ready(Ok(outcome))
    }
}

impl EstimateResolutionRepository for MemoryEstimateLedger {
    fn pending_submissions(
        &self,
    ) -> impl Future<
        Output = Result<Vec<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>, SystemPortError>,
    > + Send {
        let state = self.state.write();
        let items = state
            .submission_records
            .iter()
            .filter(|record| {
                !state
                    .task_resolutions
                    .iter()
                    .any(|(id, _)| id == record.task_id().as_estimate_id())
            })
            .cloned()
            .collect();
        ready(Ok(items))
    }

    fn calibration(
        &self,
        key: &CalibrationKey,
    ) -> impl Future<Output = Result<Option<ResolutionCalibration>, SystemPortError>> + Send {
        ready(self.state.write().resolution_calibration(key))
    }

    fn resolution(
        &self,
        id: &CalibrationEstimateId,
    ) -> impl Future<Output = Result<Option<EstimateOutcome>, SystemPortError>> + Send {
        let state = self.state.write();
        let found = state
            .task_resolutions
            .iter()
            .find(|(candidate, _)| candidate == id)
            .map(|(_, outcome)| outcome.clone());
        ready(Ok(found))
    }

    fn record_unavailable(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        reason: ResolutionDataIssue,
    ) -> impl Future<Output = Result<ResolutionCommitOutcome, SystemPortError>> + Send {
        let mut state = self.state.write();
        let id = record.task_id().as_estimate_id();
        let outcome = if state
            .task_resolutions
            .iter()
            .any(|(candidate, _)| candidate == id)
        {
            ResolutionCommitOutcome::AlreadyResolved
        } else {
            state.unavailable.push((id.clone(), reason));
            ResolutionCommitOutcome::Committed
        };
        ready(Ok(outcome))
    }

    fn commit_resolution(
        &self,
        record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        outcome: &EstimateOutcome,
        previous: &ResolutionCalibration,
        next: &Calibration,
    ) -> impl Future<Output = Result<ResolutionCommitOutcome, SystemPortError>> + Send {
        ready(
            self.state
                .write()
                .commit_resolution(record, outcome, previous, next),
        )
    }
}

#[derive(Debug, Default)]
struct ValuationDayState {
    records: Vec<(AccountId, CalendarDate, ValuationDayResult)>,
}

impl ValuationDayState {
    fn record(
        &mut self,
        account: &AccountId,
        result: &ValuationDayResult,
    ) -> Result<RecordOutcome, SystemPortError> {
        if let Some((_, _, existing)) =
            self.records
                .iter()
                .find(|(stored_account, stored_date, _)| {
                    stored_account == account && *stored_date == result.date()
                })
        {
            if existing == result {
                return Ok(RecordOutcome::AlreadyRecorded);
            }
            return Err(integrity_error());
        }
        self.records
            .push((account.clone(), result.date(), result.clone()));
        Ok(RecordOutcome::Recorded)
    }
}

#[derive(Debug, Default)]
pub struct MemoryValuationDayRepository {
    state: State<ValuationDayState>,
}

impl MemoryValuationDayRepository {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    #[must_use]
    pub fn day_result(
        &self,
        account: &AccountId,
        date: CalendarDate,
    ) -> Option<ValuationDayResult> {
        self.state
            .write()
            .records
            .iter()
            .find(|(stored_account, stored_date, _)| {
                stored_account == account && *stored_date == date
            })
            .map(|(_, _, value)| value.clone())
    }
}

impl ValuationDayRepository for MemoryValuationDayRepository {
    fn record(
        &self,
        account: &AccountId,
        result: &ValuationDayResult,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send {
        ready(self.state.write().record(account, result))
    }
}
