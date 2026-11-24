#[cfg(test)]
mod tests {
    use std::fs::{self, DirBuilder};
    use std::os::unix::fs::DirBuilderExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output};
    use std::sync::Arc;

    use application::{
        AiRunId, ExecutablePath, PeerProcessInfo, ProcessId, RunKind, ToolCallOutcome,
        ToolCallRecord, ToolName,
    };
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use mcp_server::{
        McpServerConfig, McpServerHandle, McpTool, PeerInspectorMode, PeerProcessTarget,
        RunIdentity, RunServerDeps, RunServerRequest, RunSocketSpec, ToolAccessKind, ToolFuture,
        ToolInvocation, ToolLedgerMode, ToolRegistry, ToolSpec, build_object_schema,
    };
    use serde_json::{Map, Value, json};
    use testkit::fake_ai_cli::{
        CliWireFormat, FakeAiMode, FakeAiScenario, InitOverride, InvocationRecord,
        McpClientScenario, McpFailureBehavior, McpProtocolGeneration, McpTransportSpec,
        PreflightBehavior, PreparedFakeAiCli, ScriptedTokenUsage, ScriptedToolCall,
    };
    use testkit_ports::SteppedClock;
    use testkit_ports::system_fakes::{MemoryToolCallLedger, ScriptedPeerProcessInspector};

    const START_SECONDS: i64 = 1_700_000_000_i64;

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

    fn expected_parent_pid() -> ProcessId {
        ProcessId::try_new(7777_u32).expect("부모 PID")
    }

    fn expected_exe_path() -> ExecutablePath {
        ExecutablePath::try_new("/Applications/StockCopilot.app/Contents/MacOS/mcp-bridge")
            .expect("실행 경로")
    }

    struct TempBaseDir {
        path: PathBuf,
    }

    impl TempBaseDir {
        fn new(tag: &str) -> Self {
            let mut raw = [0_u8; 4];
            getrandom::fill(&mut raw).expect("임시 폴더 난수");
            let hex = format!("{:08x}", u32::from_be_bytes(raw));
            let canonical_tmp = testkit::canonical_temp_dir().expect("임시 루트 정규화");
            let path = canonical_tmp.join(format!("m09-{tag}-{hex}"));
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

    fn ignore_io_result(result: std::io::Result<()>) {
        match result {
            Ok(()) => {}
            Err(_error) => {}
        }
    }

    impl Drop for TempBaseDir {
        fn drop(&mut self) {
            if self.path.exists() {
                ignore_io_result(fs::remove_dir_all(&self.path));
            }
        }
    }

    struct FixtureQuoteTool {
        spec: ToolSpec,
    }

    impl FixtureQuoteTool {
        fn new() -> Self {
            let mut props = Map::new();
            props.insert(
                String::from("symbol"),
                json!({ "type": "string", "minLength": 1_u64, "maxLength": 12_u64 }),
            );
            let spec = ToolSpec::try_new(
                ToolName::try_new("get_fixture_quote").expect("도구 이름"),
                "Contract fixture returning a fixed quote.",
                ToolAccessKind::ReadOnly,
                build_object_schema(props, &["symbol"]),
                None,
            )
            .expect("도구 사양");
            Self { spec }
        }
    }

    impl McpTool for FixtureQuoteTool {
        fn spec(&self) -> ToolSpec {
            self.spec.clone()
        }

        fn invoke<'a>(&'a self, invocation: ToolInvocation<'a>) -> ToolFuture<'a> {
            Box::pin(async move {
                let symbol = invocation
                    .arguments
                    .get("symbol")
                    .and_then(Value::as_str)
                    .map_or("", |val| val);
                Ok(json!({ "symbol": symbol, "price": "71200" }))
            })
        }
    }

    struct RunningServer {
        handle: McpServerHandle<SteppedClock>,
        ledger: Arc<MemoryToolCallLedger>,
        clock: Arc<SteppedClock>,
    }

    fn start_server(base: &TempBaseDir, run_id: &str) -> RunningServer {
        let clock = Arc::new(SteppedClock::new(timestamp_at(START_SECONDS)));
        let mut registry = ToolRegistry::new();
        registry
            .register(Arc::new(FixtureQuoteTool::new()))
            .expect("등록");
        let ledger = Arc::new(MemoryToolCallLedger::recording());
        let inspector = Arc::new(ScriptedPeerProcessInspector::new(vec![Ok(
            PeerProcessInfo::new(expected_parent_pid(), expected_exe_path()),
        )]));
        let spec = RunSocketSpec::generate_under(base.path()).expect("소켓 사양");
        let deps = RunServerDeps::new(
            Arc::clone(&clock),
            PeerInspectorMode::Configured(inspector),
            McpServerConfig::default_config().expect("기본 설정"),
        )
        .with_tools(
            Arc::new(registry),
            ToolLedgerMode::Configured(Arc::clone(&ledger)),
        );
        let request = RunServerRequest::new(
            RunIdentity::new(AiRunId::try_new(run_id).expect("run id"), RunKind::Analysis),
            spec,
            PeerProcessTarget::with_parent(expected_parent_pid(), expected_exe_path()),
            deps,
        );
        RunningServer {
            handle: McpServerHandle::start(request).expect("도구 서버 시작"),
            ledger,
            clock,
        }
    }

    fn scenario(protocol: McpProtocolGeneration, socket_path: &Path) -> FakeAiScenario {
        let wire_format = match protocol {
            McpProtocolGeneration::V2026_07_28 => CliWireFormat::ClaudeStreamJson,
            McpProtocolGeneration::V2025_06_18 => CliWireFormat::CodexExecJsonl,
        };
        FakeAiScenario {
            mode: FakeAiMode::ScriptedMcpClient(Box::new(McpClientScenario {
                protocol,
                wire_format,
                transport: McpTransportSpec::DirectUnixSocket {
                    socket_path: socket_path.to_path_buf(),
                },
                server_name: String::from("app"),
                tool_calls: vec![
                    ScriptedToolCall {
                        name: String::from("get_fixture_quote"),
                        arguments: json!({ "symbol": "005930" }),
                    },
                    ScriptedToolCall {
                        name: String::from("get_fixture_quote"),
                        arguments: json!({ "symbol": "005930", "quantity": 10_u64 }),
                    },
                ],
                final_text: String::from("done"),
                token_usage: ScriptedTokenUsage {
                    input: 1_u64,
                    output: 1_u64,
                    cache_read: 0_u64,
                    cache_creation: 0_u64,
                },
                init_override: InitOverride::default(),
                on_mcp_failure: McpFailureBehavior::ContinueWithFailedInit,
                preflight: PreflightBehavior::Immediate,
            })),
            capture_path: None,
        }
    }

    struct CliRun {
        output: Output,
        record: InvocationRecord,
    }

    async fn run_fake_cli(
        dir: &TempBaseDir,
        name: &str,
        fake: &FakeAiScenario,
        token: String,
    ) -> CliRun {
        let prepared = PreparedFakeAiCli::prepare(dir.path(), name, fake).expect("가짜 CLI 준비");
        let executable = prepared.executable_path().to_path_buf();
        let output = tokio::task::spawn_blocking(move || {
            Command::new(executable)
                .env_clear()
                .env("STOCK_MCP_TOKEN", token)
                .output()
        })
        .await
        .expect("실행 대기")
        .expect("가짜 CLI 실행");
        let record = prepared
            .read_invocations()
            .expect("호출 기록")
            .pop()
            .expect("호출 기록 1건");
        CliRun { output, record }
    }

    fn token_text(handle: &McpServerHandle<SteppedClock>) -> String {
        std::str::from_utf8(handle.token().as_exposed_bytes())
            .expect("UTF-8 토큰")
            .to_owned()
    }

    fn assert_token_absent(run: &CliRun, token: &str) {
        let stdout = String::from_utf8_lossy(&run.output.stdout);
        let stderr = String::from_utf8_lossy(&run.output.stderr);
        assert!(
            !stdout.contains(token) && !stderr.contains(token),
            "가짜 CLI 출력에 실행 토큰이 나오면 안 된다"
        );
    }

    fn versions(records: &[ToolCallRecord]) -> Vec<String> {
        records
            .iter()
            .map(|record| record.protocol_version().as_str().to_owned())
            .collect()
    }

    fn assert_generation_round_trip(protocol: McpProtocolGeneration, expected_version: &str) {
        runtime().block_on(async {
            let base = TempBaseDir::new("ok");
            let server = start_server(&base, "run-m09-e2e");
            let token = token_text(&server.handle);
            let fake = scenario(protocol, server.handle.socket_path());
            let run = run_fake_cli(&base, "fake-e2e", &fake, token.clone()).await;
            assert!(run.output.status.success(), "가짜 CLI 정상 종료");
            assert_eq!(
                run.record.mcp_tools_discovered,
                vec![String::from("get_fixture_quote")],
                "tools/list가 등록 도구만 보여 준다"
            );
            assert_eq!(run.record.mcp_calls_executed, 2_usize, "도구 호출 두 번");
            let records = server.ledger.records();
            assert_eq!(
                records
                    .iter()
                    .map(ToolCallRecord::outcome)
                    .collect::<Vec<ToolCallOutcome>>(),
                vec![ToolCallOutcome::Succeeded, ToolCallOutcome::Rejected],
                "정상 호출은 성공, 스키마 밖 필드는 거부로 원장에 남는다"
            );
            assert_eq!(
                versions(&records),
                vec![expected_version.to_owned(); 2],
                "원장에 협상한 프로토콜 세대가 남는다"
            );
            assert_token_absent(&run, &token);
            let _events = server.handle.shutdown().await;
        });
    }

    #[test]
    fn claude_generation_2026_07_28_round_trips_through_real_socket() {
        assert_generation_round_trip(McpProtocolGeneration::V2026_07_28, "2026-07-28");
    }

    #[test]
    fn codex_generation_2025_06_18_round_trips_through_real_socket() {
        assert_generation_round_trip(McpProtocolGeneration::V2025_06_18, "2025-06-18");
    }

    fn assert_rejected_without_calls(run: &CliRun, server: &RunningServer) {
        assert!(
            run.record.mcp_tools_discovered.is_empty(),
            "거부된 연결은 도구 목록을 받지 못한다"
        );
        assert_eq!(run.record.mcp_calls_executed, 0_usize, "도구 호출 0회");
        assert!(server.ledger.records().is_empty(), "원장에 호출이 없다");
    }

    #[test]
    fn token_of_another_run_is_rejected_for_both_generations() {
        runtime().block_on(async {
            let base = TempBaseDir::new("cross");
            let target = start_server(&base, "run-m09-target");
            let other = start_server(&base, "run-m09-other");
            let foreign_token = token_text(&other.handle);
            for (index, protocol) in [
                McpProtocolGeneration::V2026_07_28,
                McpProtocolGeneration::V2025_06_18,
            ]
            .into_iter()
            .enumerate()
            {
                let fake = scenario(protocol, target.handle.socket_path());
                let name = format!("fake-cross-{index}");
                let run = run_fake_cli(&base, &name, &fake, foreign_token.clone()).await;
                assert_rejected_without_calls(&run, &target);
                assert_token_absent(&run, &foreign_token);
            }
            assert!(
                other.ledger.records().is_empty(),
                "다른 실행 원장도 비어 있다"
            );
            let _first = target.handle.shutdown().await;
            let _second = other.handle.shutdown().await;
        });
    }

    #[test]
    fn missing_token_is_rejected() {
        runtime().block_on(async {
            let base = TempBaseDir::new("empty");
            let server = start_server(&base, "run-m09-empty");
            let fake = scenario(
                McpProtocolGeneration::V2026_07_28,
                server.handle.socket_path(),
            );
            let run = run_fake_cli(&base, "fake-empty", &fake, String::new()).await;
            assert_rejected_without_calls(&run, &server);
            let _events = server.handle.shutdown().await;
        });
    }

    #[test]
    fn expired_token_is_rejected_for_both_generations() {
        runtime().block_on(async {
            let base = TempBaseDir::new("expired");
            let server = start_server(&base, "run-m09-expired");
            let token = token_text(&server.handle);
            server
                .clock
                .advance_to(timestamp_at(START_SECONDS.saturating_add(86_400_i64)))
                .expect("시각 이동");
            for (index, protocol) in [
                McpProtocolGeneration::V2026_07_28,
                McpProtocolGeneration::V2025_06_18,
            ]
            .into_iter()
            .enumerate()
            {
                let fake = scenario(protocol, server.handle.socket_path());
                let name = format!("fake-expired-{index}");
                let run = run_fake_cli(&base, &name, &fake, token.clone()).await;
                assert_rejected_without_calls(&run, &server);
                assert_token_absent(&run, &token);
            }
            let _events = server.handle.shutdown().await;
        });
    }
}
