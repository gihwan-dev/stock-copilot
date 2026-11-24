#[cfg(test)]
mod tests {
    use ai_claude_code::{
        ApiKeyEnvState, ClaudeCodeConfig, ClaudeCodeProvider, CommandBuildError, HostEnvironment,
        McpBridgeConfig, McpConfigSource, RequiredEnvKey, prepare_run_workspace,
    };
    use application::{
        AiEventStream, AiProvider, AiRunId, AnalysisPrompt, CollectionQuery, CollectionTopic,
        InformationError, ModelSelection, ReasoningEffort, ResponseMismatch, RunInput, RunPurpose,
        RunSpec, SecretValue,
    };
    use domain::Symbol;
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::fs::{self, File};
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use testkit::fake_ai_cli::{
        EnvObservation, EnvPresence, FakeAiMode, FakeAiScenario, NonessentialTrafficFlag,
        PreparedFakeAiCli, ReplayScenario,
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
                .join("ai-claude-ai03-tmp")
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

    fn run_async<F: Future>(future: F) -> F::Output {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(future)
    }

    fn host_env_with_noise() -> HostEnvironment {
        HostEnvironment::from_pairs([
            ("HOME", "/tmp/sandbox-home"),
            ("PATH", "/usr/bin:/bin"),
            ("USER", "localuser"),
            ("LOGNAME", "localuser"),
            ("TMPDIR", "/tmp"),
            ("OPENAI_API_KEY", "sk-openai-never-forwarded"),
            ("TOSS_CLIENT_SECRET", "toss-secret-never-forwarded"),
            ("CLAUDE_CONFIG_DIR", "/tmp/should-not-forward"),
            ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "0"),
        ])
    }

