#[cfg(test)]
mod tests {
    use ai_claude_code::{
        ClaudeCodeConfig, ClaudeCodeProvider, HostEnvironment, McpBridgeConfig, RunMcpAttachment,
        SpawnedPidNotice,
    };
    use application::{
        AiEvent, AiEventStream, AiRunId, AnalysisPrompt, ChatHistory, CollectionQuery,
        InformationError, IsolationCheck, ModelSelection, ProcessId, ReasoningEffort,
        ResponseMismatch, RunInput, RunPurpose, RunSpec, SecretValue, ToolStatus,
    };
    use domain::UntrustedText;
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::fs;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::mpsc;
    use std::thread;
    use testkit::fake_ai_cli::{
        BRIDGE_FLAG, CliWireFormat, EnvPresence, FakeAiMode, FakeAiScenario, InitOverride,
        McpClientScenario, McpFailureBehavior, McpProtocolGeneration, McpTransportSpec,
        PreflightBehavior, PreparedFakeAiCli, ScriptedTokenUsage, ScriptedToolCall,
    };

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);
    const CONFIG_TOKEN: &str = "config-token-never-used";
    const RUN_TOKEN: &str = "per-run-token-fin-d";

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/tmp").join(format!("sc-find-{}-{seq}-{label}", std::process::id()));
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
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime")
            .block_on(future)
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

    fn reply_for(req: &Value) -> Value {
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        match req.get("method").and_then(Value::as_str).unwrap_or("") {
            "server/discover" => {
                json!({"jsonrpc": "2.0", "id": id, "result": {"serverInfo": {"name": "app"}}})
            }
            "tools/list" => {
                json!({"jsonrpc": "2.0", "id": id, "result": {"tools": [{"name": "get_portfolio"}]}})
            }
            "tools/call" => {
                json!({"jsonrpc": "2.0", "id": id, "result": {"content": [{"type": "text", "text": "{}"}]}})
            }
            _ => {
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601_i32, "message": "unknown"}})
            }
        }
    }

    fn serve(reader: &mut BufReader<UnixStream>, writer: &mut UnixStream) {
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0_usize) | Err(_) => break,
                Ok(_) => {}
            }
            let Ok(req) = serde_json::from_str::<Value>(line.trim_end_matches(['\r', '\n'])) else {
                break;
            };
            let encoded = serde_json::to_string(&reply_for(&req)).expect("encode");
            if writeln!(writer, "{encoded}").is_err() || writer.flush().is_err() {
                break;
            }
        }
    }

    fn spawn_run_server(socket_path: &Path, seen: mpsc::Sender<String>) -> thread::JoinHandle<()> {
        let listener = UnixListener::bind(socket_path).expect("bind uds");
        thread::spawn(move || {
            let Ok((stream, _addr)) = listener.accept() else {
                return;
            };
            let Ok(mut writer) = stream.try_clone() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            let mut auth_line = String::new();
            if reader.read_line(&mut auth_line).is_err() {
                return;
            }
            let auth = String::from(auth_line.trim_end_matches(['\r', '\n']));
            let accepted = auth == format!("AUTH {RUN_TOKEN}");
            let _sent = seen.send(auth);
            if accepted {
                serve(&mut reader, &mut writer);
            }
        })
    }

    fn scenario() -> FakeAiScenario {
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
                final_text: String::from("attached ok"),
                token_usage: ScriptedTokenUsage {
                    input: 1_u64,
                    output: 1_u64,
                    cache_read: 0_u64,
                    cache_creation: 0_u64,
                },
                init_override: InitOverride::default(),
                on_mcp_failure: McpFailureBehavior::ContinueWithFailedInit,
                preflight: PreflightBehavior::WaitForStdinTurns,
            })),
            capture_path: None,
        }
    }

    fn bridge(executable: &Path, socket_path: &Path, server: &str) -> McpBridgeConfig {
        McpBridgeConfig::try_new(
            String::from(server),
            executable.to_path_buf(),
            vec![
                String::from(BRIDGE_FLAG),
                socket_path.to_string_lossy().into_owned(),
            ],
        )
        .expect("bridge")
    }

    fn provider(executable: &Path, root: &Path) -> ClaudeCodeProvider {
        let config = ClaudeCodeConfig::new(
            executable.to_path_buf(),
            root.join("w"),
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
        )
        .expect("config")
        .with_allowed_cli_versions(BTreeSet::from([String::from("2.1.322")]))
        .with_host_environment(clean_host_env())
        .with_mcp_bridge(
            bridge(executable, &root.join("dead.sock"), "app"),
            SecretValue::new(CONFIG_TOKEN.as_bytes().to_vec()),
        )
        .with_cli_projects_dir(root.join("projects"));
        ClaudeCodeProvider::new(config)
    }

    fn analysis_spec(id: &str) -> RunSpec {
        RunSpec::new(
            AiRunId::try_new(id).expect("id"),
            RunInput::Analysis(AnalysisPrompt::try_new("check").expect("prompt")),
            ModelSelection::Default,
            ReasoningEffort::Default,
            RunPurpose::Chat,
        )
    }

    fn collection_spec(id: &str) -> RunSpec {
        RunSpec::new(
            AiRunId::try_new(id).expect("id"),
            RunInput::Collection(CollectionQuery::CodeTopics {
                instruments: Vec::new(),
                topics: Vec::new(),
            }),
            ModelSelection::Fast,
            ReasoningEffort::Low,
            RunPurpose::WebCollection,
        )
    }

    async fn drain<S: AiEventStream>(stream: &mut S) -> Result<Vec<AiEvent>, InformationError> {
        let mut out = Vec::new();
        while let Some(event) = stream.next().await? {
            out.push(event);
        }
        Ok(out)
    }

    fn succeeded_tool_calls(events: &[AiEvent]) -> usize {
        events
            .iter()
            .filter(|event| match event {
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
            })
            .count()
    }

    fn notice_slot() -> (mpsc::Receiver<Option<ProcessId>>, SpawnedPidNotice) {
        let (sender, receiver) = mpsc::channel();
        let notice = SpawnedPidNotice::new(move |pid| {
            let _sent = sender.send(pid);
        });
        (receiver, notice)
    }

    fn assert_isolation_args(argv: &[String]) {
        for flag in [
            "--restricted",
            "--disable-slash-commands",
            "--strict-mcp-config",
            "--no-session-persistence",
        ] {
            assert!(argv.iter().any(|arg| arg == flag), "{flag}");
        }
        for (flag, value) in [("--tools", ""), ("--permission-mode", "dontAsk")] {
            assert!(
                argv.iter()
                    .zip(argv.iter().skip(1))
                    .any(|(left, right)| left == flag && right == value),
                "{flag}"
            );
        }
        assert!(argv.iter().all(|arg| !arg.contains(RUN_TOKEN)));
        assert!(argv.iter().all(|arg| !arg.contains(CONFIG_TOKEN)));
    }

    #[test]
    fn attached_run_uses_the_per_run_bridge_and_token_and_reports_the_cli_pid() {
        run_async(async {
            let dir = TempDirGuard::new("attach");
            let sock = dir.path().join("run.sock");
            let (seen_tx, seen) = mpsc::channel();
            let server = spawn_run_server(&sock, seen_tx);
            let prep =
                PreparedFakeAiCli::prepare(dir.path(), "fake-cli", &scenario()).expect("prepare");
            let provider = provider(prep.executable_path(), dir.path());
            let (pids, notice) = notice_slot();
            let attachment = RunMcpAttachment::new(
                bridge(prep.executable_path(), &sock, "app"),
                SecretValue::new(RUN_TOKEN.as_bytes().to_vec()),
            )
            .with_spawned_notice(notice);
            let mut stream = provider
                .run_attached(analysis_spec("attach-run"), attachment)
                .await
                .expect("run");
            let events = drain(&mut stream).await.expect("stream");
            match server.join() {
                Ok(()) | Err(_) => {}
            }
            assert_eq!(succeeded_tool_calls(&events), 1, "{events:?}");
            assert_eq!(
                seen.try_iter().collect::<Vec<_>>(),
                vec![format!("AUTH {RUN_TOKEN}")]
            );
            let recorded: Vec<_> = pids.try_iter().collect();
            assert_eq!(recorded.len(), 1);
            assert!(recorded.first().is_some_and(Option::is_some));
            let invocations = prep.read_invocations().expect("invocations");
            let invocation = invocations.first().expect("one invocation");
            assert_eq!(invocation.env_observation.mcp_token, EnvPresence::Present);
            assert_isolation_args(&invocation.argv);
        });
    }

    #[test]
    fn attached_resume_also_routes_through_the_per_run_bridge() {
        run_async(async {
            let dir = TempDirGuard::new("resume");
            let sock = dir.path().join("run.sock");
            let (seen_tx, seen) = mpsc::channel();
            let server = spawn_run_server(&sock, seen_tx);
            let prep =
                PreparedFakeAiCli::prepare(dir.path(), "fake-cli", &scenario()).expect("prepare");
            let provider = provider(prep.executable_path(), dir.path());
            let attachment = RunMcpAttachment::new(
                bridge(prep.executable_path(), &sock, "app"),
                SecretValue::new(RUN_TOKEN.as_bytes().to_vec()),
            );
            let mut stream = provider
                .resume_attached(
                    analysis_spec("attach-resume"),
                    ChatHistory::new(Vec::new()),
                    UntrustedText::try_new("next").expect("message"),
                    attachment,
                )
                .await
                .expect("resume");
            let events = drain(&mut stream).await.expect("stream");
            match server.join() {
                Ok(()) | Err(_) => {}
            }
            assert_eq!(succeeded_tool_calls(&events), 1, "{events:?}");
            assert_eq!(
                seen.try_iter().collect::<Vec<_>>(),
                vec![format!("AUTH {RUN_TOKEN}")]
            );
        });
    }

    #[test]
    fn attached_isolation_probe_checks_the_per_run_server_connection() {
        run_async(async {
            let dir = TempDirGuard::new("probe");
            let sock = dir.path().join("run.sock");
            let (seen_tx, seen) = mpsc::channel();
            let server = spawn_run_server(&sock, seen_tx);
            let prep =
                PreparedFakeAiCli::prepare(dir.path(), "fake-cli", &scenario()).expect("prepare");
            let provider = provider(prep.executable_path(), dir.path());
            let attachment = RunMcpAttachment::new(
                bridge(prep.executable_path(), &sock, "app"),
                SecretValue::new(RUN_TOKEN.as_bytes().to_vec()),
            );
            let check = provider
                .verify_isolation_attached(attachment)
                .await
                .expect("probe");
            match server.join() {
                Ok(()) | Err(_) => {}
            }
            assert_eq!(check, IsolationCheck::Passed);
            assert_eq!(
                seen.try_iter().collect::<Vec<_>>(),
                vec![format!("AUTH {RUN_TOKEN}")]
            );
        });
    }

    #[test]
    fn attachment_is_refused_for_collection_runs_and_wrong_server_names_without_spawning() {
        run_async(async {
            let dir = TempDirGuard::new("refuse");
            let prep =
                PreparedFakeAiCli::prepare(dir.path(), "fake-cli", &scenario()).expect("prepare");
            let provider = provider(prep.executable_path(), dir.path());
            let sock = dir.path().join("unused.sock");
            for (spec, server) in [
                (collection_spec("attach-collection"), "app"),
                (analysis_spec("attach-wrong-name"), "other"),
            ] {
                let (pids, notice) = notice_slot();
                let attachment = RunMcpAttachment::new(
                    bridge(prep.executable_path(), &sock, server),
                    SecretValue::new(RUN_TOKEN.as_bytes().to_vec()),
                )
                .with_spawned_notice(notice);
                let outcome = provider.run_attached(spec, attachment).await;
                assert_eq!(
                    outcome.err(),
                    Some(InformationError::InvalidResponse(
                        ResponseMismatch::Validation
                    )),
                    "{server}"
                );
                assert_eq!(pids.try_iter().count(), 0_usize);
            }
            assert_eq!(prep.read_invocations().expect("invocations").len(), 0_usize);
        });
    }
}
