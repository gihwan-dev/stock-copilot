mod generate;
mod inputs;
use super::{
    briefing::kst_iso,
    error::{IpcError, IpcErrorKind},
    state::BoxFuture,
};
use crate::ports::{PortTypes, Ports};
use application::{Clock, DashboardCacheKey};
use application::{DashboardCache, DashboardCacheDocument};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
use store_sqlite::SqliteRepositories;
use tauri::{AppHandle, Manager, Runtime};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ResearchStatus {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "empty")]
    Empty,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "refreshing")]
    Refreshing,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ResearchDetail {
    #[serde(rename = "available")]
    Available,
    #[default]
    #[serde(rename = "unavailable")]
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ResearchEntry {
    pub symbol: Option<String>,
    pub body: String,
    #[serde(default)]
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(default)]
    pub detail: ResearchDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ResearchSource {
    pub title: String,
    pub url: Option<String>,
    #[serde(rename = "observedAt")]
    pub observed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ResearchSection {
    pub status: ResearchStatus,
    #[serde(rename = "asOf")]
    pub as_of: Option<String>,
    pub entries: Vec<ResearchEntry>,
    pub sources: Vec<ResearchSource>,
}

impl ResearchSection {
    fn status(status: ResearchStatus) -> Self {
        Self {
            status,
            as_of: None,
            entries: Vec::new(),
            sources: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct DashboardResearch {
    pub holdings: ResearchSection,
    pub candidates: ResearchSection,
    pub economy: ResearchSection,
}

#[derive(Serialize, Deserialize)]
struct Cached {
    scope: String,
    at: i64,
    section: ResearchSection,
}

type ReadResearch =
    dyn Fn() -> BoxFuture<'static, Result<DashboardResearch, IpcError>> + Send + Sync;
pub struct DashboardReader(Box<ReadResearch>);
impl std::fmt::Debug for DashboardReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("DashboardReader")
    }
}

impl DashboardReader {
    pub fn new<R: Runtime, P: PortTypes>(app: AppHandle<R>) -> Self {
        let locks = [
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
        ];
        Self(Box::new(move || {
            let handle = app.clone();
            let [hold_lock, candidate_lock, economy_lock] = locks.clone();
            Box::pin(async move {
                let ports = handle.try_state::<Ports<P>>().ok_or(IpcError {
                    kind: IpcErrorKind::Retryable,
                })?;
                let inventory = inputs::holdings(&ports).await;
                let (holdings, mut candidates) = match inventory {
                    Ok(book) => (
                        section::<R, P>(
                            &handle,
                            DashboardCacheKey::Holdings,
                            book.clone(),
                            hold_lock,
                        )
                        .await,
                        section::<R, P>(
                            &handle,
                            DashboardCacheKey::Candidates,
                            book,
                            candidate_lock,
                        )
                        .await,
                    ),
                    Err(()) => (
                        ResearchSection::status(ResearchStatus::Failed),
                        ResearchSection::status(ResearchStatus::Failed),
                    ),
                };
                let directory = handle.try_state::<crate::holdings_link::InstrumentDirectory>();
                refresh_details(&mut candidates, directory.as_deref());
                let economy =
                    section::<R, P>(&handle, DashboardCacheKey::Economy, None, economy_lock).await;
                Ok(DashboardResearch {
                    holdings,
                    candidates,
                    economy,
                })
            })
        }))
    }
}

fn refresh_details(
    candidates: &mut ResearchSection,
    directory: Option<&crate::holdings_link::InstrumentDirectory>,
) {
    for entry in &mut candidates.entries {
        let found = entry
            .symbol
            .as_ref()
            .and_then(|raw| domain::Symbol::try_new(raw).ok())
            .and_then(|symbol| directory?.name(&symbol));
        entry.detail = match found {
            Some(name) => {
                entry.display_name = Some(name.as_str().to_owned());
                ResearchDetail::Available
            }
            None => ResearchDetail::Unavailable,
        };
    }
}

fn current_section(
    payload: Option<&str>,
    scope: &str,
    now: domain::Timestamp,
) -> Option<ResearchSection> {
    let cached: Cached = serde_json::from_str(payload?).ok()?;
    fresh(&cached, scope, now)
}

fn fresh(cached: &Cached, scope: &str, now: domain::Timestamp) -> Option<ResearchSection> {
    let ttl = match cached.section.status {
        ResearchStatus::Ready => 21_600,
        ResearchStatus::Failed | ResearchStatus::Empty => 300,
        ResearchStatus::Refreshing => return None,
    };
    let age = now.utc_seconds().as_i64().checked_sub(cached.at)?;
    (cached.scope == scope && (0..ttl).contains(&age)).then(|| cached.section.clone())
}

async fn section<R: Runtime, P: PortTypes>(
    app: &AppHandle<R>,
    key: DashboardCacheKey,
    book: Option<Arc<inputs::Book>>,
    lock: Arc<Mutex<Option<Cached>>>,
) -> ResearchSection {
    let scope = match (key, &book) {
        (DashboardCacheKey::Economy, _) => String::from("market"),
        (DashboardCacheKey::Holdings, Some(value)) => value.holdings_scope.clone(),
        (DashboardCacheKey::Candidates, Some(value)) => value.candidates_scope.clone(),
        (DashboardCacheKey::Holdings | DashboardCacheKey::Candidates, None) => {
            return ResearchSection::status(ResearchStatus::Empty);
        }
    };
    let Ok(guard) = lock.try_lock_owned() else {
        return ResearchSection::status(ResearchStatus::Refreshing);
    };
    let Some(rows) = app
        .try_state::<Arc<SqliteRepositories>>()
        .map(|rows| Arc::clone(rows.inner()))
    else {
        return ResearchSection::status(ResearchStatus::Failed);
    };
    let Some(ports) = app.try_state::<Ports<P>>() else {
        return ResearchSection::status(ResearchStatus::Failed);
    };
    let now = ports.clock.now();
    if let Some(failed) = guard.as_ref().and_then(|cached| fresh(cached, &scope, now)) {
        return failed;
    }
    let Ok(payload) = rows.dashboard_section(key).await else {
        return ResearchSection::status(ResearchStatus::Failed);
    };
    if let Some(section) = current_section(
        payload.as_ref().map(DashboardCacheDocument::as_str),
        &scope,
        now,
    ) {
        return section;
    }
    let handle = app.clone();
    let _task = tauri::async_runtime::spawn(async move {
        let mut slot = guard;
        refresh::<R, P>(handle, key, book, scope, &mut slot).await;
    });
    ResearchSection::status(ResearchStatus::Refreshing)
}

async fn refresh<R: Runtime, P: PortTypes>(
    handle: AppHandle<R>,
    key: DashboardCacheKey,
    book: Option<Arc<inputs::Book>>,
    scope: String,
    slot: &mut Option<Cached>,
) {
    let Some(ports) = handle.try_state::<Ports<P>>() else {
        return;
    };
    let Some(rows) = handle.try_state::<Arc<SqliteRepositories>>() else {
        return;
    };
    let result = match tokio::time::timeout(
        std::time::Duration::from_secs(180),
        generate::generate(&handle, &ports, key, book.as_deref(), ports.clock.now()),
    )
    .await
    {
        Ok(section) => section,
        Err(_elapsed) => ResearchSection::status(ResearchStatus::Failed),
    };
    let cached = Cached {
        scope,
        at: *ports.clock.now().utc_seconds().as_i64(),
        section: result,
    };
    let saved = match serde_json::to_string(&cached) {
        Ok(encoded) => rows
            .save_dashboard_section(key, DashboardCacheDocument::new(encoded))
            .await
            .is_ok(),
        Err(_invalid) => false,
    };
    *slot = if saved {
        None
    } else {
        Some(Cached {
            section: ResearchSection::status(ResearchStatus::Failed),
            ..cached
        })
    };
}

#[tauri::command]
#[specta::specta]
pub async fn get_dashboard_research<R: Runtime>(
    app: AppHandle<R>,
) -> Result<DashboardResearch, IpcError> {
    let reader = app.try_state::<DashboardReader>().ok_or(IpcError {
        kind: IpcErrorKind::Retryable,
    })?;
    (reader.0)().await
}

#[cfg(test)]
mod tests;
