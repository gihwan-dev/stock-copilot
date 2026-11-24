#[cfg(test)]
mod tests {
    use ai_claude_code::{
        BudgetSummaryKind, ClaudeCodeConfig, ClaudeCodeProvider, DEFAULT_PROBE_TIMEOUT_MS,
        DiscoveryConfig, HostEnvironment, LoginCheckOutcome, McpBridgeConfig, QuotaDecision,
        QuotaPolicy, QuotaTracker, QuotaTransition, ResetSource, StaticPathResolver, WorkCategory,
        check_login_status_with_timeout, extract_marked_shell_path, run_claude_startup_check,
        run_claude_startup_check_with_timeout, utilization_from_percent,
    };
    use application::{
        AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, AnalysisPrompt, CheckState, Clock,
        ModelSelection, ReasoningEffort, ResetTime, RetryDelay, RunInput, RunPurpose, RunSpec,
        SecretValue,
    };
    use domain::{Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds};
    use std::collections::BTreeSet;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use testkit::fake_ai_cli::{
        FakeAiMode, FakeAiScenario, PreparedFakeAiCli, ReplayScenario, ReplayTurn, TurnStop,
    };
    use testkit_ports::SteppedClock;

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/tmp").join(format!("sc-ai05-{}-{seq}-{label}", std::process::id()));
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

    fn ts(unix_seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(unix_seconds),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("timestamp")
    }

