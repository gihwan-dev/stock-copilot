use crate::scorecard::{ScorecardBin, ScorecardDisplay, ScorecardPoint};
use crate::{CalibrationBin, CalibrationCount};
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct ScorecardDeferredDisplay(String);
impl ScorecardDeferredDisplay {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for ScorecardDeferredDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ScorecardDeferredDisplay")
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScorecardDeferredNumber {
    JudgmentPending(ScorecardDeferredDisplay),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScorecardDeferredBrier {
    NoData,
    JudgmentPending(ScorecardDeferredDisplay),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScorecardDeferredCount {
    JudgmentPending(CalibrationCount),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScorecardDeferredBin {
    NoData(CalibrationBin),
    Observed(ScorecardDeferredPoint),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScorecardDeferredPoint(ScorecardPoint);
impl ScorecardDeferredPoint {
    #[must_use]
    pub fn bin(&self) -> CalibrationBin {
        self.0.bin()
    }
    #[must_use]
    pub fn count(&self) -> ScorecardDeferredCount {
        ScorecardDeferredCount::JudgmentPending(self.0.count().clone())
    }
    #[must_use]
    pub fn mean_prediction(&self) -> ScorecardDeferredNumber {
        ScorecardDeferredNumber::JudgmentPending(ScorecardDeferredDisplay(
            self.0.mean_prediction().as_str().to_owned(),
        ))
    }
    #[must_use]
    pub fn hit_rate(&self) -> ScorecardDeferredNumber {
        ScorecardDeferredNumber::JudgmentPending(ScorecardDeferredDisplay(
            self.0.hit_rate().as_str().to_owned(),
        ))
    }
}
pub(crate) fn defer_brier(input: Option<ScorecardDisplay>) -> ScorecardDeferredBrier {
    match input {
        None => ScorecardDeferredBrier::NoData,
        Some(value) => ScorecardDeferredBrier::JudgmentPending(ScorecardDeferredDisplay(
            value.as_str().to_owned(),
        )),
    }
}
pub(crate) fn defer_bin(value: ScorecardBin) -> ScorecardDeferredBin {
    match value {
        ScorecardBin::NoData(bin) => ScorecardDeferredBin::NoData(bin),
        ScorecardBin::Observed(point) => {
            ScorecardDeferredBin::Observed(ScorecardDeferredPoint(point))
        }
    }
}
