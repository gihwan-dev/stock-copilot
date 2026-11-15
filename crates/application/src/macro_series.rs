use crate::{DataAvailability, IdentifierError, InformationError, MetricValue};
use domain::{CalendarDate, Symbol};
use std::future::Future;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroSeriesId(String);
impl MacroSeriesId {
    pub fn try_new(value: &str) -> Result<Self, IdentifierError> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
        {
            return Err(IdentifierError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroUnit {
    Percent,
    IndexPoints,
    Usd,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroSource {
    UsTreasury,
    Fred,
    TossEtf(Symbol),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroFact {
    value: MetricValue,
    unit: MacroUnit,
    as_of: CalendarDate,
    source: MacroSource,
}
impl MacroFact {
    #[must_use]
    pub const fn new(
        value: MetricValue,
        unit: MacroUnit,
        as_of: CalendarDate,
        source: MacroSource,
    ) -> Self {
        Self {
            value,
            unit,
            as_of,
            source,
        }
    }
    #[must_use]
    pub const fn value(&self) -> &MetricValue {
        &self.value
    }
    #[must_use]
    pub const fn unit(&self) -> MacroUnit {
        self.unit
    }
    #[must_use]
    pub const fn as_of(&self) -> CalendarDate {
        self.as_of
    }
    #[must_use]
    pub const fn source(&self) -> &MacroSource {
        &self.source
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacroObservation {
    series: MacroSeriesId,
    fact: MacroFact,
}
impl MacroObservation {
    #[must_use]
    pub const fn new(series: MacroSeriesId, fact: MacroFact) -> Self {
        Self { series, fact }
    }
    #[must_use]
    pub const fn series(&self) -> &MacroSeriesId {
        &self.series
    }
    #[must_use]
    pub const fn fact(&self) -> &MacroFact {
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
