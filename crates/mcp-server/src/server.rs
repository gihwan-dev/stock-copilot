use std::fmt;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use application::{
    AiRunId, Clock, ExecutablePath, PeerProcessInspector, ProcessId, RunKind, ToolCallLedger,
};
use domain::Timestamp;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};
use tokio::task::JoinHandle;

use crate::handler::{
    ConnectionEvent, ConnectionEventLog, ConnectionRejection, RaceOutcome, ShutdownSignal,
    StreamAuthParams, authenticate_and_serve_stream, pre_session_stop_to_rejection, race,
    wait_for_pre_session_stop, wait_for_shutdown,
};
use crate::peer::{
    ExpectedPeerPolicy, ObservedPeerCredentials, PeerInspectorMode, PeerVerificationFailure,
    UserId, verify_peer,
};
use crate::socket::{BoundRunSocket, RunSocketSpec, SocketCleanupGuard, SocketSetupError};
use crate::token::{AuthLineLimit, RunAuthBinding, RunAuthToken, TokenError, TokenTtl};
use crate::tools::{ToolLedgerMode, ToolRegistry, ToolRuntime, UnconfiguredToolLedger};

pub const DEFAULT_AUTH_TIMEOUT_SECS: u64 = 5_u64;
pub const MAX_AUTH_TIMEOUT_SECS: u64 = 30_u64;
pub const DEFAULT_PEER_INSPECTION_TIMEOUT_SECS: u64 = 2_u64;
pub const MAX_PEER_INSPECTION_TIMEOUT_SECS: u64 = 10_u64;
pub const DEFAULT_MAX_CONCURRENT_CONNECTIONS: usize = 4_usize;
pub const MAX_CONCURRENT_CONNECTIONS_CAP: usize = 16_usize;

