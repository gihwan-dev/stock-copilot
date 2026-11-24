#[cfg(test)]
mod tests {
    use ai_claude_code::{
        ChatProcessReuse, ChatTurnConfig, ClaudeCodeConfig, ClaudeCodeProvider, CliRecordCleanup,
        HostEnvironment, McpBridgeConfig, PersistentChatSession, cleanup_cli_project_records,
        encode_claude_project_dir_name,
    };
    use application::{
        ChatHistory, ChatMessage, ChatRole, ModelName, ModelSelection, ReasoningEffort, SecretValue,
    };
    use domain::UntrustedText;
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use testkit::fake_ai_cli::{
        FakeAiMode, FakeAiScenario, PreparedFakeAiCli, ReplayScenario, ReplayTurn, TurnStop,
    };

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/tmp").join(format!("sc-ai04-{}-{seq}-{label}", std::process::id()));
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

    fn clean_host_env(home: &Path) -> HostEnvironment {
        HostEnvironment::new_clean(
            home.to_string_lossy().into_owned(),
            String::from("/usr/bin:/bin"),
            String::from("testuser"),
            Some(String::from("testuser")),
            String::from("/tmp"),
        )
    }

    fn init_line(model: &str) -> String {
        json!({
            "type": "system",
            "subtype": "init",
            "tools": ["mcp__app__get_portfolio"],
            "mcp_servers": [{"name": "app", "status": "connected"}],
            "model": model,
            "permissionMode": "dontAsk",
            "slash_commands": [],
            "skills": [],
            "plugins": ["cc-plugin-privacy"],
            "apiKeySource": "none",
            "claude_code_version": "2.1.322"
        })
        .to_string()
    }

