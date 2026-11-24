#[cfg(test)]
mod tests {
    use ai_claude_code::{
        ClaudeCodeConfig, ClaudeCodeProvider, HostEnvironment, McpBridgeConfig, decode_stream_text,
    };
    use application::{
        AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, AnalysisPrompt, CollectionQuery,
        CollectionTopic, InformationError, IsolationCheck, IsolationFailure, ModelSelection,
        ReasoningEffort, ResetTime, ResponseMismatch, RunInput, RunKind, RunPurpose, RunSpec,
        SecretValue,
    };
    use domain::{Nanosecond, OffsetSeconds, Symbol, Timestamp, UnixSeconds, UntrustedText};
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use testkit::fake_ai_cli::{
        FakeAiMode, FakeAiScenario, PreparedFakeAiCli, ReplayScenario, ReplayTurn, TurnStop,
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
                .join("ai-claude-ai02-tmp")
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

    fn valid_init_line() -> String {
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

    fn replay_scenario(
        immediate_lines: Vec<String>,
        trailing_without_newline: Option<String>,
        exit_code: u8,
    ) -> FakeAiScenario {
        FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines,
                turns: Vec::new(),
                final_stderr_lines: Vec::new(),
                trailing_without_newline,
                exit_code,
            }),
            capture_path: None,
        }
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

    fn make_provider(executable: PathBuf, work_dir: PathBuf) -> ClaudeCodeProvider {
        let bridge = McpBridgeConfig::try_new(
            String::from("app"),
            PathBuf::from("/usr/bin/true"),
            Vec::new(),
        )
        .expect("bridge");
        let config = ClaudeCodeConfig::new(
            executable,
            work_dir,
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
        )
        .expect("config")
        .with_allowed_cli_versions(BTreeSet::from([String::from("2.1.322")]))
        .with_host_environment(clean_host_env())
        .with_mcp_bridge(bridge, SecretValue::new(b"test-token-value".to_vec()));
        ClaudeCodeProvider::new(config)
    }

    fn analysis_spec(id: &str, text: &str) -> RunSpec {
        RunSpec::new(
            AiRunId::try_new(id).expect("run id"),
            RunInput::Analysis(AnalysisPrompt::try_new(text).expect("prompt")),
            ModelSelection::Default,
            ReasoningEffort::Medium,
            RunPurpose::Chat,
        )
    }

    fn collection_spec(id: &str) -> RunSpec {
        RunSpec::new(
            AiRunId::try_new(id).expect("run id"),
            RunInput::Collection(CollectionQuery::CodeTopics {
                instruments: vec![Symbol::try_new("005930").expect("symbol")],
                topics: vec![CollectionTopic::CompanyNews],
            }),
            ModelSelection::Fast,
            ReasoningEffort::Low,
            RunPurpose::WebCollection,
        )
    }

    async fn collect_all<S: AiEventStream>(
        stream: &mut S,
    ) -> Result<Vec<AiEvent>, InformationError> {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await? {
            events.push(event);
        }
        Ok(events)
    }

    fn run_async<F: Future>(future: F) -> F::Output {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(future)
    }

