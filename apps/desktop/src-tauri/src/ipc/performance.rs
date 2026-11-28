use std::sync::Arc;

use application::{PerformanceRunError, PerformanceSummary};
use domain::{
    DisplayDirection, NumericInput, PerformanceSpan, PerformanceValue, Ratio, RiskMetric,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::briefing::kst_iso;
use super::error::IpcError;
use super::portfolio::{UiDirection, percent};
use crate::performance_book::{PerformanceBook, PeriodReading, SPANS};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetPerformanceRequest {
    pub scope: PerformanceScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum PerformanceScope {
    #[serde(rename = "current")]
    Current,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiPerformanceSpan {
    #[serde(rename = "1m")]
    OneMonth,
    #[serde(rename = "3m")]
    ThreeMonths,
    #[serde(rename = "6m")]
    SixMonths,
    #[serde(rename = "1y")]
    OneYear,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiSignedRate {
    pub rate: String,
    pub direction: UiDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiPerformanceStatus {
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "calculating")]
    Calculating,
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "missing-price")]
    MissingPrice,
    #[serde(rename = "missing-fx")]
    MissingFx,
    #[serde(rename = "missing-cash")]
    MissingCash,
    #[serde(rename = "insufficient-data")]
    InsufficientData,
    #[serde(rename = "load-failed")]
    LoadFailed,
    #[serde(rename = "non-positive-capital")]
    NonPositiveCapital,
    #[serde(rename = "unavailable")]
    Unavailable,
}

const fn failure_status(error: PerformanceRunError) -> UiPerformanceStatus {
    use domain::{PerformanceError, SnapshotError};
    match error {
        PerformanceRunError::Source(_) | PerformanceRunError::Repository(_) => {
            UiPerformanceStatus::LoadFailed
        }
        PerformanceRunError::EdgeDay(_, SnapshotError::MissingPrice) => {
            UiPerformanceStatus::MissingPrice
        }
        PerformanceRunError::EdgeDay(_, SnapshotError::MissingFx)
        | PerformanceRunError::Performance(PerformanceError::MissingFx) => {
            UiPerformanceStatus::MissingFx
        }
        PerformanceRunError::Cash(_, _)
        | PerformanceRunError::EdgeDay(_, SnapshotError::MissingCash) => {
            UiPerformanceStatus::MissingCash
        }
        PerformanceRunError::NoValuedDays
        | PerformanceRunError::Performance(PerformanceError::InvalidPeriod) => {
            UiPerformanceStatus::InsufficientData
        }
        PerformanceRunError::Performance(PerformanceError::NonPositiveCapital) => {
            UiPerformanceStatus::NonPositiveCapital
        }
        PerformanceRunError::Window(_)
        | PerformanceRunError::EdgeDay(_, _)
        | PerformanceRunError::Performance(_) => UiPerformanceStatus::Unavailable,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiPerformanceLine {
    pub span: UiPerformanceSpan,
    pub status: UiPerformanceStatus,
    pub period_return: Option<UiSignedRate>,
    pub max_drawdown: Option<UiSignedRate>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiPerformanceView {
    pub lines: Vec<UiPerformanceLine>,
}

const fn ui_span(span: PerformanceSpan) -> UiPerformanceSpan {
    match span {
        PerformanceSpan::OneMonth => UiPerformanceSpan::OneMonth,
        PerformanceSpan::ThreeMonths => UiPerformanceSpan::ThreeMonths,
        PerformanceSpan::SixMonths => UiPerformanceSpan::SixMonths,
        PerformanceSpan::OneYear => UiPerformanceSpan::OneYear,
    }
}

fn signed_rate(value: PerformanceValue) -> Option<UiSignedRate> {
    let ratio = Ratio::parse(&NumericInput::try_new(&value.to_string()).ok()?).ok()?;
    Some(UiSignedRate {
        rate: percent(&ratio)?,
        direction: match value.direction() {
            DisplayDirection::Up => UiDirection::Up,
            DisplayDirection::Down => UiDirection::Down,
            DisplayDirection::Flat => UiDirection::Flat,
        },
    })
}

fn period_line(span: PerformanceSpan, reading: Option<PeriodReading>) -> UiPerformanceLine {
    let status = match reading {
        Some(PeriodReading::Finished(Ok(found))) => return ready_line(span, &found),
        Some(PeriodReading::Finished(Err(error))) => failure_status(error),
        Some(PeriodReading::Waiting) => UiPerformanceStatus::Waiting,
        Some(PeriodReading::Calculating) => UiPerformanceStatus::Calculating,
        None => UiPerformanceStatus::Unavailable,
    };
    UiPerformanceLine {
        status,
        span: ui_span(span),
        period_return: None,
        max_drawdown: None,
        from: None,
        to: None,
    }
}

fn ready_line(span: PerformanceSpan, found: &PerformanceSummary) -> UiPerformanceLine {
    UiPerformanceLine {
        status: UiPerformanceStatus::Ready,
        span: ui_span(span),
        period_return: signed_rate(found.performance.twr()),
        max_drawdown: match found.drawdown {
            RiskMetric::Value(value) => signed_rate(value),
            RiskMetric::InsufficientSamples | RiskMetric::Unavailable(_) => None,
        },
        from: kst_iso(domain::valuation_time(found.performance.start())),
        to: kst_iso(domain::valuation_time(found.performance.end())),
    }
}

#[must_use]
pub fn performance_view(book: Option<&PerformanceBook>) -> UiPerformanceView {
    UiPerformanceView {
        lines: SPANS
            .iter()
            .map(|span| period_line(*span, book.map(|found| found.reading(*span))))
            .collect(),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_performance<R: Runtime>(
    app: AppHandle<R>,
    request: GetPerformanceRequest,
) -> Result<UiPerformanceView, IpcError> {
    let GetPerformanceRequest {
        scope: PerformanceScope::Current,
    } = request;
    let book = app
        .try_state::<Arc<PerformanceBook>>()
        .map(|state| Arc::clone(state.inner()));
    Ok(performance_view(book.as_deref()))
}

#[cfg(test)]
mod tests;
