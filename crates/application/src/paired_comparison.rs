use crate::{
    AiRunId, AnalysisFailure, BackgroundBudget, Clock, CycleAdmission, ModelName, RecordOutcome,
    SystemPortError, TrackingTarget,
};
use domain::{
    InstrumentId, OffsetSeconds, PairingDecision, PairingRate, TimeError, Timestamp, paired_sample,
};
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonArm {
    SingleCall,
    Pipeline,
    Model(ModelName),
}

pub trait ComparisonArmRunner: Send + Sync {
    fn record_only(
        &self,
        arm: &ComparisonArm,
        target: &TrackingTarget,
    ) -> impl Future<Output = Result<AiRunId, AnalysisFailure>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparisonArmResult {
    pub arm: ComparisonArm,
    pub run: Result<AiRunId, AnalysisFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedComparison {
    pub instrument: InstrumentId,
    pub at: Timestamp,
    pub baseline: ComparisonArmResult,
    pub challenger: ComparisonArmResult,
}

pub trait PairedComparisonLog: Send + Sync {
    fn record(
        &self,
        pair: &PairedComparison,
    ) -> impl Future<Output = Result<RecordOutcome, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingRun {
    NotSampled,
    OverBudget(Option<Timestamp>),
    Interrupted,
    Recorded(PairedComparison),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairingError {
    Time(TimeError),
    Budget(SystemPortError),
    Log(SystemPortError),
}

#[derive(Debug)]
pub struct RunPairedComparison<'a, Runner, Log, Budget, Time> {
    pub runner: &'a Runner,
    pub log: &'a Log,
    pub budget: &'a Budget,
    pub clock: &'a Time,
    pub rate: PairingRate,
    pub baseline: ComparisonArm,
    pub challenger: ComparisonArm,
}

impl<Runner, Log, Budget, Time> RunPairedComparison<'_, Runner, Log, Budget, Time>
where
    Runner: ComparisonArmRunner,
    Log: PairedComparisonLog,
    Budget: BackgroundBudget,
    Time: Clock,
{
    async fn arm(&self, arm: &ComparisonArm, target: &TrackingTarget) -> ComparisonArmResult {
        ComparisonArmResult {
            arm: arm.clone(),
            run: self.runner.record_only(arm, target).await,
        }
    }

    pub async fn execute(&self, target: &TrackingTarget) -> Result<PairingRun, PairingError> {
        let now = self.clock.now();
        let day = now
            .date_at_offset(OffsetSeconds::new(32_400))
            .map_err(PairingError::Time)?;
        if paired_sample(target.instrument(), day, self.rate) == PairingDecision::Skip {
            return Ok(PairingRun::NotSampled);
        }
        if let CycleAdmission::Paused { resumes_at } =
            self.budget.admit(now).await.map_err(PairingError::Budget)?
        {
            return Ok(PairingRun::OverBudget(resumes_at));
        }
        let baseline = self.arm(&self.baseline, target).await;
        let challenger = self.arm(&self.challenger, target).await;
        self.budget
            .release(self.clock.now())
            .await
            .map_err(PairingError::Budget)?;
        if [&baseline, &challenger]
            .iter()
            .any(|result| result.run == Err(AnalysisFailure::Interrupted))
        {
            return Ok(PairingRun::Interrupted);
        }
        let pair = PairedComparison {
            instrument: target.instrument().clone(),
            at: now,
            baseline,
            challenger,
        };
        self.log.record(&pair).await.map_err(PairingError::Log)?;
        Ok(PairingRun::Recorded(pair))
    }
}