    fn analysis_init_line() -> String {
        json!({
            "type": "system",
            "subtype": "init",
            "tools": ["mcp__app__get_portfolio", "mcp__app__get_quote"],
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

    fn collection_init_line() -> String {
        json!({
            "type": "system",
            "subtype": "init",
            "tools": ["WebSearch"],
            "mcp_servers": [],
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

    fn simple_replay(init_line: String) -> FakeAiScenario {
        let result = json!({"type": "result", "subtype": "success", "is_error": false}).to_string();
        FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: vec![init_line, result],
                turns: Vec::new(),
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        }
    }

    fn sample_bridge() -> McpBridgeConfig {
        McpBridgeConfig::try_new(
            String::from("app"),
            PathBuf::from("/opt/stock-copilot/mcp-bridge"),
            vec![String::from("--socket"), String::from("/tmp/run.sock")],
        )
        .expect("bridge")
    }

    fn base_config(executable: PathBuf, work_dir: PathBuf) -> ClaudeCodeConfig {
        ClaudeCodeConfig::new(
            executable,
            work_dir,
            String::from("app"),
            BTreeSet::from([
                String::from("mcp__app__get_portfolio"),
                String::from("mcp__app__get_quote"),
            ]),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
        )
        .expect("config")
        .with_host_environment(host_env_with_noise())
        .with_mcp_bridge(
            sample_bridge(),
            SecretValue::new(b"secret-run-token-xyz".to_vec()),
        )
    }

    const EXPECTED_ANALYSIS_ARGS: [&str; 26] = [
        "-p",
        "--verbose",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--include-partial-messages",
        "--restricted",
        "--disable-slash-commands",
        "--tools",
        "",
        "--allowedTools",
        "mcp__app__get_portfolio",
        "mcp__app__get_quote",
        "--permission-mode",
        "dontAsk",
        "--permission-prompts",
        "none",
        "--mcp-config",
        "/tmp/w/mcp.json",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--model",
        "claude-opus-5-5",
        "--effort",
        "high",
    ];

    const COLLECTION_SCHEMA: &str = r#"{"type":"object","properties":{"items":{"type":"array"}}}"#;

    const EXPECTED_COLLECTION_ARGS: [&str; 25] = [
        "-p",
        "--verbose",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--include-partial-messages",
        "--restricted",
        "--disable-slash-commands",
        "--tools",
        "WebSearch",
        "--allowedTools",
        "WebSearch",
        "--permission-mode",
        "dontAsk",
        "--permission-prompts",
        "none",
        "--mcp-config",
        "/tmp/w/empty.json",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--model",
        "claude-haiku-5-5",
        "--json-schema",
        COLLECTION_SCHEMA,
    ];

    #[test]
    fn analysis_profile_argv_matches_exact_snapshot() {
        let config = base_config(
            PathBuf::from("/usr/local/bin/claude"),
            PathBuf::from("/tmp/w"),
        );
        let spec = RunSpec::new(
            AiRunId::try_new("run-snap-a").expect("id"),
            RunInput::Analysis(AnalysisPrompt::try_new("check").expect("prompt")),
            ModelSelection::Highest,
            ReasoningEffort::High,
            RunPurpose::Estimate,
        );
        let plan = config
            .preview_command_plan(
                &spec,
                Path::new("/tmp/w/work-1"),
                Path::new("/tmp/w/mcp.json"),
            )
            .expect("plan");
        assert_eq!(
            plan.args(),
            &EXPECTED_ANALYSIS_ARGS,
            "analysis profile CLI args must match exact snapshot"
        );
        assert_eq!(
            plan.env_keys(),
            vec![
                "HOME",
                "PATH",
                "USER",
                "TMPDIR",
                "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
                "LOGNAME",
                "STOCK_MCP_TOKEN",
            ],
            "analysis env keys contain only allowlist plus token"
        );
        assert!(
            !format!("{plan:?}").contains("secret-run-token-xyz"),
            "CommandPlan Debug must not expose token"
        );
    }

    #[test]
    fn collection_profile_argv_matches_exact_snapshot_without_mcp_token() {
        let config = base_config(
            PathBuf::from("/usr/local/bin/claude"),
            PathBuf::from("/tmp/w"),
        )
        .with_collection_json_schema(String::from(COLLECTION_SCHEMA));
        let spec = RunSpec::new(
            AiRunId::try_new("run-snap-c").expect("id"),
            RunInput::Collection(CollectionQuery::CodeTopics {
                instruments: vec![Symbol::try_new("005930").expect("sym")],
                topics: vec![CollectionTopic::CompanyNews],
            }),
            ModelSelection::Fast,
            ReasoningEffort::Default,
            RunPurpose::WebCollection,
        );
        let plan = config
            .preview_command_plan(
                &spec,
                Path::new("/tmp/w/work-2"),
                Path::new("/tmp/w/empty.json"),
            )
            .expect("plan");
        assert_eq!(
            plan.args(),
            &EXPECTED_COLLECTION_ARGS,
            "collection profile CLI args must match exact snapshot"
        );
        assert_eq!(
            plan.env_keys(),
            vec![
                "HOME",
                "PATH",
                "USER",
                "TMPDIR",
                "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
                "LOGNAME",
            ],
            "collection env keys must never contain STOCK_MCP_TOKEN"
        );
    }

    #[test]
    fn analysis_child_receives_only_allowlisted_env_plus_mcp_token() {
        run_async(async {
            let dir = TempDirGuard::new("env-obs-a");
            let prep_a = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-a",
                &simple_replay(analysis_init_line()),
            )
            .expect("prep a");
            let prov_a = ClaudeCodeProvider::new(base_config(
                prep_a.executable_path().to_path_buf(),
                dir.path().join("work-a"),
            ));
            let mut stream_a = prov_a
                .run(RunSpec::new(
                    AiRunId::try_new("obs-a").expect("id"),
                    RunInput::Analysis(AnalysisPrompt::try_new("go").expect("p")),
                    ModelSelection::Default,
                    ReasoningEffort::Default,
                    RunPurpose::Chat,
                ))
                .await
                .expect("run a");
            while stream_a.next().await.expect("next a").is_some() {}
            let rec_a = prep_a.read_invocations().expect("inv a");
            assert_eq!(rec_a.len(), 1_usize, "one analysis invocation");
            assert_eq!(
                rec_a.first().expect("first a").env_observation,
                EnvObservation {
                    mcp_token: EnvPresence::Present,
                    anthropic_api_key: EnvPresence::Absent,
                    openai_api_key: EnvPresence::Absent,
                    user: EnvPresence::Present,
                    disable_nonessential_traffic: NonessentialTrafficFlag::Disabled,
                },
                "analysis child environment matches allowlist"
            );
        });
    }

    #[test]
    fn collection_child_receives_allowlisted_env_without_mcp_token() {
        run_async(async {
            let dir = TempDirGuard::new("env-obs-c");
            let prep_c = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-c",
                &simple_replay(collection_init_line()),
            )
            .expect("prep c");
            let prov_c = ClaudeCodeProvider::new(base_config(
                prep_c.executable_path().to_path_buf(),
                dir.path().join("work-c"),
            ));
            let mut stream_c = prov_c
                .run(RunSpec::new(
                    AiRunId::try_new("obs-c").expect("id"),
                    RunInput::Collection(CollectionQuery::CodeTopics {
                        instruments: vec![Symbol::try_new("005930").expect("s")],
                        topics: vec![CollectionTopic::Disclosures],
                    }),
                    ModelSelection::Fast,
                    ReasoningEffort::Default,
                    RunPurpose::WebCollection,
                ))
                .await
                .expect("run c");
            while stream_c.next().await.expect("next c").is_some() {}
            let rec_c = prep_c.read_invocations().expect("inv c");
            assert_eq!(rec_c.len(), 1_usize, "one collection invocation");
            assert_eq!(
                rec_c.first().expect("first c").env_observation,
                EnvObservation {
                    mcp_token: EnvPresence::Absent,
                    anthropic_api_key: EnvPresence::Absent,
                    openai_api_key: EnvPresence::Absent,
                    user: EnvPresence::Present,
                    disable_nonessential_traffic: NonessentialTrafficFlag::Disabled,
                },
                "collection child never receives MCP token or API keys"
            );
        });
    }

    #[test]
    fn api_key_env_refuses_spawn_with_zero_invocations() {
        run_async(async {
            let dir = TempDirGuard::new("api-key-refuse");
            for (key, expected_state) in [
                ("ANTHROPIC_API_KEY", ApiKeyEnvState::AnthropicApiKey),
                ("ANTHROPIC_AUTH_TOKEN", ApiKeyEnvState::AnthropicAuthToken),
            ] {
                let prepared = PreparedFakeAiCli::prepare(
                    dir.path(),
                    &format!("fake-{key}"),
                    &simple_replay(analysis_init_line()),
                )
                .expect("prep");
                let env = HostEnvironment::from_pairs([
                    ("HOME", "/tmp/h"),
                    ("PATH", "/usr/bin"),
                    ("USER", "u"),
                    ("TMPDIR", "/tmp"),
                    (key, "sk-ant-should-reject"),
                ]);
                assert!(
                    !format!("{env:?}").contains("sk-ant-should-reject"),
                    "HostEnvironment Debug must never retain API key value"
                );
                let config = base_config(
                    prepared.executable_path().to_path_buf(),
                    dir.path().join(format!("work-{key}")),
                )
                .with_host_environment(env);
                let spec = RunSpec::new(
                    AiRunId::try_new("reject-api").expect("id"),
                    RunInput::Analysis(AnalysisPrompt::try_new("go").expect("p")),
                    ModelSelection::Default,
                    ReasoningEffort::Default,
                    RunPurpose::Chat,
                );
                assert_eq!(
                    config
                        .preview_command_plan(&spec, Path::new("/tmp/w"), Path::new("/tmp/m"))
                        .expect_err("must reject api key"),
                    CommandBuildError::ForbiddenApiKeyEnv(expected_state),
                    "preview reports ForbiddenApiKeyEnv"
                );
                let provider = ClaudeCodeProvider::new(config);
                let err = provider.run(spec).await.expect_err("run must refuse");
                assert_eq!(
                    err,
                    InformationError::InvalidResponse(ResponseMismatch::Validation),
                    "provider refuses api key env"
                );
                assert!(
                    prepared.read_invocations().expect("inv").is_empty(),
                    "zero spawns when API key is present"
                );
            }
        });
    }

    #[test]
    fn missing_required_env_and_invalid_tools_refuse_build() {
        let missing_user = HostEnvironment::from_pairs([
            ("HOME", "/tmp/h"),
            ("PATH", "/usr/bin"),
            ("TMPDIR", "/tmp"),
        ]);
        let config_no_user = base_config(PathBuf::from("/usr/bin/claude"), PathBuf::from("/tmp/w"))
            .with_host_environment(missing_user);
        let spec = RunSpec::new(
            AiRunId::try_new("no-user").expect("id"),
            RunInput::Analysis(AnalysisPrompt::try_new("go").expect("p")),
            ModelSelection::Default,
            ReasoningEffort::Default,
            RunPurpose::Chat,
        );
        assert_eq!(
            config_no_user
                .preview_command_plan(&spec, Path::new("/tmp/w"), Path::new("/tmp/m"))
                .expect_err("missing USER"),
            CommandBuildError::MissingRequiredEnv(RequiredEnvKey::User),
            "USER is mandatory for Keychain subscription auth"
        );

        let bad_tools_config = ClaudeCodeConfig::new(
            PathBuf::from("/usr/bin/claude"),
            PathBuf::from("/tmp/w"),
            String::from("app"),
            BTreeSet::from([String::from("Bash")]),
            BTreeSet::new(),
        )
        .expect("cfg")
        .with_host_environment(host_env_with_noise());
        assert_eq!(
            bad_tools_config
                .preview_command_plan(&spec, Path::new("/tmp/w"), Path::new("/tmp/m"))
                .expect_err("non-mcp tool"),
            CommandBuildError::InvalidAnalysisTools,
            "analysis profile rejects non-MCP tool names"
        );
    }

    #[test]
    fn workspace_is_empty_0700_and_mcp_json_is_0600_with_env_placeholder_only() {
        let dir = TempDirGuard::new("workspace-perms");
        let secret = SecretValue::new(b"TOP_SECRET_TOKEN_123456".to_vec());
        let run_id = AiRunId::try_new("ws-perm-1").expect("id");
        let bridge = sample_bridge();
        let workspace = prepare_run_workspace(
            dir.path(),
            &run_id,
            McpConfigSource::Bridge(&bridge),
            Some(&secret),
        )
        .expect("workspace");
        let work_meta = fs::symlink_metadata(workspace.work_dir()).expect("work meta");
        assert_eq!(
            work_meta.permissions().mode() & 0o777_u32,
            0o700_u32,
            "work dir must be 0700"
        );
        assert_eq!(
            fs::read_dir(workspace.work_dir())
                .expect("read work")
                .count(),
            0_usize,
            "work dir must be completely empty"
        );
        let mcp_meta = fs::symlink_metadata(workspace.mcp_config_path()).expect("mcp meta");
        assert_eq!(
            mcp_meta.permissions().mode() & 0o777_u32,
            0o600_u32,
            "mcp.json must be 0600"
        );
        let raw_mcp = fs::read_to_string(workspace.mcp_config_path()).expect("read mcp");
        assert!(
            !raw_mcp.contains("TOP_SECRET_TOKEN_123456"),
            "literal token must never appear in mcp.json"
        );
        let parsed: Value = serde_json::from_str(&raw_mcp).expect("json");
        assert_eq!(
            parsed["mcpServers"]["app"]["env"]["STOCK_MCP_TOKEN"],
            Value::String(String::from("${STOCK_MCP_TOKEN}")),
            "mcp.json uses env variable placeholder"
        );
        let work_path = workspace.work_dir().to_path_buf();
        let cfg_path = workspace.config_dir().to_path_buf();
        workspace.cleanup();
        assert!(!work_path.exists(), "work dir removed after cleanup");
        assert!(!cfg_path.exists(), "config dir removed after cleanup");
    }

    #[test]
    fn collection_workspace_writes_empty_mcp_servers_json() {
        let dir = TempDirGuard::new("workspace-col-empty");
        let empty_ws = prepare_run_workspace(
            dir.path(),
            &AiRunId::try_new("ws-col-1").expect("id"),
            McpConfigSource::Empty,
            None,
        )
        .expect("empty ws");
        let empty_parsed: Value = serde_json::from_str(
            &fs::read_to_string(empty_ws.mcp_config_path()).expect("read empty"),
        )
        .expect("parse empty");
        assert_eq!(
            empty_parsed,
            json!({"mcpServers": {}}),
            "collection mcp.json has empty mcpServers"
        );
    }

    #[test]
    fn existing_mcp_config_containing_literal_secret_is_rejected() {
        let dir = TempDirGuard::new("secret-in-cfg");
        let leaked_path = dir.path().join("leaked.json");
        fs::write(
            &leaked_path,
            r#"{"mcpServers":{"app":{"command":"/bin/true","env":{"STOCK_MCP_TOKEN":"LITERAL_TOKEN_999"}}}}"#,
        )
        .expect("write");
        let secret = SecretValue::new(b"LITERAL_TOKEN_999".to_vec());
        let err = prepare_run_workspace(
            dir.path(),
            &AiRunId::try_new("leak-1").expect("id"),
            McpConfigSource::ExistingFile(&leaked_path),
            Some(&secret),
        )
        .expect_err("must reject literal secret in config");
        assert_eq!(
            err,
            CommandBuildError::SecretInConfig,
            "literal secret in mcp config rejected"
        );
    }

    #[test]
    fn parent_opened_file_descriptor_is_not_inherited_by_child() {
        let dir = TempDirGuard::new("fd-cloexec");
        let mut held_files = Vec::new();
        for idx in 0_usize..16_usize {
            let file_path = dir.path().join(format!("probe-{idx}.txt"));
            fs::write(&file_path, "secret-fd-content").expect("write probe");
            held_files.push(File::open(&file_path).expect("open probe"));
        }
        let probe_fd = held_files.last().expect("last file").as_raw_fd();
        let check_script = format!("(: <&{probe_fd}) 2>/dev/null");
        let status = Command::new("/bin/sh")
            .env_clear()
            .args(["-c", &check_script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("spawn sh");
        assert!(
            !status.success(),
            "parent file descriptor {probe_fd} must not be inherited by child process"
        );
    }
}
