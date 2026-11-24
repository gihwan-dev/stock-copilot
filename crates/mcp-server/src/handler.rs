use std::borrow::Cow;
use std::collections::VecDeque;
use std::fmt;
use std::future::{Future, poll_fn, ready};
use std::io::{self, ErrorKind};
use std::pin::{Pin, pin};
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::Duration;

use application::{AiRunId, Clock, RunKind, ToolCallLedger};
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestMethod, CallToolRequestParams, CallToolResponse, CompleteRequestParams,
    CompleteResult, DiscoverResult, Implementation, InitializeRequestParams, InitializeResult,
    JsonObject, ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer, RunningService, ServiceExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, ReadBuf};
use tokio::sync::watch;
use zeroize::Zeroizing;

use crate::peer::PeerVerificationFailure;
use crate::token::{AuthLineLimit, AuthRejection, AuthenticatedRun, RunAuthBinding, TokenStatus};
use crate::tools::{DispatchCallInput, ToolRuntime, UnconfiguredToolLedger, dispatch_tool_call};

const SERVER_NAME: &str = "stock-copilot-mcp";
const SERVER_VERSION: &str = "0.1.0";
const MAX_CONNECTION_EVENTS: usize = 256_usize;
const EXPIRY_CHECK_INTERVAL: Duration = Duration::from_millis(50_u64);

const SUPPORTED_VERSIONS: [ProtocolVersion; 3] = [
    ProtocolVersion::V_2026_07_28,
    ProtocolVersion::V_2025_11_25,
    ProtocolVersion::V_2025_06_18,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShutdownSignal {
    Running,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SessionStopSignal {
    Active,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PreSessionStopReason {
    ServerShuttingDown,
    TokenRevoked,
    TokenExpired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionRejection {
    ConnectionLimitReached,
    ServerShuttingDown,
    Peer(PeerVerificationFailure),
    AuthTimeout,
    AuthLineTooLong,
    AuthIoError,
    Auth(AuthRejection),
    ProtocolInitializeFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    Accepted {
        run_id: AiRunId,
        run_kind: RunKind,
    },
    Rejected {
        run_id: AiRunId,
        reason: ConnectionRejection,
    },
    Closed {
        run_id: AiRunId,
    },
    AcceptFailed {
        run_id: AiRunId,
    },
    SessionTaskFailed {
        run_id: AiRunId,
    },
}

#[derive(Debug)]
pub(crate) struct ConnectionEventLog {
    events: RwLock<VecDeque<ConnectionEvent>>,
    max_events: usize,
}

impl ConnectionEventLog {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self::with_capacity_limit(MAX_CONNECTION_EVENTS)
    }

    #[must_use]
    pub(crate) fn with_capacity_limit(max_events: usize) -> Self {
        let cap = max_events.max(1_usize);
        Self {
            events: RwLock::new(VecDeque::with_capacity(cap)),
            max_events: cap,
        }
    }

    pub(crate) fn push(&self, event: ConnectionEvent) {
        let mut guard = match self.events.write() {
            Ok(locked) => locked,
            Err(poisoned) => poisoned.into_inner(),
        };
        if guard.len() >= self.max_events {
            let _evicted = guard.pop_front();
        }
        guard.push_back(event);
    }

    #[must_use]
    pub(crate) fn snapshot(&self) -> Vec<ConnectionEvent> {
        let guard = match self.events.read() {
            Ok(locked) => locked,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.iter().cloned().collect()
    }
}

fn signal_session_stop(tx: &watch::Sender<SessionStopSignal>) {
    match tx.send(SessionStopSignal::Stopped) {
        Ok(()) => {}
        Err(_no_receivers) => {}
    }
}

struct ExpiryGuardedReader<R, C: Clock + 'static> {
    inner: R,
    auth: Arc<RunAuthBinding<C>>,
    stop_tx: watch::Sender<SessionStopSignal>,
}

impl<R: AsyncRead + Unpin, C: Clock + 'static> AsyncRead for ExpiryGuardedReader<R, C> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.auth.ensure_active().is_err() {
            signal_session_stop(&this.stop_tx);
            return Poll::Ready(Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "run token is no longer active",
            )));
        }
        Pin::new(&mut this.inner).poll_read(cx, buf)
    }
}

