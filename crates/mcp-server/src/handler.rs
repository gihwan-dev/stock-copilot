use std::borrow::Cow;
use std::fmt;
use std::future::{Future, poll_fn, ready};
use std::pin::pin;
use std::sync::{Arc, RwLock};
use std::task::Poll;
use std::time::Duration;

use application::{AiRunId, Clock, RunKind};
use rmcp::ErrorData as McpError;
use rmcp::ServerHandler;
use rmcp::model::{
    CallToolRequestMethod, CallToolRequestParams, CallToolResponse, CompleteRequestParams,
    CompleteResult, DiscoverResult, Implementation, InitializeRequestParams, InitializeResult,
    ListPromptsResult, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ServerCapabilities, ServerConfig,
};
use rmcp::service::{RequestContext, RoleServer, RunningService, ServiceExt};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::sync::{Notify, watch};
use zeroize::Zeroizing;

use crate::peer::PeerVerificationFailure;
use crate::token::{AuthLineLimit, AuthRejection, AuthenticatedRun, RunAuthBinding, TokenStatus};

const SERVER_NAME: &str = "stock-copilot-mcp";
const SERVER_VERSION: &str = "0.1.0";

const SUPPORTED_VERSIONS: [ProtocolVersion; 3] = [
    ProtocolVersion::V_2026_07_28,
    ProtocolVersion::V_2025_11_25,
    ProtocolVersion::V_2025_06_18,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    Running,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionRejection {
    ConnectionLimitReached,
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
}

#[derive(Debug, Default)]
pub struct ConnectionEventLog {
    events: RwLock<Vec<ConnectionEvent>>,
}

impl ConnectionEventLog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, event: ConnectionEvent) {
        let mut guard = match self.events.write() {
            Ok(locked) => locked,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.push(event);
    }

    #[must_use]
    pub fn snapshot(&self) -> Vec<ConnectionEvent> {
        let guard = match self.events.read() {
            Ok(locked) => locked,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.clone()
    }
}

pub struct McpSkeletonHandler<C: Clock + 'static> {
    auth: Arc<RunAuthBinding<C>>,
    cancel_session: Arc<Notify>,
}

impl<C: Clock + 'static> fmt::Debug for McpSkeletonHandler<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpSkeletonHandler")
            .field("run_id", self.auth.run_id())
            .field("run_kind", &self.auth.run_kind())
            .finish_non_exhaustive()
    }
}

impl<C: Clock + 'static> McpSkeletonHandler<C> {
    #[must_use]
    pub fn new(auth: Arc<RunAuthBinding<C>>, cancel_session: Arc<Notify>) -> Self {
        Self {
            auth,
            cancel_session,
        }
    }

    fn ensure_active(&self) -> Result<(), McpError> {
        match self.auth.ensure_active() {
            Ok(()) => Ok(()),
            Err(AuthRejection::TokenRevoked | AuthRejection::TokenExpired) => {
                self.cancel_session.notify_waiters();
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
                self.cancel_session.notify_waiters();
                Err(McpError::invalid_request(
                    "run session authorization rejected",
                    None,
                ))
            }
        }
    }
}

impl<C: Clock + 'static> ServerHandler for McpSkeletonHandler<C> {
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
            self.negotiate_initialize(&request)
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
        ready(self.ensure_active().map(|()| ListToolsResult::default()))
    }

    fn call_tool(
        &self,
        _: CallToolRequestParams,
        _: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CallToolResponse, McpError>> + Send + '_ {
        let result = self
            .ensure_active()
            .and_then(|()| Err(McpError::method_not_found::<CallToolRequestMethod>()));
        ready(result)
    }
}

pub struct StreamAuthParams<C: Clock + 'static> {
    auth: Arc<RunAuthBinding<C>>,
    auth_line_limit: AuthLineLimit,
    auth_timeout: Duration,
    shutdown_rx: watch::Receiver<ShutdownSignal>,
    events: Arc<ConnectionEventLog>,
}

impl<C: Clock + 'static> fmt::Debug for StreamAuthParams<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamAuthParams")
            .field("auth", &self.auth)
            .field("auth_line_limit", &self.auth_line_limit)
            .field("auth_timeout", &self.auth_timeout)
            .finish_non_exhaustive()
    }
}

impl<C: Clock + 'static> StreamAuthParams<C> {
    #[must_use]
    pub fn new(
        auth: Arc<RunAuthBinding<C>>,
        auth_line_limit: AuthLineLimit,
        auth_timeout: Duration,
        shutdown_rx: watch::Receiver<ShutdownSignal>,
        events: Arc<ConnectionEventLog>,
    ) -> Self {
        Self {
            auth,
            auth_line_limit,
            auth_timeout,
            shutdown_rx,
            events,
        }
    }
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

