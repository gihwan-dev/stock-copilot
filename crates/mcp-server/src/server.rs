use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use application::{AiRunId, Clock, ExecutablePath, PeerProcessInspector, ProcessId, RunKind};
use domain::Timestamp;
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};
use tokio::task::JoinHandle;

use crate::handler::{
    ConnectionEvent, ConnectionEventLog, ConnectionRejection, RaceOutcome, ShutdownSignal,
    StreamAuthParams, authenticate_and_serve_stream, race, wait_for_revocation, wait_for_shutdown,
};
use crate::peer::{
    ExpectedPeerPolicy, ObservedPeerCredentials, PeerInspectorMode, PeerVerificationFailure,
    UserId, verify_peer,
};
use crate::socket::{BoundRunSocket, RunSocketSpec, SocketCleanupGuard, SocketSetupError};
use crate::token::{
    AuthLineLimit, AuthRejection, RunAuthBinding, RunAuthToken, TokenError, TokenTtl,
};

pub const DEFAULT_AUTH_TIMEOUT_SECS: u64 = 5_u64;
pub const MAX_AUTH_TIMEOUT_SECS: u64 = 30_u64;
pub const DEFAULT_PEER_INSPECTION_TIMEOUT_SECS: u64 = 2_u64;
pub const MAX_PEER_INSPECTION_TIMEOUT_SECS: u64 = 10_u64;
pub const DEFAULT_MAX_CONCURRENT_CONNECTIONS: usize = 4_usize;
pub const MAX_CONCURRENT_CONNECTIONS_CAP: usize = 16_usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpServerError {
    InvalidAuthTimeout,
    InvalidPeerInspectionTimeout,
    InvalidConcurrencyLimit,
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
pub struct PeerProcessTarget {
    parent_pid: ProcessId,
    executable_path: ExecutablePath,
}

impl PeerProcessTarget {
    #[must_use]
    pub const fn new(parent_pid: ProcessId, executable_path: ExecutablePath) -> Self {
        Self {
            parent_pid,
            executable_path,
        }
    }
}

pub struct RunServerDeps<C, P> {
    clock: Arc<C>,
    peer_inspector: PeerInspectorMode<P>,
    config: McpServerConfig,
}

impl<C, P> fmt::Debug for RunServerDeps<C, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunServerDeps")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<C, P> RunServerDeps<C, P> {
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
        }
    }
}

pub struct RunServerRequest<C, P> {
    identity: RunIdentity,
    socket_spec: RunSocketSpec,
    peer_target: PeerProcessTarget,
    deps: RunServerDeps<C, P>,
}

impl<C, P> fmt::Debug for RunServerRequest<C, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RunServerRequest")
            .field("identity", &self.identity)
            .field("socket_spec", &self.socket_spec)
            .field("peer_target", &self.peer_target)
            .field("deps", &self.deps)
            .finish()
    }
}

