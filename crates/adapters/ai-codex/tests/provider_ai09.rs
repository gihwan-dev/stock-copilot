#[cfg(test)]
mod tests {
    use ai_codex::{
        CodexActivation, CodexAnalysisProfile, CodexApiKeyEnv, CodexHostEnvironment, CodexProvider,
        CodexProviderConfig, decode_app_server_text,
    };
    use application::{
        AiEvent, AiEventStream, AiProvider, AiRunId, AnalysisPrompt, InformationError,
        IsolationCheck, IsolationFailure, ModelName, ModelSelection, ReasoningEffort, ResetTime,
        RunInput, RunKind, RunPurpose, RunSpec,
    };
    use serde_json::{Value, json};
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use testkit::fake_ai_cli::{
        EnvPresence, FakeAiMode, FakeAiScenario, PreparedFakeAiCli, ReplayScenario, ReplayTurn,
        TurnStop,
    };

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/tmp").join(format!("sc-ai09-{}-{seq}-{label}", std::process::id()));
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

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
        {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("expected ready"),
        }
    }

    fn fixture_lines(name: &str) -> Vec<String> {
        fs::read_to_string(Path::new("../../../fixtures/ai").join(name))
            .expect("fixture")
            .lines()
            .map(String::from)
            .collect()
    }

    fn turn(lines: Vec<String>) -> ReplayTurn {
        ReplayTurn {
            stdout_lines: lines,
            stderr_lines: Vec::new(),
            stop_after: TurnStop::Continue,
        }
    }

    fn scenario_from(lines: &[String]) -> FakeAiScenario {
        let (first, rest) = lines.split_first().expect("thread/start");
        let (second, tail) = rest.split_first().expect("mcp list");
        FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: Vec::new(),
                turns: vec![
                    turn(vec![first.clone()]),
                    turn(vec![second.clone()]),
                    turn(tail.to_vec()),
                ],
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 0_u8,
            }),
            capture_path: None,
        }
    }

    fn host(api_key: CodexApiKeyEnv) -> CodexHostEnvironment {
        CodexHostEnvironment::new(
            String::from("/tmp/fake-home"),
            String::from("/usr/bin:/bin"),
            String::from("testuser"),
            api_key,
        )
    }

    fn config(executable: &Path, root: &Path, api_key: CodexApiKeyEnv) -> CodexProviderConfig {
        CodexProviderConfig::new(
            executable.to_path_buf(),
            root.join("work"),
            host(api_key),
            CodexAnalysisProfile {
                expected_mcp_server: String::from("app"),
                analysis_tools: BTreeSet::from([String::from("get_portfolio")]),
                mcp_server_config: json!({"command": "/usr/bin/true", "required": true}),
            },
            ModelName::try_new("gpt-6.1-sol").expect("model"),
        )
        .with_allowed_cli_versions(BTreeSet::from([String::from("0.167.1")]))
    }

    fn spec() -> RunSpec {
        RunSpec::new(
            AiRunId::try_new("codex-run-1").expect("id"),
            RunInput::Analysis(AnalysisPrompt::try_new("portfolio check").expect("prompt")),
            ModelSelection::Default,
            ReasoningEffort::Medium,
            RunPurpose::Chat,
        )
    }

    fn drain(stream: &mut ai_codex::CodexEventStream) -> Result<Vec<AiEvent>, InformationError> {
        let mut events = Vec::new();
        while let Some(event) = run(stream.next())? {
            events.push(event);
        }
        Ok(events)
    }

    fn stdin_methods(prepared: &PreparedFakeAiCli) -> Vec<String> {
        prepared
            .read_invocations()
            .expect("invocations")
            .first()
            .expect("one")
            .stdin_lines
            .iter()
            .map(|line| {
                let value: Value = serde_json::from_str(line).expect("json");
                String::from(value["method"].as_str().expect("method"))
            })
            .collect()
    }

    #[test]
    fn locked_until_s09_spawns_nothing() {
        let dir = TempDirGuard::new("locked");
        let prepared = PreparedFakeAiCli::prepare(
            dir.path(),
            "fake-codex",
            &scenario_from(&fixture_lines("codex-app-server-analysis.jsonl")),
        )
        .expect("prepare");
        let provider = CodexProvider::new(config(
            prepared.executable_path(),
            dir.path(),
            CodexApiKeyEnv::Absent,
        ));
        assert_eq!(
            run(provider.run(spec())).map(|_stream| ()),
            Err(InformationError::Unavailable(ResetTime::NotProvided))
        );
        assert_eq!(
            run(provider.verify_isolation(RunKind::Analysis)),
            Err(InformationError::Unavailable(ResetTime::NotProvided))
        );
        assert_eq!(prepared.read_invocations().expect("none").len(), 0);
    }

    #[test]
    fn api_key_in_host_env_refuses_even_when_unlocked() {
        let dir = TempDirGuard::new("apikey");
        let prepared = PreparedFakeAiCli::prepare(
            dir.path(),
            "fake-codex",
            &scenario_from(&fixture_lines("codex-app-server-analysis.jsonl")),
        )
        .expect("prepare");
        let provider = CodexProvider::new(
            config(
                prepared.executable_path(),
                dir.path(),
                CodexApiKeyEnv::Present,
            )
            .with_activation(CodexActivation::S09Passed),
        );
        assert_eq!(
            run(provider.run(spec())).map(|_stream| ()),
            Err(InformationError::Unavailable(ResetTime::NotProvided))
        );
        assert_eq!(prepared.read_invocations().expect("none").len(), 0);
        let pairs = vec![
            (String::from("HOME"), String::from("/h")),
            (String::from("PATH"), String::from("/bin")),
            (String::from("USER"), String::from("u")),
            (String::from("OPENAI_API_KEY"), String::from("sk-x")),
        ];
        assert_eq!(
            CodexHostEnvironment::from_pairs(&pairs),
            Some(CodexHostEnvironment::new(
                String::from("/h"),
                String::from("/bin"),
                String::from("u"),
                CodexApiKeyEnv::Present
            ))
        );
    }

    #[test]
    fn unlocked_provider_replays_fixture_like_the_decoder() {
        let dir = TempDirGuard::new("replay");
        let lines = fixture_lines("codex-app-server-analysis.jsonl");
        let prepared = PreparedFakeAiCli::prepare(dir.path(), "fake-codex", &scenario_from(&lines))
            .expect("prepare");
        let cfg = config(
            prepared.executable_path(),
            dir.path(),
            CodexApiKeyEnv::Absent,
        )
        .with_activation(CodexActivation::S09Passed);
        let expected =
            decode_app_server_text(cfg.expectation_for(RunKind::Analysis), &lines.join("\n"))
                .expect("decoder");
        let provider = CodexProvider::new(cfg);
        let mut stream = run(provider.run(spec())).expect("run");
        assert_eq!(drain(&mut stream).expect("events"), expected);
        assert_eq!(
            stdin_methods(&prepared),
            vec!["thread/start", "mcpServerStatus/list", "turn/start"]
        );
        let record = prepared.read_invocations().expect("inv");
        let first = record.first().expect("one");
        assert_eq!(first.env_observation.openai_api_key, EnvPresence::Absent);
        assert_eq!(first.env_observation.mcp_token, EnvPresence::Absent);
        assert_eq!(first.argv.first().map(String::as_str), Some("app-server"));
        for flag in [
            "web_search=\"disabled\"",
            "shell_tool",
            "unified_exec",
            "history.persistence=\"none\"",
        ] {
            assert!(first.argv.iter().any(|arg| arg == flag), "{flag}");
        }
        let work_root = dir.path().join("work");
        assert_eq!(
            fs::read_dir(&work_root).expect("root").count(),
            0,
            "작업 폴더 삭제"
        );
    }

    #[test]
    fn rejected_isolation_sends_no_turn() {
        let dir = TempDirGuard::new("reject");
        let mut lines = fixture_lines("codex-app-server-analysis.jsonl");
        let disconnected =
            json!({"id": 2_i32, "result": {"data": [{"name": "app", "runtimeStatus": "failed", "tools": []}]}})
                .to_string();
        if let Some(second) = lines.get_mut(1) {
            *second = disconnected;
        }
        let prepared = PreparedFakeAiCli::prepare(dir.path(), "fake-codex", &scenario_from(&lines))
            .expect("prepare");
        let provider = CodexProvider::new(
            config(
                prepared.executable_path(),
                dir.path(),
                CodexApiKeyEnv::Absent,
            )
            .with_activation(CodexActivation::S09Passed),
        );
        let check = run(provider.verify_isolation(RunKind::Analysis)).expect("check");
        assert!(
            matches_rejected(check),
            "연결 안 된 앱 서버는 거절: {check:?}"
        );
        assert_eq!(
            stdin_methods(&prepared),
            vec!["thread/start", "mcpServerStatus/list"]
        );
        let expectation = provider.config().expectation_for(RunKind::Collection);
        assert_eq!(expectation.expected_mcp_server(), None);
    }

    const fn matches_rejected(check: IsolationCheck) -> bool {
        match check {
            IsolationCheck::Rejected(
                IsolationFailure::McpServers
                | IsolationFailure::Tools
                | IsolationFailure::Plugins
                | IsolationFailure::Skills
                | IsolationFailure::SlashCommands
                | IsolationFailure::PermissionMode
                | IsolationFailure::ApiKeySource
                | IsolationFailure::CliVersion
                | IsolationFailure::LoginRequired
                | IsolationFailure::NotInstalled,
            ) => true,
            IsolationCheck::Passed => false,
        }
    }
}