struct ExpiryGuardedWriter<W, C: Clock + 'static> {
    inner: W,
    auth: Arc<RunAuthBinding<C>>,
    stop_tx: watch::Sender<SessionStopSignal>,
}

impl<W: AsyncWrite + Unpin, C: Clock + 'static> AsyncWrite for ExpiryGuardedWriter<W, C> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        if this.auth.ensure_active().is_err() {
            signal_session_stop(&this.stop_tx);
            return Poll::Ready(Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "run token is no longer active",
            )));
        }
        Pin::new(&mut this.inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.auth.ensure_active().is_err() {
            signal_session_stop(&this.stop_tx);
            return Poll::Ready(Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "run token is no longer active",
            )));
        }
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_shutdown(cx)
    }
}

pub(crate) struct McpSkeletonHandler<
    C: Clock + 'static,
    L: ToolCallLedger + 'static = UnconfiguredToolLedger,
> {
    auth: Arc<RunAuthBinding<C>>,
    stop_tx: watch::Sender<SessionStopSignal>,
    tools: Option<ToolRuntime<L>>,
    negotiated_protocol: RwLock<ProtocolVersion>,
}

impl<C: Clock + 'static, L: ToolCallLedger + 'static> fmt::Debug for McpSkeletonHandler<C, L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpSkeletonHandler")
            .field("run_id", self.auth.run_id())
            .field("run_kind", &self.auth.run_kind())
            .finish_non_exhaustive()
    }
}

impl<C: Clock + 'static, L: ToolCallLedger + 'static> McpSkeletonHandler<C, L> {
    #[must_use]
    pub(crate) fn with_optional_tools(
        auth: Arc<RunAuthBinding<C>>,
        stop_tx: watch::Sender<SessionStopSignal>,
        tools: Option<ToolRuntime<L>>,
    ) -> Self {
        Self {
            auth,
            stop_tx,
            tools,
            negotiated_protocol: RwLock::new(ProtocolVersion::V_2026_07_28),
        }
    }

    fn resolve_protocol_version(&self, context: &RequestContext<RoleServer>) -> String {
        if let Some(meta_version) = context
            .meta
            .get("io.modelcontextprotocol/protocolVersion")
            .and_then(serde_json::Value::as_str)
        {
            return meta_version.to_owned();
        }
        let guard = match self.negotiated_protocol.read() {
            Ok(locked) => locked,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.to_string()
    }

    fn ensure_active(&self) -> Result<(), McpError> {
        match self.auth.ensure_active() {
            Ok(()) => Ok(()),
            Err(AuthRejection::TokenRevoked | AuthRejection::TokenExpired) => {
                signal_session_stop(&self.stop_tx);
                Err(McpError::invalid_request(
                    "run session is no longer active",
                    None,
                ))
            }
            Err(
                AuthRejection::RunMismatch
                | AuthRejection::AuthMissing
                | AuthRejection::InvalidAuthFormat
                | AuthRejection::MissingToken
                | AuthRejection::InvalidToken,
            ) => {
                signal_session_stop(&self.stop_tx);
                Err(McpError::invalid_request(
                    "run session authorization rejected",
                    None,
                ))
            }
        }
    }
}

impl<C: Clock + 'static, L: ToolCallLedger + 'static> ServerHandler for McpSkeletonHandler<C, L> {
    fn get_info(&self) -> ServerConfig {
        let mut config = ServerConfig::new(ServerCapabilities::builder().enable_tools().build());
        config.protocol_version = ProtocolVersion::V_2026_07_28;
        config.server_info = Implementation::new(SERVER_NAME, SERVER_VERSION);
        config
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(&SUPPORTED_VERSIONS)
    }

    fn ping(
        &self,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<(), McpError>> + Send + '_ {
        ready(self.ensure_active())
    }