    #[test]
    fn provider_replays_analysis_fixture_matching_decoder() {
        run_async(async {
            let dir = TempDirGuard::new("analysis");
            let lines = fixture_lines("claude-analysis.jsonl");
            let raw = lines.join("\n");
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-analysis",
                &replay_scenario(lines, None, 0_u8),
            )
            .expect("prepare");
            let provider = make_provider(
                prepared.executable_path().to_path_buf(),
                dir.path().join("work"),
            );
            let mut stream = provider
                .run(analysis_spec("run-a", "check portfolio"))
                .await
                .expect("run analysis");
            let actual = collect_all(&mut stream).await.expect("collect analysis");
            let expected =
                decode_stream_text(provider.config().expectation_for(RunKind::Analysis), &raw)
                    .expect("decode expected");
            assert_eq!(actual, expected, "analysis fixture stream matches decoder");
        });
    }

    #[test]
    fn provider_replays_collection_fixture_matching_decoder() {
        run_async(async {
            let dir = TempDirGuard::new("collection");
            let lines = fixture_lines("claude-collection.jsonl");
            let raw = lines.join("\n");
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-collection",
                &replay_scenario(lines, None, 0_u8),
            )
            .expect("prepare");
            let provider = make_provider(
                prepared.executable_path().to_path_buf(),
                dir.path().join("work"),
            );
            let mut stream = provider
                .run(collection_spec("run-c"))
                .await
                .expect("run collection");
            let actual = collect_all(&mut stream).await.expect("collect collection");
            let expected =
                decode_stream_text(provider.config().expectation_for(RunKind::Collection), &raw)
                    .expect("decode expected col");
            assert_eq!(
                actual, expected,
                "collection fixture stream matches decoder"
            );
        });
    }

    #[test]
    fn provider_replays_rate_limited_fixture_as_waiting_not_error() {
        run_async(async {
            let dir = TempDirGuard::new("rate-limited");
            let lines = fixture_lines("claude-rate-limited.jsonl");
            let raw = lines.join("\n");
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-rl",
                &replay_scenario(lines, None, 1_u8),
            )
            .expect("prepare");
            let provider = make_provider(
                prepared.executable_path().to_path_buf(),
                dir.path().join("work"),
            );
            let mut stream = provider
                .run(analysis_spec("run-rl", "analyze"))
                .await
                .expect("run rl");
            let events = collect_all(&mut stream).await.expect("rate limit ends ok");
            let expected =
                decode_stream_text(provider.config().expectation_for(RunKind::Analysis), &raw)
                    .expect("expected rl");
            assert_eq!(events, expected, "rate limited events match fixture");
            let expected_reset = Timestamp::from_parts(
                UnixSeconds::new(1_794_394_800_i64),
                Nanosecond::new(0_u32),
                OffsetSeconds::new(0_i32),
            )
            .expect("timestamp");
            assert_eq!(
                provider.usage().await.expect("usage"),
                AiUsage::Waiting(ResetTime::Known(expected_reset)),
                "known reset time preserved without being overwritten by NotProvided"
            );
        });
    }

    #[test]
    fn provider_tolerates_unknown_event_types_and_unknown_fields() {
        run_async(async {
            let dir = TempDirGuard::new("unknown-fields");
            let init = json!({
                "type": "system",
                "subtype": "init",
                "tools": ["mcp__app__get_portfolio"],
                "mcp_servers": [{"name": "app", "status": "connected", "extra": true}],
                "model": "claude-opus-5-5",
                "permissionMode": "dontAsk",
                "slash_commands": [],
                "skills": [],
                "plugins": [{"name": "cc-plugin-privacy", "extra_nested": [1_i32, 2_i32]}],
                "apiKeySource": "none",
                "claude_code_version": "2.1.322",
                "brand_new_field": {"nested": "ignored"}
            })
            .to_string();
            let future_event =
                json!({"type": "future_telemetry", "payload": {"v": 99_i32}}).to_string();
            let result = json!({"type": "result", "subtype": "success", "is_error": false, "future_metric": 123_i32}).to_string();
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-unk",
                &replay_scenario(vec![init, future_event.clone(), result], None, 0_u8),
            )
            .expect("prepare");
            let provider = make_provider(
                prepared.executable_path().to_path_buf(),
                dir.path().join("work"),
            );
            let mut stream = provider
                .run(analysis_spec("run-unk", "hi"))
                .await
                .expect("run");
            let events = collect_all(&mut stream).await.expect("collect");
            assert_eq!(
                events,
                vec![
                    AiEvent::IsolationChecked(IsolationCheck::Passed),
                    AiEvent::Unknown(UntrustedText::try_new(&future_event).expect("untrusted")),
                ],
                "unknown events preserved and unknown fields tolerated"
            );
        });
    }

    #[test]
    fn provider_blocks_premature_output_before_init() {
        run_async(async {
            let dir = TempDirGuard::new("early-output");
            let early_text = json!({
                "type": "stream_event",
                "event": {"type": "content_block_delta", "delta": {"type": "text_delta", "text": "leaked"}}
            })
            .to_string();
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-early",
                &replay_scenario(vec![early_text], None, 0_u8),
            )
            .expect("prepare");
            let provider = make_provider(
                prepared.executable_path().to_path_buf(),
                dir.path().join("work"),
            );
            let mut stream = provider
                .run(analysis_spec("run-early", "hi"))
                .await
                .expect("spawn early");
            let err = stream.next().await.expect_err("must fail before init");
            assert_eq!(
                err,
                InformationError::InvalidResponse(ResponseMismatch::Validation),
                "premature event before init fails closed with zero leaked events"
            );
        });
    }

    #[test]
    fn provider_blocks_all_output_after_isolation_rejection() {
        run_async(async {
            let dir = TempDirGuard::new("reject-gate");
            let bad_init = json!({
                "type": "system",
                "subtype": "init",
                "tools": ["mcp__app__get_portfolio", "Bash"],
                "mcp_servers": [{"name": "app", "status": "connected"}],
                "model": "claude-opus-5-5",
                "permissionMode": "dontAsk",
                "slash_commands": [],
                "skills": [],
                "plugins": ["cc-plugin-privacy"],
                "apiKeySource": "none",
                "claude_code_version": "2.1.322"
            })
            .to_string();
            let leaked = json!({
                "type": "stream_event",
                "event": {"type": "content_block_delta", "delta": {"type": "text_delta", "text": "leaked"}}
            })
            .to_string();
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-reject",
                &replay_scenario(vec![bad_init, leaked], None, 0_u8),
            )
            .expect("prepare");
            let provider = make_provider(
                prepared.executable_path().to_path_buf(),
                dir.path().join("work"),
            );
            let mut stream = provider
                .run(analysis_spec("run-reject", "hi"))
                .await
                .expect("spawn reject");
            let events = collect_all(&mut stream)
                .await
                .expect("rejection emits only rejected event");
            assert_eq!(
                events,
                vec![AiEvent::IsolationChecked(IsolationCheck::Rejected(
                    IsolationFailure::Tools
                ))],
                "no output leaks after isolation rejection"
            );
        });
    }

    #[test]
    fn provider_fails_closed_on_broken_trailing_and_oversized_lines() {
        run_async(async {
            let dir = TempDirGuard::new("malformed-oversized");
            let prep_mal = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-mal",
                &replay_scenario(vec![valid_init_line()], Some(String::from("{broken")), 0_u8),
            )
            .expect("prepare mal");
            let prov_mal = make_provider(
                prep_mal.executable_path().to_path_buf(),
                dir.path().join("work-mal"),
            );
            let mut stream_mal = prov_mal
                .run(analysis_spec("run-mal", "hi"))
                .await
                .expect("run");
            assert_eq!(
                stream_mal.next().await.expect("first"),
                Some(AiEvent::IsolationChecked(IsolationCheck::Passed)),
                "init passes first"
            );
            assert_eq!(
                stream_mal.next().await,
                Err(InformationError::InvalidResponse(ResponseMismatch::Schema)),
                "broken trailing line fails closed"
            );

            let prep_big = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-big",
                &replay_scenario(
                    vec![valid_init_line(), "x".repeat(70_000_usize)],
                    None,
                    0_u8,
                ),
            )
            .expect("prepare big");
            let prov_big = make_provider(
                prep_big.executable_path().to_path_buf(),
                dir.path().join("work-big"),
            );
            let mut stream_big = prov_big
                .run(analysis_spec("run-big", "hi"))
                .await
                .expect("run");
            assert_eq!(
                stream_big.next().await.expect("first"),
                Some(AiEvent::IsolationChecked(IsolationCheck::Passed)),
                "init passes first"
            );
            assert_eq!(
                stream_big.next().await,
                Err(InformationError::InvalidResponse(ResponseMismatch::Schema)),
                "oversized line >65536 bytes fails closed"
            );
        });
    }

    #[test]
    fn provider_fails_closed_on_isolation_regression() {
        run_async(async {
            let dir = TempDirGuard::new("regress");
            let regressed_init = json!({
                "type": "system",
                "subtype": "init",
                "tools": ["mcp__app__get_portfolio", "WebSearch"],
                "mcp_servers": [{"name": "app", "status": "connected"}],
                "model": "claude-opus-5-5",
                "permissionMode": "dontAsk",
                "slash_commands": [],
                "skills": [],
                "plugins": ["cc-plugin-privacy"],
                "apiKeySource": "none",
                "claude_code_version": "2.1.322"
            })
            .to_string();
            let prep = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-reg",
                &replay_scenario(vec![valid_init_line(), regressed_init], None, 0_u8),
            )
            .expect("prepare reg");
            let prov = make_provider(
                prep.executable_path().to_path_buf(),
                dir.path().join("work"),
            );
            let mut stream = prov.run(analysis_spec("run-reg", "hi")).await.expect("run");
            assert_eq!(
                stream.next().await.expect("first"),
                Some(AiEvent::IsolationChecked(IsolationCheck::Passed)),
                "first init passes"
            );
            assert_eq!(
                stream.next().await,
                Err(InformationError::InvalidResponse(
                    ResponseMismatch::Validation
                )),
                "isolation regression aborts immediately"
            );
        });
    }

    #[test]
    fn provider_fails_closed_on_nonzero_exit() {
        run_async(async {
            let dir = TempDirGuard::new("nonzero");
            let prep = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-nz",
                &replay_scenario(vec![valid_init_line()], None, 2_u8),
            )
            .expect("prepare nz");
            let prov = make_provider(
                prep.executable_path().to_path_buf(),
                dir.path().join("work"),
            );
            let mut stream = prov.run(analysis_spec("run-nz", "hi")).await.expect("run");
            assert_eq!(
                stream.next().await.expect("first"),
                Some(AiEvent::IsolationChecked(IsolationCheck::Passed)),
                "init passes"
            );
            assert_eq!(
                stream.next().await,
                Err(InformationError::Unavailable(ResetTime::NotProvided)),
                "non-zero exit without rate limit returns error"
            );
        });
    }

    fn two_turn_continue_scenario() -> FakeAiScenario {
        FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: Vec::new(),
                turns: vec![
                    ReplayTurn {
                        stdout_lines: vec![valid_init_line()],
                        stderr_lines: Vec::new(),
                        stop_after: TurnStop::Continue,
                    },
                    ReplayTurn {
                        stdout_lines: Vec::new(),
                        stderr_lines: Vec::new(),
                        stop_after: TurnStop::Terminate,
                    },
                ],
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        }
    }

    #[test]
    fn dropping_stream_and_debug_never_leak_secrets() {
        run_async(async {
            let dir = TempDirGuard::new("drop-debug");
            let prepared =
                PreparedFakeAiCli::prepare(dir.path(), "fake-drop", &two_turn_continue_scenario())
                    .expect("prepare");
            let secret_token = "SUPER_SECRET_MCP_TOKEN_99999";
            let secret_prompt = "CONFIDENTIAL_HOLDING_12345678";
            let bridge = McpBridgeConfig::try_new(
                String::from("app"),
                PathBuf::from("/usr/bin/true"),
                Vec::new(),
            )
            .expect("bridge");
            let config = ClaudeCodeConfig::new(
                prepared.executable_path().to_path_buf(),
                dir.path().join("work-drop"),
                String::from("app"),
                BTreeSet::from([String::from("mcp__app__get_portfolio")]),
                BTreeSet::from([String::from("cc-plugin-privacy")]),
            )
            .expect("config")
            .with_host_environment(clean_host_env())
            .with_mcp_bridge(bridge, SecretValue::new(secret_token.as_bytes().to_vec()));
            assert!(
                !format!("{config:?}").contains(secret_token),
                "config Debug must not leak token"
            );
            let provider = ClaudeCodeProvider::new(config);
            assert!(
                !format!("{provider:?}").contains(secret_token),
                "provider Debug must not leak token"
            );
            let mut stream = provider
                .run(analysis_spec("run-drop", secret_prompt))
                .await
                .expect("run");
            let stream_debug = format!("{stream:?}");
            assert!(
                !stream_debug.contains(secret_token) && !stream_debug.contains(secret_prompt),
                "stream Debug must not leak token or prompt"
            );
            assert_eq!(
                stream.next().await.expect("first"),
                Some(AiEvent::IsolationChecked(IsolationCheck::Passed)),
                "received init before early drop"
            );
        });
    }
}
