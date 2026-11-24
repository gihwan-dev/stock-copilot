#[cfg(test)]
mod tests {
    use ai_claude_code::{
        ClaudeCodeConfig, ClaudeCodeProvider, HostEnvironment, McpBridgeConfig, PREFLIGHT_MESSAGE,
    };
    use application::{
        AiEvent, AiEventStream, AiProvider, AiRunId, AnalysisPrompt, CollectionQuery,
        CollectionTopic, InformationError, IsolationCheck, IsolationFailure, ModelSelection,
        ReasoningEffort, ResponseMismatch, RunInput, RunKind, RunPurpose, RunSpec, SecretValue,
        ToolStatus,
    };
    use domain::{Symbol, UntrustedText};
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use testkit::fake_ai_cli::{
        BRIDGE_FLAG, CliWireFormat, FakeAiMode, FakeAiScenario, InitOverride, McpClientScenario,
        McpFailureBehavior, McpProtocolGeneration, McpTransportSpec, PreflightBehavior,
        PreparedFakeAiCli, ReplayScenario, ReplayTurn, ScriptedTokenUsage, ScriptedToolCall,
        TurnStop,
    };

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ServerMode {
        Started,
        Offline,
    }

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/tmp").join(format!("sc-ai11-{}-{seq}-{label}", std::process::id()));
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

    fn run_async<F: Future>(future: F) -> F::Output {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(future)
    }

    fn clean_host_env() -> HostEnvironment {
        HostEnvironment::new_clean(
            String::from("/tmp/fake-home"),
            String::from("/usr/bin:/bin"),
            String::from("testuser"),
            Some(String::from("testuser")),
            String::from("/tmp"),
        )
    }

    fn spawn_mock_mcp_uds(socket_path: &Path, expected_token: &str) -> thread::JoinHandle<()> {
        let listener = UnixListener::bind(socket_path).expect("bind uds");
        let expected_auth = format!("AUTH {expected_token}");
        thread::spawn(move || {
            let Ok((stream, _addr)) = listener.accept() else {
                return;
            };
            let Ok(mut writer) = stream.try_clone() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            let mut auth_line = String::new();
            if reader.read_line(&mut auth_line).is_err()
                || auth_line.trim_end_matches(['\r', '\n']) != expected_auth
            {
                return;
            }
            serve_mcp_requests(&mut reader, &mut writer);
        })
    }

