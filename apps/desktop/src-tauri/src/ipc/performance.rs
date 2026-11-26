use std::sync::Arc;

use application::PerformanceSummary;
use domain::{
    DisplayDirection, NumericInput, PerformanceSpan, PerformanceValue, Ratio, RiskMetric,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::briefing::kst_iso;
use super::error::IpcError;
use super::portfolio::{UiDirection, percent};
use crate::performance_book::{PerformanceBook, SPANS};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiPerformanceLine {
    pub span: UiPerformanceSpan,
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

fn line(span: PerformanceSpan, summary: Option<&PerformanceSummary>) -> UiPerformanceLine {
    let Some(found) = summary else {
        return UiPerformanceLine {
            span: ui_span(span),
            period_return: None,
            max_drawdown: None,
            from: None,
            to: None,
        };
    };
    UiPerformanceLine {
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
            .map(|span| {
                let summary = book.and_then(|found| found.summary(*span));
                line(*span, summary.as_ref())
            })
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
