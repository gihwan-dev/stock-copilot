use std::fmt;
use std::sync::Arc;

use application::{
    Clock, GetScorecard, GetScorecardError, ScorecardObservationRepository, ScorecardSelection,
    ScorecardView,
};
use domain::{
    CalibrationBin, CalibrationKey, HorizonSelection, Scorecard, ScorecardBin, Scorecards,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};
use super::state::BoxFuture;
use application::ClassifiedError;

pub trait ScorecardSource: Send + Sync {
    fn load(&self) -> BoxFuture<'_, Result<ScorecardView, GetScorecardError>>;
}

struct RepositoryScorecards<R, C> {
    repository: Arc<R>,
    clock: Arc<C>,
}

impl<R: ScorecardObservationRepository + 'static, C: Clock + 'static> ScorecardSource
    for RepositoryScorecards<R, C>
{
    fn load(&self) -> BoxFuture<'_, Result<ScorecardView, GetScorecardError>> {
        Box::pin(async move {
            GetScorecard::new(self.repository.as_ref(), self.clock.as_ref())
                .execute(ScorecardSelection::All)
                .await
        })
    }
}

#[derive(Clone)]
pub struct ScorecardIpcState(Arc<dyn ScorecardSource>);

impl fmt::Debug for ScorecardIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ScorecardIpcState")
    }
}

impl ScorecardIpcState {
    #[must_use]
    pub fn from_repository<R: ScorecardObservationRepository + 'static, C: Clock + 'static>(
        repository: Arc<R>,
        clock: Arc<C>,
    ) -> Self {
        Self(Arc::new(RepositoryScorecards { repository, clock }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Type)]
pub enum ScorecardScope {
    #[serde(rename = "all")]
    All,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetScorecardRequest {
    pub scope: ScorecardScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiScorecardVerdict {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "ready")]
    Ready,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiScorecardBin {
    #[serde(rename = "below-50")]
    Below50,
    #[serde(rename = "50-60")]
    From50To60,
    #[serde(rename = "60-70")]
    From60To70,
    #[serde(rename = "70-plus")]
    AtLeast70,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiScorecardHorizon {
    #[serde(rename = "hours")]
    Hours,
    #[serde(rename = "few-days")]
    FewDays,
    #[serde(rename = "weeks-to-months")]
    WeeksToMonths,
    #[serde(rename = "half-year-or-more")]
    HalfYearOrMore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiScorecardBinRow {
    pub bin: UiScorecardBin,
    pub count: String,
    pub mean_prediction: Option<String>,
    pub hit_rate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiScorecardCombination {
    pub model: String,
    pub horizon: UiScorecardHorizon,
    pub judged: String,
    pub verdict: UiScorecardVerdict,
    pub brier: Option<String>,
    pub bins: Vec<UiScorecardBinRow>,
    pub comparisons: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiScorecardView {
    pub combinations: Vec<UiScorecardCombination>,
}

const fn bin(value: CalibrationBin) -> UiScorecardBin {
    match value {
        CalibrationBin::Below50 => UiScorecardBin::Below50,
        CalibrationBin::From50To60 => UiScorecardBin::From50To60,
        CalibrationBin::From60To70 => UiScorecardBin::From60To70,
        CalibrationBin::AtLeast70 => UiScorecardBin::AtLeast70,
    }
}

const fn horizon(selection: HorizonSelection) -> UiScorecardHorizon {
    match selection {
        HorizonSelection::Hours => UiScorecardHorizon::Hours,
        HorizonSelection::FewDays => UiScorecardHorizon::FewDays,
        HorizonSelection::WeeksToMonths => UiScorecardHorizon::WeeksToMonths,
        HorizonSelection::HalfYearOrMore => UiScorecardHorizon::HalfYearOrMore,
    }
}

fn model_name(key: &CalibrationKey) -> String {
    String::from(key.model().name().as_str())
}

fn combination(card: &Scorecard, comparisons: usize) -> UiScorecardCombination {
    match card {
        Scorecard::Pending(pending) => UiScorecardCombination {
            model: model_name(pending.key()),
            horizon: horizon(pending.key().horizon().selection()),
            judged: pending.judgment_count().as_usize().to_string(),
            verdict: UiScorecardVerdict::Pending,
            brier: None,
            bins: Vec::new(),
            comparisons: comparisons.to_string(),
        },
        Scorecard::Ready(metrics) => UiScorecardCombination {
            model: model_name(metrics.key()),
            horizon: horizon(metrics.key().horizon().selection()),
            judged: metrics.judgment_count().as_usize().to_string(),
            verdict: UiScorecardVerdict::Ready,
            brier: Some(String::from(metrics.brier().as_str())),
            bins: metrics
                .reliability()
                .iter()
                .map(|row| match row {
                    ScorecardBin::NoData(empty) => UiScorecardBinRow {
                        bin: bin(*empty),
                        count: String::from("0"),
                        mean_prediction: None,
                        hit_rate: None,
                    },
                    ScorecardBin::Observed(point) => UiScorecardBinRow {
                        bin: bin(point.bin()),
                        count: point.count().as_usize().to_string(),
                        mean_prediction: Some(String::from(point.mean_prediction().as_str())),
                        hit_rate: Some(String::from(point.hit_rate().as_str())),
                    },
                })
                .collect(),
            comparisons: comparisons.to_string(),
        },
    }
}

#[must_use]
pub fn to_scorecard_view(cards: &Scorecards) -> UiScorecardView {
    let comparisons = cards.comparison_count().as_usize();
    UiScorecardView {
        combinations: cards
            .cards()
            .iter()
            .map(|card| combination(card, comparisons))
            .collect(),
    }
}

pub async fn load_scorecard_view(state: &ScorecardIpcState) -> Result<UiScorecardView, IpcError> {
    match state.0.load().await {
        Ok(ScorecardView::Cards(cards)) => Ok(to_scorecard_view(&cards)),
        Ok(ScorecardView::Single(card)) => Ok(UiScorecardView {
            combinations: vec![combination(&card, 1)],
        }),
        Ok(ScorecardView::NoObservations | ScorecardView::UnknownCombination) => {
            Ok(UiScorecardView {
                combinations: Vec::new(),
            })
        }
        Err(GetScorecardError::Storage(error)) => Err(IpcError::from(error.class())),
        Err(GetScorecardError::Calculation(_)) => Err(IpcError {
            kind: IpcErrorKind::Retryable,
        }),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn get_scorecard<R: Runtime>(
    app: AppHandle<R>,
    request: GetScorecardRequest,
) -> Result<UiScorecardView, IpcError> {
    let GetScorecardRequest {
        scope: ScorecardScope::All,
    } = request;
    let managed = app
        .try_state::<ScorecardIpcState>()
        .map(|state| state.inner().clone());
    match managed {
        Some(state) => load_scorecard_view(&state).await,
        None => Err(IpcError {
            kind: IpcErrorKind::NoData,
        }),
    }
}

#[cfg(test)]
mod tests;
