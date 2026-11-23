use crate::{
    BaseProbability, Candle, EstimateContext, EstimateDistance, EstimateTask, Horizon,
    HoursPermission, HoursPermissionState, MasterInstrument, MasterInstrumentError,
    MasterInstrumentRow, NoEstimateTask, OrderSide, Price, Ratio, RiskAppetite, Timestamp,
    TradingDayHorizon, Volatility, VolatilityError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateRestoreEvidence {
    pub master: MasterInstrumentRow,
    pub side: OrderSide,
    pub candles: Vec<Candle>,
    pub reference_price: Price,
    pub reference_at: Timestamp,
    pub task_at: Timestamp,
    pub horizon: Horizon,
    pub permission: HoursPermissionState,
    pub appetite: RiskAppetite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateDisplayFacts {
    pub reference_price: Price,
    pub reference_at: Timestamp,
    pub created_at: Timestamp,
    pub horizon: TradingDayHorizon,
    pub target: Ratio,
    pub stop: Ratio,
    pub base_probability: Ratio,
}

impl EstimateDisplayFacts {
    fn from_task(task: &EstimateTask) -> Self {
        Self {
            reference_price: task.reference().price(),
            reference_at: task.reference().observed_at(),
            created_at: task.created_at(),
            horizon: task.horizon(),
            target: Ratio::new(task.target().as_decimal()),
            stop: Ratio::new(task.stop().as_decimal()),
            base_probability: Ratio::new(task.base_probability().as_decimal()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredEstimateEvidence {
    pub evidence: EstimateRestoreEvidence,
    pub original: EstimateRestoreEvidence,
    pub expected: EstimateDisplayFacts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateRestoreError {
    HistoricalMaster(MasterInstrumentError),
    CurrentMaster(MasterInstrumentError),
    CurrentInstrumentMismatch,
    CandleCurrencyMismatch,
    Volatility(VolatilityError),
    Task(NoEstimateTask),
    ChangedResult,
    ChangedEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalEstimate(EstimateTask);

impl HistoricalEstimate {
    #[must_use]
    pub fn reference_price(&self) -> Price {
        self.0.reference().price()
    }
    #[must_use]
    pub fn created_at(&self) -> Timestamp {
        self.0.created_at()
    }
    #[must_use]
    pub fn horizon(&self) -> TradingDayHorizon {
        self.0.horizon()
    }
    #[must_use]
    pub fn target(&self) -> EstimateDistance {
        self.0.target()
    }
    #[must_use]
    pub fn stop(&self) -> EstimateDistance {
        self.0.stop()
    }
    #[must_use]
    pub fn base_probability(&self) -> BaseProbability {
        self.0.base_probability()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MasterReplayMode {
    LiveOrderGate,
    HistoricalIdentity,
}

impl EstimateRestoreEvidence {
    pub fn capture(
        &self,
        original: &EstimateTask,
        current: &MasterInstrument,
    ) -> Result<StoredEstimateEvidence, EstimateRestoreError> {
        let (rebuilt, _) = self.replay(current, MasterReplayMode::LiveOrderGate)?;
        if &rebuilt != original {
            return Err(EstimateRestoreError::ChangedResult);
        }
        check_task_offsets(&rebuilt, original)?;
        Ok(StoredEstimateEvidence {
            evidence: self.clone(),
            original: self.clone(),
            expected: EstimateDisplayFacts::from_task(original),
        })
    }

    pub(crate) fn replay(
        &self,
        current: &MasterInstrument,
        mode: MasterReplayMode,
    ) -> Result<(EstimateTask, crate::QuoteInstrument), EstimateRestoreError> {
        let historical = MasterInstrument::try_new(self.master.clone())
            .map_err(EstimateRestoreError::HistoricalMaster)?;
        let old_quote = historical
            .quote_instrument()
            .map_err(EstimateRestoreError::HistoricalMaster)?;
        let fresh_quote = current
            .quote_instrument()
            .map_err(EstimateRestoreError::CurrentMaster)?;
        if old_quote != fresh_quote {
            return Err(EstimateRestoreError::CurrentInstrumentMismatch);
        }
        historical
            .order_gate_facts()
            .and_then(|facts| facts.instrument_for(self.side))
            .map_err(EstimateRestoreError::HistoricalMaster)?;
        let reference = match mode {
            MasterReplayMode::LiveOrderGate => current
                .price_observation_for(self.side, self.reference_price, self.reference_at)
                .map_err(EstimateRestoreError::CurrentMaster)?,
            MasterReplayMode::HistoricalIdentity => fresh_quote
                .historical_price_observation(self.reference_price, self.reference_at)
                .map_err(EstimateRestoreError::CurrentMaster)?,
        };
        if self
            .candles
            .iter()
            .any(|candle| candle.values().currency() != self.master.currency)
        {
            return Err(EstimateRestoreError::CandleCurrencyMismatch);
        }
        let volatility =
            Volatility::calculate(&self.candles).map_err(EstimateRestoreError::Volatility)?;
        let task = EstimateTask::create(
            EstimateContext::new(reference, self.task_at),
            volatility,
            self.horizon,
            HoursPermission::restore_from_saved_state(self.permission),
            self.appetite,
        )
        .map_err(EstimateRestoreError::Task)?;
        Ok((task, fresh_quote))
    }
}

impl StoredEstimateEvidence {
    pub(crate) fn replay(
        &self,
        current: &MasterInstrument,
        mode: MasterReplayMode,
    ) -> Result<(EstimateTask, crate::QuoteInstrument), EstimateRestoreError> {
        let (result, quote) = self.evidence.replay(current, mode)?;
        if self.evidence.reference_at.offset_seconds()
            != self.original.reference_at.offset_seconds()
            || self.evidence.task_at.offset_seconds() != self.original.task_at.offset_seconds()
        {
            return Err(EstimateRestoreError::ChangedEvidence);
        }
        let actual = EstimateDisplayFacts::from_task(&result);
        if actual != self.expected
            || actual.reference_at.offset_seconds() != self.expected.reference_at.offset_seconds()
            || actual.created_at.offset_seconds() != self.expected.created_at.offset_seconds()
        {
            return Err(EstimateRestoreError::ChangedResult);
        }
        if self.evidence != self.original
            || self.evidence.master.observed_at.offset_seconds()
                != self.original.master.observed_at.offset_seconds()
            || self
                .evidence
                .candles
                .iter()
                .zip(&self.original.candles)
                .any(|(left, right)| {
                    left.time().timestamp().offset_seconds()
                        != right.time().timestamp().offset_seconds()
                        || left.collected_at().offset_seconds()
                            != right.collected_at().offset_seconds()
                        || left
                            .adjustment()
                            .adjustment_basis()
                            .map(|basis| basis.as_timestamp().offset_seconds())
                            != right
                                .adjustment()
                                .adjustment_basis()
                                .map(|basis| basis.as_timestamp().offset_seconds())
                })
        {
            return Err(EstimateRestoreError::ChangedEvidence);
        }
        Ok((result, quote))
    }

    pub fn restore_history(
        &self,
        current: &MasterInstrument,
    ) -> Result<HistoricalEstimate, EstimateRestoreError> {
        self.replay(current, MasterReplayMode::HistoricalIdentity)
            .map(|(task, _)| HistoricalEstimate(task))
    }
}

pub(crate) fn check_task_offsets(
    left: &EstimateTask,
    right: &EstimateTask,
) -> Result<(), EstimateRestoreError> {
    if left.created_at().offset_seconds() != right.created_at().offset_seconds()
        || left.reference().observed_at().offset_seconds()
            != right.reference().observed_at().offset_seconds()
        || left
            .volatility()
            .adjustment_basis()
            .as_timestamp()
            .offset_seconds()
            != right
                .volatility()
                .adjustment_basis()
                .as_timestamp()
                .offset_seconds()
    {
        return Err(EstimateRestoreError::ChangedResult);
    }
    Ok(())
}
