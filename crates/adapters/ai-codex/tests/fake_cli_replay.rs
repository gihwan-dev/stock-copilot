#[cfg(test)]
mod tests {
    use ai_codex::{CodexIsolationExpectation, decode_app_server_text, decode_exec_text};
    use application::{
        AiEvent, IsolationCheck, ModelName, RunKind, TokenCount, TokenUsage, ToolStatus,
    };
    use domain::UntrustedText;
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::fs::{self, DirBuilder};
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::fs::DirBuilderExt;
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use testkit::fake_ai_cli::{
        CliWireFormat, FakeAiMode, FakeAiScenario, InitOverride, McpClientScenario,
        McpFailureBehavior, McpProtocolGeneration, McpTransportSpec, PreflightBehavior,
        PreparedFakeAiCli, ReplayScenario, ScriptedTokenUsage, ScriptedToolCall,
    };

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let exe = testkit::test_executable().expect("exe");
            let target_dir = exe
                .ancestors()
                .find(|dir| dir.file_name().and_then(|name| name.to_str()) == Some("target"))
                .expect("target dir");
            let path = target_dir
                .join("ai-codex-replay-tmp")
                .join(format!("{label}-{}-{seq}", std::process::id()));
            fs::create_dir_all(&path).expect("create dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            match fs::remove_dir_all(&self.0) {
                Ok(()) | Err(_) => {}
            }
        }
    }

    struct TempSocketGuard {
        dir: PathBuf,
        socket: PathBuf,
    }

    impl TempSocketGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let dir = Path::new("/tmp").join(format!("sc-cx-{label}-{}-{seq}", std::process::id()));
            match fs::remove_dir_all(&dir) {
                Ok(()) | Err(_) => {}
            }
            DirBuilder::new()
                .mode(0o700_u32)
                .create(&dir)
                .expect("create private socket dir");
            let socket = dir.join("s.sock");
            Self { dir, socket }
        }

        fn path(&self) -> &Path {
            &self.socket
        }
    }

    impl Drop for TempSocketGuard {
        fn drop(&mut self) {
            match fs::remove_dir_all(&self.dir) {
                Ok(()) | Err(_) => {}
            }
        }
    }

    fn fixture_lines(name: &str) -> Vec<String> {
        let exe = testkit::test_executable().expect("exe");
        let root = exe
            .ancestors()
            .find(|dir| dir.join("fixtures").join("ai").is_dir())
            .expect("workspace root");
        fs::read_to_string(root.join("fixtures").join("ai").join(name))
            .expect("read fixture")
            .lines()
            .map(String::from)
            .collect()
    }

    fn analysis_expectation() -> CodexIsolationExpectation {
        CodexIsolationExpectation::new(
            RunKind::Analysis,
            BTreeSet::from([String::from("get_portfolio")]),
            Some(String::from("app")),
            Some(BTreeSet::from([String::from("0.167.1")])),
        )
    }

    fn text(raw: &str) -> UntrustedText {
        UntrustedText::try_new(raw).expect("untrusted")
    }

    fn spawn_codex_mock_mcp(socket: &Path, expected_token: &str) -> thread::JoinHandle<()> {
        let listener = UnixListener::bind(socket).expect("bind socket");
        let token = String::from(expected_token);
        thread::spawn(move || serve_codex_mock_mcp(&listener, &token))
    }

    fn serve_codex_mock_mcp(listener: &UnixListener, token: &str) {
        let (stream, _addr) = listener.accept().expect("accept");
        let mut writer = stream.try_clone().expect("clone");
        let mut reader = BufReader::new(stream);
        let mut auth = String::new();
        let read_len = reader.read_line(&mut auth).expect("read auth");
        assert!(read_len > 0_usize, "auth line received");
        assert_eq!(auth.trim_end(), format!("AUTH {token}"), "valid auth line");
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).expect("read rpc") == 0_usize {
                break;
            }
            let req: Value = serde_json::from_str(line.trim_end()).expect("json");
            let id = req.get("id").cloned();
            let method = req.get("method").and_then(Value::as_str).expect("method");
            let reply = match method {
                "initialize" => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "protocolVersion": "2025-06-18", "capabilities": { "tools": {} } }
                })),
                "notifications/initialized" => None,
                "tools/list" => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": [{ "name": "get_portfolio" }] }
                })),
                "tools/call" => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": r#"{"cash_krw":"1000000"}"# }],
                        "isError": false
                    }
                })),
                _ => panic!("unexpected method {method}"),
            };
            if let Some(payload) = reply {
                writeln!(
                    writer,
                    "{}",
                    serde_json::to_string(&payload).expect("encode")
                )
                .expect("write");
                writer.flush().expect("flush");
            }
        }
    }

    #[test]
    fn fake_cli_replays_codex_app_server_and_exec_fixtures_without_path() {
        let dir = TempDirGuard::new("codex-fixtures");
        let app_lines = fixture_lines("codex-app-server-analysis.jsonl");
        let app_raw = app_lines.join("\n");
        let app_scenario = FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: app_lines,
                turns: Vec::new(),
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        };
        let app_prep = PreparedFakeAiCli::prepare(dir.path(), "codex-app", &app_scenario)
            .expect("prepare app");
        let app_out = Command::new(app_prep.executable_path())
            .env_clear()
            .output()
            .expect("spawn app");
        assert!(app_out.status.success(), "app replay exit 0");
        let app_stdout = String::from_utf8(app_out.stdout).expect("utf8");
        assert_eq!(
            decode_app_server_text(analysis_expectation(), &app_stdout).expect("decode proc"),
            decode_app_server_text(analysis_expectation(), &app_raw).expect("decode raw"),
            "app-server replay matches direct decode"
        );

        let exec_lines = fixture_lines("codex-exec-analysis.jsonl");
        let exec_raw = exec_lines.join("\n");
        let exec_scenario = FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: exec_lines,
                turns: Vec::new(),
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        };
        let exec_prep = PreparedFakeAiCli::prepare(dir.path(), "codex-exec", &exec_scenario)
            .expect("prepare exec");
        let exec_out = Command::new(exec_prep.executable_path())
            .env_clear()
            .output()
            .expect("spawn exec");
        assert!(exec_out.status.success(), "exec replay exit 0");
        let exec_stdout = String::from_utf8(exec_out.stdout).expect("utf8");
        assert_eq!(
            decode_exec_text(IsolationCheck::Passed, &exec_stdout).expect("decode exec proc"),
            decode_exec_text(IsolationCheck::Passed, &exec_raw).expect("decode exec raw"),
            "exec replay matches direct decode"
        );
    }

    fn codex_mcp_integration_scenario(socket: &Path) -> FakeAiScenario {
        FakeAiScenario {
            mode: FakeAiMode::ScriptedMcpClient(Box::new(McpClientScenario {
                protocol: McpProtocolGeneration::V2025_06_18,
                wire_format: CliWireFormat::CodexAppServerJsonRpc,
                transport: McpTransportSpec::DirectUnixSocket {
                    socket_path: socket.to_path_buf(),
                },
                server_name: String::from("app"),
                tool_calls: vec![ScriptedToolCall {
                    name: String::from("get_portfolio"),
                    arguments: json!({ "currency": "KRW" }),
                }],
                final_text: String::from("portfolio ok"),
                token_usage: ScriptedTokenUsage {
                    input: 200_u64,
                    output: 50_u64,
                    cache_read: 20_u64,
                    cache_creation: 5_u64,
                },
                init_override: InitOverride::default(),
                on_mcp_failure: McpFailureBehavior::ContinueWithFailedInit,
                preflight: PreflightBehavior::Immediate,
            })),
            capture_path: None,
        }
    }

    #[test]
    fn fake_cli_codex_2025_06_18_mcp_session_decodes_to_same_ai_event_model() {
        let dir = TempDirGuard::new("codex-mcp-dec");
        let socket = TempSocketGuard::new("dec");
        let token = "CODEX_INTEGRATION_TOKEN_902";
        let server = spawn_codex_mock_mcp(socket.path(), token);
        let prepared = PreparedFakeAiCli::prepare(
            dir.path(),
            "codex-mcp",
            &codex_mcp_integration_scenario(socket.path()),
        )
        .expect("prepare");
        let output = Command::new(prepared.executable_path())
            .env_clear()
            .env("STOCK_MCP_TOKEN", token)
            .output()
            .expect("spawn codex mcp");
        assert!(output.status.success(), "codex mcp exit 0");
        server.join().expect("join server");
        let stdout = String::from_utf8(output.stdout).expect("utf8");
        assert!(!stdout.contains(token), "token never enters stdout");
        let events =
            decode_app_server_text(analysis_expectation(), &stdout).expect("decode events");
        assert_eq!(
            events,
            vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::ToolCall {
                    server: text("app"),
                    tool: text("get_portfolio"),
                    args: text(r#"{"currency":"KRW"}"#),
                    result: text(""),
                    status: ToolStatus::Running,
                },
                AiEvent::ToolCall {
                    server: text("app"),
                    tool: text("get_portfolio"),
                    args: text(r#"{"currency":"KRW"}"#),
                    result: text(r#"{"cash_krw":"1000000"}"#),
                    status: ToolStatus::Succeeded,
                },
                AiEvent::TextDelta(text("portfolio ok")),
                AiEvent::TokenUsage(TokenUsage::new(
                    TokenCount::new(200_u64),
                    TokenCount::new(50_u64),
                    TokenCount::new(20_u64),
                    TokenCount::new(5_u64),
                )),
                AiEvent::ActualModel(ModelName::try_new("gpt-6.1-sol").expect("model")),
            ],
            "codex scripted mcp stream decodes to shared AiEvent sequence"
        );
    }
}
