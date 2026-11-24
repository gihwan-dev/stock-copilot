#[cfg(test)]
mod tests {
    use ai_claude_code::{IsolationExpectation, decode_stream_text};
    use application::{
        AiEvent, IsolationCheck, IsolationFailure, ModelName, RunKind, TokenCount, TokenUsage,
        ToolStatus,
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
        BRIDGE_FLAG, CliWireFormat, FakeAiMode, FakeAiScenario, InitOverride, McpClientScenario,
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
                .join("ai-claude-replay-tmp")
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
            let dir = Path::new("/tmp").join(format!("sc-cc-{label}-{}-{seq}", std::process::id()));
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

    fn analysis_expectation() -> IsolationExpectation {
        IsolationExpectation::new(
            RunKind::Analysis,
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            Some(String::from("app")),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
            Some(BTreeSet::from([String::from("2.1.322")])),
        )
    }

    fn text(raw: &str) -> UntrustedText {
        UntrustedText::try_new(raw).expect("untrusted text")
    }

    fn spawn_claude_mock_mcp(socket: &Path, expected_token: &str) -> thread::JoinHandle<()> {
        let listener = UnixListener::bind(socket).expect("bind socket");
        let token = String::from(expected_token);
        thread::spawn(move || serve_claude_mock_mcp(&listener, &token))
    }

    fn serve_claude_mock_mcp(listener: &UnixListener, token: &str) {
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
                "server/discover" => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "resultType": "complete", "supportedVersions": ["2026-07-28"] }
                }),
                "tools/list" => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": [{ "name": "get_portfolio" }] }
                }),
                "tools/call" => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": r#"{"cash_krw":"1000000"}"# }],
                        "isError": false
                    }
                }),
                _ => panic!("unexpected method {method}"),
            };
            writeln!(writer, "{}", serde_json::to_string(&reply).expect("encode")).expect("write");
            writer.flush().expect("flush");
        }
    }

    #[test]
    fn fake_cli_replays_claude_fixture_with_empty_path_into_identical_ai_events() {
        let dir = TempDirGuard::new("fixture");
        let lines = fixture_lines("claude-analysis.jsonl");
        let raw_fixture = lines.join("\n");
        let scenario = FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: lines,
                turns: Vec::new(),
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        };
        let prepared =
            PreparedFakeAiCli::prepare(dir.path(), "claude-fake", &scenario).expect("prepare");
        let output = Command::new(prepared.executable_path())
            .env_clear()
            .output()
            .expect("spawn fake claude");
        assert!(output.status.success(), "replay exit 0");
        let stdout = String::from_utf8(output.stdout).expect("utf8");
        let from_process =
            decode_stream_text(analysis_expectation(), &stdout).expect("decode process stdout");
        let direct =
            decode_stream_text(analysis_expectation(), &raw_fixture).expect("decode direct");
        assert_eq!(
            from_process, direct,
            "replayed stream matches direct decode"
        );
    }

    fn claude_mcp_integration_scenario() -> FakeAiScenario {
        FakeAiScenario {
            mode: FakeAiMode::ScriptedMcpClient(Box::new(McpClientScenario {
                protocol: McpProtocolGeneration::V2026_07_28,
                wire_format: CliWireFormat::ClaudeStreamJson,
                transport: McpTransportSpec::FromClaudeMcpConfig,
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

    fn write_mcp_config(dir: &Path, bridge: &Path, socket: &Path) -> PathBuf {
        let cfg_path = dir.join("mcp.json");
        let cfg = json!({
            "mcpServers": {
                "app": {
                    "type": "stdio",
                    "command": bridge,
                    "args": [BRIDGE_FLAG, socket],
                    "env": { "STOCK_MCP_TOKEN": "${STOCK_MCP_TOKEN}" }
                }
            }
        });
        fs::write(&cfg_path, serde_json::to_string(&cfg).expect("json")).expect("write cfg");
        cfg_path
    }

    #[test]
    fn fake_cli_scripted_mcp_session_decodes_cleanly_into_ai_events() {
        let dir = TempDirGuard::new("mcp-decode");
        let socket = TempSocketGuard::new("dec");
        let token = "CLAUDE_INTEGRATION_TOKEN_901";
        let server = spawn_claude_mock_mcp(socket.path(), token);
        let prepared = PreparedFakeAiCli::prepare(
            dir.path(),
            "claude-mcp",
            &claude_mcp_integration_scenario(),
        )
        .expect("prepare");
        let cfg_path = write_mcp_config(dir.path(), prepared.executable_path(), socket.path());
        let output = Command::new(prepared.executable_path())
            .arg("--mcp-config")
            .arg(&cfg_path)
            .env_clear()
            .env("STOCK_MCP_TOKEN", token)
            .output()
            .expect("spawn claude mcp");
        assert!(output.status.success(), "mcp exit 0");
        server.join().expect("join server");
        let stdout = String::from_utf8(output.stdout).expect("utf8");
        assert!(!stdout.contains(token), "token never enters stdout");
        let events = decode_stream_text(analysis_expectation(), &stdout).expect("decode events");
        assert_eq!(
            events,
            vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::ActualModel(ModelName::try_new("claude-opus-5-5").expect("model")),
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
            ],
            "scripted mcp stream decodes to exact AiEvent sequence"
        );
    }

    #[test]
    fn fake_cli_rejected_sec2_override_is_caught_by_decoder() {
        let dir = TempDirGuard::new("sec2-reject");
        let scenario = FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: vec![String::from(
                    r#"{"type":"system","subtype":"init","tools":["mcp__app__get_portfolio","Bash"],"mcp_servers":[{"name":"app","status":"connected"}],"model":"claude-opus-5-5","permissionMode":"dontAsk","slash_commands":[],"skills":[],"plugins":["cc-plugin-privacy"],"apiKeySource":"none","claude_code_version":"2.1.322"}"#,
                )],
                turns: Vec::new(),
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        };
        let prepared =
            PreparedFakeAiCli::prepare(dir.path(), "claude-bad", &scenario).expect("prepare");
        let output = Command::new(prepared.executable_path())
            .env_clear()
            .output()
            .expect("run");
        let stdout = String::from_utf8(output.stdout).expect("utf8");
        let events = decode_stream_text(analysis_expectation(), &stdout).expect("decode");
        assert_eq!(
            events,
            vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                IsolationFailure::Tools
            ))],
            "unexpected Bash tool triggers IsolationFailure::Tools"
        );
    }
}