async fn wait_for_session_stop(
    shutdown_rx: &mut watch::Receiver<ShutdownSignal>,
    revoke_rx: &mut watch::Receiver<TokenStatus>,
    cancel_session: &Notify,
) {
    let stop_or_revoke = race(
        wait_for_shutdown(shutdown_rx),
        wait_for_revocation(revoke_rx),
    );
    match race(stop_or_revoke, cancel_session.notified()).await {
        RaceOutcome::First(RaceOutcome::First(()) | RaceOutcome::Second(()))
        | RaceOutcome::Second(()) => {}
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

async fn read_and_verify_auth<R, C>(
    reader: &mut R,
    params: &mut StreamAuthParams<C>,
    revoke_rx: &mut watch::Receiver<TokenStatus>,
) -> Result<AuthenticatedRun, ConnectionRejection>
where
    R: AsyncRead + Unpin,
    C: Clock + 'static,
{
    let stop = race(
        wait_for_shutdown(&mut params.shutdown_rx),
        wait_for_revocation(revoke_rx),
    );
    let read_auth = read_bounded_auth_line(reader, params.auth_line_limit, params.auth_timeout);
    let line = match race(stop, read_auth).await {
        RaceOutcome::First(_stopped) => {
            return Err(ConnectionRejection::Auth(AuthRejection::TokenRevoked));
        }
        RaceOutcome::Second(result) => result?,
    };
    params
        .auth
        .verify_auth_line(line.as_slice(), params.auth.run_id())
        .map_err(ConnectionRejection::Auth)
}

async fn await_service_handle(
    handle: tokio::task::JoinHandle<Result<rmcp::service::QuitReason, tokio::task::JoinError>>,
) {
    match handle.await {
        Ok(Ok(_quit)) => {}
        Ok(Err(_join)) => {}
        Err(_abort) => {}
    }
}

async fn start_and_run_mcp_service<R, W, C>(
    reader: R,
    writer: W,
    params: &mut StreamAuthParams<C>,
    revoke_rx: &mut watch::Receiver<TokenStatus>,
    run_id: &AiRunId,
) where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
    C: Clock + 'static,
{
    let cancel_session = Arc::new(Notify::new());
    let handler = McpSkeletonHandler::new(Arc::clone(&params.auth), Arc::clone(&cancel_session));

    let running: RunningService<RoleServer, McpSkeletonHandler<C>> = match race(
        wait_for_session_stop(&mut params.shutdown_rx, revoke_rx, &cancel_session),
        handler.serve((reader, writer)),
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

    let cancellation = running.cancellation_token();
    let mut wait_handle = tokio::spawn(async move { running.waiting().await });
    match race(
        wait_for_session_stop(&mut params.shutdown_rx, revoke_rx, &cancel_session),
        &mut wait_handle,
    )
    .await
    {
        RaceOutcome::First(()) => {
            cancellation.cancel();
            await_service_handle(wait_handle).await;
        }
        RaceOutcome::Second(_completed) => {}
    }
    params.events.push(ConnectionEvent::Closed {
        run_id: run_id.clone(),
    });
}

pub async fn authenticate_and_serve_stream<R, W, C>(
    mut reader: R,
    writer: W,
    mut params: StreamAuthParams<C>,
) where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
    C: Clock + 'static,
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
    use tokio::sync::{Notify, watch};

    use crate::token::{AuthLineLimit, AuthRejection, RunAuthBinding, TokenTtl};

    use super::{
        ConnectionEvent, ConnectionEventLog, ConnectionRejection, McpSkeletonHandler,
        ShutdownSignal, StreamAuthParams, authenticate_and_serve_stream,
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

    async fn read_rpc(reader: &mut BufReader<tokio::io::ReadHalf<DuplexStream>>) -> Option<Value> {
        let mut line = String::new();
        let count = tokio::time::timeout(Duration::from_secs(2_u64), reader.read_line(&mut line))
            .await
            .expect("읽기 제한 시간")
            .expect("줄 읽기");
        if count == 0_usize {
            return None;
        }
        Some(serde_json::from_str(&line).expect("JSON 파싱"))
    }

    #[test]
    fn handler_info_declares_server_identity_and_supported_protocol_versions() {
        let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
        let auth = Arc::new(
            RunAuthBinding::issue(
                AiRunId::try_new("run-info").expect("id"),
                RunKind::Analysis,
                clock,
                TokenTtl::default_ttl().expect("ttl"),
            )
            .expect("발급"),
        );
        let handler = McpSkeletonHandler::new(Arc::clone(&auth), Arc::new(Notify::new()));
        let info = handler.get_info();
        assert_eq!(info.server_info.name, "stock-copilot-mcp", "서버 이름");
        assert_eq!(info.server_info.version, "0.1.0", "서버 버전");
        assert_eq!(
            info.protocol_version,
            ProtocolVersion::V_2026_07_28,
            "기본 규격 버전"
        );
        let versions = handler.supported_protocol_versions();
        assert!(
            versions.contains(&ProtocolVersion::V_2026_07_28)
                && versions.contains(&ProtocolVersion::V_2025_06_18),
            "두 프로토콜 세대 모두 지원"
        );
        let raw = std::str::from_utf8(auth.token().as_exposed_bytes()).expect("UTF-8");
        assert!(
            !format!("{handler:?}").contains(raw),
            "핸들러 Debug에 원문 토큰 노출 금지"
        );
    }

    #[test]
    fn duplex_stream_serves_all_skeleton_methods_and_aborts_on_shutdown_during_auth() {
        runtime().block_on(async {
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let auth = Arc::new(
                RunAuthBinding::issue(
                    AiRunId::try_new("run-duplex").expect("id"),
                    RunKind::Analysis,
                    clock,
                    TokenTtl::default_ttl().expect("ttl"),
                )
                .expect("발급"),
            );
            let events = Arc::new(ConnectionEventLog::new());
            let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
            let (client_io, server_io) = duplex(8192_usize);
            let (server_read, server_write) = tokio::io::split(server_io);
            let params = StreamAuthParams::new(
                Arc::clone(&auth),
                AuthLineLimit::default_limit(),
                Duration::from_secs(2_u64),
                shutdown_rx,
                Arc::clone(&events),
            );
            let server_task =
                tokio::spawn(authenticate_and_serve_stream(server_read, server_write, params));

            let (client_read, mut client_write) = tokio::io::split(client_io);
            let mut reader = BufReader::new(client_read);
            let raw = std::str::from_utf8(auth.token().as_exposed_bytes()).expect("UTF-8");
            client_write
                .write_all(format!("AUTH {raw}\n").as_bytes())
                .await
                .expect("인증");

            let requests = [
                r#"{"jsonrpc":"2.0","id":1,"method":"ping","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#,
                r#"{"jsonrpc":"2.0","id":2,"method":"prompts/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#,
                r#"{"jsonrpc":"2.0","id":3,"method":"resources/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#,
                r#"{"jsonrpc":"2.0","id":4,"method":"resources/templates/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#,
                r#"{"jsonrpc":"2.0","id":5,"method":"completion/complete","params":{"ref":{"type":"ref/prompt","name":"none"},"argument":{"name":"a","value":""},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#,
            ];
            for request in requests {
                client_write
                    .write_all(format!("{request}\n").as_bytes())
                    .await
                    .expect("요청 전송");
                let resp = read_rpc(&mut reader).await.expect("응답 수신");
                assert!(resp.get("result").is_some(), "정상 결과 응답: {resp}");
            }

            shutdown_tx.send(ShutdownSignal::Stopped).expect("종료 신호");
            server_task.await.expect("서버 태스크 종료");
            assert!(
                events.snapshot().iter().any(matches_closed),
                "종료 이벤트 기록"
            );
        });
    }

    fn matches_closed(event: &ConnectionEvent) -> bool {
        match event {
            ConnectionEvent::Closed { .. } => true,
            ConnectionEvent::Accepted { .. } | ConnectionEvent::Rejected { .. } => false,
        }
    }

    #[test]
    fn shutdown_signal_interrupts_pending_auth_read_immediately() {
        runtime().block_on(async {
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let auth = Arc::new(
                RunAuthBinding::issue(
                    AiRunId::try_new("run-stop-auth").expect("id"),
                    RunKind::Analysis,
                    clock,
                    TokenTtl::default_ttl().expect("ttl"),
                )
                .expect("발급"),
            );
            let events = Arc::new(ConnectionEventLog::new());
            let (shutdown_tx, shutdown_rx) = watch::channel(ShutdownSignal::Running);
            let (_client_io, server_io) = duplex(1024_usize);
            let (server_read, server_write) = tokio::io::split(server_io);
            let params = StreamAuthParams::new(
                auth,
                AuthLineLimit::default_limit(),
                Duration::from_secs(30_u64),
                shutdown_rx,
                Arc::clone(&events),
            );
            let task = tokio::spawn(authenticate_and_serve_stream(
                server_read,
                server_write,
                params,
            ));
            tokio::time::sleep(Duration::from_millis(20_u64)).await;
            shutdown_tx
                .send(ShutdownSignal::Stopped)
                .expect("종료 신호");
            tokio::time::timeout(Duration::from_secs(1_u64), task)
                .await
                .expect("종료 신호는 인증 대기를 즉시 중단해야 한다")
                .expect("태스크 종료");
            assert_eq!(
                events.snapshot(),
                vec![ConnectionEvent::Rejected {
                    run_id: AiRunId::try_new("run-stop-auth").expect("id"),
                    reason: ConnectionRejection::Auth(AuthRejection::TokenRevoked),
                }],
                "종료 중단 거부 사유 기록"
            );
        });
    }
}
