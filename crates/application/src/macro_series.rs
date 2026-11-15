use crate::{DataAvailability, InformationError, MetricFact};
use domain::CalendarDate;
use std::future::Future;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroSeriesId(String);
impl MacroSeriesId {
    pub fn try_new(value: &str) -> Result<Self, InformationError> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        {
            return Err(InformationError::InvalidIdentifier);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroObservation {
    series: MacroSeriesId,
    fact: MetricFact,
}
impl MacroObservation {
    #[must_use]
    pub const fn new(series: MacroSeriesId, fact: MetricFact) -> Self {
        Self { series, fact }
    }
    #[must_use]
    pub const fn series(&self) -> &MacroSeriesId {
        &self.series
    }
    #[must_use]
    pub const fn fact(&self) -> &MetricFact {
        &self.fact
    }
}

pub trait MacroSeries: Send + Sync {
    fn daily_values(
        &self,
        series: &MacroSeriesId,
        from: CalendarDate,
        through: CalendarDate,
    ) -> impl Future<Output = Result<DataAvailability<Vec<MacroObservation>>, InformationError>> + Send;
}