    fn initialize(
        &self,
        request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, McpError>> + Send + '_ {
        let result = self.ensure_active().and_then(|()| {
            context.peer.set_peer_info(request.clone());
            let negotiated = self.negotiate_initialize(&request)?;
            let mut guard = match self.negotiated_protocol.write() {
                Ok(locked) => locked,
                Err(poisoned) => poisoned.into_inner(),
            };
            *guard = negotiated.protocol_version.clone();
            Ok(negotiated)
        });
        ready(result)
    }

    fn discover(
        &self,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<DiscoverResult, McpError>> + Send + '_ {
        let result = self.ensure_active().map(|()| {
            DiscoverResult::from_server_info(
                self.supported_protocol_versions().into_owned(),
                self.get_info(),
            )
        });
        ready(result)
    }

    fn complete(
        &self,
        _: CompleteRequestParams,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CompleteResult, McpError>> + Send + '_ {
        ready(self.ensure_active().map(|()| CompleteResult::default()))
    }

    fn list_prompts(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, McpError>> + Send + '_ {
        ready(self.ensure_active().map(|()| ListPromptsResult::default()))
    }

    fn list_resources(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + Send + '_ {
        ready(
            self.ensure_active()
                .map(|()| ListResourcesResult::default()),
        )
    }

    fn list_resource_templates(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, McpError>> + Send + '_ {
        ready(
            self.ensure_active()
                .map(|()| ListResourceTemplatesResult::default()),
        )
    }

    fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + Send + '_ {
        let result = self.ensure_active().map(|()| {
            let mut list = ListToolsResult::default();
            if let Some(runtime) = &self.tools {
                list.tools = runtime.registry.list_rmcp_tools();
            }
            list
        });
        ready(result)
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        self.ensure_active()?;
        let Some(runtime) = &self.tools else {
            return Err(McpError::method_not_found::<CallToolRequestMethod>());
        };
        let request_id = context.id.to_string();
        let protocol_version = self.resolve_protocol_version(&context);
        let empty_args = JsonObject::new();
        let arguments = match &request.arguments {
            Some(args) => args,
            None => &empty_args,
        };
        let call_result = dispatch_tool_call(
            runtime,
            &self.auth,
            DispatchCallInput {
                request_id_raw: &request_id,
                protocol_version_raw: &protocol_version,
                raw_tool_name: request.name.as_ref(),
                arguments,
            },
        )
        .await;
        self.ensure_active()?;
        Ok(CallToolResponse::from(call_result))
    }
}

pub(crate) struct StreamAuthParams<
    C: Clock + 'static,
    L: ToolCallLedger + 'static = UnconfiguredToolLedger,
> {
    pub(crate) auth: Arc<RunAuthBinding<C>>,
    pub(crate) auth_line_limit: AuthLineLimit,
    pub(crate) auth_timeout: Duration,
    pub(crate) shutdown_rx: watch::Receiver<ShutdownSignal>,
    pub(crate) events: Arc<ConnectionEventLog>,
    pub(crate) tools: Option<ToolRuntime<L>>,
}

pub(crate) enum RaceOutcome<A, B> {
    First(A),
    Second(B),
}

pub(crate) async fn race<A, B, FA, FB>(first: FA, second: FB) -> RaceOutcome<A, B>
where
    FA: Future<Output = A>,
    FB: Future<Output = B>,
{
    let mut pinned_first = pin!(first);
    let mut pinned_second = pin!(second);
    poll_fn(|cx| {
        if let Poll::Ready(value) = pinned_first.as_mut().poll(cx) {
            return Poll::Ready(RaceOutcome::First(value));
        }
        if let Poll::Ready(value) = pinned_second.as_mut().poll(cx) {
            return Poll::Ready(RaceOutcome::Second(value));
        }
        Poll::Pending
    })
    .await
}

pub(crate) async fn wait_for_shutdown(shutdown_rx: &mut watch::Receiver<ShutdownSignal>) {
    let initial = *shutdown_rx.borrow();
    match initial {
        ShutdownSignal::Stopped => return,
        ShutdownSignal::Running => {}
    }
    while shutdown_rx.changed().await.is_ok() {
        let current = *shutdown_rx.borrow();
        match current {
            ShutdownSignal::Stopped => return,
            ShutdownSignal::Running => {}
        }
    }
}

