use application::SystemPortError;
use std::sync::Arc;
use store_sqlite::{SharedClosure, SharedDatabase};

pub struct SharedDatabaseShutdownHook {
    inner: Arc<dyn crate::platform::process_cleanup::ShutdownHook>,
    shared: SharedDatabase,
    closure: tokio::sync::Mutex<Option<Result<SharedClosure, SystemPortError>>>,
}

impl std::fmt::Debug for SharedDatabaseShutdownHook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SharedDatabaseShutdownHook")
    }
}

impl SharedDatabaseShutdownHook {
    #[must_use]
    pub fn new(
        inner: Arc<dyn crate::platform::process_cleanup::ShutdownHook>,
        shared: SharedDatabase,
    ) -> Self {
        Self {
            inner,
            shared,
            closure: tokio::sync::Mutex::new(None),
        }
    }

    pub async fn closure(&self) -> Option<Result<SharedClosure, SystemPortError>> {
        *self.closure.lock().await
    }
}

impl crate::platform::process_cleanup::ShutdownHook for SharedDatabaseShutdownHook {
    fn on_shutdown(
        &self,
    ) -> crate::platform::process_cleanup::CleanupFuture<
        '_,
        crate::platform::process_cleanup::ProcessShutdownSummary,
    > {
        Box::pin(async move {
            let summary = self.inner.on_shutdown().await;
            let closed = self.shared.close().await;
            *self.closure.lock().await = Some(closed);
            summary
        })
    }
}
