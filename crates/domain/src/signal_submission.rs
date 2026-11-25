use crate::{
    AiProbability, CalibratedProbability, CalibrationError, CalibrationKey, CalibrationLedgerHead,
    CalibrationSnapshot, CostEligibleTask, EstimateTask, Evidence, Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalTaskKey(EstimateTask);
impl SignalTaskKey {
    #[must_use]
    pub fn from_task(task: &CostEligibleTask) -> Self {
        Self(task.task().clone())
    }
    pub fn restore_from_saved_state(
        saved: crate::SavedEstimateTask,
    ) -> Result<Self, crate::NoEstimateTask> {
        EstimateTask::restore_from_saved_state(saved).map(Self)
    }
    #[must_use]
    pub const fn task(&self) -> &EstimateTask {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalSubmissionError {
    BeforeTask,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalSubmission {
    task: SignalTaskKey,
    key: CalibrationKey,
    probability: AiProbability,
    evidence: Option<Evidence>,
    submitted_at: Timestamp,
}
impl SignalSubmission {
    pub fn create(
        task: SignalTaskKey,
        key: CalibrationKey,
        probability: AiProbability,
        evidence: Option<Evidence>,
        submitted_at: Timestamp,
    ) -> Result<Self, SignalSubmissionError> {
        if submitted_at < task.task().created_at() {
            return Err(SignalSubmissionError::BeforeTask);
        }
        Ok(Self {
            task,
            key,
            probability,
            evidence,
            submitted_at,
        })
    }
    #[must_use]
    pub const fn task_key(&self) -> &SignalTaskKey {
        &self.task
    }
    #[must_use]
    pub const fn calibration_key(&self) -> &CalibrationKey {
        &self.key
    }
    #[must_use]
    pub const fn probability(&self) -> AiProbability {
        self.probability
    }
    #[must_use]
    pub const fn evidence(&self) -> Option<&Evidence> {
        self.evidence.as_ref()
    }
    #[must_use]
    pub const fn submitted_at(&self) -> Timestamp {
        self.submitted_at
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmittedCalibration {
    submission: SignalSubmission,
    calibration: CalibratedProbability,
    through: CalibrationLedgerHead,
}
impl SubmittedCalibration {
    #[must_use]
    pub const fn submission(&self) -> &SignalSubmission {
        &self.submission
    }
    #[must_use]
    pub const fn calibration(&self) -> &CalibratedProbability {
        &self.calibration
    }
    #[must_use]
    pub const fn through(&self) -> &CalibrationLedgerHead {
        &self.through
    }
}
pub fn calibrate_signal_submission(
    snapshot: Option<&CalibrationSnapshot>,
    head: &CalibrationLedgerHead,
    submission: SignalSubmission,
) -> Result<SubmittedCalibration, CalibrationError> {
    if head.key() != submission.calibration_key() {
        return Err(CalibrationError::MismatchedCombination);
    }
    let present = snapshot.ok_or(CalibrationError::MissingCalibration)?;
    if present.through().key() != head.key() {
        return Err(CalibrationError::MismatchedCombination);
    }
    if present.through().position() != head.position() {
        return Err(CalibrationError::StaleCalibration);
    }
    let probability = present.calibration().calibrate_bound(
        submission.probability(),
        submission.task_key().task().base_probability(),
    );
    Ok(SubmittedCalibration {
        submission,
        calibration: probability,
        through: head.clone(),
    })
}