pub(crate) async fn wait_for_revocation(revoke_rx: &mut watch::Receiver<TokenStatus>) {
    let initial = *revoke_rx.borrow();
    match initial {
        TokenStatus::Revoked => return,
        TokenStatus::Active => {}
    }
    while revoke_rx.changed().await.is_ok() {
        let current = *revoke_rx.borrow();
        match current {
            TokenStatus::Revoked => return,
            TokenStatus::Active => {}
        }
    }
}

async fn wait_for_session_signal(stop_rx: &mut watch::Receiver<SessionStopSignal>) {
    let initial = *stop_rx.borrow();
    match initial {
        SessionStopSignal::Stopped => return,
        SessionStopSignal::Active => {}
    }
    while stop_rx.changed().await.is_ok() {
        let current = *stop_rx.borrow();
        match current {
            SessionStopSignal::Stopped => return,
            SessionStopSignal::Active => {}
        }
    }
}

pub(crate) async fn wait_for_expiry<C: Clock + 'static>(auth: &RunAuthBinding<C>) {
    loop {
        if auth.ensure_active().is_err() {
            return;
        }
        let sleep_for = auth.remaining_until_expiry().min(EXPIRY_CHECK_INTERVAL);
        if sleep_for.is_zero() {
            return;
        }
        tokio::time::sleep(sleep_for).await;
    }
}

pub(crate) async fn wait_for_pre_session_stop<C: Clock + 'static>(
    shutdown_rx: &mut watch::Receiver<ShutdownSignal>,
    revoke_rx: &mut watch::Receiver<TokenStatus>,
    auth: &RunAuthBinding<C>,
) -> PreSessionStopReason {
    let stop_or_revoke = race(
        wait_for_shutdown(shutdown_rx),
        wait_for_revocation(revoke_rx),
    );
    match race(stop_or_revoke, wait_for_expiry(auth)).await {
        RaceOutcome::First(RaceOutcome::First(())) => PreSessionStopReason::ServerShuttingDown,
        RaceOutcome::First(RaceOutcome::Second(())) => PreSessionStopReason::TokenRevoked,
        RaceOutcome::Second(()) => match auth.ensure_active() {
            Err(AuthRejection::TokenRevoked) => PreSessionStopReason::TokenRevoked,
            Ok(())
            | Err(
                AuthRejection::TokenExpired
                | AuthRejection::RunMismatch
                | AuthRejection::AuthMissing
                | AuthRejection::InvalidAuthFormat
                | AuthRejection::MissingToken
                | AuthRejection::InvalidToken,
            ) => PreSessionStopReason::TokenExpired,
        },
    }
}

pub(crate) const fn pre_session_stop_to_rejection(
    reason: PreSessionStopReason,
) -> ConnectionRejection {
    match reason {
        PreSessionStopReason::ServerShuttingDown => ConnectionRejection::ServerShuttingDown,
        PreSessionStopReason::TokenRevoked => {
            ConnectionRejection::Auth(AuthRejection::TokenRevoked)
        }
        PreSessionStopReason::TokenExpired => {
            ConnectionRejection::Auth(AuthRejection::TokenExpired)
        }
    }
}

async fn wait_for_session_stop<C: Clock + 'static>(
    shutdown_rx: &mut watch::Receiver<ShutdownSignal>,
    revoke_rx: &mut watch::Receiver<TokenStatus>,
    stop_rx: &mut watch::Receiver<SessionStopSignal>,
    auth: &RunAuthBinding<C>,
) {
    let pre_stop = wait_for_pre_session_stop(shutdown_rx, revoke_rx, auth);
    match race(pre_stop, wait_for_session_signal(stop_rx)).await {
        RaceOutcome::First(_) | RaceOutcome::Second(()) => {}
    }
}

