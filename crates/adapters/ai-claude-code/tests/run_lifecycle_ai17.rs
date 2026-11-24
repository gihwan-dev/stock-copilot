#[cfg(test)]
mod tests {
    use ai_claude_code::{
        ClaudeCodeConfig, ClaudeCodeProvider, HostEnvironment, McpBridgeConfig,
        encode_claude_project_dir_name,
    };
    use application::{
        AiEvent, AiEventStream, AiProvider, AiRunId, AnalysisPrompt, InformationError,
        IsolationCheck, ModelSelection, ReasoningEffort, RunInput, RunPurpose, RunSpec,
        SecretValue,
    };
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;
    use testkit::fake_ai_cli::{
        FakeAiMode, FakeAiScenario, PreparedFakeAiCli, ReplayScenario, ReplayTurn, TurnStop,
    };

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);
    const RUN_TIMEOUT: Duration = Duration::from_secs(20);

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/tmp").join(format!("sc-ai17-{}-{seq}-{label}", std::process::id()));
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

    fn init_line() -> String {
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

    fn turn(lines: Vec<String>) -> ReplayTurn {
        ReplayTurn {
            stdout_lines: lines,
            stderr_lines: Vec::new(),
            stop_after: TurnStop::Continue,
        }
    }

    fn preflight_turn() -> ReplayTurn {
        turn(vec![
            init_line(),
            json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 0})
                .to_string(),
        ])
    }

    fn text_line(text: &str) -> String {
        json!({
            "type": "assistant",
            "message": {"model": "claude-opus-5-5", "content": [{"type": "text", "text": text}]}
        })
        .to_string()
    }

    fn result_line() -> String {
        json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 1})
            .to_string()
    }

    fn scenario(turns: Vec<ReplayTurn>) -> FakeAiScenario {
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

    fn provider(executable: PathBuf, root: &Path) -> ClaudeCodeProvider {
        let bridge = McpBridgeConfig::try_new(
            String::from("app"),
            PathBuf::from("/usr/bin/true"),
            Vec::new(),
        )
        .expect("bridge");
        let config = ClaudeCodeConfig::new(
            executable,
            root.join("work"),
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
        )
        .expect("config")
        .with_allowed_cli_versions(BTreeSet::from([String::from("2.1.322")]))
        .with_host_environment(HostEnvironment::new_clean(
            String::from("/tmp/fake-home"),
            String::from("/usr/bin:/bin"),
            String::from("testuser"),
            Some(String::from("testuser")),
            String::from("/tmp"),
        ))
        .with_mcp_bridge(bridge, SecretValue::new(b"ai17-token-value".to_vec()))
        .with_cli_projects_dir(root.join("projects"));
        ClaudeCodeProvider::new(config)
    }

    fn spec(id: &str) -> RunSpec {
        RunSpec::new(
            AiRunId::try_new(id).expect("run id"),
            RunInput::Analysis(AnalysisPrompt::try_new("check portfolio").expect("prompt")),
            ModelSelection::Default,
            ReasoningEffort::Medium,
            RunPurpose::Chat,
        )
    }

    async fn drain<S: AiEventStream>(stream: &mut S) -> Result<Vec<AiEvent>, InformationError> {
        let mut events = Vec::new();
        while let Some(event) = stream.next().await? {
            events.push(event);
        }
        Ok(events)
    }

    fn run_work_dirs(root: &Path, id: &str) -> Vec<PathBuf> {
        let prefix = format!("work-{id}-");
        let entries = match fs::read_dir(root.join("work")) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(error) => panic!("work root: {error}"),
        };
        entries
            .map(|entry| entry.expect("entry").path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(&prefix))
            })
            .collect()
    }

    const fn is_text(event: &AiEvent) -> bool {
        match event {
            AiEvent::TextDelta(_) => true,
            AiEvent::IsolationChecked(_)
            | AiEvent::ActualModel(_)
            | AiEvent::ToolCall { .. }
            | AiEvent::WebSearch { .. }
            | AiEvent::TokenUsage(_)
            | AiEvent::Quota(_)
            | AiEvent::RateLimited { .. }
            | AiEvent::ModelRerouted(_)
            | AiEvent::McpUnavailable
            | AiEvent::Unknown(_) => false,
        }
    }

    fn plant_records(root: &Path, work_dir: &Path) -> Vec<PathBuf> {
        let resolved = fs::canonicalize(work_dir).expect("real path");
        [work_dir.to_path_buf(), resolved]
            .iter()
            .map(|candidate| {
                let record = root
                    .join("projects")
                    .join(encode_claude_project_dir_name(candidate));
                fs::create_dir_all(&record).expect("record dir");
                fs::write(record.join("session.jsonl"), "{}\n").expect("record file");
                record
            })
            .collect()
    }

    async fn wait_until_no_work_dirs(root: &Path, id: &str) -> Vec<PathBuf> {
        for _attempt in 0_u32..200_u32 {
            if run_work_dirs(root, id).is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        run_work_dirs(root, id)
    }

    async fn next_text<S: AiEventStream>(stream: &mut S) {
        loop {
            let event = tokio::time::timeout(RUN_TIMEOUT, stream.next())
                .await
                .expect("event in time")
                .expect("stream")
                .expect("event");
            if is_text(&event) {
                return;
            }
        }
    }

    async fn abandon_after_first_text(provider: &ClaudeCodeProvider, root: &Path) {
        let mut stream = provider.run(spec("run-drop")).await.expect("run");
        next_text(&mut stream).await;
        assert_eq!(run_work_dirs(root, "run-drop").len(), 1);
    }

    #[test]
    fn stdin_closes_after_result_so_waiting_cli_exits() {
        run_async(async {
            let dir = TempDirGuard::new("close");
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-close",
                &scenario(vec![
                    preflight_turn(),
                    turn(vec![text_line("answer"), result_line()]),
                    turn(vec![text_line("never sent")]),
                ]),
            )
            .expect("prepare");
            let provider = provider(prepared.executable_path().to_path_buf(), dir.path());
            let mut stream = provider.run(spec("run-close")).await.expect("run");
            let events = tokio::time::timeout(RUN_TIMEOUT, drain(&mut stream))
                .await
                .expect("CLI가 다음 입력을 기다리면 결과 뒤 stdin을 닫아 끝낸다")
                .expect("stream");
            assert_eq!(
                events.first(),
                Some(&AiEvent::IsolationChecked(IsolationCheck::Passed))
            );
            assert_eq!(
                events.iter().filter(|event| is_text(event)).count(),
                1,
                "결과 뒤 턴은 오지 않는다"
            );
            assert_eq!(
                run_work_dirs(dir.path(), "run-close"),
                Vec::<PathBuf>::new()
            );
        });
    }

    #[test]
    fn run_end_removes_cli_record_folder_named_after_work_dir() {
        run_async(async {
            let dir = TempDirGuard::new("records");
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-records",
                &scenario(vec![
                    preflight_turn(),
                    turn(vec![text_line("answer"), result_line()]),
                ]),
            )
            .expect("prepare");
            let provider = provider(prepared.executable_path().to_path_buf(), dir.path());
            let mut stream = provider.run(spec("run-records")).await.expect("run");
            let first = stream.next().await.expect("first").expect("event");
            assert_eq!(first, AiEvent::IsolationChecked(IsolationCheck::Passed));
            let work_dirs = run_work_dirs(dir.path(), "run-records");
            assert_eq!(work_dirs.len(), 1, "실행 중 작업 폴더 하나");
            let record_dirs: Vec<PathBuf> = work_dirs
                .iter()
                .flat_map(|work_dir| plant_records(dir.path(), work_dir))
                .collect();
            let unrelated = dir.path().join("projects").join("-other-project");
            fs::create_dir_all(&unrelated).expect("unrelated");
            tokio::time::timeout(RUN_TIMEOUT, drain(&mut stream))
                .await
                .expect("finish")
                .expect("stream");
            for record in record_dirs {
                assert!(!record.exists(), "CLI 기록 폴더 삭제: {}", record.display());
            }
            assert!(unrelated.exists(), "다른 프로젝트 기록은 그대로");
            assert_eq!(
                run_work_dirs(dir.path(), "run-records"),
                Vec::<PathBuf>::new()
            );
        });
    }

    #[test]
    fn dropping_stream_before_result_kills_group_and_cleans_workspace() {
        run_async(async {
            let dir = TempDirGuard::new("drop");
            let prepared = PreparedFakeAiCli::prepare(
                dir.path(),
                "fake-drop",
                &scenario(vec![
                    preflight_turn(),
                    turn(vec![text_line("partial")]),
                    turn(vec![result_line()]),
                ]),
            )
            .expect("prepare");
            let provider = provider(prepared.executable_path().to_path_buf(), dir.path());
            abandon_after_first_text(&provider, dir.path()).await;
            assert_eq!(
                wait_until_no_work_dirs(dir.path(), "run-drop").await,
                Vec::<PathBuf>::new(),
                "버리면 자식이 끝나고 폴더가 지워진다"
            );
        });
    }
}