impl<C, P> RunServerRequest<C, P> {
    #[must_use]
    pub fn new(
        identity: RunIdentity,
        socket_spec: RunSocketSpec,
        peer_target: PeerProcessTarget,
        deps: RunServerDeps<C, P>,
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
    pub fn start<P: PeerProcessInspector + 'static>(
        request: RunServerRequest<C, P>,
    ) -> Result<Self, McpServerError> {
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
        let expected_peer = Arc::new(ExpectedPeerPolicy::new(
            owner_uid,
            request.peer_target.parent_pid,
            request.peer_target.executable_path,
        ));
        let events = Arc::new(ConnectionEventLog::new());
        let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
        let semaphore = Arc::new(Semaphore::new(
            request.deps.config.max_concurrent_connections().as_usize(),
        ));

        let loop_state = AcceptLoopState {
            auth: Arc::clone(&auth),
            expected_peer,
            inspector: request.deps.peer_inspector,
            config: request.deps.config,
            semaphore,
            events: Arc::clone(&events),
        };
        let accept_task = tokio::spawn(run_accept_loop(listener, shutdown_rx, loop_state));

        Ok(Self {
            auth,
            owner_uid,
            run_dir,
            socket_path,
            events,
            shutdown_tx,
            accept_task: Some(accept_task),
            cleanup,
        })
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

    pub async fn shutdown(mut self) {
        self.auth.revoke();
        signal_shutdown(&self.shutdown_tx);
        if let Some(task) = self.accept_task.take() {
            match task.await {
                Ok(()) => {}
                Err(_error) => {}
            }
        }
        self.cleanup.cleanup();
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

struct AcceptLoopState<C: Clock + 'static, P> {
    auth: Arc<RunAuthBinding<C>>,
    expected_peer: Arc<ExpectedPeerPolicy>,
    inspector: PeerInspectorMode<P>,
    config: McpServerConfig,
    semaphore: Arc<Semaphore>,
    events: Arc<ConnectionEventLog>,
}

async fn run_accept_loop<C: Clock + 'static, P: PeerProcessInspector + 'static>(
    listener: UnixListener,
    mut shutdown_rx: watch::Receiver<ShutdownSignal>,
    state: AcceptLoopState<C, P>,
) {
    let mut active_tasks: Vec<JoinHandle<()>> = Vec::new();
    accept_until_shutdown(listener, &mut shutdown_rx, &state, &mut active_tasks).await;
    for task in active_tasks {
        match task.await {
            Ok(()) => {}
            Err(_error) => {}
        }
    }
}

async fn accept_until_shutdown<C: Clock + 'static, P: PeerProcessInspector + 'static>(
    listener: UnixListener,
    shutdown_rx: &mut watch::Receiver<ShutdownSignal>,
    state: &AcceptLoopState<C, P>,
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
            RaceOutcome::Second(Err(_error)) => {}
        }
    }
}

fn dispatch_connection<C: Clock + 'static, P: PeerProcessInspector + 'static>(
    stream: UnixStream,
    state: &AcceptLoopState<C, P>,
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
    };
    Some(tokio::spawn(async move {
        handle_unix_connection(stream, permit, context).await;
    }))
}

struct ConnectionContext<C: Clock + 'static, P> {
    auth: Arc<RunAuthBinding<C>>,
    expected_peer: Arc<ExpectedPeerPolicy>,
    inspector: PeerInspectorMode<P>,
    config: McpServerConfig,
    shutdown_rx: watch::Receiver<ShutdownSignal>,
    events: Arc<ConnectionEventLog>,
}

