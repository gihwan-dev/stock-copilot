use crate::platform::process_cleanup::{CleanupFuture, ProcessShutdownSummary, ShutdownHook};
use application::Clock;
use scheduler::{AiJobQueue, LoopControl, Scheduler};
use std::fmt::{Debug, Formatter};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tauri::async_runtime::JoinHandle;
use tokio::sync::{Mutex, watch};

const STOP_GRACE_MS: u64 = 500;

pub type CollectorFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

pub trait CollectorWorker: Send + Sync {
    fn run(&self, shutdown: watch::Receiver<LoopControl>) -> CollectorFuture;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct IdleCollectorWorker;

impl CollectorWorker for IdleCollectorWorker {
    fn run(&self, mut shutdown: watch::Receiver<LoopControl>) -> CollectorFuture {
        Box::pin(async move {
            while *shutdown.borrow() != LoopControl::Stop && shutdown.changed().await.is_ok() {}
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackgroundRunState {
    Running,
    Stopped,
}

pub struct BackgroundRuntime {
    control: watch::Sender<LoopControl>,
    tasks: Mutex<Vec<JoinHandle<()>>>,
    state: RwLock<BackgroundRunState>,
}

impl Debug for BackgroundRuntime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundRuntime")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl BackgroundRuntime {
    #[must_use]
    pub fn spawn<C, Q>(
        scheduler: Arc<Scheduler<C, Q>>,
        tick_interval: Duration,
        collector: Arc<dyn CollectorWorker>,
    ) -> Self
    where
        C: Clock + 'static,
        Q: AiJobQueue + 'static,
    {
        let (control, scheduler_rx) = watch::channel(LoopControl::Run);
        let collector_rx = control.subscribe();
        let scheduler_task = tauri::async_runtime::spawn(async move {
            scheduler.run_loop(tick_interval, scheduler_rx).await;
        });
        let collector_task = tauri::async_runtime::spawn(async move {
            collector.run(collector_rx).await;
        });
        Self {
            control,
            tasks: Mutex::new(vec![scheduler_task, collector_task]),
            state: RwLock::new(BackgroundRunState::Running),
        }
    }

    #[must_use]
    pub fn state(&self) -> BackgroundRunState {
        match self.state.read() {
            Ok(guard) => *guard,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    pub async fn stop(&self) {
        let _ignored = self.control.send(LoopControl::Stop);
        let mut tasks = self.tasks.lock().await;
        while let Some(task) = tasks.pop() {
            finish_task(task, Duration::from_millis(STOP_GRACE_MS)).await;
        }
        match self.state.write() {
            Ok(mut guard) => *guard = BackgroundRunState::Stopped,
            Err(poisoned) => *poisoned.into_inner() = BackgroundRunState::Stopped,
        }
    }
}

async fn finish_task(mut task: JoinHandle<()>, grace: Duration) {
    let joined = tokio::time::timeout(grace, &mut task).await;
    if joined.is_err() {
        task.abort();
        let _aborted = task.await;
    }
}

pub struct BackgroundAwareShutdownHook {
    background: Option<Arc<BackgroundRuntime>>,
    inner: Arc<dyn ShutdownHook>,
}

impl Debug for BackgroundAwareShutdownHook {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundAwareShutdownHook")
            .field("has_background", &self.background.is_some())
            .finish_non_exhaustive()
    }
}

impl BackgroundAwareShutdownHook {
    #[must_use]
    pub const fn new(
        background: Option<Arc<BackgroundRuntime>>,
        inner: Arc<dyn ShutdownHook>,
    ) -> Self {
        Self { background, inner }
    }
}

impl ShutdownHook for BackgroundAwareShutdownHook {
    fn on_shutdown(&self) -> CleanupFuture<'_, ProcessShutdownSummary> {
        Box::pin(async move {
            if let Some(runtime) = &self.background {
                runtime.stop().await;
            }
            self.inner.on_shutdown().await
        })
    }
}