    fn serve_mcp_requests(
        reader: &mut BufReader<std::os::unix::net::UnixStream>,
        writer: &mut std::os::unix::net::UnixStream,
    ) {
        loop {
            let mut line = String::new();
            let Ok(bytes) = reader.read_line(&mut line) else {
                break;
            };
            if bytes == 0_usize {
                break;
            }
            let Ok(req) = serde_json::from_str::<Value>(line.trim_end_matches(['\r', '\n'])) else {
                break;
            };
            let id = req.get("id").cloned().unwrap_or(Value::Null);
            let method = req.get("method").and_then(Value::as_str).unwrap_or("");
            let reply = match method {
                "server/discover" => {
                    json!({"jsonrpc": "2.0", "id": id, "result": {"serverInfo": {"name": "app"}}})
                }
                "tools/list" => {
                    json!({"jsonrpc": "2.0", "id": id, "result": {"tools": [{"name": "get_portfolio"}]}})
                }
                "tools/call" => {
                    json!({"jsonrpc": "2.0", "id": id, "result": {"content": [{"type": "text", "text": "{\"cash\":\"1000\"}"}]}})
                }
                _ => {
                    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601_i32, "message": "unknown"}})
                }
            };
            let encoded = serde_json::to_string(&reply).expect("encode reply");
            if writeln!(writer, "{encoded}").is_err() || writer.flush().is_err() {
                break;
            }
        }
    }

    fn scripted_mcp_scenario(init_override: InitOverride) -> FakeAiScenario {
        FakeAiScenario {
            mode: FakeAiMode::ScriptedMcpClient(Box::new(McpClientScenario {
                protocol: McpProtocolGeneration::V2026_07_28,
                wire_format: CliWireFormat::ClaudeStreamJson,
                transport: McpTransportSpec::FromClaudeMcpConfig,
                server_name: String::from("app"),
                tool_calls: vec![ScriptedToolCall {
                    name: String::from("get_portfolio"),
                    arguments: json!({}),
                }],
                final_text: String::from("portfolio ok"),
                token_usage: ScriptedTokenUsage {
                    input: 10_u64,
                    output: 5_u64,
                    cache_read: 0_u64,
                    cache_creation: 0_u64,
                },
                init_override,
                on_mcp_failure: McpFailureBehavior::ContinueWithFailedInit,
                preflight: PreflightBehavior::WaitForStdinTurns,
            })),
            capture_path: None,
        }
    }

    fn make_mcp_provider(
        executable: &Path,
        work_dir: PathBuf,
        socket_path: &Path,
        token: &str,
    ) -> ClaudeCodeProvider {
        let bridge = McpBridgeConfig::try_new(
            String::from("app"),
            executable.to_path_buf(),
            vec![
                String::from(BRIDGE_FLAG),
                socket_path.to_string_lossy().into_owned(),
            ],
        )
        .expect("bridge");
        let config = ClaudeCodeConfig::new(
            executable.to_path_buf(),
            work_dir,
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
        )
        .expect("config")
        .with_allowed_cli_versions(BTreeSet::from([String::from("2.1.322")]))
        .with_host_environment(clean_host_env())
        .with_mcp_bridge(bridge, SecretValue::new(token.as_bytes().to_vec()));
        ClaudeCodeProvider::new(config)
    }

    fn analysis_spec(id: &str, prompt: &str) -> RunSpec {
        RunSpec::new(
            AiRunId::try_new(id).expect("id"),
            RunInput::Analysis(AnalysisPrompt::try_new(prompt).expect("prompt")),
            ModelSelection::Default,
            ReasoningEffort::Default,
            RunPurpose::Chat,
        )
    }

    async fn drain_events<S: AiEventStream>(
        stream: &mut S,
    ) -> Result<Vec<AiEvent>, InformationError> {
        let mut out = Vec::new();
        while let Some(ev) = stream.next().await? {
            out.push(ev);
        }
        Ok(out)
    }

    fn is_succeeded_tool_call(event: &AiEvent) -> bool {
        match event {
            AiEvent::ToolCall { status, .. } => *status == ToolStatus::Succeeded,
            AiEvent::IsolationChecked(_)
            | AiEvent::TextDelta(_)
            | AiEvent::WebSearch { .. }
            | AiEvent::TokenUsage(_)
            | AiEvent::Quota(_)
            | AiEvent::RateLimited { .. }
            | AiEvent::ModelRerouted(_)
            | AiEvent::ActualModel(_)
            | AiEvent::McpUnavailable
            | AiEvent::Unknown(_) => false,
        }
    }

    fn join_optional(server: Option<thread::JoinHandle<()>>) {
        if let Some(handle) = server {
            match handle.join() {
                Ok(()) | Err(_) => {}
            }
        }
    }

    fn assert_rejection_sends_zero_real_requests(
        label: &str,
        init_override: InitOverride,
        expected_failure: IsolationFailure,
        server_mode: ServerMode,
    ) {
        run_async(async {
            let dir = TempDirGuard::new(label);
            let sock = dir.path().join("m.sock");
            let token = "preflight-secret-token";
            let server = match server_mode {
                ServerMode::Started => Some(spawn_mock_mcp_uds(&sock, token)),
                ServerMode::Offline => None,
            };
            let prep = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-cli",
                &scripted_mcp_scenario(init_override),
            )
            .expect("prepare");
            let provider =
                make_mcp_provider(prep.executable_path(), dir.path().join("w"), &sock, token);
            let mut stream = provider
                .run(analysis_spec("rej", "SECRET_PROMPT_MUST_NOT_BE_SENT"))
                .await
                .expect("spawn");
            let events = drain_events(&mut stream).await.expect("rejected stream");
            assert_eq!(
                events,
                vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                    expected_failure
                ))],
                "{label}: must emit only IsolationChecked::Rejected"
            );
            join_optional(server);
            let records = prep.read_invocations().expect("invocations");
            assert_eq!(records.len(), 1_usize, "{label}: one invocation");
            let rec = records.first().expect("rec");
            assert_eq!(rec.mcp_calls_executed, 0_usize, "{label}: 0 MCP tool calls");
            assert_eq!(rec.stdin_lines.len(), 1_usize, "{label}: only /usage sent");
            assert!(
                rec.stdin_lines
                    .first()
                    .expect("line 1")
                    .contains(PREFLIGHT_MESSAGE),
                "{label}: first stdin line is /usage"
            );
            assert!(
                !rec.stdin_lines
                    .iter()
                    .any(|line| line.contains("SECRET_PROMPT_MUST_NOT_BE_SENT")),
                "{label}: real prompt must never be sent"
            );
        });
    }

    #[test]
    fn preflight_rejects_first_four_sec2_violations_with_zero_requests() {
        assert_rejection_sends_zero_real_requests(
            "tools",
            InitOverride {
                extra_tools: vec![String::from("Bash")],
                ..InitOverride::default()
            },
            IsolationFailure::Tools,
            ServerMode::Started,
        );
        assert_rejection_sends_zero_real_requests(
            "mcp-down",
            InitOverride::default(),
            IsolationFailure::McpServers,
            ServerMode::Offline,
        );
        assert_rejection_sends_zero_real_requests(
            "plugins",
            InitOverride {
                plugins: Some(vec![String::from("user-evil-plugin")]),
                ..InitOverride::default()
            },
            IsolationFailure::Plugins,
            ServerMode::Started,
        );
        assert_rejection_sends_zero_real_requests(
            "skills",
            InitOverride {
                skills: Some(vec![String::from("user-skill")]),
                ..InitOverride::default()
            },
            IsolationFailure::Skills,
            ServerMode::Started,
        );
    }

    #[test]
    fn preflight_rejects_remaining_four_sec2_violations_with_zero_requests() {
        assert_rejection_sends_zero_real_requests(
            "slash",
            InitOverride {
                slash_commands: Some(vec![String::from("/commit")]),
                ..InitOverride::default()
            },
            IsolationFailure::SlashCommands,
            ServerMode::Started,
        );
        assert_rejection_sends_zero_real_requests(
            "perm",
            InitOverride {
                permission_mode: Some(String::from("bypassPermissions")),
                ..InitOverride::default()
            },
            IsolationFailure::PermissionMode,
            ServerMode::Started,
        );
        assert_rejection_sends_zero_real_requests(
            "apikey",
            InitOverride {
                api_key_source: Some(String::from("env")),
                ..InitOverride::default()
            },
            IsolationFailure::ApiKeySource,
            ServerMode::Started,
        );
        assert_rejection_sends_zero_real_requests(
            "ver",
            InitOverride {
                cli_version: Some(String::from("9.9.9")),
                ..InitOverride::default()
            },
            IsolationFailure::CliVersion,
            ServerMode::Started,
        );
    }

    #[test]
    fn preflight_pass_sends_real_prompt_and_executes_mcp_tool() {
        run_async(async {
            let dir = TempDirGuard::new("pass-e2e");
            let sock = dir.path().join("m.sock");
            let token = "valid-pass-token-01";
            let server = spawn_mock_mcp_uds(&sock, token);
            let prep = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-cli",
                &scripted_mcp_scenario(InitOverride::default()),
            )
            .expect("prepare");
            let provider =
                make_mcp_provider(prep.executable_path(), dir.path().join("w"), &sock, token);
            let mut stream = provider
                .run(analysis_spec("pass-1", "REAL_ANALYSIS_PROMPT"))
                .await
                .expect("run");
            let events = drain_events(&mut stream).await.expect("drain");
            server.join().expect("server join");
            assert_eq!(
                events.first(),
                Some(&AiEvent::IsolationChecked(IsolationCheck::Passed)),
                "first event is IsolationChecked::Passed"
            );
            assert!(
                events.iter().any(is_succeeded_tool_call),
                "MCP tool call succeeded after preflight passed"
            );
            assert!(
                events.iter().any(|ev| {
                    *ev == AiEvent::TextDelta(UntrustedText::try_new("portfolio ok").expect("t"))
                }),
                "final text received"
            );
            let records = prep.read_invocations().expect("inv");
            let rec = records.first().expect("rec");
            assert_eq!(rec.mcp_calls_executed, 1_usize, "1 MCP tool call executed");
            assert_eq!(rec.stdin_lines.len(), 2_usize, "/usage then real prompt");
            assert!(
                rec.stdin_lines
                    .get(1_usize)
                    .expect("line 2")
                    .contains("REAL_ANALYSIS_PROMPT"),
                "second stdin line carries real prompt"
            );
            assert_eq!(
                provider.last_cli_version().await.as_deref(),
                Some("2.1.322"),
                "CLI version recorded"
            );
        });
    }

    fn turn_replay(turns: Vec<ReplayTurn>) -> FakeAiScenario {
        FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: Vec::new(),
                turns,
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        }
    }

    fn valid_analysis_init() -> String {
        json!({
            "type": "system",
            "subtype": "init",
            "tools": ["mcp__app__get_portfolio"],
            "mcp_servers": [{"name": "app", "status": "connected"}],
            "model": "claude-opus-5-5",
            "permissionMode": "dontAsk",
            "slash_commands": [],
            "skills": [],
            "plugins": ["cc-plugin-privacy"],
            "apiKeySource": "none",
            "claude_code_version": "2.1.322"
        })
        .to_string()
    }

    fn preflight_echo_lines() -> Vec<String> {
        vec![
            valid_analysis_init(),
            json!({
                "type": "assistant",
                "message": {"content": [{"type": "text", "text": "/usage isn't available in this environment."}]}
            })
            .to_string(),
            json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 0_u64})
                .to_string(),
        ]
    }

    fn bad_collection_init() -> String {
        json!({
            "type": "system",
            "subtype": "init",
            "tools": ["WebSearch"],
            "mcp_servers": [{"name": "app", "status": "connected"}],
            "model": "claude-haiku-5-5",
            "permissionMode": "dontAsk",
            "slash_commands": [],
            "skills": [],
            "plugins": ["cc-plugin-privacy"],
            "apiKeySource": "none",
            "claude_code_version": "2.1.322"
        })
        .to_string()
    }

    #[test]
    fn collection_preflight_rejects_unexpected_mcp_server_with_zero_requests() {
        run_async(async {
            let dir = TempDirGuard::new("col-mcp");
            let scenario = turn_replay(vec![
                ReplayTurn {
                    stdout_lines: vec![bad_collection_init()],
                    stderr_lines: Vec::new(),
                    stop_after: TurnStop::Continue,
                },
                ReplayTurn {
                    stdout_lines: vec![
                        json!({"type": "result", "subtype": "success", "is_error": false})
                            .to_string(),
                    ],
                    stderr_lines: Vec::new(),
                    stop_after: TurnStop::Terminate,
                },
            ]);
            let prep = PreparedFakeAiCli::prepare(dir.path(), "fake-col", &scenario).expect("prep");
            let config = ClaudeCodeConfig::new(
                prep.executable_path().to_path_buf(),
                dir.path().join("w"),
                String::from("app"),
                BTreeSet::from([String::from("mcp__app__get_portfolio")]),
                BTreeSet::from([String::from("cc-plugin-privacy")]),
            )
            .expect("cfg")
            .with_host_environment(clean_host_env());
            let provider = ClaudeCodeProvider::new(config);
            let spec = RunSpec::new(
                AiRunId::try_new("col-1").expect("id"),
                RunInput::Collection(CollectionQuery::CodeTopics {
                    instruments: vec![Symbol::try_new("005930").expect("sym")],
                    topics: vec![CollectionTopic::CompanyNews],
                }),
                ModelSelection::Fast,
                ReasoningEffort::Default,
                RunPurpose::WebCollection,
            );
            let mut stream = provider.run(spec).await.expect("run");
            let events = drain_events(&mut stream).await.expect("drain");
            assert_eq!(
                events,
                vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                    IsolationFailure::McpServers
                ))],
                "collection must reject non-empty mcp_servers"
            );
            let rec = prep.read_invocations().expect("inv");
            assert_eq!(
                rec.first().expect("rec").stdin_lines.len(),
                1_usize,
                "only /usage sent before collection rejection"
            );
        });
    }

    #[test]
    fn real_cli_usage_preflight_echo_is_suppressed() {
        run_async(async {
            let dir = TempDirGuard::new("echo-ok");
            let real_turn = vec![
                valid_analysis_init(),
                json!({
                    "type": "stream_event",
                    "event": {"type": "content_block_delta", "delta": {"type": "text_delta", "text": "actual answer"}}
                })
                .to_string(),
                json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1_u64})
                    .to_string(),
            ];
            let scenario = turn_replay(vec![
                ReplayTurn {
                    stdout_lines: preflight_echo_lines(),
                    stderr_lines: Vec::new(),
                    stop_after: TurnStop::Continue,
                },
                ReplayTurn {
                    stdout_lines: real_turn,
                    stderr_lines: Vec::new(),
                    stop_after: TurnStop::Terminate,
                },
            ]);
            let prep =
                PreparedFakeAiCli::prepare(dir.path(), "fake-echo", &scenario).expect("prep");
            let sock = dir.path().join("unused.sock");
            let provider = make_mcp_provider(
                prep.executable_path(),
                dir.path().join("w1"),
                &sock,
                "tok-1",
            );
            let mut stream = provider
                .run(analysis_spec("echo-1", "question"))
                .await
                .expect("run");
            let events = drain_events(&mut stream).await.expect("drain");
            assert_eq!(
                events,
                vec![
                    AiEvent::IsolationChecked(IsolationCheck::Passed),
                    AiEvent::TextDelta(UntrustedText::try_new("actual answer").expect("t")),
                ],
                "preflight /usage echo text must never leak to user"
            );
        });
    }

    #[test]
    fn eof_immediately_after_preflight_echo_fails_closed() {
        run_async(async {
            let dir = TempDirGuard::new("echo-eof");
            let scenario = turn_replay(vec![ReplayTurn {
                stdout_lines: preflight_echo_lines(),
                stderr_lines: Vec::new(),
                stop_after: TurnStop::Terminate,
            }]);
            let prep =
                PreparedFakeAiCli::prepare(dir.path(), "fake-trunc", &scenario).expect("prep");
            let sock = dir.path().join("unused.sock");
            let provider = make_mcp_provider(
                prep.executable_path(),
                dir.path().join("w2"),
                &sock,
                "tok-2",
            );
            let mut stream = provider
                .run(analysis_spec("echo-2", "question"))
                .await
                .expect("run");
            let err = drain_events(&mut stream)
                .await
                .expect_err("must fail closed");
            assert_eq!(
                err,
                InformationError::InvalidResponse(ResponseMismatch::Validation),
                "EOF right after /usage preflight with no real turn fails closed"
            );
        });
    }

    #[test]
    fn verify_isolation_sends_only_usage_and_never_invokes_mcp_tools() {
        run_async(async {
            let dir = TempDirGuard::new("verify-iso");
            let sock = dir.path().join("m.sock");
            let token = "verify-token-99";
            let server = spawn_mock_mcp_uds(&sock, token);
            let prep = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-cli",
                &scripted_mcp_scenario(InitOverride::default()),
            )
            .expect("prep");
            let provider =
                make_mcp_provider(prep.executable_path(), dir.path().join("w"), &sock, token);
            let outcome = provider
                .verify_isolation(RunKind::Analysis)
                .await
                .expect("verify");
            server.join().expect("server");
            assert_eq!(outcome, IsolationCheck::Passed, "isolation check passes");
            let rec = prep.read_invocations().expect("inv");
            let first = rec.first().expect("rec");
            assert_eq!(first.stdin_lines.len(), 1_usize, "only /usage sent");
            assert_eq!(first.mcp_calls_executed, 0_usize, "0 MCP tool calls");
        });
    }

    #[test]
    fn child_closing_stdout_while_alive_is_killed_without_hanging() {
        use std::os::unix::fs::PermissionsExt;
        run_async(async {
            let dir = TempDirGuard::new("eof-hang");
            let script = dir.path().join("hang-after-eof");
            let init = valid_analysis_init();
            let body = format!(
                "#!/bin/sh\nread _first\nprintf '%s\\n' '{init}'\nexec 1>&-\nexec /bin/sleep 30\n"
            );
            fs::write(&script, body).expect("write");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o700_u32)).expect("chmod");
            let sock = dir.path().join("unused.sock");
            let provider = make_mcp_provider(&script, dir.path().join("w"), &sock, "tok-hang");
            let mut stream = provider
                .run(analysis_spec("hang-1", "hi"))
                .await
                .expect("run");
            assert_eq!(
                stream.next().await.expect("init"),
                Some(AiEvent::IsolationChecked(IsolationCheck::Passed)),
                "received init before stdout closed"
            );
            let outcome =
                tokio::time::timeout(std::time::Duration::from_secs(5_u64), stream.next())
                    .await
                    .expect("must reap hanging child within bounded grace without deadlock");
            assert_eq!(
                outcome,
                Err(InformationError::Unavailable(
                    application::ResetTime::NotProvided
                )),
                "killed hanging child ends with error and no orphan"
            );
        });
    }

    #[test]
    fn interrupted_run_ends_cleanly_without_unavailable_error() {
        use std::os::unix::fs::PermissionsExt;
        run_async(async {
            let dir = TempDirGuard::new("sigint-clean");
            let script = dir.path().join("sigint-child");
            let init = valid_analysis_init();
            let body = format!(
                "#!/bin/sh\nread _first\nprintf '%s\\n' '{init}'\nread _second\nexec /bin/sleep 10\n"
            );
            fs::write(&script, body).expect("write");
            fs::set_permissions(&script, fs::Permissions::from_mode(0o700_u32)).expect("chmod");
            let sock = dir.path().join("unused.sock");
            let provider = make_mcp_provider(&script, dir.path().join("w"), &sock, "tok-int");
            let run_id = AiRunId::try_new("int-1").expect("id");
            let mut stream = provider
                .run(analysis_spec("int-1", "hi"))
                .await
                .expect("run");
            assert_eq!(
                stream.next().await.expect("init"),
                Some(AiEvent::IsolationChecked(IsolationCheck::Passed)),
                "init passed"
            );
            provider.interrupt(&run_id).await.expect("interrupt");
            let end = tokio::time::timeout(std::time::Duration::from_secs(5_u64), stream.next())
                .await
                .expect("interrupted stream finishes promptly")
                .expect("interrupted run ends cleanly, not Unavailable error");
            assert_eq!(end, None, "stream closed cleanly after interrupt");
        });
    }
}
