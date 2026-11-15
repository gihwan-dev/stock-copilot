use crate::{DataAvailability, InformationError};
use domain::{Amount, CalendarDate, Currency, Symbol};
use std::future::Future;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricUnit {
    Currency(Currency),
    Percent,
    IndexPoints,
    Multiple,
    Count,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetricSource {
    Dart,
    UsTreasury,
    Fred,
    TossEtf(Symbol),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricValue(Amount);
impl MetricValue {
    #[must_use]
    pub const fn new(value: Amount) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn amount(&self) -> &Amount {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricFact {
    value: MetricValue,
    unit: MetricUnit,
    as_of: CalendarDate,
    source: MetricSource,
}
impl MetricFact {
    #[must_use]
    pub const fn new(
        value: MetricValue,
        unit: MetricUnit,
        as_of: CalendarDate,
        source: MetricSource,
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
    pub const fn unit(&self) -> MetricUnit {
        self.unit
    }
    #[must_use]
    pub const fn as_of(&self) -> CalendarDate {
        self.as_of
    }
    #[must_use]
    pub const fn source(&self) -> &MetricSource {
        &self.source
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinancialLine {
    label: domain::UntrustedText,
    fact: MetricFact,
}
impl FinancialLine {
    #[must_use]
    pub const fn new(label: domain::UntrustedText, fact: MetricFact) -> Self {
        Self { label, fact }
    }
    #[must_use]
    pub const fn label(&self) -> &domain::UntrustedText {
        &self.label
    }
    #[must_use]
    pub const fn fact(&self) -> &MetricFact {
        &self.fact
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinancialStatement {
    symbol: Symbol,
    period_end: CalendarDate,
    lines: Vec<FinancialLine>,
}
impl FinancialStatement {
    #[must_use]
    pub const fn new(symbol: Symbol, period_end: CalendarDate, lines: Vec<FinancialLine>) -> Self {
        Self {
            symbol,
            period_end,
            lines,
        }
    }
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn period_end(&self) -> CalendarDate {
        self.period_end
    }
    #[must_use]
    pub const fn lines(&self) -> &Vec<FinancialLine> {
        &self.lines
    }
}

pub trait Fundamentals: Send + Sync {
    fn statements(
        &self,
        symbol: &Symbol,
        period_end: CalendarDate,
    ) -> impl Future<Output = Result<DataAvailability<Vec<FinancialStatement>>, InformationError>> + Send;
}