async fn handle_unix_connection<C: Clock + 'static, P: PeerProcessInspector>(
    stream: UnixStream,
    _permit: OwnedSemaphorePermit,
    mut context: ConnectionContext<C, P>,
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
    let stop = race(
        wait_for_shutdown(&mut context.shutdown_rx),
        wait_for_revocation(&mut revoke_rx),
    );
    let peer_check = verify_peer(
        observed,
        &context.expected_peer,
        &context.inspector,
        context.config.peer_inspection_timeout(),
    );
    match race(stop, peer_check).await {
        RaceOutcome::First(_stopped) => {
            context.events.push(ConnectionEvent::Rejected {
                run_id,
                reason: ConnectionRejection::Auth(AuthRejection::TokenRevoked),
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
    let params = StreamAuthParams::new(
        context.auth,
        context.config.auth_line_limit(),
        context.config.auth_timeout(),
        context.shutdown_rx,
        context.events,
    );
    authenticate_and_serve_stream(read_half, write_half, params).await;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::token::{AuthLineLimit, TokenTtl};

    use super::{
        DEFAULT_AUTH_TIMEOUT_SECS, DEFAULT_MAX_CONCURRENT_CONNECTIONS,
        DEFAULT_PEER_INSPECTION_TIMEOUT_SECS, MAX_AUTH_TIMEOUT_SECS,
        MAX_CONCURRENT_CONNECTIONS_CAP, MAX_PEER_INSPECTION_TIMEOUT_SECS, MaxConcurrentConnections,
        McpServerConfig, McpServerError,
    };

    #[test]
    fn server_config_defaults_and_concurrency_bounds() {
        let default_cfg = McpServerConfig::default_config().expect("기본 설정");
        assert_eq!(
            default_cfg.auth_timeout(),
            Duration::from_secs(DEFAULT_AUTH_TIMEOUT_SECS),
            "기본 인증 제한 시간"
        );
        assert_eq!(
            default_cfg.peer_inspection_timeout(),
            Duration::from_secs(DEFAULT_PEER_INSPECTION_TIMEOUT_SECS),
            "기본 피어 조회 제한 시간"
        );
        assert_eq!(
            default_cfg.max_concurrent_connections().as_usize(),
            DEFAULT_MAX_CONCURRENT_CONNECTIONS,
            "기본 동시 연결 수"
        );
        assert_eq!(
            MaxConcurrentConnections::try_new(0_usize),
            Err(McpServerError::InvalidConcurrencyLimit),
            "0개 연결 상한 거부"
        );
        assert_eq!(
            MaxConcurrentConnections::try_new(
                MAX_CONCURRENT_CONNECTIONS_CAP.saturating_add(1_usize)
            ),
            Err(McpServerError::InvalidConcurrencyLimit),
            "최대 초과 연결 상한 거부"
        );
        assert_eq!(
            MaxConcurrentConnections::try_new(MAX_CONCURRENT_CONNECTIONS_CAP)
                .map(MaxConcurrentConnections::as_usize),
            Ok(MAX_CONCURRENT_CONNECTIONS_CAP),
            "상한 경계 허용"
        );
    }

    #[test]
    fn server_config_validates_timeout_bounds() {
        let ttl = TokenTtl::default_ttl().expect("기본 TTL");
        let limit = AuthLineLimit::default_limit();
        let concurrency = MaxConcurrentConnections::default_limit();

        assert_eq!(
            McpServerConfig::try_new(
                limit,
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
                limit,
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
                limit,
                Duration::from_secs(1_u64),
                Duration::ZERO,
                concurrency,
                ttl,
            ),
            Err(McpServerError::InvalidPeerInspectionTimeout),
            "0초 피어 조회 제한 시간 거부"
        );
        assert_eq!(
            McpServerConfig::try_new(
                limit,
                Duration::from_secs(1_u64),
                Duration::from_secs(MAX_PEER_INSPECTION_TIMEOUT_SECS.saturating_add(1_u64)),
                concurrency,
                ttl,
            ),
            Err(McpServerError::InvalidPeerInspectionTimeout),
            "상한 초과 피어 조회 제한 시간 거부"
        );
    }

    #[test]
    fn server_config_accepts_upper_and_lower_boundary_values() {
        let ttl = TokenTtl::default_ttl().expect("기본 TTL");
        let limit = AuthLineLimit::default_limit();
        let cfg = McpServerConfig::try_new(
            limit,
            Duration::from_secs(MAX_AUTH_TIMEOUT_SECS),
            Duration::from_secs(MAX_PEER_INSPECTION_TIMEOUT_SECS),
            MaxConcurrentConnections::try_new(1_usize).expect("1연결"),
            ttl,
        )
        .expect("상한 경계 설정은 허용한다");
        assert_eq!(cfg.auth_line_limit(), limit, "인증 줄 한도 보존");
        assert_eq!(
            cfg.auth_timeout(),
            Duration::from_secs(MAX_AUTH_TIMEOUT_SECS),
            "인증 제한 시간 보존"
        );
        assert_eq!(
            cfg.peer_inspection_timeout(),
            Duration::from_secs(MAX_PEER_INSPECTION_TIMEOUT_SECS),
            "피어 조회 제한 시간 보존"
        );
        assert_eq!(
            cfg.max_concurrent_connections().as_usize(),
            1_usize,
            "최소 동시 연결 수 보존"
        );
        assert_eq!(cfg.token_ttl(), ttl, "토큰 TTL 보존");
    }
}