    fn span(seconds: u64) -> TimeSpan {
        TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(0_u32)).expect("span")
    }

    fn run_async<F: Future>(future: F) -> F::Output {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        runtime.block_on(future)
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

    fn clean_host_env() -> HostEnvironment {
        HostEnvironment::new_clean(
            String::from("/tmp/fake-home"),
            String::from("/usr/bin:/bin"),
            String::from("testuser"),
            Some(String::from("testuser")),
            String::from("/tmp"),
        )
    }

    fn write_executable(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        fs::write(path, body).expect("write script");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700_u32)).expect("chmod");
    }

    fn make_rate_limit_provider(
        dir: &Path,
        clock: &Arc<SteppedClock>,
    ) -> (PreparedFakeAiCli, ClaudeCodeProvider) {
        let lines = fixture_lines("claude-rate-limited.jsonl");
        let init_line = lines.first().cloned().expect("init line");
        let rest: Vec<String> = lines.into_iter().skip(1_usize).collect();
        let scenario = FakeAiScenario {
            mode: FakeAiMode::Replay(ReplayScenario {
                immediate_lines: Vec::new(),
                turns: vec![
                    ReplayTurn {
                        stdout_lines: vec![
                            init_line,
                            String::from(
                                "{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":0}",
                            ),
                        ],
                        stderr_lines: Vec::new(),
                        stop_after: TurnStop::Continue,
                    },
                    ReplayTurn {
                        stdout_lines: rest,
                        stderr_lines: Vec::new(),
                        stop_after: TurnStop::Terminate,
                    },
                ],
                final_stderr_lines: Vec::new(),
                trailing_without_newline: None,
                exit_code: 1_u8,
            }),
            capture_path: None,
        };
        let cli = PreparedFakeAiCli::prepare(dir, "claude", &scenario).expect("prepare cli");
        let bridge = McpBridgeConfig::try_new(
            String::from("app"),
            PathBuf::from("/usr/bin/true"),
            Vec::new(),
        )
        .expect("bridge");
        let policy = QuotaPolicy::default_policy().expect("default policy");
        let config = ClaudeCodeConfig::new(
            cli.executable_path().to_path_buf(),
            dir.join("work"),
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::from([String::from("cc-plugin-privacy")]),
        )
        .expect("config")
        .with_host_environment(clean_host_env())
        .with_mcp_bridge(bridge, SecretValue::new(b"tok-ai05".to_vec()));
        let cloned = Arc::clone(clock);
        let clock_dyn: Arc<dyn Clock> = cloned;
        let provider = ClaudeCodeProvider::new(config).with_clock_and_quota(clock_dyn, policy);
        (cli, provider)
    }

    #[test]
    fn rate_limit_fixture_enters_waiting_state_without_error_and_resumes_at_resets_at() {
        let dir = TempDirGuard::new("fixture-resume");
        let clock = Arc::new(SteppedClock::new(ts(1_794_394_000_i64)));
        let (_cli, provider) = make_rate_limit_provider(dir.path(), &clock);
        let spec = RunSpec::new(
            AiRunId::try_new("ai05-rate-limit").expect("id"),
            RunInput::Analysis(AnalysisPrompt::try_new("portfolio check").expect("prompt")),
            ModelSelection::Default,
            ReasoningEffort::Default,
            RunPurpose::MorningBriefing,
        );
        let events = run_async(async {
            let mut stream = provider.run(spec).await.expect("spawn stream");
            let mut collected = Vec::new();
            while let Some(event) = stream.next().await.expect("rate limit must not error") {
                collected.push(event);
            }
            collected
        });
        let reset_ts = ts(1_794_394_800_i64);
        assert!(
            events.contains(&AiEvent::RateLimited {
                resets_at: ResetTime::Known(reset_ts),
            }),
            "fixture emits RateLimited with known resetsAt"
        );
        assert_eq!(
            run_async(provider.usage()).expect("usage before reset"),
            AiUsage::Waiting(ResetTime::Known(reset_ts)),
            "provider reports Waiting(Known(resetsAt)) before reset"
        );
        let mut tracker = run_async(provider.quota_tracker()).expect("tracker present");
        let expected_wait = QuotaDecision::WaitRateLimit {
            resumes_at: reset_ts,
            source: ResetSource::ProviderReset,
        };
        for category in [WorkCategory::Scheduled, WorkCategory::Chat] {
            assert_eq!(
                tracker.evaluate_admission(category, clock.now()),
                expected_wait,
                "both categories wait during hard rate limit"
            );
        }
        clock.advance_by(span(800_u64)).expect("advance to reset");
        assert_eq!(
            run_async(provider.usage()).expect("usage at reset"),
            AiUsage::Unknown,
            "waiting state clears automatically at resetsAt"
        );
        assert_eq!(
            tracker.refresh(clock.now()),
            QuotaTransition::Changed,
            "refresh reports transition back to normal"
        );
        assert_eq!(
            tracker.evaluate_admission(WorkCategory::Scheduled, clock.now()),
            QuotaDecision::Allowed,
            "scheduled work resumes at exact resetsAt"
        );
    }

    #[test]
    fn expired_known_reset_is_not_retained_and_unknown_reset_uses_fallback_cooldown() {
        let policy = QuotaPolicy::from_percent_and_cooldown_seconds(70_u8, 120_u64).expect("pol");
        let mut tracker = QuotaTracker::new(policy);

        assert_eq!(
            tracker.observe_rate_limited(ResetTime::Known(ts(1_000_i64)), ts(900_i64)),
            QuotaTransition::Changed,
            "enters waiting"
        );
        assert_eq!(
            tracker.observe_rate_limited(ResetTime::NotProvided, ts(905_i64)),
            QuotaTransition::Unchanged,
            "synthetic follow-up preserves active future provider reset"
        );
        assert_eq!(
            tracker.resumes_at(ts(905_i64)),
            Some(ts(1_000_i64)),
            "keeps provider resetsAt while still in future"
        );

        assert_eq!(
            tracker.observe_rate_limited(ResetTime::NotProvided, ts(1_500_i64)),
            QuotaTransition::Unchanged,
            "after previous window expired, new unknown rate limit applies fresh fallback"
        );
        assert_eq!(
            tracker.evaluate_admission(WorkCategory::Scheduled, ts(1_500_i64)),
            QuotaDecision::WaitRateLimit {
                resumes_at: ts(1_620_i64),
                source: ResetSource::FallbackCooldown,
            },
            "never keeps expired timestamp 1000; uses fresh cooldown 1500+120"
        );
        assert_eq!(
            tracker.evaluate_admission(WorkCategory::Scheduled, ts(1_620_i64)),
            QuotaDecision::Allowed,
            "fallback cooldown resumes at exact boundary"
        );
        assert_eq!(
            tracker.observe_rate_limited(ResetTime::Known(ts(1_600_i64)), ts(1_700_i64)),
            QuotaTransition::Changed,
            "stale past resetsAt from live rejection falls back to cooldown instead of spinning"
        );
        assert_eq!(
            tracker.evaluate_admission(WorkCategory::Chat, ts(1_700_i64)),
            QuotaDecision::WaitRateLimit {
                resumes_at: ts(1_820_i64),
                source: ResetSource::FallbackCooldown,
            },
            "stale past resetsAt uses fallback cooldown"
        );
    }

    #[test]
    fn scheduled_cap_blocks_scheduled_at_70_percent_while_allowing_chat() {
        let policy = QuotaPolicy::default_policy().expect("default");
        let mut tracker = QuotaTracker::new(policy);

        let u69 = utilization_from_percent(69_u8).expect("69%");
        assert_eq!(
            tracker.observe_usage(AiUsage::Available(u69), ts(2_000_i64)),
            QuotaTransition::Unchanged,
            "69% stays in normal budget state"
        );
        assert_eq!(
            tracker.evaluate_admission(WorkCategory::Scheduled, ts(2_000_i64)),
            QuotaDecision::Allowed,
            "69% allows scheduled work"
        );

        let u70 = utilization_from_percent(70_u8).expect("70%");
        assert_eq!(
            tracker.observe_usage(AiUsage::Available(u70), ts(2_010_i64)),
            QuotaTransition::Changed,
            "crossing 70% triggers budget transition"
        );
        assert_eq!(
            tracker.budget_summary(ts(2_010_i64)),
            BudgetSummaryKind::ScheduledCapReached,
            "summary reflects cap reached"
        );
        assert_eq!(
            tracker.observe_usage(AiUsage::Available(u70), ts(2_020_i64)),
            QuotaTransition::Unchanged,
            "repeated 70% observation is deduplicated"
        );
        assert_eq!(
            tracker.evaluate_admission(WorkCategory::Scheduled, ts(2_020_i64)),
            QuotaDecision::WaitScheduledBudget {
                utilization: u70,
                cap: policy.scheduled_cap(),
            },
            "scheduled blocked at exact 70% boundary"
        );
        assert_eq!(
            tracker.evaluate_admission(WorkCategory::Chat, ts(2_020_i64)),
            QuotaDecision::Allowed,
            "chat remains allowed at 70% utilization"
        );
    }

    #[test]
    fn discovery_probe_timeout_grandchild_and_async_context_regressions() {
        const {
            assert!(
                DEFAULT_PROBE_TIMEOUT_MS >= 10_000_u64,
                "default discovery probe timeout gives slow CLI startup room"
            );
        }
        let dir = TempDirGuard::new("disc-reg");
        let slow_cli = dir.path().join("slow-claude");
        write_executable(&slow_cli, "#!/bin/sh\nexec /bin/sleep 5\n");
        let env = clean_host_env();
        assert_eq!(
            check_login_status_with_timeout(&slow_cli, &dir.path().join("w1"), &env, 150_u64),
            LoginCheckOutcome::TimedOut,
            "slow auth status returns TimedOut instead of CheckUnavailable"
        );
        let cfg_slow = DiscoveryConfig::new(env.clone()).with_explicit_override(slow_cli);
        let report_slow = run_claude_startup_check_with_timeout(
            &cfg_slow,
            &StaticPathResolver::new(None),
            &dir.path().join("w2"),
            150_u64,
        );
        assert_eq!(
            report_slow.claude_login,
            CheckState::Waiting(RetryDelay::Unknown),
            "timeout maps to Waiting, not Bug"
        );

        let grandchild_cli = dir.path().join("grandchild-claude");
        write_executable(
            &grandchild_cli,
            "#!/bin/sh\n(/bin/sleep 30) &\nprintf '{\"loggedIn\":true,\"apiKeySource\":\"none\"}\\n'\nexit 0\n",
        );
        let cfg_gc = DiscoveryConfig::new(env).with_explicit_override(grandchild_cli);
        let report_in_async = run_async(async {
            run_claude_startup_check(
                &cfg_gc,
                &StaticPathResolver::new(None),
                &dir.path().join("w3"),
            )
        });
        assert_eq!(
            report_in_async.claude_login,
            CheckState::Healthy,
            "grandchild holding stdout is killed and sync check never panics inside Tokio runtime"
        );
    }

    #[test]
    fn marked_shell_path_extraction_ignores_dotfile_noise() {
        let noisy = "welcome banner\n__SC_LOGIN_PATH_BEGIN__/opt/homebrew/bin:/usr/bin__SC_LOGIN_PATH_END__\ntrailing line\n";
        assert_eq!(
            extract_marked_shell_path(noisy),
            Some("/opt/homebrew/bin:/usr/bin"),
            "extracts exact PATH between markers"
        );
        assert_eq!(
            extract_marked_shell_path("only noise without markers"),
            None,
            "returns None without markers"
        );
    }
}
