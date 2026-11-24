#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs::{self, DirBuilder};
    use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, symlink};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use application::{
        AiRunId, ExecutablePath, PeerProcessInfo, PeerProcessInspector, PeerProcessLookupError,
        ProcessId, RunKind,
    };
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use mcp_server::{
        AuthLineLimit, AuthRejection, ConnectionEvent, ConnectionRejection,
        MaxConcurrentConnections, McpServerConfig, McpServerError, McpServerHandle,
        PeerInspectorMode, PeerProcessTarget, PeerVerificationFailure, RunIdentity, RunServerDeps,
        RunServerRequest, RunSocketSpec, SocketSetupError, TokenTtl,
    };
    use serde_json::Value;
    use testkit_ports::SteppedClock;
    use testkit_ports::system_fakes::ScriptedPeerProcessInspector;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    const LIST_TOOLS_2026: &str = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#;

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("테스트 런타임")
    }

    fn ignore_io_result(result: std::io::Result<()>) {
        match result {
            Ok(()) => {}
            Err(_error) => {}
        }
    }

    struct TempBaseDir {
        path: PathBuf,
    }

    impl TempBaseDir {
        fn new(tag: &str) -> Self {
            let mut raw = [0_u8; 4];
            getrandom::fill(&mut raw).expect("임시 폴더 난수");
            let hex = format!("{:08x}", u32::from_be_bytes(raw));
            let canonical_tmp = fs::canonicalize(std::env::temp_dir()).expect("임시 루트 정규화");
            let path = canonical_tmp.join(format!("m01-{tag}-{hex}"));
            DirBuilder::new()
                .mode(0o700_u32)
                .create(&path)
                .expect("테스트 기본 폴더 생성");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempBaseDir {
        fn drop(&mut self) {
            if self.path.exists() {
                ignore_io_result(fs::remove_dir_all(&self.path));
            }
        }
    }

    struct HangingPeerInspector;

    impl PeerProcessInspector for HangingPeerInspector {
        async fn inspect(&self, _: ProcessId) -> Result<PeerProcessInfo, PeerProcessLookupError> {
            tokio::time::sleep(Duration::from_secs(30_u64)).await;
            Err(PeerProcessLookupError::Unavailable)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum LockSection {
        Outside,
        Dependencies,
    }

    fn timestamp_at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("유효한 시각")
    }

    fn expected_parent_pid() -> ProcessId {
        ProcessId::try_new(7777_u32).expect("부모 PID")
    }

    fn expected_exe_path() -> ExecutablePath {
        ExecutablePath::try_new("/Applications/StockCopilot.app/Contents/MacOS/mcp-bridge")
            .expect("실행 경로")
    }

    fn matching_peer_info() -> PeerProcessInfo {
        PeerProcessInfo::new(expected_parent_pid(), expected_exe_path())
    }

    fn start_request<P>(
        run_id: &str,
        run_kind: RunKind,
        spec: RunSocketSpec,
        deps: RunServerDeps<SteppedClock, P>,
    ) -> RunServerRequest<SteppedClock, P> {
        RunServerRequest::new(
            RunIdentity::new(AiRunId::try_new(run_id).expect("run id"), run_kind),
            spec,
            PeerProcessTarget::with_parent(expected_parent_pid(), expected_exe_path()),
            deps,
        )
    }

    fn start_unconfigured(
        run_id: &str,
        spec: RunSocketSpec,
        clock: &Arc<SteppedClock>,
    ) -> Result<McpServerHandle<SteppedClock>, McpServerError> {
        let deps = RunServerDeps::new(
            Arc::clone(clock),
            PeerInspectorMode::Unconfigured::<ScriptedPeerProcessInspector>,
            McpServerConfig::default_config()?,
        );
        McpServerHandle::start(start_request(run_id, RunKind::Analysis, spec, deps))
    }

    fn start_matching_server(
        base: &TempBaseDir,
        run_id: &str,
        clock: &Arc<SteppedClock>,
        allow_count: usize,
        cfg: McpServerConfig,
    ) -> McpServerHandle<SteppedClock> {
        let replies: Vec<_> = (0_usize..allow_count)
            .map(|_idx| Ok(matching_peer_info()))
            .collect();
        let inspector = Arc::new(ScriptedPeerProcessInspector::new(replies));
        let deps = RunServerDeps::new(
            Arc::clone(clock),
            PeerInspectorMode::Configured(inspector),
            cfg,
        );
        let spec = RunSocketSpec::generate_under(base.path()).expect("소켓");
        McpServerHandle::start(start_request(run_id, RunKind::Analysis, spec, deps))
            .expect("서버 시작")
    }

    fn auth_line_for(handle: &McpServerHandle<SteppedClock>) -> Vec<u8> {
        let mut line = Vec::with_capacity(70_usize);
        line.extend_from_slice(b"AUTH ");
        line.extend_from_slice(handle.token().as_exposed_bytes());
        line.push(b'\n');
        line
    }

    fn is_rejected_with(event: &ConnectionEvent, expected: ConnectionRejection) -> bool {
        match event {
            ConnectionEvent::Rejected { reason, .. } => *reason == expected,
            ConnectionEvent::Accepted { .. }
            | ConnectionEvent::Closed { .. }
            | ConnectionEvent::AcceptFailed { .. }
            | ConnectionEvent::SessionTaskFailed { .. } => false,
        }
    }

    async fn wait_for_rejection(
        handle: &McpServerHandle<SteppedClock>,
        expected: ConnectionRejection,
    ) -> Vec<ConnectionEvent> {
        for _attempt in 0_usize..100_usize {
            let events = handle.events();
            if events.iter().any(|event| is_rejected_with(event, expected)) {
                return events;
            }
            tokio::time::sleep(Duration::from_millis(10_u64)).await;
        }
        handle.events()
    }

    async fn read_json_line(
        reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
    ) -> Option<Value> {
        let mut line = String::new();
        let count = tokio::time::timeout(Duration::from_secs(3_u64), reader.read_line(&mut line))
            .await
            .expect("응답 대기 시간 초과")
            .expect("응답 읽기");
        if count == 0_usize {
            return None;
        }
        Some(serde_json::from_str(&line).expect("JSON-RPC 응답 파싱"))
    }

    async fn write_lines(writer: &mut tokio::net::unix::OwnedWriteHalf, payload: &[u8]) {
        if !payload.is_empty() {
            match writer.write_all(payload).await {
                Ok(()) => {}
                Err(_error) => {}
            }
            match writer.flush().await {
                Ok(()) => {}
                Err(_error) => {}
            }
        }
    }

    async fn send_bytes_expect_close(socket_path: &Path, payload: &[u8]) {
        let stream = UnixStream::connect(socket_path).await.expect("소켓 연결");
        let (read_half, mut write_half) = stream.into_split();
        write_lines(&mut write_half, payload).await;
        let mut reader = BufReader::new(read_half);
        assert_eq!(
            read_json_line(&mut reader).await,
            None,
            "거부된 연결은 즉시 닫혀야 한다"
        );
    }

    #[test]
    fn socket_enforces_0700_dir_0600_file_and_owner_uid() {
        runtime().block_on(async {
            let base = TempBaseDir::new("perm");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let spec = RunSocketSpec::generate_under(base.path()).expect("소켓 경로 생성");
            assert!(
                spec.socket_path().as_os_str().as_encoded_bytes().len() <= 103_usize,
                "소켓 경로는 NUL 포함 104바이트 이내여야 한다"
            );
            let handle = start_unconfigured("run-perms", spec, &clock).expect("서버 시작");
            assert_eq!(
                handle.run_id(),
                &AiRunId::try_new("run-perms").expect("id"),
                "실행 ID 보존"
            );
            assert_eq!(handle.run_kind(), RunKind::Analysis, "실행 종류 보존");
            assert_eq!(
                handle.expires_at(),
                timestamp_at(1_700_001_800_i64),
                "기본 만료 시각 보존"
            );

            let dir_meta = fs::symlink_metadata(handle.run_dir()).expect("폴더 메타데이터");
            assert!(
                !dir_meta.file_type().is_symlink(),
                "폴더는 심볼릭 링크 금지"
            );
            assert_eq!(
                dir_meta.mode() & 0o7777_u32,
                0o700_u32,
                "실행 폴더 권한은 0700이어야 한다"
            );

            let sock_meta = fs::symlink_metadata(handle.socket_path()).expect("소켓 메타데이터");
            assert!(
                !sock_meta.file_type().is_symlink(),
                "소켓은 심볼릭 링크 금지"
            );
            assert!(
                sock_meta.file_type().is_socket(),
                "유닉스 소켓 파일이어야 한다"
            );
            assert_eq!(
                sock_meta.mode() & 0o7777_u32,
                0o600_u32,
                "소켓 권한은 0600이어야 한다"
            );
            assert_eq!(
                sock_meta.uid(),
                handle.owner_uid().as_u32(),
                "소켓 소유자는 앱 프로세스 UID와 같아야 한다"
            );
            handle.shutdown().await;
        });
    }

    #[test]
    fn collection_run_cannot_start_mcp_server_or_create_socket() {
        runtime().block_on(async {
            let base = TempBaseDir::new("coll");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let spec = RunSocketSpec::generate_under(base.path()).expect("소켓 경로");
            let deps = RunServerDeps::new(
                Arc::clone(&clock),
                PeerInspectorMode::Unconfigured::<ScriptedPeerProcessInspector>,
                McpServerConfig::default_config().expect("기본 설정"),
            );
            let result = McpServerHandle::start(start_request(
                "run-collection",
                RunKind::Collection,
                spec.clone(),
                deps,
            ));
            assert_eq!(
                result.err(),
                Some(McpServerError::Token(
                    mcp_server::TokenError::CollectionRunNotAllowed
                )),
                "수집 실행은 앱 MCP 서버를 시작할 수 없다"
            );
            assert!(
                !spec.run_dir().exists(),
                "거부 시 폴더를 만들지 않아야 한다"
            );
            assert!(
                !spec.socket_path().exists(),
                "거부 시 소켓을 만들지 않아야 한다"
            );
        });
    }

    #[test]
    fn socket_cleans_up_on_shutdown_and_drop() {
        runtime().block_on(async {
            let base = TempBaseDir::new("clean");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));

            let shutdown_spec = RunSocketSpec::generate_under(base.path()).expect("종료 경로");
            let shutdown_handle = start_unconfigured("run-shutdown", shutdown_spec, &clock)
                .expect("종료용 서버 시작");
            let run_dir = shutdown_handle.run_dir().to_path_buf();
            let sock_path = shutdown_handle.socket_path().to_path_buf();
            shutdown_handle.shutdown().await;
            assert!(!sock_path.exists(), "종료 후 소켓 파일 삭제");
            assert!(!run_dir.exists(), "종료 후 실행 폴더 삭제");

            let drop_spec = RunSocketSpec::generate_under(base.path()).expect("드롭 경로");
            let (drop_dir, drop_sock) = {
                let drop_handle =
                    start_unconfigured("run-drop", drop_spec, &clock).expect("드롭용 서버 시작");
                (
                    drop_handle.run_dir().to_path_buf(),
                    drop_handle.socket_path().to_path_buf(),
                )
            };
            assert!(!drop_sock.exists(), "Drop 시 소켓 파일 정리");
            assert!(!drop_dir.exists(), "Drop 시 실행 폴더 정리");
        });
    }

    #[test]
    fn socket_refuses_stale_leftovers_without_deleting_them() {
        runtime().block_on(async {
            let base = TempBaseDir::new("stale");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let spec = RunSocketSpec::generate_under(base.path()).expect("경로");

            DirBuilder::new()
                .mode(0o700_u32)
                .create(spec.run_dir())
                .expect("남은 폴더 시뮬레이션");
            {
                let _stale = std::os::unix::net::UnixListener::bind(spec.socket_path())
                    .expect("남은 소켓 생성");
            }
            let stale_result = start_unconfigured("run-stale", spec.clone(), &clock);
            assert_eq!(
                stale_result.err(),
                Some(McpServerError::Socket(
                    SocketSetupError::LeftoverPathRefused
                )),
                "남은 소켓·폴더 재사용은 거부해야 한다"
            );
            assert!(
                spec.socket_path().exists(),
                "재사용 거부 시 기존 소켓을 임의로 지우지 않아야 한다"
            );
        });
    }

    #[test]
    fn socket_refuses_symlink_base_and_oversized_paths() {
        runtime().block_on(async {
            let base = TempBaseDir::new("link");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let symlink_base = base.path().join("link-base");
            symlink(base.path(), &symlink_base).expect("심볼릭 링크 생성");
            let symlink_spec =
                RunSocketSpec::generate_under(&symlink_base).expect("링크 아래 경로");
            let symlink_result = start_unconfigured("run-link", symlink_spec, &clock);
            assert_eq!(
                symlink_result.err(),
                Some(McpServerError::Socket(SocketSetupError::SymlinkNotAllowed)),
                "심볼릭 링크 부모 폴더 아래 바인딩은 거부해야 한다"
            );

            let long_parent = base.path().join("x".repeat(90_usize));
            assert_eq!(
                RunSocketSpec::generate_under(&long_parent),
                Err(SocketSetupError::PathTooLong),
                "104바이트 한도 초과 경로는 닫힌 실패해야 한다"
            );
        });
    }

    fn assert_discover_response(discover_resp: &Value, raw_token: &str) {
        assert!(
            !discover_resp.to_string().contains(raw_token),
            "토큰 노출 금지"
        );
        assert_eq!(
            discover_resp.pointer("/result/_meta/io.modelcontextprotocol~1serverInfo/name"),
            Some(&Value::String(String::from("stock-copilot-mcp"))),
            "서버 이름 일치: {discover_resp}"
        );
        let versions = discover_resp
            .pointer("/result/supportedVersions")
            .and_then(Value::as_array)
            .expect("supportedVersions 배열");
        assert!(
            versions.iter().any(|ver| ver == "2026-07-28")
                && versions.iter().any(|ver| ver == "2025-06-18"),
            "2026-07-28과 2025-06-18 모두 지원해야 한다"
        );
    }

    #[test]
    fn mcp_2026_07_28_serves_discover_and_empty_tool_skeleton() {
        runtime().block_on(async {
            let base = TempBaseDir::new("p26");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let inspector = Arc::new(ScriptedPeerProcessInspector::new([Ok(
                matching_peer_info(),
            )]));
            let deps = RunServerDeps::new(
                Arc::clone(&clock),
                PeerInspectorMode::Configured(Arc::clone(&inspector)),
                McpServerConfig::default_config().expect("기본 설정"),
            );
            let handle = McpServerHandle::start(start_request(
                "run-p26",
                RunKind::Analysis,
                RunSocketSpec::generate_under(base.path()).expect("소켓"),
                deps,
            ))
            .expect("서버 시작");
            let raw_token = std::str::from_utf8(handle.token().as_exposed_bytes())
                .expect("UTF-8")
                .to_owned();

            let stream = UnixStream::connect(handle.socket_path()).await.expect("연결");
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);

            let discover = r#"{"jsonrpc":"2.0","id":1,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#;
            let list_tools = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#;
            let call_tool = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"propose_order","arguments":{},"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}"#;
            let mut payload = auth_line_for(&handle);
            payload.extend_from_slice(format!("{discover}\n").as_bytes());
            write_lines(&mut write_half, &payload).await;
            let discover_resp = read_json_line(&mut reader).await.expect("discover 응답");
            assert_discover_response(&discover_resp, &raw_token);

            write_lines(&mut write_half, format!("{list_tools}\n").as_bytes()).await;
            let tools_resp = read_json_line(&mut reader).await.expect("tools/list 응답");
            assert_eq!(
                tools_resp.pointer("/result/tools"),
                Some(&Value::Array(Vec::new())),
                "M-01 도구 목록은 비어 있어야 한다"
            );
            write_lines(&mut write_half, format!("{call_tool}\n").as_bytes()).await;
            let call_resp = read_json_line(&mut reader).await.expect("tools/call 응답");
            assert_eq!(
                call_resp.pointer("/error/code"),
                Some(&Value::from(-32601_i64)),
                "미등록 도구는 METHOD_NOT_FOUND로 거부한다"
            );
            assert_eq!(inspector.calls().len(), 1_usize, "피어 조회 포트 호출");
            handle.shutdown().await;
        });
    }

    #[test]
    fn mcp_2025_06_18_negotiates_initialize_and_serves_empty_tools() {
        runtime().block_on(async {
            let base = TempBaseDir::new("p25");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let inspector = Arc::new(ScriptedPeerProcessInspector::new([Ok(
                matching_peer_info(),
            )]));
            let deps = RunServerDeps::new(
                Arc::clone(&clock),
                PeerInspectorMode::Configured(inspector),
                McpServerConfig::default_config().expect("기본 설정"),
            );
            let handle = McpServerHandle::start(start_request(
                "run-p25",
                RunKind::Analysis,
                RunSocketSpec::generate_under(base.path()).expect("소켓"),
                deps,
            ))
            .expect("서버 시작");

            let stream = UnixStream::connect(handle.socket_path()).await.expect("연결");
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);

            let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"codex-test","version":"0.1.0"}}}"#;
            let mut payload = auth_line_for(&handle);
            payload.extend_from_slice(format!("{init}\n").as_bytes());
            write_lines(&mut write_half, &payload).await;

            let init_resp = read_json_line(&mut reader).await.expect("initialize 응답");
            assert_eq!(
                init_resp.pointer("/result/protocolVersion"),
                Some(&Value::String(String::from("2025-06-18"))),
                "2025-06-18 협상 성공"
            );

            let initialized = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
            let list_tools = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
            write_lines(
                &mut write_half,
                format!("{initialized}\n{list_tools}\n").as_bytes(),
            )
            .await;

            let tools_resp = read_json_line(&mut reader).await.expect("tools/list 응답");
            assert_eq!(
                tools_resp.pointer("/result/tools"),
                Some(&Value::Array(Vec::new())),
                "레거시 세대에서도 도구 목록은 비어 있어야 한다"
            );
            handle.shutdown().await;
        });
    }

    #[test]
    fn auth_rejects_missing_other_run_oversized_and_timed_out_tokens() {
        runtime().block_on(async {
            let base = TempBaseDir::new("auth1");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let cfg = McpServerConfig::try_new(
                AuthLineLimit::try_new(80_usize).expect("80바이트 한도"),
                Duration::from_millis(120_u64),
                Duration::from_millis(150_u64),
                MaxConcurrentConnections::default_limit(),
                TokenTtl::try_from_seconds(30_u64).expect("30초 TTL"),
            )
            .expect("테스트 설정");
            let server_a = start_matching_server(&base, "run-a", &clock, 5_usize, cfg);
            let server_b = start_unconfigured(
                "run-b",
                RunSocketSpec::generate_under(base.path()).expect("소켓 B"),
                &clock,
            )
            .expect("서버 B");

            send_bytes_expect_close(server_a.socket_path(), b"AUTH \n").await;
            send_bytes_expect_close(server_a.socket_path(), b"BAD prefix\n").await;
            send_bytes_expect_close(server_a.socket_path(), &auth_line_for(&server_b)).await;
            let oversized = format!("AUTH {}\n", "a".repeat(120_usize));
            send_bytes_expect_close(server_a.socket_path(), oversized.as_bytes()).await;
            send_bytes_expect_close(server_a.socket_path(), b"").await;

            let events = wait_for_rejection(&server_a, ConnectionRejection::AuthTimeout).await;
            for expected in [
                ConnectionRejection::Auth(AuthRejection::MissingToken),
                ConnectionRejection::Auth(AuthRejection::InvalidAuthFormat),
                ConnectionRejection::Auth(AuthRejection::InvalidToken),
                ConnectionRejection::AuthLineTooLong,
                ConnectionRejection::AuthTimeout,
            ] {
                assert!(
                    events.iter().any(|event| is_rejected_with(event, expected)),
                    "거부 사유 기록 누락: {expected:?}"
                );
            }
            let raw_a = std::str::from_utf8(server_a.token().as_exposed_bytes()).expect("UTF-8");
            assert!(
                !format!("{events:?}").contains(raw_a) && !format!("{server_a:?}").contains(raw_a),
                "이벤트·핸들 Debug에 원문 토큰 노출 금지"
            );
            server_a.shutdown().await;
            server_b.shutdown().await;
        });
    }

    #[test]
    fn auth_rejects_expired_token_on_open_and_new_connections() {
        runtime().block_on(async {
            let base = TempBaseDir::new("exp");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let cfg = McpServerConfig::try_new(
                AuthLineLimit::default_limit(),
                Duration::from_secs(1_u64),
                Duration::from_secs(1_u64),
                MaxConcurrentConnections::default_limit(),
                TokenTtl::try_from_seconds(30_u64).expect("30초 TTL"),
            )
            .expect("설정");
            let server = start_matching_server(&base, "run-exp", &clock, 2_usize, cfg);

            let stream = UnixStream::connect(server.socket_path())
                .await
                .expect("연결");
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            let mut initial = auth_line_for(&server);
            initial.extend_from_slice(format!("{LIST_TOOLS_2026}\n").as_bytes());
            write_lines(&mut write_half, &initial).await;
            assert!(
                read_json_line(&mut reader)
                    .await
                    .is_some_and(|val| val.pointer("/result/tools").is_some()),
                "만료 전 응답 성공"
            );

            clock
                .advance_to(timestamp_at(1_700_000_030_i64))
                .expect("시계 전진");
            write_lines(&mut write_half, format!("{LIST_TOOLS_2026}\n").as_bytes()).await;
            if let Some(resp) = read_json_line(&mut reader).await {
                assert!(
                    resp.get("error").is_some(),
                    "만료 후 열린 연결 요청은 오류로 거부되어야 한다"
                );
            }
            send_bytes_expect_close(server.socket_path(), &auth_line_for(&server)).await;
            let events = wait_for_rejection(
                &server,
                ConnectionRejection::Auth(AuthRejection::TokenExpired),
            )
            .await;
            assert!(
                events.iter().any(|event| {
                    is_rejected_with(
                        event,
                        ConnectionRejection::Auth(AuthRejection::TokenExpired),
                    )
                }),
                "만료 거부 기록"
            );
            server.shutdown().await;
        });
    }

    #[test]
    fn auth_rejects_revoked_token_and_closes_open_connection() {
        runtime().block_on(async {
            let base = TempBaseDir::new("rev");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let inspector = Arc::new(ScriptedPeerProcessInspector::new([
                Ok(matching_peer_info()),
                Ok(matching_peer_info()),
            ]));
            let server = McpServerHandle::start(start_request(
                "run-rev",
                RunKind::Analysis,
                RunSocketSpec::generate_under(base.path()).expect("소켓"),
                RunServerDeps::new(
                    Arc::clone(&clock),
                    PeerInspectorMode::Configured(inspector),
                    McpServerConfig::default_config().expect("기본 설정"),
                ),
            ))
            .expect("폐기 서버");

            let stream = UnixStream::connect(server.socket_path())
                .await
                .expect("연결");
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            let mut initial = auth_line_for(&server);
            initial.extend_from_slice(format!("{LIST_TOOLS_2026}\n").as_bytes());
            write_lines(&mut write_half, &initial).await;
            assert!(read_json_line(&mut reader).await.is_some(), "폐기 전 응답");

            server.revoke_token();
            assert_eq!(
                read_json_line(&mut reader).await,
                None,
                "폐기 즉시 열린 연결이 종료되어야 한다"
            );
            send_bytes_expect_close(server.socket_path(), &auth_line_for(&server)).await;
            let events = wait_for_rejection(
                &server,
                ConnectionRejection::Auth(AuthRejection::TokenRevoked),
            )
            .await;
            assert!(
                events.iter().any(|event| {
                    is_rejected_with(
                        event,
                        ConnectionRejection::Auth(AuthRejection::TokenRevoked),
                    )
                }),
                "폐기 거부 기록"
            );
            server.shutdown().await;
        });
    }

    #[test]
    fn peer_unconfigured_inspector_fails_closed() {
        runtime().block_on(async {
            let base = TempBaseDir::new("unconf");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let server = start_unconfigured(
                "run-unconf",
                RunSocketSpec::generate_under(base.path()).expect("소켓"),
                &clock,
            )
            .expect("미제공 서버");
            send_bytes_expect_close(server.socket_path(), &auth_line_for(&server)).await;
            let events = wait_for_rejection(
                &server,
                ConnectionRejection::Peer(PeerVerificationFailure::InspectorMissing),
            )
            .await;
            assert!(
                events.iter().any(|event| {
                    is_rejected_with(
                        event,
                        ConnectionRejection::Peer(PeerVerificationFailure::InspectorMissing),
                    )
                }),
                "InspectorMissing 기록"
            );
            server.shutdown().await;
        });
    }

    #[test]
    fn peer_lookup_error_and_mismatches_fail_closed() {
        runtime().block_on(async {
            let base = TempBaseDir::new("peermis");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let inspector = Arc::new(ScriptedPeerProcessInspector::new([
                Err(PeerProcessLookupError::Unavailable),
                Ok(PeerProcessInfo::new(
                    ProcessId::try_new(9999_u32).expect("다른 부모"),
                    expected_exe_path(),
                )),
                Ok(PeerProcessInfo::new(
                    expected_parent_pid(),
                    ExecutablePath::try_new("/tmp/other-binary").expect("다른 경로"),
                )),
            ]));
            let server = McpServerHandle::start(start_request(
                "run-peer",
                RunKind::Analysis,
                RunSocketSpec::generate_under(base.path()).expect("소켓"),
                RunServerDeps::new(
                    Arc::clone(&clock),
                    PeerInspectorMode::Configured(inspector),
                    McpServerConfig::default_config().expect("기본 설정"),
                ),
            ))
            .expect("피어 서버");

            for _round in 0_usize..3_usize {
                send_bytes_expect_close(server.socket_path(), &auth_line_for(&server)).await;
            }
            let events = wait_for_rejection(
                &server,
                ConnectionRejection::Peer(PeerVerificationFailure::ExecutablePathMismatch),
            )
            .await;
            for expected in [
                ConnectionRejection::Peer(PeerVerificationFailure::InspectionFailed(
                    PeerProcessLookupError::Unavailable,
                )),
                ConnectionRejection::Peer(PeerVerificationFailure::ParentPidMismatch),
                ConnectionRejection::Peer(PeerVerificationFailure::ExecutablePathMismatch),
            ] {
                assert!(
                    events.iter().any(|event| is_rejected_with(event, expected)),
                    "피어 거부 사유 기록 누락: {expected:?}"
                );
            }
            server.shutdown().await;
        });
    }

    #[test]
    fn peer_timeout_and_concurrency_cap_fail_closed() {
        runtime().block_on(async {
            let base = TempBaseDir::new("hang");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let cfg = McpServerConfig::try_new(
                AuthLineLimit::default_limit(),
                Duration::from_secs(1_u64),
                Duration::from_millis(80_u64),
                MaxConcurrentConnections::try_new(1_usize).expect("동시 1연결"),
                TokenTtl::default_ttl().expect("기본 TTL"),
            )
            .expect("타임아웃 설정");
            let server = McpServerHandle::start(start_request(
                "run-hang",
                RunKind::Analysis,
                RunSocketSpec::generate_under(base.path()).expect("소켓"),
                RunServerDeps::new(
                    Arc::clone(&clock),
                    PeerInspectorMode::Configured(Arc::new(HangingPeerInspector)),
                    cfg,
                ),
            ))
            .expect("서버 시작");

            let first = UnixStream::connect(server.socket_path())
                .await
                .expect("첫 연결");
            tokio::time::sleep(Duration::from_millis(20_u64)).await;
            send_bytes_expect_close(server.socket_path(), b"").await;

            let (read_first, _write_first) = first.into_split();
            let mut reader_first = BufReader::new(read_first);
            assert_eq!(
                read_json_line(&mut reader_first).await,
                None,
                "조회 시간 초과 연결은 닫힌 실패한다"
            );

            let events = wait_for_rejection(
                &server,
                ConnectionRejection::Peer(PeerVerificationFailure::InspectionTimedOut),
            )
            .await;
            assert!(
                events.iter().any(|event| {
                    is_rejected_with(event, ConnectionRejection::ConnectionLimitReached)
                }),
                "동시 연결 상한 초과 기록"
            );
            assert!(
                events.iter().any(|event| {
                    is_rejected_with(
                        event,
                        ConnectionRejection::Peer(PeerVerificationFailure::InspectionTimedOut),
                    )
                }),
                "조회 제한 시간 초과 기록"
            );
            server.shutdown().await;
        });
    }

    fn parse_lock_dep_name(line: &str) -> Option<&str> {
        let quoted = line.strip_prefix('"')?;
        let inner = quoted.split('"').next()?;
        inner.split_whitespace().next()
    }

    fn parse_cargo_lock_graph(lock_text: &str) -> BTreeMap<String, Vec<String>> {
        let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut current_name: Option<String> = None;
        let mut section = LockSection::Outside;
        for raw_line in lock_text.lines() {
            let line = raw_line.trim();
            if line == "[[package]]" {
                current_name = None;
                section = LockSection::Outside;
            } else if let Some(rest) = line.strip_prefix("name = \"")
                && let Some(name) = rest.strip_suffix('"')
            {
                current_name = Some(String::from(name));
                graph.entry(String::from(name)).or_default();
            } else if line == "dependencies = [" {
                section = LockSection::Dependencies;
            } else if section == LockSection::Dependencies && line == "]" {
                section = LockSection::Outside;
            } else if section == LockSection::Dependencies
                && let Some(dep_name) = parse_lock_dep_name(line)
                && let Some(owner) = &current_name
            {
                graph
                    .entry(owner.clone())
                    .or_default()
                    .push(String::from(dep_name));
            }
        }
        graph
    }

    #[test]
    fn dependency_graph_has_zero_tcp_http_or_paste_crates() {
        let root = PathBuf::from("../..");
        let lock_text = fs::read_to_string(root.join("Cargo.lock")).expect("Cargo.lock 읽기");
        let graph = parse_cargo_lock_graph(&lock_text);
        assert!(
            graph.contains_key("mcp-server"),
            "Cargo.lock에 mcp-server 패키지가 있어야 한다"
        );

        let mut visited = BTreeSet::new();
        let mut stack = vec![String::from("mcp-server")];
        while let Some(package) = stack.pop() {
            if visited.insert(package.clone())
                && let Some(deps) = graph.get(&package)
            {
                stack.extend(deps.iter().cloned());
            }
        }

        for forbidden in [
            "axum",
            "hyper",
            "hyper-util",
            "hyper-rustls",
            "http",
            "http-body",
            "http-body-util",
            "reqwest",
            "tower",
            "tower-http",
            "tower-service",
            "sse-stream",
            "tonic",
            "warp",
            "actix-web",
            "rocket",
            "salvo",
            "poem",
            "paste",
        ] {
            assert!(
                !visited.contains(forbidden),
                "mcp-server 의존 그래프에 금지 TCP/HTTP/paste 크레이트가 들어왔다: {forbidden}"
            );
        }
    }

    fn assert_source_has_no_forbidden_tokens(path: &Path) {
        let source = fs::read_to_string(path).expect("소스 읽기");
        for forbidden_token in [
            "TcpListener",
            "TcpStream",
            "TcpSocket",
            "UdpSocket",
            "chrono",
            "pastey",
            "paste!",
            "unsafe",
        ] {
            assert!(
                !source.contains(forbidden_token),
                "{}에 금지 식별자가 포함되면 안 된다: {forbidden_token}",
                path.display()
            );
        }
    }

    fn collect_rust_sources_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("소스 디렉터리 읽기") {
            let path = entry.expect("엔트리").path();
            if path.is_dir() {
                collect_rust_sources_recursive(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    #[test]
    fn manifest_and_sources_forbid_tcp_chrono_pastey_and_unsafe() {
        let root = PathBuf::from("../..");
        let manifest =
            fs::read_to_string(root.join("crates/mcp-server/Cargo.toml")).expect("Cargo.toml 읽기");
        assert!(
            manifest.contains("rmcp = { version = \"=3.9.0\", default-features = false, features = [\"server\", \"transport-async-rw\"] }"),
            "rmcp는 정확히 =3.9.0 최소 기능(server, transport-async-rw)만 켜야 한다"
        );
        for forbidden_manifest in ["chrono", "pastey", "transport-streamable-http", "reqwest"] {
            assert!(
                !manifest.contains(forbidden_manifest),
                "mcp-server Cargo.toml에 금지 항목이 포함되면 안 된다: {forbidden_manifest}"
            );
        }

        let src_dir = root.join("crates/mcp-server/src");
        let mut files = Vec::new();
        collect_rust_sources_recursive(&src_dir, &mut files);
        assert!(
            !files.is_empty(),
            "mcp-server/src 아래에 검사 대상 Rust 파일이 있어야 한다"
        );
        for path in files {
            assert_source_has_no_forbidden_tokens(&path);
        }
    }

    async fn assert_handle_debug_redacted(server: &McpServerHandle<SteppedClock>) {
        let handle_debug = format!("{server:?}");
        let raw_token = std::str::from_utf8(server.token().as_exposed_bytes()).expect("UTF-8");
        assert!(
            handle_debug.contains("McpServerHandle")
                && handle_debug.contains("RunAuthToken(<redacted>)")
                && !handle_debug.contains(raw_token),
            "서버 핸들 Debug는 구조와 가림 표식을 출력하고 원문을 노출하지 않는다"
        );
        send_bytes_expect_close(server.socket_path(), &auth_line_for(server)).await;
        let pre_events = wait_for_rejection(
            server,
            ConnectionRejection::Peer(PeerVerificationFailure::ParentPidUnbound),
        )
        .await;
        assert!(
            pre_events.iter().any(|event| is_rejected_with(
                event,
                ConnectionRejection::Peer(PeerVerificationFailure::ParentPidUnbound)
            )),
            "부모 PID 결합 전에는 닫힌 실패로 거부한다"
        );
    }

    async fn assert_bound_bridge_and_mismatch(server: &McpServerHandle<SteppedClock>) {
        let list_resp = {
            let stream = UnixStream::connect(server.socket_path())
                .await
                .expect("결합 후 연결");
            let (read_half, mut write_half) = stream.into_split();
            let mut reader = BufReader::new(read_half);
            let mut payload = auth_line_for(server);
            payload.extend_from_slice(format!("{LIST_TOOLS_2026}\n").as_bytes());
            write_lines(&mut write_half, &payload).await;
            read_json_line(&mut reader)
                .await
                .expect("결합 후 정상 도구 목록 응답")
        };
        assert_eq!(
            list_resp.pointer("/result/tools"),
            Some(&serde_json::json!([])),
            "결합된 부모 PID 브리지는 정상 응답한다"
        );
        send_bytes_expect_close(server.socket_path(), &auth_line_for(server)).await;
        let mismatch_events = wait_for_rejection(
            server,
            ConnectionRejection::Peer(PeerVerificationFailure::ParentPidMismatch),
        )
        .await;
        assert!(
            mismatch_events.iter().any(|event| is_rejected_with(
                event,
                ConnectionRejection::Peer(PeerVerificationFailure::ParentPidMismatch)
            )),
            "다른 부모 PID는 결합 후에도 거부한다"
        );
    }

    #[test]
    fn late_binding_parent_pid_fails_closed_until_bound_once() {
        runtime().block_on(async {
            let base = TempBaseDir::new("latebind");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let cli_pid = ProcessId::try_new(8888_u32).expect("CLI PID");
            let wrong_pid = ProcessId::try_new(9999_u32).expect("다른 PID");
            let inspector = Arc::new(ScriptedPeerProcessInspector::new([
                Ok(PeerProcessInfo::new(cli_pid, expected_exe_path())),
                Ok(PeerProcessInfo::new(wrong_pid, expected_exe_path())),
            ]));
            let deps = RunServerDeps::new(
                Arc::clone(&clock),
                PeerInspectorMode::Configured(inspector),
                McpServerConfig::default_config().expect("설정"),
            );
            let spec = RunSocketSpec::generate_under(base.path()).expect("소켓");
            let request = RunServerRequest::new(
                RunIdentity::new(
                    AiRunId::try_new("run-late-bind").expect("id"),
                    RunKind::Analysis,
                ),
                spec,
                PeerProcessTarget::pending_parent(expected_exe_path()),
                deps,
            );
            assert!(
                format!("{request:?}").contains("RunServerRequest"),
                "요청 Debug 출력 확인"
            );
            let server = McpServerHandle::start(request).expect("서버 시작");
            assert_handle_debug_redacted(&server).await;

            assert_eq!(
                server.bind_expected_parent_pid(cli_pid),
                Ok(()),
                "CLI 기동 직후 1회 부모 PID 결합 성공"
            );
            assert_eq!(
                server.bind_expected_parent_pid(wrong_pid),
                Err(McpServerError::ParentPidAlreadyBound),
                "이미 결합된 부모 PID 재설정은 거부한다"
            );
            assert_bound_bridge_and_mismatch(&server).await;

            let final_events = tokio::time::timeout(Duration::from_secs(2_u64), server.shutdown())
                .await
                .expect("종료는 2초 안에 끝나야 한다");
            assert!(
                final_events.iter().any(|event| match event {
                    ConnectionEvent::Closed { .. } => true,
                    ConnectionEvent::Accepted { .. }
                    | ConnectionEvent::Rejected { .. }
                    | ConnectionEvent::AcceptFailed { .. }
                    | ConnectionEvent::SessionTaskFailed { .. } => false,
                }),
                "정상 종료 시 Closed 이벤트가 기록된다"
            );
        });
    }

    #[test]
    fn expired_token_rejects_post_discover_ping() {
        runtime().block_on(async {
            let base = TempBaseDir::new("expping");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let cfg = McpServerConfig::try_new(
                AuthLineLimit::default_limit(),
                Duration::from_secs(2_u64),
                Duration::from_secs(1_u64),
                MaxConcurrentConnections::try_new(1_usize).expect("동시 1"),
                TokenTtl::try_from_seconds(30_u64).expect("30초"),
            )
            .expect("설정");
            let server = start_matching_server(&base, "run-exp-ping", &clock, 4_usize, cfg);

            let ping_after_expiry = {
                let stream = UnixStream::connect(server.socket_path())
                    .await
                    .expect("연결 1");
                let (read_half, mut write_half) = stream.into_split();
                let mut reader = BufReader::new(read_half);
                let mut first_msg = auth_line_for(&server);
                first_msg.extend_from_slice(
                    b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"server/discover\",\"params\":{\"_meta\":{\"io.modelcontextprotocol/protocolVersion\":\"2026-07-28\",\"io.modelcontextprotocol/clientCapabilities\":{}}}}\n",
                );
                write_lines(&mut write_half, &first_msg).await;
                let discover = read_json_line(&mut reader)
                    .await
                    .expect("만료 전 discover 응답");
                assert!(
                    discover.pointer("/result/supportedVersions").is_some(),
                    "만료 전 discover 정상"
                );

                clock
                    .advance_to(timestamp_at(1_700_000_030_i64))
                    .expect("시각 이동");
                write_lines(
                    &mut write_half,
                    b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n",
                )
                .await;
                read_json_line(&mut reader).await
            };
            assert_eq!(
                ping_after_expiry, None,
                "만료 뒤 discover 이후 ping은 성공 응답 없이 즉시 닫혀야 한다"
            );

            let _events = tokio::time::timeout(Duration::from_secs(2_u64), server.shutdown())
                .await
                .expect("서버 종료");
        });
    }

    #[test]
    fn expired_idle_session_closes_and_releases_concurrency_slot() {
        runtime().block_on(async {
            let base = TempBaseDir::new("expidle");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
            let cfg = McpServerConfig::try_new(
                AuthLineLimit::default_limit(),
                Duration::from_secs(2_u64),
                Duration::from_secs(1_u64),
                MaxConcurrentConnections::try_new(1_usize).expect("동시 1"),
                TokenTtl::try_from_seconds(30_u64).expect("30초"),
            )
            .expect("설정");
            let server = start_matching_server(&base, "run-exp-idle", &clock, 4_usize, cfg);
            let idle_after_expiry = {
                let idle_stream = UnixStream::connect(server.socket_path())
                    .await
                    .expect("유휴 연결");
                let (idle_read, mut idle_write) = idle_stream.into_split();
                let mut idle_reader = BufReader::new(idle_read);
                write_lines(&mut idle_write, &auth_line_for(&server)).await;
                tokio::time::sleep(Duration::from_millis(40_u64)).await;

                clock
                    .advance_to(timestamp_at(1_700_000_030_i64))
                    .expect("시각 이동");
                read_json_line(&mut idle_reader).await
            };
            assert_eq!(
                idle_after_expiry, None,
                "아무 요청 없는 유휴 세션도 만료 뒤 자동으로 닫혀야 한다"
            );

            send_bytes_expect_close(server.socket_path(), &auth_line_for(&server)).await;
            let events = wait_for_rejection(
                &server,
                ConnectionRejection::Auth(AuthRejection::TokenExpired),
            )
            .await;
            assert!(
                events.iter().any(|event| is_rejected_with(
                    event,
                    ConnectionRejection::Auth(AuthRejection::TokenExpired)
                )),
                "유휴 세션 종료 후 슬롯 반환으로 TokenExpired가 기록된다"
            );
            let _events = tokio::time::timeout(Duration::from_secs(2_u64), server.shutdown())
                .await
                .expect("서버 종료");
        });
    }

    #[test]
    fn socket_rejects_ancestor_symlink_and_unsticky_world_writable_base_dir() {
        runtime().block_on(async {
            let base = TempBaseDir::new("anc");
            let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));

            let real_dir = base.path().join("real");
            let nested = real_dir.join("child");
            DirBuilder::new()
                .mode(0o700_u32)
                .create(&real_dir)
                .expect("실제 폴더");
            DirBuilder::new()
                .mode(0o700_u32)
                .create(&nested)
                .expect("하위 폴더");
            let ancestor_link = base.path().join("link");
            symlink(&real_dir, &ancestor_link).expect("상위 심볼릭 링크");
            let via_ancestor_link =
                RunSocketSpec::explicit(ancestor_link.join("child").join("r1"), "m.sock")
                    .expect("경로 스펙");
            assert_eq!(
                start_unconfigured("run-anc-link", via_ancestor_link, &clock).err(),
                Some(McpServerError::Socket(SocketSetupError::SymlinkNotAllowed)),
                "상위 경로 심볼릭 링크는 거부한다"
            );

            let world_dir = base.path().join("open777");
            DirBuilder::new()
                .mode(0o700_u32)
                .create(&world_dir)
                .expect("임시 폴더");
            fs::set_permissions(
                &world_dir,
                std::os::unix::fs::PermissionsExt::from_mode(0o777_u32),
            )
            .expect("0777 설정");
            let open_spec = RunSocketSpec::explicit(world_dir.join("r1"), "m.sock").expect("스펙");
            assert_eq!(
                start_unconfigured("run-open-base", open_spec, &clock).err(),
                Some(McpServerError::Socket(
                    SocketSetupError::BaseDirectoryPermissionsInvalid
                )),
                "sticky 없는 0777 기반 폴더는 거부한다"
            );
            assert!(
                !world_dir.join("r1").exists(),
                "기반 폴더 권한 거부 시 하위 실행 폴더를 만들지 않는다"
            );
        });
    }

    #[test]
    fn start_outside_tokio_runtime_returns_error_without_creating_directory() {
        let base = TempBaseDir::new("nort");
        let clock = Arc::new(SteppedClock::new(timestamp_at(1_700_000_000_i64)));
        let spec = RunSocketSpec::generate_under(base.path()).expect("소켓 스펙");
        let run_dir = spec.run_dir().to_path_buf();
        assert_eq!(
            start_unconfigured("run-no-rt", spec, &clock).err(),
            Some(McpServerError::RuntimeUnavailable),
            "Tokio 런타임 밖 호출은 패닉 대신 RuntimeUnavailable 오류를 반환한다"
        );
        assert!(
            !run_dir.exists(),
            "런타임 부재 오류 시 실행 폴더를 만들지 않는다"
        );
    }
}
