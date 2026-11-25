use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::Mutex;

use super::{FoundUpdate, ReleaseNotes, SourceFuture, UpdateFailure, UpdateSource, UpdateVersion};

enum Staged {
    Found(Box<Update>),
    Downloaded(Box<Update>, Vec<u8>),
}

pub struct TauriUpdateSource<R: Runtime> {
    app: AppHandle<R>,
    staged: Mutex<Option<Staged>>,
}

impl<R: Runtime> std::fmt::Debug for TauriUpdateSource<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TauriUpdateSource")
    }
}

impl<R: Runtime> TauriUpdateSource<R> {
    pub const fn new(app: AppHandle<R>) -> Self {
        Self {
            app,
            staged: Mutex::const_new(None),
        }
    }
}

#[must_use]
pub fn configured_pubkey<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|updater| updater.get("pubkey"))
        .and_then(serde_json::Value::as_str)
        .map(String::from)
}

fn release_notes(update: &Update) -> ReleaseNotes {
    match &update.body {
        Some(body) => ReleaseNotes::new(body.clone()),
        None => ReleaseNotes::new(String::new()),
    }
}

const fn ignore_chunk(_received: usize, _total: Option<u64>) {}

const fn ignore_finish() {}

impl<R: Runtime> UpdateSource for TauriUpdateSource<R> {
    fn check(&self) -> SourceFuture<'_, Option<FoundUpdate>> {
        Box::pin(async move {
            let updater = self.app.updater().map_err(|_error| UpdateFailure::Check)?;
            let found = updater
                .check()
                .await
                .map_err(|_error| UpdateFailure::Check)?;
            let Some(update) = found else {
                return Ok(None);
            };
            let summary = FoundUpdate {
                version: UpdateVersion::new(update.version.clone()),
                notes: release_notes(&update),
            };
            *self.staged.lock().await = Some(Staged::Found(Box::new(update)));
            Ok(Some(summary))
        })
    }

    fn download(&self) -> SourceFuture<'_, ()> {
        Box::pin(async move {
            let taken = self.staged.lock().await.take();
            let Some(Staged::Found(update) | Staged::Downloaded(update, _)) = taken else {
                return Err(UpdateFailure::Download);
            };
            let bytes = update
                .download(ignore_chunk, ignore_finish)
                .await
                .map_err(|_error| UpdateFailure::Download)?;
            *self.staged.lock().await = Some(Staged::Downloaded(update, bytes));
            Ok(())
        })
    }

    fn install(&self) -> Result<(), UpdateFailure> {
        let Ok(mut staged) = self.staged.try_lock() else {
            return Err(UpdateFailure::Install);
        };
        match staged.take() {
            Some(Staged::Downloaded(update, bytes)) => match update.install(&bytes) {
                Ok(()) => Ok(()),
                Err(_error) => {
                    *staged = Some(Staged::Downloaded(update, bytes));
                    Err(UpdateFailure::Install)
                }
            },
            other @ (Some(Staged::Found(_)) | None) => {
                *staged = other;
                Err(UpdateFailure::Install)
            }
        }
    }
}
