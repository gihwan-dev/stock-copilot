use crate::{RiskAppetite, Timestamp, WeightCaps};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome<Value> {
    Complete(Value),
    Partial(Value),
    Failed,
    AccessBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRecord<Value> {
    recorded_at: Timestamp,
    outcome: SyncOutcome<Value>,
}

impl<Value> SyncRecord<Value> {
    #[must_use]
    pub const fn new(recorded_at: Timestamp, outcome: SyncOutcome<Value>) -> Self {
        Self {
            recorded_at,
            outcome,
        }
    }
    #[must_use]
    pub const fn recorded_at(&self) -> Timestamp {
        self.recorded_at
    }
    #[must_use]
    pub const fn outcome(&self) -> &SyncOutcome<Value> {
        &self.outcome
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskSettingsChange {
    EnableOrders,
    AllowMarketOrders,
    AllowHours,
    RelaxWeightCaps(WeightCaps),
    IncreaseRiskAppetite(RiskAppetite),
}
