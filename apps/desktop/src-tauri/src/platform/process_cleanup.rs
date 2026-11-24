use application::{SystemFailure, SystemPort, SystemPortError};
use domain::Timestamp;
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::num::NonZeroU32;
use std::pin::Pin;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::Notify;

pub const DEFAULT_TERM_GRACE_MS: u64 = 3_000;
pub const DEFAULT_AUDIT_DRAIN_MS: u64 = 5_000;
const POLL_INTERVAL_MS: u64 = 10;

pub type CleanupFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessGroupId(NonZeroU32);

impl ProcessGroupId {
    #[must_use]
    pub const fn new(value: NonZeroU32) -> Self {
        Self(value)
    }

    #[must_use]
    pub fn try_new(value: u32) -> Option<Self> {
        NonZeroU32::new(value).map(Self)
    }

    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessRunId(String);

impl ProcessRunId {
    #[must_use]
    pub fn try_new(value: &str) -> Option<Self> {
        if value.trim().is_empty() {
            return None;
        }
        Some(Self(String::from(value)))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessGroupRecord {
    pub run_id: ProcessRunId,
    pub pgid: ProcessGroupId,
    pub started_at: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationSignal {
    Term,
    Kill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalOutcome {
    Signalled,
    AlreadyExited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessLiveness {
    Alive,
    Exited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessCleanupError {
    Ledger(SystemPortError),
    SignalFailed(String),
    ProtectedSelfGroup(ProcessGroupId),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CleanupReport {
    pub terminated: Vec<ProcessGroupId>,
    pub force_killed: Vec<ProcessGroupId>,
    pub already_exited: Vec<ProcessGroupId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditDrainStatus {
    Drained,
    TimedOut { remaining: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessShutdownSummary {
    pub process_cleanup: Result<CleanupReport, ProcessCleanupError>,
    pub audit_drain: AuditDrainStatus,
}

pub trait ProcessLedgerPort: Send + Sync {
    fn register(
        &self,
        record: ProcessGroupRecord,
    ) -> CleanupFuture<'_, Result<(), SystemPortError>>;
    fn remove(&self, pgid: ProcessGroupId) -> CleanupFuture<'_, Result<(), SystemPortError>>;
    fn list_active(&self) -> CleanupFuture<'_, Result<Vec<ProcessGroupRecord>, SystemPortError>>;
}

pub trait ProcessSignaler: Send + Sync {
    fn send_signal(
        &self,
        pgid: ProcessGroupId,
        signal: TerminationSignal,
    ) -> CleanupFuture<'_, Result<SignalOutcome, ProcessCleanupError>>;
    fn probe(
        &self,
        pgid: ProcessGroupId,
    ) -> CleanupFuture<'_, Result<ProcessLiveness, ProcessCleanupError>>;
}

#[derive(Debug, Default)]
pub struct InMemoryProcessLedger {
    records: RwLock<BTreeMap<ProcessGroupId, ProcessGroupRecord>>,
}

impl InMemoryProcessLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn seed(&self, record: ProcessGroupRecord) -> Result<(), SystemPortError> {
        let mut records = self
            .records
            .write()
            .map_err(|_error| ledger_unavailable())?;
        records.insert(record.pgid, record);
        Ok(())
    }

    pub fn snapshot(&self) -> Result<Vec<ProcessGroupRecord>, SystemPortError> {
        let records = self.records.read().map_err(|_error| ledger_unavailable())?;
        Ok(records.values().cloned().collect())
    }
}

impl ProcessLedgerPort for InMemoryProcessLedger {
    fn register(
        &self,
        record: ProcessGroupRecord,
    ) -> CleanupFuture<'_, Result<(), SystemPortError>> {
        let result = self.seed(record);
        Box::pin(std::future::ready(result))
    }

    fn remove(&self, pgid: ProcessGroupId) -> CleanupFuture<'_, Result<(), SystemPortError>> {
        let result = self
            .records
            .write()
            .map_err(|_error| ledger_unavailable())
            .map(|mut records| {
                let _removed = records.remove(&pgid);
            });
        Box::pin(std::future::ready(result))
    }

    fn list_active(&self) -> CleanupFuture<'_, Result<Vec<ProcessGroupRecord>, SystemPortError>> {
        let result = self.snapshot();
        Box::pin(std::future::ready(result))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CommandProcessSignaler;

impl CommandProcessSignaler {
    fn guard_not_self(pgid: ProcessGroupId) -> Result<(), ProcessCleanupError> {
        if pgid.as_u32() == std::process::id() {
            return Err(ProcessCleanupError::ProtectedSelfGroup(pgid));
        }
        Ok(())
    }

    async fn run_kill(
        flag: &'static str,
        pgid: ProcessGroupId,
    ) -> Result<std::process::Output, ProcessCleanupError> {
        Self::guard_not_self(pgid)?;
        let target = format!("-{}", pgid.as_u32());
        tauri::async_runtime::spawn_blocking(move || {
            Command::new("/bin/kill")
                .arg(flag)
                .arg("--")
                .arg(target)
                .output()
        })
        .await
        .map_err(|error| ProcessCleanupError::SignalFailed(error.to_string()))?
        .map_err(|error| ProcessCleanupError::SignalFailed(error.to_string()))
    }
}

impl ProcessSignaler for CommandProcessSignaler {
    fn send_signal(
        &self,
        pgid: ProcessGroupId,
        signal: TerminationSignal,
    ) -> CleanupFuture<'_, Result<SignalOutcome, ProcessCleanupError>> {
        let flag = match signal {
            TerminationSignal::Term => "-TERM",
            TerminationSignal::Kill => "-KILL",
        };
        Box::pin(async move {
            let output = Self::run_kill(flag, pgid).await?;
            if output.status.success() {
                return Ok(SignalOutcome::Signalled);
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("No such process") {
                return Ok(SignalOutcome::AlreadyExited);
            }
            Err(ProcessCleanupError::SignalFailed(stderr.trim().to_owned()))
        })
    }

    fn probe(
        &self,
        pgid: ProcessGroupId,
    ) -> CleanupFuture<'_, Result<ProcessLiveness, ProcessCleanupError>> {
        Box::pin(async move {
            let output = Self::run_kill("-0", pgid).await?;
            if output.status.success() {
                return Ok(ProcessLiveness::Alive);
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("No such process") {
                return Ok(ProcessLiveness::Exited);
            }
            Err(ProcessCleanupError::SignalFailed(stderr.trim().to_owned()))
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct AuditDrainTracker {
    in_flight: Arc<AtomicUsize>,
    drained: Arc<Notify>,
}

impl AuditDrainTracker {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn begin_write(&self) -> AuditWriteGuard {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        AuditWriteGuard {
            in_flight: Arc::clone(&self.in_flight),
            drained: Arc::clone(&self.drained),
        }
    }

    #[must_use]
    pub fn in_flight_count(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    pub async fn wait_for_drain(&self, timeout: Duration) -> AuditDrainStatus {
        if self.in_flight_count() == 0 {
            return AuditDrainStatus::Drained;
        }
        match tokio::time::timeout(timeout, self.await_zero()).await {
            Ok(()) => AuditDrainStatus::Drained,
            Err(_elapsed) => {
                let remaining = self.in_flight_count();
                if remaining == 0 {
                    AuditDrainStatus::Drained
                } else {
                    AuditDrainStatus::TimedOut { remaining }
                }
            }
        }
    }

    async fn await_zero(&self) {
        loop {
            let notified = self.drained.notified();
            let pinned = std::pin::pin!(notified);
            if self.in_flight_count() == 0 {
                break;
            }
            pinned.await;
            if self.in_flight_count() == 0 {
                break;
            }
        }
    }
}

#[derive(Debug)]
pub struct AuditWriteGuard {
    in_flight: Arc<AtomicUsize>,
    drained: Arc<Notify>,
}

impl Drop for AuditWriteGuard {
    fn drop(&mut self) {
        let previous = self.in_flight.fetch_sub(1, Ordering::SeqCst);
        if previous <= 1 {
            self.drained.notify_waiters();
        }
    }
}

#[derive(Debug)]
pub struct ChildProcessManager<L, S> {
    ledger: Arc<L>,
    signaler: Arc<S>,
}

impl<L: ProcessLedgerPort, S: ProcessSignaler> ChildProcessManager<L, S> {
    #[must_use]
    pub const fn new(ledger: Arc<L>, signaler: Arc<S>) -> Self {
        Self { ledger, signaler }
    }

    #[must_use]
    pub fn ledger(&self) -> &Arc<L> {
        &self.ledger
    }

    #[must_use]
    pub fn signaler(&self) -> &Arc<S> {
        &self.signaler
    }

    pub async fn register_group(
        &self,
        record: ProcessGroupRecord,
    ) -> Result<(), ProcessCleanupError> {
        self.ledger
            .register(record)
            .await
            .map_err(ProcessCleanupError::Ledger)
    }

    pub async fn unregister_group(&self, pgid: ProcessGroupId) -> Result<(), ProcessCleanupError> {
        self.ledger
            .remove(pgid)
            .await
            .map_err(ProcessCleanupError::Ledger)
    }

    pub async fn cleanup_orphans_on_startup(
        &self,
        grace: Duration,
    ) -> Result<CleanupReport, ProcessCleanupError> {
        self.terminate_all_active(grace).await
    }

    pub async fn shutdown_groups(
        &self,
        grace: Duration,
    ) -> Result<CleanupReport, ProcessCleanupError> {
        self.terminate_all_active(grace).await
    }

    pub async fn shutdown_for_exit(
        &self,
        grace: Duration,
        audit_drain: &AuditDrainTracker,
        audit_timeout: Duration,
    ) -> ProcessShutdownSummary {
        let process_cleanup = self.terminate_all_active(grace).await;
        let audit_drain_status = audit_drain.wait_for_drain(audit_timeout).await;
        ProcessShutdownSummary {
            process_cleanup,
            audit_drain: audit_drain_status,
        }
    }

    async fn terminate_all_active(
        &self,
        grace: Duration,
    ) -> Result<CleanupReport, ProcessCleanupError> {
        let active = self
            .ledger
            .list_active()
            .await
            .map_err(ProcessCleanupError::Ledger)?;
        let mut report = CleanupReport::default();
        let mut pending = Vec::new();
        for record in active {
            match self
                .signaler
                .send_signal(record.pgid, TerminationSignal::Term)
                .await?
            {
                SignalOutcome::AlreadyExited => report.already_exited.push(record.pgid),
                SignalOutcome::Signalled => pending.push(record.pgid),
            }
        }
        let still_alive = self
            .wait_for_graceful_exit(&mut report, pending, grace)
            .await?;
        for pgid in still_alive {
            match self
                .signaler
                .send_signal(pgid, TerminationSignal::Kill)
                .await?
            {
                SignalOutcome::AlreadyExited => report.terminated.push(pgid),
                SignalOutcome::Signalled => report.force_killed.push(pgid),
            }
        }
        self.remove_all_recorded(&report).await?;
        Ok(report)
    }

    async fn wait_for_graceful_exit(
        &self,
        report: &mut CleanupReport,
        mut pending: Vec<ProcessGroupId>,
        grace: Duration,
    ) -> Result<Vec<ProcessGroupId>, ProcessCleanupError> {
        if pending.is_empty() {
            return Ok(Vec::new());
        }
        if grace.is_zero() {
            return self.probe_remaining(report, &pending).await;
        }
        match tokio::time::timeout(grace, self.poll_until_exited(report, &mut pending)).await {
            Ok(result) => {
                result?;
                Ok(Vec::new())
            }
            Err(_elapsed) => Ok(pending),
        }
    }

    async fn poll_until_exited(
        &self,
        report: &mut CleanupReport,
        pending: &mut Vec<ProcessGroupId>,
    ) -> Result<(), ProcessCleanupError> {
        loop {
            *pending = self.probe_remaining(report, pending).await?;
            if pending.is_empty() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
        }
    }

    async fn probe_remaining(
        &self,
        report: &mut CleanupReport,
        pending: &[ProcessGroupId],
    ) -> Result<Vec<ProcessGroupId>, ProcessCleanupError> {
        let mut still_alive = Vec::new();
        for pgid in pending {
            match self.signaler.probe(*pgid).await? {
                ProcessLiveness::Exited => report.terminated.push(*pgid),
                ProcessLiveness::Alive => still_alive.push(*pgid),
            }
        }
        Ok(still_alive)
    }

    async fn remove_all_recorded(&self, report: &CleanupReport) -> Result<(), ProcessCleanupError> {
        for pgid in report
            .already_exited
            .iter()
            .chain(report.terminated.iter())
            .chain(report.force_killed.iter())
        {
            self.ledger
                .remove(*pgid)
                .await
                .map_err(ProcessCleanupError::Ledger)?;
        }
        Ok(())
    }
}

pub trait ShutdownHook: Send + Sync {
    fn on_shutdown(&self) -> CleanupFuture<'_, ProcessShutdownSummary>;
}

pub struct ProcessShutdownHook<L, S> {
    manager: Arc<ChildProcessManager<L, S>>,
    audit_drain: AuditDrainTracker,
    term_grace: Duration,
    audit_timeout: Duration,
}

impl<L, S> Debug for ProcessShutdownHook<L, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessShutdownHook")
            .field("term_grace", &self.term_grace)
            .field("audit_timeout", &self.audit_timeout)
            .finish_non_exhaustive()
    }
}

impl<L, S> ProcessShutdownHook<L, S>
where
    L: ProcessLedgerPort + 'static,
    S: ProcessSignaler + 'static,
{
    #[must_use]
    pub const fn new(
        manager: Arc<ChildProcessManager<L, S>>,
        audit_drain: AuditDrainTracker,
        term_grace: Duration,
        audit_timeout: Duration,
    ) -> Self {
        Self {
            manager,
            audit_drain,
            term_grace,
            audit_timeout,
        }
    }
}

impl<L, S> ShutdownHook for ProcessShutdownHook<L, S>
where
    L: ProcessLedgerPort + 'static,
    S: ProcessSignaler + 'static,
{
    fn on_shutdown(&self) -> CleanupFuture<'_, ProcessShutdownSummary> {
        Box::pin(async move {
            self.manager
                .shutdown_for_exit(self.term_grace, &self.audit_drain, self.audit_timeout)
                .await
        })
    }
}

pub struct ProcessRuntime {
    startup_cleanup: Arc<
        dyn Fn() -> CleanupFuture<'static, Result<CleanupReport, ProcessCleanupError>>
            + Send
            + Sync,
    >,
    shutdown_hook: Arc<dyn ShutdownHook>,
    audit_drain: AuditDrainTracker,
}

impl Debug for ProcessRuntime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProcessRuntime(<active>)")
    }
}

impl ProcessRuntime {
    #[must_use]
    pub fn new<L, S>(
        manager: Arc<ChildProcessManager<L, S>>,
        audit_drain: AuditDrainTracker,
        term_grace: Duration,
        audit_timeout: Duration,
    ) -> Self
    where
        L: ProcessLedgerPort + 'static,
        S: ProcessSignaler + 'static,
    {
        let startup_manager = Arc::clone(&manager);
        let startup_cleanup = Arc::new(move || {
            let mgr = Arc::clone(&startup_manager);
            let future: CleanupFuture<'static, Result<CleanupReport, ProcessCleanupError>> =
                Box::pin(async move { mgr.cleanup_orphans_on_startup(term_grace).await });
            future
        });
        let shutdown_hook: Arc<dyn ShutdownHook> = Arc::new(ProcessShutdownHook::new(
            manager,
            audit_drain.clone(),
            term_grace,
            audit_timeout,
        ));
        Self {
            startup_cleanup,
            shutdown_hook,
            audit_drain,
        }
    }

    #[must_use]
    pub fn default_in_memory() -> Self {
        let manager = Arc::new(ChildProcessManager::new(
            Arc::new(InMemoryProcessLedger::new()),
            Arc::new(CommandProcessSignaler),
        ));
        Self::new(
            manager,
            AuditDrainTracker::new(),
            Duration::from_millis(DEFAULT_TERM_GRACE_MS),
            Duration::from_millis(DEFAULT_AUDIT_DRAIN_MS),
        )
    }

    pub async fn cleanup_orphans_on_startup(&self) -> Result<CleanupReport, ProcessCleanupError> {
        (self.startup_cleanup)().await
    }

    #[must_use]
    pub fn shutdown_hook(&self) -> Arc<dyn ShutdownHook> {
        Arc::clone(&self.shutdown_hook)
    }

    #[must_use]
    pub fn audit_drain(&self) -> &AuditDrainTracker {
        &self.audit_drain
    }
}

fn ledger_unavailable() -> SystemPortError {
    SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(application::RetryDelay::Unknown),
    )
}