async fn read_bounded_auth_line<R: AsyncRead + Unpin>(
    reader: &mut R,
    limit: AuthLineLimit,
    timeout: Duration,
) -> Result<Zeroizing<Vec<u8>>, ConnectionRejection> {
    let max_bytes = limit.as_usize();
    let read_future = async {
        let mut buffer = Zeroizing::new(Vec::with_capacity(max_bytes));
        let mut single = [0_u8; 1];
        loop {
            let count = reader
                .read(&mut single)
                .await
                .map_err(|_error| ConnectionRejection::AuthIoError)?;
            if count == 0_usize {
                return Err(ConnectionRejection::Auth(AuthRejection::AuthMissing));
            }
            let [byte] = single;
            if buffer.len() >= max_bytes {
                return Err(ConnectionRejection::AuthLineTooLong);
            }
            buffer.push(byte);
            if byte == b'\n' {
                return Ok(buffer);
            }
        }
    };
    tokio::time::timeout(timeout, read_future)
        .await
        .map_err(|_elapsed| ConnectionRejection::AuthTimeout)?
}

async fn read_and_verify_auth<R, C, L>(
    reader: &mut R,
    params: &mut StreamAuthParams<C, L>,
    revoke_rx: &mut watch::Receiver<TokenStatus>,
) -> Result<AuthenticatedRun, ConnectionRejection>
where
    R: AsyncRead + Unpin,
    C: Clock + 'static,
    L: ToolCallLedger + 'static,
{
    let stop = wait_for_pre_session_stop(&mut params.shutdown_rx, revoke_rx, &params.auth);
    let read_auth = read_bounded_auth_line(reader, params.auth_line_limit, params.auth_timeout);
    let line = match race(stop, read_auth).await {
        RaceOutcome::First(reason) => return Err(pre_session_stop_to_rejection(reason)),
        RaceOutcome::Second(result) => result?,
    };
    params
        .auth
        .verify_auth_line(line.as_slice(), params.auth.run_id())
        .map_err(ConnectionRejection::Auth)
}

async fn drive_running_mcp_service<C: Clock + 'static, L: ToolCallLedger + 'static>(
    running: RunningService<RoleServer, McpSkeletonHandler<C, L>>,
    params: &mut StreamAuthParams<C, L>,
    revoke_rx: &mut watch::Receiver<TokenStatus>,
    stop_rx: &mut watch::Receiver<SessionStopSignal>,
    run_id: &AiRunId,
) {
    let cancellation = running.cancellation_token();
    let mut wait_handle = tokio::spawn(async move { running.waiting().await });
    match race(
        wait_for_session_stop(&mut params.shutdown_rx, revoke_rx, stop_rx, &params.auth),
        &mut wait_handle,
    )
    .await
    {
        RaceOutcome::First(()) => {
            cancellation.cancel();
            match wait_handle.await {
                Ok(Ok(_quit)) => {}
                Ok(Err(_)) | Err(_) => {
                    params.events.push(ConnectionEvent::SessionTaskFailed {
                        run_id: run_id.clone(),
                    });
                }
            }
        }
        RaceOutcome::Second(Ok(Ok(_quit))) => {}
        RaceOutcome::Second(Ok(Err(_)) | Err(_)) => {
            params.events.push(ConnectionEvent::SessionTaskFailed {
                run_id: run_id.clone(),
            });
        }
    }
    params.events.push(ConnectionEvent::Closed {
        run_id: run_id.clone(),
    });
}

async fn start_and_run_mcp_service<R, W, C, L>(
    reader: R,
    writer: W,
    params: &mut StreamAuthParams<C, L>,
    revoke_rx: &mut watch::Receiver<TokenStatus>,
    run_id: &AiRunId,
) where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
    C: Clock + 'static,
    L: ToolCallLedger + 'static,
{
    let (stop_tx, mut stop_rx) = watch::channel(SessionStopSignal::Active);
    let guarded_reader = ExpiryGuardedReader {
        inner: reader,
        auth: Arc::clone(&params.auth),
        stop_tx: stop_tx.clone(),
    };
    let guarded_writer = ExpiryGuardedWriter {
        inner: writer,
        auth: Arc::clone(&params.auth),
        stop_tx: stop_tx.clone(),
    };
    let handler = McpSkeletonHandler::with_optional_tools(
        Arc::clone(&params.auth),
        stop_tx,
        params.tools.clone(),
    );

    let running = match race(
        wait_for_session_stop(
            &mut params.shutdown_rx,
            revoke_rx,
            &mut stop_rx,
            &params.auth,
        ),
        handler.serve((guarded_reader, guarded_writer)),
    )
    .await
    {
        RaceOutcome::First(()) => {
            params.events.push(ConnectionEvent::Closed {
                run_id: run_id.clone(),
            });
            return;
        }
        RaceOutcome::Second(Ok(service)) => service,
        RaceOutcome::Second(Err(_error)) => {
            params.events.push(ConnectionEvent::Rejected {
                run_id: run_id.clone(),
                reason: ConnectionRejection::ProtocolInitializeFailed,
            });
            return;
        }
    };

    drive_running_mcp_service(running, params, revoke_rx, &mut stop_rx, run_id).await;
}

