use std::fmt;
use std::sync::Arc;

use application::{
    BriefingFailure, BriefingOutcome, BriefingRecord, BriefingRepository, BriefingSource,
    ClassifiedError, SystemPortError,
};
use domain::{InformationSource, OffsetSeconds, Timestamp};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager, Runtime};

use super::error::{IpcError, IpcErrorKind};
use super::event::{UiBriefingId, UiInformationSource};
use super::state::BoxFuture;

const KST_OFFSET: i32 = 32_400;
const PAST_LIMIT: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
#[serde(deny_unknown_fields)]
#[specta(skip_attr = "serde")]
pub struct GetBriefingsRequest {
    pub scope: BriefingsScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Type)]
pub enum BriefingsScope {
    #[serde(rename = "recent")]
    Recent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiBriefingStatus {
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiBriefingFailure {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "ai-unavailable")]
    AiUnavailable,
    #[serde(rename = "rate-limited")]
    RateLimited,
    #[serde(rename = "empty")]
    Empty,
    #[serde(rename = "rejected")]
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
pub enum UiBriefingSourceKind {
    #[serde(rename = "web")]
    Web,
    #[serde(rename = "news")]
    News,
    #[serde(rename = "disclosure")]
    Disclosure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiBriefingSource {
    pub title: String,
    pub url: Option<String>,
    pub kind: UiBriefingSourceKind,
    pub cite: String,
    pub origin: UiInformationSource,
    pub observed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiBriefing {
    pub id: UiBriefingId,
    pub finished_at: String,
    pub status: UiBriefingStatus,
    pub failure: UiBriefingFailure,
    pub body: String,
    pub sources: Vec<UiBriefingSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type)]
pub struct UiBriefingsView {
    pub latest: Option<UiBriefing>,
    pub past: Vec<UiBriefing>,
}

pub trait BriefingHistory: Send + Sync {
    fn recent(&self) -> BoxFuture<'_, Result<Vec<BriefingRecord>, SystemPortError>>;
}

struct RepositoryHistory<R> {
    repository: Arc<R>,
}

impl<R: BriefingRepository + 'static> BriefingHistory for RepositoryHistory<R> {
    fn recent(&self) -> BoxFuture<'_, Result<Vec<BriefingRecord>, SystemPortError>> {
        Box::pin(self.repository.recent())
    }
}

#[derive(Clone)]
pub struct BriefingIpcState {
    history: Arc<dyn BriefingHistory>,
}

impl fmt::Debug for BriefingIpcState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BriefingIpcState")
    }
}

impl BriefingIpcState {
    #[must_use]
    pub fn from_repository<R: BriefingRepository + 'static>(repository: Arc<R>) -> Self {
        Self {
            history: Arc::new(RepositoryHistory { repository }),
        }
    }

    #[must_use]
    pub fn new(history: Arc<dyn BriefingHistory>) -> Self {
        Self { history }
    }
}

fn two(value: i64) -> String {
    format!("{value:02}")
}

pub(crate) fn kst_iso(at: Timestamp) -> Option<String> {
    let date = at.date_at_offset(OffsetSeconds::new(KST_OFFSET)).ok()?;
    let local = at
        .utc_seconds()
        .as_i64()
        .checked_add(i64::from(KST_OFFSET))?;
    let of_day = local.rem_euclid(86_400);
    Some(format!(
        "{:04}-{}-{}T{}:{}:{}+09:00",
        date.year().as_i32(),
        two(i64::from(*date.month().as_u8())),
        two(i64::from(*date.day().as_u8())),
        two(of_day.div_euclid(3_600)),
        two(of_day.rem_euclid(3_600).div_euclid(60)),
        two(of_day.rem_euclid(60)),
    ))
}

const fn map_failure(failure: BriefingFailure) -> UiBriefingFailure {
    match failure {
        BriefingFailure::ProviderUnavailable | BriefingFailure::IsolationRejected => {
            UiBriefingFailure::AiUnavailable
        }
        BriefingFailure::RateLimited => UiBriefingFailure::RateLimited,
        BriefingFailure::EmptyResult => UiBriefingFailure::Empty,
        BriefingFailure::PromptRejected => UiBriefingFailure::Rejected,
    }
}

fn map_source(index: usize, source: &BriefingSource) -> UiBriefingSource {
    UiBriefingSource {
        title: source.title.as_untrusted_str().to_owned(),
        url: source.url.as_ref().map(|url| url.as_str().to_owned()),
        kind: match source.origin {
            InformationSource::Web => UiBriefingSourceKind::Web,
            InformationSource::News(_) => UiBriefingSourceKind::News,
            InformationSource::Disclosure(_) => UiBriefingSourceKind::Disclosure,
        },
        cite: index.saturating_add(1).to_string(),
        origin: source.origin.clone().into(),
        observed_at: kst_iso(source.observed_at),
    }
}

fn map_record(record: &BriefingRecord) -> Option<UiBriefing> {
    let finished_at = kst_iso(record.finished_at)?;
    let (status, failure, body, cited) = match &record.outcome {
        BriefingOutcome::Completed {
            body,
            sources: origins,
        } => (
            UiBriefingStatus::Completed,
            UiBriefingFailure::None,
            body.as_untrusted_str().to_owned(),
            origins
                .iter()
                .enumerate()
                .map(|(index, source)| map_source(index, source))
                .collect(),
        ),
        BriefingOutcome::Failed(failure) => (
            UiBriefingStatus::Failed,
            map_failure(*failure),
            String::new(),
            Vec::new(),
        ),
    };
    Some(UiBriefing {
        id: record.id.into(),
        finished_at,
        status,
        failure,
        body,
        sources: cited,
    })
}

#[must_use]
pub fn to_briefings_view(records: &[BriefingRecord]) -> UiBriefingsView {
    let mut ordered: Vec<&BriefingRecord> = records.iter().collect();
    ordered.sort_by_key(|record| std::cmp::Reverse(record.finished_at));
    let mut mapped = ordered.into_iter().filter_map(map_record);
    let latest = mapped.next();
    let past = mapped.take(PAST_LIMIT).collect();
    UiBriefingsView { latest, past }
}

pub async fn load_briefings_view(state: &BriefingIpcState) -> Result<UiBriefingsView, IpcError> {
    let records = state
        .history
        .recent()
        .await
        .map_err(|error| IpcError::from(error.class()))?;
    Ok(to_briefings_view(&records))
}

#[tauri::command]
#[specta::specta]
pub async fn get_briefings<R: Runtime>(
    app: AppHandle<R>,
    request: GetBriefingsRequest,
) -> Result<UiBriefingsView, IpcError> {
    let GetBriefingsRequest {
        scope: BriefingsScope::Recent,
    } = request;
    let managed = app
        .try_state::<BriefingIpcState>()
        .map(|state| state.inner().clone());
    match managed {
        Some(state) => load_briefings_view(&state).await,
        None => Err(IpcError {
            kind: IpcErrorKind::NoData,
        }),
    }
}

#[cfg(test)]
mod tests;
