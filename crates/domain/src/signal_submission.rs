use crate::{
    AiProbability, CalibratedProbability, Calibration, CalibrationKey, CostEligibleTask,
    EstimateTask, Evidence, Timestamp,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalTaskKey(EstimateTask);
impl SignalTaskKey {
    #[must_use]
    pub fn from_task(task: &CostEligibleTask) -> Self {
        Self(task.task().clone())
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
}
#[must_use]
pub fn calibrate_signal_submission(
    calibration: &Calibration,
    submission: SignalSubmission,
) -> SubmittedCalibration {
    let probability = calibration.calibrate(submission.calibration_key(), submission.probability());
    SubmittedCalibration {
        submission,
        calibration: probability,
    }
}