const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(50_u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerError {
    RuntimeUnavailable,
    InvalidAuthTimeout,
    InvalidPeerInspectionTimeout,
    InvalidConcurrencyLimit,
    ParentPidAlreadyBound,
    Token(TokenError),
    Socket(SocketSetupError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxConcurrentConnections(usize);

impl MaxConcurrentConnections {
    pub fn try_new(limit: usize) -> Result<Self, McpServerError> {
        if !(1_usize..=MAX_CONCURRENT_CONNECTIONS_CAP).contains(&limit) {
            return Err(McpServerError::InvalidConcurrencyLimit);
        }
        Ok(Self(limit))
    }

    #[must_use]
    pub const fn default_limit() -> Self {
        Self(DEFAULT_MAX_CONCURRENT_CONNECTIONS)
    }

    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpServerConfig {
    auth_line_limit: AuthLineLimit,
    auth_timeout: Duration,
    peer_inspection_timeout: Duration,
    max_concurrent_connections: MaxConcurrentConnections,
    token_ttl: TokenTtl,
}

impl McpServerConfig {
    pub fn default_config() -> Result<Self, McpServerError> {
        let token_ttl = TokenTtl::default_ttl().map_err(McpServerError::Token)?;
        Ok(Self {
            auth_line_limit: AuthLineLimit::default_limit(),
            auth_timeout: Duration::from_secs(DEFAULT_AUTH_TIMEOUT_SECS),
            peer_inspection_timeout: Duration::from_secs(DEFAULT_PEER_INSPECTION_TIMEOUT_SECS),
            max_concurrent_connections: MaxConcurrentConnections::default_limit(),
            token_ttl,
        })
    }

    pub fn try_new(
        auth_line_limit: AuthLineLimit,
        auth_timeout: Duration,
        peer_inspection_timeout: Duration,
        max_concurrent_connections: MaxConcurrentConnections,
        token_ttl: TokenTtl,
    ) -> Result<Self, McpServerError> {
        if auth_timeout.is_zero() || auth_timeout > Duration::from_secs(MAX_AUTH_TIMEOUT_SECS) {
            return Err(McpServerError::InvalidAuthTimeout);
        }
        if peer_inspection_timeout.is_zero()
            || peer_inspection_timeout > Duration::from_secs(MAX_PEER_INSPECTION_TIMEOUT_SECS)
        {
            return Err(McpServerError::InvalidPeerInspectionTimeout);
        }
        Ok(Self {
            auth_line_limit,
            auth_timeout,
            peer_inspection_timeout,
            max_concurrent_connections,
            token_ttl,
        })
    }

    #[must_use]
    pub const fn auth_line_limit(self) -> AuthLineLimit {
        self.auth_line_limit
    }

    #[must_use]
    pub const fn auth_timeout(self) -> Duration {
        self.auth_timeout
    }

    #[must_use]
    pub const fn peer_inspection_timeout(self) -> Duration {
        self.peer_inspection_timeout
    }

    #[must_use]
    pub const fn max_concurrent_connections(self) -> MaxConcurrentConnections {
        self.max_concurrent_connections
    }

    #[must_use]
    pub const fn token_ttl(self) -> TokenTtl {
        self.token_ttl
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunIdentity {
    run_id: AiRunId,
    run_kind: RunKind,
}

impl RunIdentity {
    #[must_use]
    pub const fn new(run_id: AiRunId, run_kind: RunKind) -> Self {
        Self { run_id, run_kind }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum InitialParentTarget {
    Pending,
    Bound(ProcessId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerProcessTarget {
    parent_pid: InitialParentTarget,
    executable_path: ExecutablePath,
}

impl PeerProcessTarget {
    #[must_use]
    pub const fn pending_parent(executable_path: ExecutablePath) -> Self {
        Self {
            parent_pid: InitialParentTarget::Pending,
            executable_path,
        }
    }

    #[must_use]
    pub const fn with_parent(parent_pid: ProcessId, executable_path: ExecutablePath) -> Self {
        Self {
            parent_pid: InitialParentTarget::Bound(parent_pid),
            executable_path,
        }
    }
}

pub struct RunServerDeps<C, P, L = UnconfiguredToolLedger> {
    clock: Arc<C>,
    peer_inspector: PeerInspectorMode<P>,
    config: McpServerConfig,
    tools: Option<ToolRuntime<L>>,
}

impl<C, P, L> fmt::Debug for RunServerDeps<C, P, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunServerDeps")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<C, P> RunServerDeps<C, P, UnconfiguredToolLedger> {
    #[must_use]
    pub fn new(
        clock: Arc<C>,
        peer_inspector: PeerInspectorMode<P>,
        config: McpServerConfig,
    ) -> Self {
        Self {
            clock,
            peer_inspector,
            config,
            tools: None,
        }
    }

    #[must_use]
    pub fn with_tools<L>(
        self,
        registry: Arc<ToolRegistry>,
        ledger: ToolLedgerMode<L>,
    ) -> RunServerDeps<C, P, L> {
        RunServerDeps {
            clock: self.clock,
            peer_inspector: self.peer_inspector,
            config: self.config,
            tools: Some(ToolRuntime { registry, ledger }),
        }
    }
}

pub struct RunServerRequest<C, P, L = UnconfiguredToolLedger> {
    identity: RunIdentity,
    socket_spec: RunSocketSpec,
    peer_target: PeerProcessTarget,
    deps: RunServerDeps<C, P, L>,
}

impl<C, P, L> fmt::Debug for RunServerRequest<C, P, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunServerRequest")
            .field("identity", &self.identity)
            .field("socket_spec", &self.socket_spec)
            .field("peer_target", &self.peer_target)
            .field("deps", &self.deps)
            .finish()
    }
}

impl<C, P, L> RunServerRequest<C, P, L> {
    #[must_use]
    pub fn new(
        identity: RunIdentity,
        socket_spec: RunSocketSpec,
        peer_target: PeerProcessTarget,
        deps: RunServerDeps<C, P, L>,
    ) -> Self {
        Self {
            identity,
            socket_spec,
            peer_target,
            deps,
        }
    }
}

pub struct McpServerHandle<C: Clock + 'static> {
    auth: Arc<RunAuthBinding<C>>,
    expected_peer: Arc<ExpectedPeerPolicy>,
    owner_uid: UserId,
    run_dir: PathBuf,
    socket_path: PathBuf,
    events: Arc<ConnectionEventLog>,
    shutdown_tx: watch::Sender<ShutdownSignal>,
    accept_task: Option<JoinHandle<()>>,
    cleanup: SocketCleanupGuard,
}

impl<C: Clock + 'static> fmt::Debug for McpServerHandle<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpServerHandle")
            .field("auth", &self.auth)
            .field("owner_uid", &self.owner_uid)
            .field("run_dir", &self.run_dir)
            .field("socket_path", &self.socket_path)
            .finish_non_exhaustive()
    }
}

impl<C: Clock + 'static> McpServerHandle<C> {
    pub fn start<P: PeerProcessInspector + 'static, L: ToolCallLedger + 'static>(
        request: RunServerRequest<C, P, L>,
    ) -> Result<Self, McpServerError> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_error| McpServerError::RuntimeUnavailable)?;
        let auth = Arc::new(
            RunAuthBinding::issue(
                request.identity.run_id,
                request.identity.run_kind,
                request.deps.clock,
                request.deps.config.token_ttl(),
            )
            .map_err(McpServerError::Token)?,
        );
        let bound = BoundRunSocket::bind(&request.socket_spec).map_err(McpServerError::Socket)?;
        let (listener, owner_uid, run_dir, socket_path, cleanup) = bound.into_parts();
        let expected_peer = Arc::new(match request.peer_target.parent_pid {
            InitialParentTarget::Pending => {
                ExpectedPeerPolicy::new_pending(owner_uid, request.peer_target.executable_path)
            }
            InitialParentTarget::Bound(parent_pid) => ExpectedPeerPolicy::new_bound(
                owner_uid,
                parent_pid,
                request.peer_target.executable_path,
            ),
        });
        let events = Arc::new(ConnectionEventLog::new());
        let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
        let semaphore = Arc::new(Semaphore::new(
            request.deps.config.max_concurrent_connections().as_usize(),
        ));

        let loop_state = AcceptLoopState {
            auth: Arc::clone(&auth),
            expected_peer: Arc::clone(&expected_peer),
            inspector: request.deps.peer_inspector,
            config: request.deps.config,
            semaphore,
            events: Arc::clone(&events),
            tools: request.deps.tools,
        };
        let accept_task = runtime.spawn(run_accept_loop(listener, shutdown_rx, loop_state));

        Ok(Self {
            auth,
            expected_peer,
            owner_uid,
            run_dir,
            socket_path,
            events,
            shutdown_tx,
            accept_task: Some(accept_task),
            cleanup,
        })
    }

    pub fn bind_expected_parent_pid(&self, parent_pid: ProcessId) -> Result<(), McpServerError> {
        self.expected_peer
            .bind_parent_pid(parent_pid)
            .map_err(|_existing| McpServerError::ParentPidAlreadyBound)
    }

    #[must_use]
    pub fn run_id(&self) -> &AiRunId {
        self.auth.run_id()
    }

    #[must_use]
    pub fn run_kind(&self) -> RunKind {
        self.auth.run_kind()
    }

    #[must_use]
    pub const fn owner_uid(&self) -> UserId {
        self.owner_uid
    }

    #[must_use]
    pub fn run_dir(&self) -> &Path {
        &self.run_dir
    }

    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    #[must_use]
    pub fn expires_at(&self) -> Timestamp {
        self.auth.expires_at()
    }

    #[must_use]
    pub fn token(&self) -> &RunAuthToken {
        self.auth.token()
    }

    #[must_use]
    pub fn events(&self) -> Vec<ConnectionEvent> {
        self.events.snapshot()
    }

    pub fn revoke_token(&self) {
        self.auth.revoke();
    }

    pub async fn shutdown(mut self) -> Vec<ConnectionEvent> {
        self.auth.revoke();
        signal_shutdown(&self.shutdown_tx);
        if let Some(task) = self.accept_task.take() {
            match task.await {
                Ok(()) => {}
                Err(_error) => {
                    self.events.push(ConnectionEvent::SessionTaskFailed {
                        run_id: self.auth.run_id().clone(),
                    });
                }
            }
        }
        self.cleanup.cleanup();
        self.events.snapshot()
    }
}

impl<C: Clock + 'static> Drop for McpServerHandle<C> {
    fn drop(&mut self) {
        self.auth.revoke();
        signal_shutdown(&self.shutdown_tx);
        if let Some(task) = self.accept_task.take() {
            task.abort();
        }
        self.cleanup.cleanup();
    }
}