pub(crate) async fn authenticate_and_serve_stream<R, W, C, L>(
    mut reader: R,
    writer: W,
    mut params: StreamAuthParams<C, L>,
) where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
    C: Clock + 'static,
    L: ToolCallLedger + 'static,
{
    let run_id = params.auth.run_id().clone();
    let mut revoke_rx = params.auth.subscribe_revocation();
    let authenticated = match read_and_verify_auth(&mut reader, &mut params, &mut revoke_rx).await {
        Ok(session) => session,
        Err(reason) => {
            params
                .events
                .push(ConnectionEvent::Rejected { run_id, reason });
            return;
        }
    };

    params.events.push(ConnectionEvent::Accepted {
        run_id: authenticated.run_id().clone(),
        run_kind: authenticated.run_kind(),
    });
    start_and_run_mcp_service(reader, writer, &mut params, &mut revoke_rx, &run_id).await;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use application::{AiRunId, RunKind};
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use rmcp::ServerHandler;
    use rmcp::model::ProtocolVersion;
    use serde_json::Value;
    use testkit_ports::SteppedClock;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, duplex};
    use tokio::sync::watch;

    use crate::token::{AuthLineLimit, AuthRejection, RunAuthBinding, TokenTtl};

    use super::{
        ConnectionEvent, ConnectionEventLog, ConnectionRejection, McpSkeletonHandler,
        SessionStopSignal, ShutdownSignal, StreamAuthParams, authenticate_and_serve_stream,
    };

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("테스트 런타임")
    }

    fn timestamp_at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("유효한 시각")
    }

    async fn read_json_line(reader: &mut BufReader<tokio::io::ReadHalf<DuplexStream>>) -> Value {
        let mut line = String::new();
        let count = tokio::time::timeout(Duration::from_secs(2_u64), reader.read_line(&mut line))
            .await
            .expect("응답 시간 초과")
            .expect("응답 읽기");
        assert!(count > 0_usize, "응답 줄이 비어 있으면 안 된다");
        serde_json::from_str(&line).expect("유효 JSON-RPC 응답")
    }

    #[test]
    fn event_log_evicts_oldest_entries_when_bounded() {
        let log = ConnectionEventLog::with_capacity_limit(2_usize);
        let first = AiRunId::try_new("run-1").expect("id1");
        let second = AiRunId::try_new("run-2").expect("id2");
        let third = AiRunId::try_new("run-3").expect("id3");
        log.push(ConnectionEvent::Closed { run_id: first });
        log.push(ConnectionEvent::Closed {
            run_id: second.clone(),
        });
        log.push(ConnectionEvent::Closed {
            run_id: third.clone(),
        });
        assert_eq!(
            log.snapshot(),
            vec![
                ConnectionEvent::Closed { run_id: second },
                ConnectionEvent::Closed { run_id: third },
            ],
            "이벤트 로그 상한을 넘으면 가장 오래된 항목을 밀어낸다"
        );
    }

    #[test]
    fn skeleton_handler_declares_both_protocol_generations_and_debug_is_redacted() {
        let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
        let binding = Arc::new(
            RunAuthBinding::issue(
                AiRunId::try_new("run-proto-1").expect("실행 ID"),
                RunKind::Analysis,
                clock,
                TokenTtl::try_from_seconds(60_u64).expect("TTL"),
            )
            .expect("발급"),
        );
        let (stop_tx, _stop_rx) = watch::channel(SessionStopSignal::Active);
        let handler: McpSkeletonHandler<SteppedClock> =
            McpSkeletonHandler::with_optional_tools(Arc::clone(&binding), stop_tx, None);
        let versions = handler.supported_protocol_versions();
        assert!(
            versions.contains(&ProtocolVersion::V_2026_07_28),
            "2026-07-28 지원 선언"
        );
        assert!(
            versions.contains(&ProtocolVersion::V_2025_06_18),
            "2025-06-18 지원 선언"
        );
        assert_eq!(
            handler.get_info().protocol_version,
            ProtocolVersion::V_2026_07_28,
            "기본 프로토콜은 2026-07-28"
        );
        let debug_text = format!("{handler:?}");
        let raw = std::str::from_utf8(binding.token().as_exposed_bytes()).expect("UTF-8");
        assert!(
            debug_text.contains("McpSkeletonHandler") && debug_text.contains("run-proto-1"),
            "핸들러 Debug 출력은 구조와 실행 ID를 담는다"
        );
        assert!(
            !debug_text.contains(raw),
            "핸들러 Debug에 원문 토큰이 없어야 한다"
        );
    }

    async fn send_auth_and_skeleton_requests(
        client_write: &mut tokio::io::WriteHalf<DuplexStream>,
        reader: &mut BufReader<tokio::io::ReadHalf<DuplexStream>>,
        binding: &RunAuthBinding<SteppedClock>,
    ) {
        let mut auth_line = b"AUTH ".to_vec();
        auth_line.extend_from_slice(binding.token().as_exposed_bytes());
        auth_line.push(b'\n');
        client_write.write_all(&auth_line).await.expect("인증 전송");

        let meta = r#""_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}"#;
        let requests = [
            format!(r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":{{{meta}}}}}"#),
            format!(r#"{{"jsonrpc":"2.0","id":2,"method":"prompts/list","params":{{{meta}}}}}"#),
            format!(r#"{{"jsonrpc":"2.0","id":3,"method":"resources/list","params":{{{meta}}}}}"#),
            format!(
                r#"{{"jsonrpc":"2.0","id":4,"method":"resources/templates/list","params":{{{meta}}}}}"#
            ),
            format!(
                r#"{{"jsonrpc":"2.0","id":5,"method":"completion/complete","params":{{{meta},"ref":{{"type":"ref/prompt","name":"x"}},"argument":{{"name":"a","value":"b"}}}}}}"#
            ),
        ];
        for request in requests {
            client_write
                .write_all(format!("{request}\n").as_bytes())
                .await
                .expect("요청 전송");
            let response = read_json_line(reader).await;
            assert!(
                response.get("result").is_some(),
                "뼈대 메서드 정상 응답: {response}"
            );
        }
    }

    #[test]
    fn duplex_stream_serves_all_skeleton_methods_and_blocks_expired_ping() {
        runtime().block_on(async {
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let run_id = AiRunId::try_new("run-duplex-1").expect("실행 ID");
            let ttl = TokenTtl::try_from_seconds(60_u64).expect("TTL");
            let binding = Arc::new(
                RunAuthBinding::issue(run_id.clone(), RunKind::Analysis, Arc::clone(&clock), ttl)
                    .expect("발급"),
            );
            let events = Arc::new(ConnectionEventLog::new());
            let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
            let (client, server) = duplex(16_384_usize);
            let (server_read, server_write) = tokio::io::split(server);
            let params: StreamAuthParams<SteppedClock> = StreamAuthParams {
                auth: Arc::clone(&binding),
                auth_line_limit: AuthLineLimit::default_limit(),
                auth_timeout: Duration::from_secs(2_u64),
                shutdown_rx,
                events: Arc::clone(&events),
                tools: None,
            };
            let task = tokio::spawn(authenticate_and_serve_stream(
                server_read,
                server_write,
                params,
            ));

            let (client_read, mut client_write) = tokio::io::split(client);
            let mut reader = BufReader::new(client_read);
            send_auth_and_skeleton_requests(&mut client_write, &mut reader, &binding).await;

            clock
                .advance_to(timestamp_at(1_700_000_060_i64))
                .expect("시각 이동");
            client_write
                .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":6,\"method\":\"ping\"}\n")
                .await
                .expect("만료 후 ping 전송");
            let mut after_expiry = String::new();
            let after_count = tokio::time::timeout(
                Duration::from_secs(2_u64),
                reader.read_line(&mut after_expiry),
            )
            .await
            .expect("만료 후 연결 종료 대기")
            .expect("만료 후 읽기");
            assert_eq!(after_count, 0_usize, "만료 뒤 ping은 연결을 닫는다");
            let _closed = shutdown_tx.send(ShutdownSignal::Stopped).is_err();
            task.await.expect("서버 태스크 종료");
            let recorded = events.snapshot();
            assert!(
                recorded.contains(&ConnectionEvent::Accepted {
                    run_id: run_id.clone(),
                    run_kind: RunKind::Analysis,
                }) && recorded.contains(&ConnectionEvent::Closed { run_id }),
                "인증 성공 및 종료 이벤트 기록"
            );
        });
    }

    #[test]
    fn duplex_stream_rejects_shutdown_during_auth() {
        runtime().block_on(async {
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let run_id = AiRunId::try_new("run-duplex-2").expect("실행 ID");
            let binding = Arc::new(
                RunAuthBinding::issue(
                    run_id.clone(),
                    RunKind::Analysis,
                    clock,
                    TokenTtl::try_from_seconds(60_u64).expect("TTL"),
                )
                .expect("발급"),
            );
            let events = Arc::new(ConnectionEventLog::new());
            let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
            let (client, server) = duplex(1024_usize);
            let (server_read, server_write) = tokio::io::split(server);
            let params: StreamAuthParams<SteppedClock> = StreamAuthParams {
                auth: Arc::clone(&binding),
                auth_line_limit: AuthLineLimit::default_limit(),
                auth_timeout: Duration::from_secs(5_u64),
                shutdown_rx,
                events: Arc::clone(&events),
                tools: None,
            };
            let task = tokio::spawn(authenticate_and_serve_stream(
                server_read,
                server_write,
                params,
            ));

            match shutdown_tx.send(ShutdownSignal::Stopped) {
                Ok(()) => {}
                Err(_closed) => {}
            }
            task.await.expect("종료 중 인증 취소 완료");
            let _keep_client_open = client;
            assert!(
                events.snapshot().contains(&ConnectionEvent::Rejected {
                    run_id,
                    reason: ConnectionRejection::ServerShuttingDown,
                }),
                "서버 종료 중 인증 대기는 ServerShuttingDown으로 거부"
            );
        });
    }

    #[test]
    fn duplex_stream_rejects_revocation_during_auth() {
        runtime().block_on(async {
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let run_id = AiRunId::try_new("run-duplex-3").expect("실행 ID");
            let binding = Arc::new(
                RunAuthBinding::issue(
                    run_id.clone(),
                    RunKind::Analysis,
                    clock,
                    TokenTtl::try_from_seconds(60_u64).expect("TTL"),
                )
                .expect("발급"),
            );
            let events = Arc::new(ConnectionEventLog::new());
            let (_tx, rx) = watch::channel(ShutdownSignal::Running);
            let (client, server) = duplex(1024_usize);
            let (sr, sw) = tokio::io::split(server);
            let params: StreamAuthParams<SteppedClock> = StreamAuthParams {
                auth: Arc::clone(&binding),
                auth_line_limit: AuthLineLimit::default_limit(),
                auth_timeout: Duration::from_secs(5_u64),
                shutdown_rx: rx,
                events: Arc::clone(&events),
                tools: None,
            };
            let task = tokio::spawn(authenticate_and_serve_stream(sr, sw, params));
            binding.revoke();
            task.await.expect("폐기 중 인증 취소 완료");
            let _keep_client = client;
            assert!(
                events.snapshot().contains(&ConnectionEvent::Rejected {
                    run_id,
                    reason: ConnectionRejection::Auth(AuthRejection::TokenRevoked),
                }),
                "토큰 폐기 중 인증 대기는 TokenRevoked로 거부"
            );
        });
    }
}
