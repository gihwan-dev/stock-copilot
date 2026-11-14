use crate::{MasterCode, ResolvedOrderInstrument, Timestamp};
use std::fmt;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StockWarning {
    LiquidationTrading,
    Overheated,
    InvestmentWarning,
    InvestmentRisk,
    ViStaticAndDynamic,
    ViStatic,
    ViDynamic,
    StockWarrants,
    Unknown(MasterCode),
}
impl StockWarning {
    #[must_use]
    pub fn from_code(code: MasterCode) -> Self {
        match code.as_str() {
            "LIQUIDATION_TRADING" => Self::LiquidationTrading,
            "OVERHEATED" => Self::Overheated,
            "INVESTMENT_WARNING" => Self::InvestmentWarning,
            "INVESTMENT_RISK" => Self::InvestmentRisk,
            "VI_STATIC_AND_DYNAMIC" => Self::ViStaticAndDynamic,
            "VI_STATIC" => Self::ViStatic,
            "VI_DYNAMIC" => Self::ViDynamic,
            "STOCK_WARRANTS" => Self::StockWarrants,
            _ => Self::Unknown(code),
        }
    }
}
#[derive(Clone, Eq)]
pub struct StockWarnings {
    instrument: ResolvedOrderInstrument,
    warnings: Vec<StockWarning>,
    observed_at: Timestamp,
}
impl StockWarnings {
    #[must_use]
    pub fn from_lookup_result(
        instrument: ResolvedOrderInstrument,
        warnings: Vec<StockWarning>,
        observed_at: Timestamp,
    ) -> Self {
        let mut unique = Vec::new();
        for warning in warnings {
            if !unique.contains(&warning) {
                unique.push(warning);
            }
        }
        Self {
            instrument,
            warnings: unique,
            observed_at,
        }
    }
    #[must_use]
    pub fn instrument(&self) -> &ResolvedOrderInstrument {
        &self.instrument
    }
    #[must_use]
    pub fn warnings(&self) -> &[StockWarning] {
        &self.warnings
    }
    #[must_use]
    pub fn observed_at(&self) -> Timestamp {
        self.observed_at
    }
}
impl PartialEq for StockWarnings {
    fn eq(&self, other: &Self) -> bool {
        self.instrument == other.instrument
            && self.observed_at == other.observed_at
            && self.warnings.len() == other.warnings.len()
            && self
                .warnings
                .iter()
                .all(|warning| other.warnings.contains(warning))
    }
}
impl fmt::Debug for StockWarnings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StockWarnings").finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StockWarningDecision {
    Clear,
    Blocked,
    DifferentInstrument,
}
#[must_use]
pub fn check_stock_warnings(
    fact: &StockWarnings,
    instrument: &ResolvedOrderInstrument,
) -> StockWarningDecision {
    if fact.instrument() != instrument {
        StockWarningDecision::DifferentInstrument
    } else if fact.warnings().is_empty() {
        StockWarningDecision::Clear
    } else {
        StockWarningDecision::Blocked
    }
}