fn signal_shutdown(shutdown_tx: &watch::Sender<ShutdownSignal>) {
    match shutdown_tx.send(ShutdownSignal::Stopped) {
        Ok(()) => {}
        Err(_no_receivers) => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AcceptErrorAction {
    RetryAfterBackoff,
    StopLoop,
}

fn classify_accept_error(kind: ErrorKind) -> AcceptErrorAction {
    if kind == ErrorKind::InvalidInput
        || kind == ErrorKind::PermissionDenied
        || kind == ErrorKind::NotConnected
    {
        AcceptErrorAction::StopLoop
    } else {
        AcceptErrorAction::RetryAfterBackoff
    }
}

struct AcceptLoopState<C: Clock + 'static, P, L> {
    auth: Arc<RunAuthBinding<C>>,
    expected_peer: Arc<ExpectedPeerPolicy>,
    inspector: PeerInspectorMode<P>,
    config: McpServerConfig,
    semaphore: Arc<Semaphore>,
    events: Arc<ConnectionEventLog>,
    tools: Option<ToolRuntime<L>>,
}

async fn run_accept_loop<
    C: Clock + 'static,
    P: PeerProcessInspector + 'static,
    L: ToolCallLedger + 'static,
>(
    listener: UnixListener,
    mut shutdown_rx: watch::Receiver<ShutdownSignal>,
    state: AcceptLoopState<C, P, L>,
) {
    let mut active_tasks: Vec<JoinHandle<()>> = Vec::new();
    accept_until_shutdown(listener, &mut shutdown_rx, &state, &mut active_tasks).await;
    for task in active_tasks {
        match task.await {
            Ok(()) => {}
            Err(_error) => {
                state.events.push(ConnectionEvent::SessionTaskFailed {
                    run_id: state.auth.run_id().clone(),
                });
            }
        }
    }
}

async fn handle_accept_error(
    error_kind: ErrorKind,
    shutdown_rx: &mut watch::Receiver<ShutdownSignal>,
) -> AcceptErrorAction {
    match classify_accept_error(error_kind) {
        AcceptErrorAction::StopLoop => AcceptErrorAction::StopLoop,
        AcceptErrorAction::RetryAfterBackoff => {
            match race(
                wait_for_shutdown(shutdown_rx),
                tokio::time::sleep(ACCEPT_ERROR_BACKOFF),
            )
            .await
            {
                RaceOutcome::First(()) => AcceptErrorAction::StopLoop,
                RaceOutcome::Second(()) => AcceptErrorAction::RetryAfterBackoff,
            }
        }
    }
}

async fn accept_until_shutdown<
    C: Clock + 'static,
    P: PeerProcessInspector + 'static,
    L: ToolCallLedger + 'static,
>(
    listener: UnixListener,
    shutdown_rx: &mut watch::Receiver<ShutdownSignal>,
    state: &AcceptLoopState<C, P, L>,
    active_tasks: &mut Vec<JoinHandle<()>>,
) {
    loop {
        active_tasks.retain(|handle| !handle.is_finished());
        match race(wait_for_shutdown(shutdown_rx), listener.accept()).await {
            RaceOutcome::First(()) => break,
            RaceOutcome::Second(Ok((stream, _addr))) => {
                if let Some(task) = dispatch_connection(stream, state, shutdown_rx) {
                    active_tasks.push(task);
                }
            }
            RaceOutcome::Second(Err(error)) => {
                state.events.push(ConnectionEvent::AcceptFailed {
                    run_id: state.auth.run_id().clone(),
                });
                match handle_accept_error(error.kind(), shutdown_rx).await {
                    AcceptErrorAction::StopLoop => break,
                    AcceptErrorAction::RetryAfterBackoff => {}
                }
            }
        }
    }
}

fn dispatch_connection<
    C: Clock + 'static,
    P: PeerProcessInspector + 'static,
    L: ToolCallLedger + 'static,
>(
    stream: UnixStream,
    state: &AcceptLoopState<C, P, L>,
    shutdown_rx: &watch::Receiver<ShutdownSignal>,
) -> Option<JoinHandle<()>> {
    let Ok(permit) = Arc::clone(&state.semaphore).try_acquire_owned() else {
        state.events.push(ConnectionEvent::Rejected {
            run_id: state.auth.run_id().clone(),
            reason: ConnectionRejection::ConnectionLimitReached,
        });
        return None;
    };
    let context = ConnectionContext {
        auth: Arc::clone(&state.auth),
        expected_peer: Arc::clone(&state.expected_peer),
        inspector: state.inspector.clone(),
        config: state.config,
        shutdown_rx: shutdown_rx.clone(),
        events: Arc::clone(&state.events),
        tools: state.tools.clone(),
    };
    Some(tokio::spawn(async move {
        handle_unix_connection(stream, permit, context).await;
    }))
}

struct ConnectionContext<C: Clock + 'static, P, L> {
    auth: Arc<RunAuthBinding<C>>,
    expected_peer: Arc<ExpectedPeerPolicy>,
    inspector: PeerInspectorMode<P>,
    config: McpServerConfig,
    shutdown_rx: watch::Receiver<ShutdownSignal>,
    events: Arc<ConnectionEventLog>,
    tools: Option<ToolRuntime<L>>,
}

