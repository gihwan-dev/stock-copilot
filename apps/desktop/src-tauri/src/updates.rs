pub mod tauri_source;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;

pub const STARTUP_DELAY: Duration = Duration::from_secs(15);
pub const CHECK_INTERVAL: Duration = Duration::from_hours(6);
pub const PUBKEY_PLACEHOLDER: &str = "STOCK_COPILOT_UPDATER_PUBKEY_PLACEHOLDER";

pub type SourceFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, UpdateFailure>> + Send + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateVersion(String);

impl UpdateVersion {
    pub fn new<V: Into<String>>(value: V) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseNotes(String);

impl ReleaseNotes {
    pub fn new<V: Into<String>>(value: V) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundUpdate {
    pub version: UpdateVersion,
    pub notes: ReleaseNotes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateFailure {
    Check,
    Download,
    Install,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateMode {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdatePhase {
    Off,
    Idle,
    Checking,
    Downloading(FoundUpdate),
    UpToDate,
    Ready(FoundUpdate),
    Failed(UpdateFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStart {
    Started,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed(UpdateVersion),
    NothingReady(UpdatePhase),
    Failed(UpdateFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleStep {
    Continue,
    Stop,
}

pub trait UpdateSource: Send + Sync {
    fn check(&self) -> SourceFuture<'_, Option<FoundUpdate>>;
    fn download(&self) -> SourceFuture<'_, ()>;
    fn install(&self) -> Result<(), UpdateFailure>;
}

pub struct UpdateCenter {
    source: Arc<dyn UpdateSource>,
    phase: Mutex<UpdatePhase>,
}

impl std::fmt::Debug for UpdateCenter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UpdateCenter")
    }
}

impl UpdateCenter {
    pub fn new(mode: UpdateMode, source: Arc<dyn UpdateSource>) -> Self {
        let phase = match mode {
            UpdateMode::Enabled => UpdatePhase::Idle,
            UpdateMode::Disabled => UpdatePhase::Off,
        };
        Self {
            source,
            phase: Mutex::new(phase),
        }
    }

    pub async fn phase(&self) -> UpdatePhase {
        self.phase.lock().await.clone()
    }

    pub async fn begin(&self) -> CheckStart {
        let mut phase = self.phase.lock().await;
        match *phase {
            UpdatePhase::Off
            | UpdatePhase::Checking
            | UpdatePhase::Downloading(_)
            | UpdatePhase::Ready(_) => CheckStart::Skipped,
            UpdatePhase::Idle | UpdatePhase::UpToDate | UpdatePhase::Failed(_) => {
                *phase = UpdatePhase::Checking;
                CheckStart::Started
            }
        }
    }

    pub async fn run(&self) -> UpdatePhase {
        let next = match self.source.check().await {
            Ok(None) => UpdatePhase::UpToDate,
            Ok(Some(found)) => {
                self.set(UpdatePhase::Downloading(found.clone())).await;
                match self.source.download().await {
                    Ok(()) => UpdatePhase::Ready(found),
                    Err(failure) => UpdatePhase::Failed(failure),
                }
            }
            Err(failure) => UpdatePhase::Failed(failure),
        };
        self.set(next.clone()).await;
        next
    }

    pub async fn check(&self) -> UpdatePhase {
        match self.begin().await {
            CheckStart::Started => self.run().await,
            CheckStart::Skipped => self.phase().await,
        }
    }

    pub async fn install_ready(&self) -> InstallOutcome {
        let mut phase = self.phase.lock().await;
        match phase.clone() {
            UpdatePhase::Ready(found) => match self.source.install() {
                Ok(()) => InstallOutcome::Installed(found.version),
                Err(failure) => {
                    *phase = UpdatePhase::Failed(failure);
                    InstallOutcome::Failed(failure)
                }
            },
            other @ (UpdatePhase::Off
            | UpdatePhase::Idle
            | UpdatePhase::Checking
            | UpdatePhase::Downloading(_)
            | UpdatePhase::UpToDate
            | UpdatePhase::Failed(_)) => InstallOutcome::NothingReady(other),
        }
    }

    async fn set(&self, next: UpdatePhase) {
        *self.phase.lock().await = next;
    }
}

pub async fn run_schedule<W, F>(center: Arc<UpdateCenter>, wait: W)
where
    W: Fn() -> F,
    F: Future<Output = ScheduleStep>,
{
    loop {
        let _phase = center.check().await;
        match wait().await {
            ScheduleStep::Continue => {}
            ScheduleStep::Stop => return,
        }
    }
}

#[must_use]
pub fn mode_for(build: BuildKind, pubkey: Option<&str>) -> UpdateMode {
    match (build, pubkey) {
        (BuildKind::Debug, _) | (BuildKind::Release, None) => UpdateMode::Disabled,
        (BuildKind::Release, Some(key)) => match key.trim() {
            "" | PUBKEY_PLACEHOLDER => UpdateMode::Disabled,
            _ => UpdateMode::Enabled,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildKind {
    Debug,
    Release,
}

#[must_use]
pub const fn current_build() -> BuildKind {
    if tauri::is_dev() {
        BuildKind::Debug
    } else {
        BuildKind::Release
    }
}

#[cfg(test)]
mod tests;