    fn preflight_turn(model: &str) -> ReplayTurn {
        ReplayTurn {
            stdout_lines: vec![
                init_line(model),
                json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 0})
                    .to_string(),
            ],
            stderr_lines: Vec::new(),
            stop_after: TurnStop::Continue,
        }
    }

    fn answer_turn(init_model: &str, actual_model: &str, text: &str, stop: TurnStop) -> ReplayTurn {
        ReplayTurn {
            stdout_lines: vec![
                init_line(init_model),
                json!({
                    "type": "assistant",
                    "message": {
                        "model": actual_model,
                        "content": [{"type": "text", "text": text}]
                    }
                })
                .to_string(),
                json!({
                    "type": "result",
                    "subtype": "success",
                    "is_error": false,
                    "num_turns": 1,
                    "usage": {"input_tokens": 120, "output_tokens": 30}
                })
                .to_string(),
            ],
            stderr_lines: Vec::new(),
            stop_after: stop,
        }
    }

    fn scenario_with_turns(turns: Vec<ReplayTurn>) -> FakeAiScenario {
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

    fn has_flag_pair(argv: &[String], flag: &str, value: &str) -> bool {
        let mut iter = argv.iter().peekable();
        while let Some(current) = iter.next() {
            if current == flag && iter.peek().is_some_and(|next| next.as_str() == value) {
                return true;
            }
        }
        false
    }

    fn make_provider(dir: &Path, executable: PathBuf) -> Arc<ClaudeCodeProvider> {
        let bridge = McpBridgeConfig::try_new(
            String::from("app"),
            PathBuf::from("/usr/bin/true"),
            Vec::new(),
        )
        .expect("bridge");
        let config = ClaudeCodeConfig::new(
            executable,
            dir.join("work"),
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
        )
        .expect("config")
        .with_host_environment(clean_host_env(&dir.join("home")))
        .with_mcp_bridge(bridge, SecretValue::new(b"tok-ai04".to_vec()));
        Arc::new(ClaudeCodeProvider::new(config))
    }

    fn sample_history(user: &str, assistant: &str) -> ChatHistory {
        ChatHistory::new(vec![
            ChatMessage::new(ChatRole::User, UntrustedText::try_new(user).expect("u")),
            ChatMessage::new(
                ChatRole::Assistant,
                UntrustedText::try_new(assistant).expect("a"),
            ),
        ])
    }

    async fn send_msg(
        session: &PersistentChatSession,
        history: &ChatHistory,
        text: &str,
        cfg: ChatTurnConfig,
    ) -> ai_claude_code::ChatTurnOutput {
        let msg = UntrustedText::try_new(text).expect("msg");
        session.send_turn(history, &msg, cfg).await.expect("turn")
    }

    #[test]
    fn chat_keeps_one_live_process_across_turns_without_resume_and_records_actual_model() {
        let dir = TempDirGuard::new("reuse");
        let scenario = scenario_with_turns(vec![
            preflight_turn("claude-opus-5-5"),
            answer_turn(
                "claude-opus-5-5",
                "claude-opus-5-5",
                "first",
                TurnStop::Continue,
            ),
            answer_turn(
                "claude-opus-5-5",
                "claude-sonnet-5-5",
                "second",
                TurnStop::Continue,
            ),
        ]);
        let cli = PreparedFakeAiCli::prepare(dir.path(), "claude", &scenario).expect("cli");
        let session = PersistentChatSession::new(make_provider(
            dir.path(),
            cli.executable_path().to_path_buf(),
        ));
        let cfg = ChatTurnConfig {
            model: ModelSelection::Highest,
            effort: ReasoningEffort::High,
        };
        let empty = ChatHistory::new(Vec::new());
        let history = sample_history("hello 1", "first");
        let (out1, out2) = run_async(async {
            let t1 = send_msg(&session, &empty, "hello 1", cfg.clone()).await;
            let t2 = send_msg(&session, &history, "follow up 2", cfg).await;
            session.close().await;
            (t1, t2)
        });
        let sonnet = ModelName::try_new("claude-sonnet-5-5").expect("m");
        assert_eq!(out1.reuse, ChatProcessReuse::StartedFreshWithHistory, "t1");
        assert_eq!(out2.reuse, ChatProcessReuse::ReusedLiveProcess, "t2 reused");
        assert_eq!(out1.run_id, out2.run_id, "same live run id");
        assert_eq!(out2.actual_model, Some(sonnet.clone()), "actual model");
        assert_eq!(out2.rerouted_models, vec![sonnet], "reroute recorded");
        let invocations = cli.read_invocations().expect("invocations");
        assert_eq!(invocations.len(), 1_usize, "only one process spawned");
        let only = invocations.first().expect("only");
        assert!(
            only.argv
                .iter()
                .any(|arg| arg == "--no-session-persistence"),
            "no persistence"
        );
        assert!(
            !only.argv.iter().any(|arg| arg == "--resume"),
            "never uses --resume"
        );
        assert_eq!(only.stdin_lines.len(), 3_usize, "/usage + turn1 + turn2");
        let t2_stdin = only.stdin_lines.get(2_usize).expect("t2");
        assert!(
            !t2_stdin.contains("<conversation_history>"),
            "live process gets only new msg"
        );
    }

    #[test]
    fn process_exit_and_model_change_restart_and_reinject_stored_history() {
        let dir = TempDirGuard::new("restart");
        let scenario = scenario_with_turns(vec![
            preflight_turn("claude-opus-5-5"),
            answer_turn(
                "claude-opus-5-5",
                "claude-opus-5-5",
                "reply before exit",
                TurnStop::Terminate,
            ),
        ]);
        let cli = PreparedFakeAiCli::prepare(dir.path(), "claude", &scenario).expect("cli");
        let session = PersistentChatSession::new(make_provider(
            dir.path(),
            cli.executable_path().to_path_buf(),
        ));
        let cfg1 = ChatTurnConfig {
            model: ModelSelection::Highest,
            effort: ReasoningEffort::Medium,
        };
        let cfg2 = ChatTurnConfig {
            model: ModelSelection::Fast,
            effort: ReasoningEffort::Low,
        };
        let empty = ChatHistory::new(Vec::new());
        let history = sample_history("prior question", "reply before exit");
        let (out1, out2, out3) = run_async(async {
            let t1 = send_msg(&session, &empty, "prior question", cfg1.clone()).await;
            let t2 = send_msg(&session, &history, "after crash", cfg1).await;
            let t3 = send_msg(&session, &history, "with fast model", cfg2).await;
            session.close().await;
            (t1, t2, t3)
        });
        assert_eq!(
            [out1.reuse, out2.reuse, out3.reuse],
            [ChatProcessReuse::StartedFreshWithHistory; 3],
            "initial, post-exit, and model-change turns all start fresh"
        );
        let invocations = cli.read_invocations().expect("invocations");
        assert_eq!(invocations.len(), 3_usize, "3 separate processes spawned");
        let second_prompt = &invocations.get(1_usize).expect("s").stdin_lines[1];
        assert!(
            second_prompt.contains("<conversation_history>")
                && second_prompt.contains("prior question")
                && second_prompt.contains("reply before exit")
                && second_prompt.contains("after crash"),
            "app history re-injected into fresh process"
        );
        let third_argv = &invocations.get(2_usize).expect("t").argv;
        assert!(
            has_flag_pair(third_argv, "--model", "claude-haiku-5-5")
                && has_flag_pair(third_argv, "--effort", "low"),
            "new model and effort flags applied on restart"
        );
    }

    fn write_record_creating_fake_cli(path: &Path, projects_root: &Path) {
        let init_json = init_line("claude-opus-5-5");
        let script = format!(
            "#!/bin/sh\n\
            ENC=$(printf '%s' \"$PWD\" | tr -c 'A-Za-z0-9' '-')\n\
            mkdir -p \"{root}/$ENC/memory\"\n\
            printf 'plain' > \"{root}/$ENC/memory/session.jsonl\"\n\
            IFS= read -r _line1\n\
            printf '%s\\n' '{init}'\n\
            printf '{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":0}}\\n'\n\
            IFS= read -r _line2\n\
            printf '%s\\n' '{init}'\n\
            printf '{{\"type\":\"assistant\",\"message\":{{\"model\":\"claude-opus-5-5\",\"content\":[{{\"type\":\"text\",\"text\":\"ok\"}}]}}}}\\n'\n\
            printf '{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1}}\\n'\n\
            IFS= read -r _wait\n",
            root = projects_root.display(),
            init = init_json,
        );
        fs::write(path, script).expect("write script");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700_u32)).expect("chmod");
    }

    #[test]
    fn cli_project_records_are_cleaned_up_on_close_and_symlinks_rejected() {
        let dir = TempDirGuard::new("records");
        let projects_root = dir.path().join("home").join(".claude").join("projects");
        fs::create_dir_all(&projects_root).expect("mkdir projects");
        let fake_cli = dir.path().join("claude");
        write_record_creating_fake_cli(&fake_cli, &projects_root);

        let session = PersistentChatSession::new(make_provider(dir.path(), fake_cli))
            .with_cli_projects_dir(projects_root.clone());
        run_async(async {
            let _turn = session
                .send_turn(
                    &ChatHistory::new(Vec::new()),
                    &UntrustedText::try_new("check cleanup").expect("msg"),
                    ChatTurnConfig {
                        model: ModelSelection::Default,
                        effort: ReasoningEffort::Default,
                    },
                )
                .await
                .expect("turn ok");
            session.close().await;
        });
        let remaining: Vec<_> = fs::read_dir(&projects_root)
            .expect("read projects")
            .flatten()
            .collect();
        assert!(
            remaining.is_empty(),
            "zero CLI record files remain after close"
        );

        let victim = dir.path().join("victim-dir");
        fs::create_dir_all(&victim).expect("mkdir victim");
        let work_path = PathBuf::from("/tmp/fake/work");
        let encoded = encode_claude_project_dir_name(&work_path);
        assert_eq!(
            encoded, "-tmp-fake-work",
            "non-alphanumeric replaced with -"
        );
        symlink(&victim, projects_root.join(&encoded)).expect("symlink");
        assert_eq!(
            cleanup_cli_project_records(&projects_root, &[work_path]),
            CliRecordCleanup::RejectedUnsafePath,
            "symlink target must never be deleted"
        );
        assert!(victim.exists(), "victim directory preserved");
    }
}