async fn handle_unix_connection<
    C: Clock + 'static,
    P: PeerProcessInspector,
    L: ToolCallLedger + 'static,
>(
    stream: UnixStream,
    _permit: OwnedSemaphorePermit,
    mut context: ConnectionContext<C, P, L>,
) {
    let run_id = context.auth.run_id().clone();
    let observed = match stream.peer_cred() {
        Ok(cred) => ObservedPeerCredentials::new(UserId::new(cred.uid()), cred.pid()),
        Err(_error) => {
            context.events.push(ConnectionEvent::Rejected {
                run_id,
                reason: ConnectionRejection::Peer(PeerVerificationFailure::CredentialsUnavailable),
            });
            return;
        }
    };

    let mut revoke_rx = context.auth.subscribe_revocation();
    let stop = wait_for_pre_session_stop(&mut context.shutdown_rx, &mut revoke_rx, &context.auth);
    let peer_check = verify_peer(
        observed,
        &context.expected_peer,
        &context.inspector,
        context.config.peer_inspection_timeout(),
    );
    match race(stop, peer_check).await {
        RaceOutcome::First(reason) => {
            context.events.push(ConnectionEvent::Rejected {
                run_id,
                reason: pre_session_stop_to_rejection(reason),
            });
            return;
        }
        RaceOutcome::Second(Ok(_verified)) => {}
        RaceOutcome::Second(Err(failure)) => {
            context.events.push(ConnectionEvent::Rejected {
                run_id,
                reason: ConnectionRejection::Peer(failure),
            });
            return;
        }
    }

    let (read_half, write_half) = stream.into_split();
    let params = StreamAuthParams {
        auth: context.auth,
        auth_line_limit: context.config.auth_line_limit(),
        auth_timeout: context.config.auth_timeout(),
        shutdown_rx: context.shutdown_rx,
        events: context.events,
        tools: context.tools,
    };
    Box::pin(authenticate_and_serve_stream(read_half, write_half, params)).await;
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;
    use std::time::Duration;

    use crate::token::{AuthLineLimit, TokenTtl};

    use super::{
        AcceptErrorAction, DEFAULT_AUTH_TIMEOUT_SECS, DEFAULT_MAX_CONCURRENT_CONNECTIONS,
        DEFAULT_PEER_INSPECTION_TIMEOUT_SECS, MAX_AUTH_TIMEOUT_SECS,
        MAX_CONCURRENT_CONNECTIONS_CAP, MAX_PEER_INSPECTION_TIMEOUT_SECS, MaxConcurrentConnections,
        McpServerConfig, McpServerError, classify_accept_error,
    };

    #[test]
    fn server_config_defaults_and_concurrency_bounds() {
        let default = McpServerConfig::default_config().expect("기본 설정");
        assert_eq!(
            default.auth_timeout(),
            Duration::from_secs(DEFAULT_AUTH_TIMEOUT_SECS),
            "기본 인증 제한 시간"
        );
        assert_eq!(
            default.peer_inspection_timeout(),
            Duration::from_secs(DEFAULT_PEER_INSPECTION_TIMEOUT_SECS),
            "기본 프로세스 조회 제한 시간"
        );
        assert_eq!(
            default.max_concurrent_connections().as_usize(),
            DEFAULT_MAX_CONCURRENT_CONNECTIONS,
            "기본 동시 연결 수"
        );
        assert_eq!(
            MaxConcurrentConnections::try_new(0_usize),
            Err(McpServerError::InvalidConcurrencyLimit),
            "동시 연결 0 거부"
        );
        assert_eq!(
            MaxConcurrentConnections::try_new(
                MAX_CONCURRENT_CONNECTIONS_CAP.saturating_add(1_usize)
            ),
            Err(McpServerError::InvalidConcurrencyLimit),
            "동시 연결 상한 초과 거부"
        );
        assert_eq!(
            classify_accept_error(ErrorKind::InvalidInput),
            AcceptErrorAction::StopLoop,
            "치명 accept 오류는 루프를 종료한다"
        );
        assert_eq!(
            classify_accept_error(ErrorKind::Interrupted),
            AcceptErrorAction::RetryAfterBackoff,
            "일시 accept 오류는 대기 후 재시도한다"
        );
    }

    #[test]
    fn server_config_rejects_invalid_timeouts() {
        let ttl = TokenTtl::default_ttl().expect("기본 TTL");
        let line_limit = AuthLineLimit::default_limit();
        let concurrency = MaxConcurrentConnections::try_new(1_usize).expect("동시 연결 1");
        assert_eq!(
            McpServerConfig::try_new(
                line_limit,
                Duration::ZERO,
                Duration::from_secs(1_u64),
                concurrency,
                ttl,
            ),
            Err(McpServerError::InvalidAuthTimeout),
            "0초 인증 제한 시간 거부"
        );
        assert_eq!(
            McpServerConfig::try_new(
                line_limit,
                Duration::from_secs(MAX_AUTH_TIMEOUT_SECS.saturating_add(1_u64)),
                Duration::from_secs(1_u64),
                concurrency,
                ttl,
            ),
            Err(McpServerError::InvalidAuthTimeout),
            "상한 초과 인증 제한 시간 거부"
        );
        assert_eq!(
            McpServerConfig::try_new(
                line_limit,
                Duration::from_secs(1_u64),
                Duration::ZERO,
                concurrency,
                ttl,
            ),
            Err(McpServerError::InvalidPeerInspectionTimeout),
            "0초 조회 제한 시간 거부"
        );
        assert_eq!(
            McpServerConfig::try_new(
                line_limit,
                Duration::from_secs(1_u64),
                Duration::from_secs(MAX_PEER_INSPECTION_TIMEOUT_SECS.saturating_add(1_u64)),
                concurrency,
                ttl,
            ),
            Err(McpServerError::InvalidPeerInspectionTimeout),
            "상한 초과 조회 제한 시간 거부"
        );
    }
}
