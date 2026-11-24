#[cfg(test)]
mod tests {
    use ai_claude_code::{
        AiRunnerQueue, BudgetSummaryKind, ChatSlotAdmission, ChatSlotGrant, ClaudeCodeConfig,
        ClaudeCodeProvider, HostEnvironment, McpBridgeConfig, QuotaDecision, QuotaTransition,
        RunCredentials, ScheduledReleaseOutcome, ScheduledResumeKind, ScheduledSlotAdmission,
        ScheduledSlotGrant, utilization_from_percent,
    };
    use application::{
        AiEvent, AiEventStream, AiProvider, AiRunId, AiUsage, AnalysisPrompt, BackgroundPreemption,
        ChatQueueAcquisition, ChatQueuePort, Clock, EventBus, EventBusBuilder, EventSubscriber,
        ModelSelection, ReasoningEffort, ResetTime, RunInput, RunPurpose, RunSpec, SecretValue,
        SubscriberFailure,
    };
    use domain::{
        DomainEvent, DomainEventKind, Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp,
        UnixSeconds,
    };
    use std::collections::BTreeSet;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use testkit_ports::SteppedClock;

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(1_u64);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = TEMP_SEQ.fetch_add(1_u64, Ordering::Relaxed);
            let pid = std::process::id();
            let path = PathBuf::from(format!("/tmp/sc-ai06-{label}-{pid}-{seq}"));
            let _removed = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("mkdir temp");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _removed = fs::remove_dir_all(&self.path);
        }
    }

    struct BudgetEventCounter {
        count: Arc<AtomicUsize>,
    }

    impl EventSubscriber for BudgetEventCounter {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            if *event == DomainEvent::AiBudgetChanged {
                self.count.fetch_add(1_usize, Ordering::SeqCst);
            }
            Ok(())
        }
    }

    fn make_budget_bus() -> (Arc<EventBus>, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0_usize));
        let mut builder = EventBusBuilder::default();
        let _id = builder.subscribe(
            BTreeSet::from([DomainEventKind::AiBudgetChanged]),
            BudgetEventCounter {
                count: Arc::clone(&count),
            },
        );
        (Arc::new(builder.build()), count)
    }

    fn ts(secs: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(secs),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(0_i32),
        )
        .expect("ts")
    }

    fn span_secs(secs: u64) -> TimeSpan {
        TimeSpan::try_new(SpanSeconds::new(secs), Nanosecond::new(0_u32)).expect("span")
    }

    fn dyn_clock(clock: &Arc<SteppedClock>) -> Arc<dyn Clock> {
        to_dyn_clock(Arc::clone(clock))
    }

    fn to_dyn_clock(clock: Arc<SteppedClock>) -> Arc<dyn Clock> {
        clock
    }

    fn run_async<F: Future>(future: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt")
            .block_on(future)
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum AdmissionKind {
        Admitted,
        PausedForChat,
        Busy,
        BudgetReached,
        RateLimited,
    }

    const fn admission_kind(adm: &ScheduledSlotAdmission) -> AdmissionKind {
        match adm {
            ScheduledSlotAdmission::Admitted(_) => AdmissionKind::Admitted,
            ScheduledSlotAdmission::PausedForChat => AdmissionKind::PausedForChat,
            ScheduledSlotAdmission::Busy => AdmissionKind::Busy,
            ScheduledSlotAdmission::BudgetReached { .. } => AdmissionKind::BudgetReached,
            ScheduledSlotAdmission::RateLimited { .. } => AdmissionKind::RateLimited,
        }
    }

    fn expect_admitted(adm: ScheduledSlotAdmission) -> ScheduledSlotGrant {
        match adm {
            ScheduledSlotAdmission::Admitted(grant) => grant,
            ScheduledSlotAdmission::PausedForChat
            | ScheduledSlotAdmission::Busy
            | ScheduledSlotAdmission::BudgetReached { .. }
            | ScheduledSlotAdmission::RateLimited { .. } => {
                panic!("expected Admitted, got {adm:?}")
            }
        }
    }

    fn expect_chat_acquired(adm: ChatSlotAdmission) -> ChatSlotGrant {
        match adm {
            ChatSlotAdmission::Acquired(grant) => grant,
            ChatSlotAdmission::Busy => panic!("expected Acquired"),
        }
    }

    fn write_sigint_aware_cli(path: &Path, marker_file: &Path) {
        let init = r#"{"type":"system","subtype":"init","tools":["mcp__stock-copilot__get_portfolio"],"mcp_servers":[{"name":"stock-copilot","status":"connected"}],"plugins":[],"skills":[],"slash_commands":[],"permissionMode":"dontAsk","apiKeySource":"none","claude_code_version":"2.1.120"}"#;
        let script = format!(
            "#!/bin/sh\n\
            trap 'printf \"INT\" > \"{marker}\"; printf \"{result}\\n\"; exit 0' INT\n\
            IFS= read -r _line1\n\
            printf '%s\\n' '{init}'\n\
            printf '{{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":0}}\\n'\n\
            IFS= read -r _line2\n\
            printf '{{\"type\":\"assistant\",\"message\":{{\"model\":\"claude-opus-5-5\",\"content\":[{{\"type\":\"text\",\"text\":\"step1\"}}]}}}}\\n'\n\
            while [ ! -f \"{marker}\" ]; do\n\
                /bin/sleep 0.05\n\
            done\n",
            marker = marker_file.display(),
            init = init,
            result = r#"{\"type\":\"result\",\"subtype\":\"success\",\"is_error\":false,\"num_turns\":1}"#,
        );
        fs::write(path, script).expect("write cli");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700_u32)).expect("chmod");
    }

    fn make_provider(dir: &Path, cli: PathBuf) -> Arc<ClaudeCodeProvider> {
        let work_root = dir.join("work");
        fs::create_dir_all(&work_root).expect("work");
        let bridge = McpBridgeConfig::try_new(
            String::from("stock-copilot"),
            PathBuf::from("/usr/local/bin/mcp-bridge"),
            vec![
                String::from("--socket"),
                dir.join("mcp.sock").to_string_lossy().into_owned(),
            ],
        )
        .expect("bridge");
        let env = HostEnvironment::from_pairs([
            ("HOME", dir.join("home").to_string_lossy().into_owned()),
            ("PATH", String::from("/usr/bin:/bin")),
            ("USER", String::from("tester")),
            ("TMPDIR", String::from("/tmp")),
        ]);
        let config = ClaudeCodeConfig::new(
            cli,
            work_root,
            String::from("stock-copilot"),
            BTreeSet::from([String::from("mcp__stock-copilot__get_portfolio")]),
            BTreeSet::new(),
        )
        .expect("cfg")
        .with_host_environment(env)
        .with_mcp_bridge(bridge, SecretValue::new(b"tok-ai06".to_vec()));
        Arc::new(ClaudeCodeProvider::new(config))
    }

    fn briefing_spec(run_id: AiRunId) -> RunSpec {
        RunSpec::new(
            run_id,
            RunInput::Analysis(AnalysisPrompt::try_new("run briefing").expect("p")),
            ModelSelection::Default,
            ReasoningEffort::Default,
            RunPurpose::MorningBriefing,
        )
    }

    #[test]
    fn chat_preempts_running_scheduled_job_with_sigint_and_resumes_after_chat() {
        let dir = TempDirGuard::new("sigint");
        let marker = dir.path().join("sigint.flag");
        let cli = dir.path().join("claude");
        write_sigint_aware_cli(&cli, &marker);
        let provider = make_provider(dir.path(), cli);
        let clock = Arc::new(SteppedClock::new(ts(1_700_000_000)));
        let (bus, _events) = make_budget_bus();
        let queue =
            AiRunnerQueue::with_default_policy(Arc::clone(&provider), dyn_clock(&clock), bus)
                .expect("q");
        run_async(async {
            let first = expect_admitted(
                queue
                    .request_scheduled_slot("morning-briefing")
                    .await
                    .expect("s1"),
            );
            assert_eq!(first.resume_kind, ScheduledResumeKind::Fresh, "first fresh");
            let mut stream = provider
                .run(briefing_spec(first.credentials.run_id().clone()))
                .await
                .expect("run");
            let _init = stream.next().await.expect("init");
            let _delta = stream.next().await.expect("step1");
            let chat = expect_chat_acquired(queue.acquire_chat_slot().await.expect("chat"));
            assert_eq!(
                chat.preempted_background,
                BackgroundPreemption::SignaledPause
            );
            assert_eq!(queue.snapshot().await.active_slot_count, 2_usize, "1+1");
            while let Some(_evt) = stream.next().await.expect("drain") {}
            assert!(marker.exists(), "scheduled child received real SIGINT");
            let _rel = queue
                .release_scheduled_slot(
                    first.credentials.run_id(),
                    ScheduledReleaseOutcome::PausedAtStepBoundary,
                )
                .await
                .expect("release sched");
            let blocked = queue
                .request_scheduled_slot("morning-briefing")
                .await
                .expect("b");
            assert_eq!(admission_kind(&blocked), AdmissionKind::PausedForChat);
            let _rc = queue
                .release_chat_slot(chat.credentials.run_id())
                .await
                .expect("rc");
            let resumed = expect_admitted(
                queue
                    .request_scheduled_slot("morning-briefing")
                    .await
                    .expect("r"),
            );
            assert_eq!(resumed.resume_kind, ScheduledResumeKind::ResumedAfterChat);
            assert_ne!(resumed.credentials.run_id(), first.credentials.run_id());
        });
    }

    #[test]
    fn scheduled_cap_70_percent_blocks_scheduled_allows_chat_and_dedups_budget_events() {
        let dir = TempDirGuard::new("budget-cap");
        let provider = make_provider(dir.path(), dir.path().join("unused"));
        let clock = Arc::new(SteppedClock::new(ts(1_700_000_000)));
        let (bus, budget_events) = make_budget_bus();
        let queue =
            AiRunnerQueue::with_default_policy(provider, dyn_clock(&clock), bus).expect("q");
        run_async(async {
            let below = AiUsage::Available(utilization_from_percent(65_u8).expect("65"));
            assert_eq!(queue.observe_usage(below).await, QuotaTransition::Unchanged);
            assert_eq!(budget_events.load(Ordering::SeqCst), 0_usize);
            let at_cap = AiUsage::Available(utilization_from_percent(70_u8).expect("70"));
            assert_eq!(queue.observe_usage(at_cap).await, QuotaTransition::Changed);
            assert_eq!(budget_events.load(Ordering::SeqCst), 1_usize);
            let higher = AiUsage::Available(utilization_from_percent(82_u8).expect("82"));
            assert_eq!(
                queue.observe_usage(higher).await,
                QuotaTransition::Unchanged
            );
            assert_eq!(budget_events.load(Ordering::SeqCst), 1_usize);
            let blocked = queue.request_scheduled_slot("est").await.expect("cap");
            assert_eq!(admission_kind(&blocked), AdmissionKind::BudgetReached);
            assert_eq!(queue.evaluate_chat_quota().await, QuotaDecision::Allowed);
            let chat = expect_chat_acquired(queue.acquire_chat_slot().await.expect("chat ok"));
            let _rel = queue.release_chat_slot(chat.credentials.run_id()).await;
            assert_eq!(
                queue.set_scheduled_cap_percent(90_u8).await.expect("cap90"),
                QuotaTransition::Changed
            );
            assert_eq!(budget_events.load(Ordering::SeqCst), 2_usize);
            let _ok = expect_admitted(queue.request_scheduled_slot("est").await.expect("ok"));
        });
    }

    #[test]
    fn rate_limit_blocks_until_reset_and_emits_budget_event_on_resume() {
        let dir = TempDirGuard::new("budget-rl");
        let provider = make_provider(dir.path(), dir.path().join("unused"));
        let clock = Arc::new(SteppedClock::new(ts(1_700_000_000)));
        let (bus, budget_events) = make_budget_bus();
        let queue =
            AiRunnerQueue::with_default_policy(provider, dyn_clock(&clock), bus).expect("q");
        run_async(async {
            let rl_evt = AiEvent::RateLimited {
                resets_at: ResetTime::Known(ts(1_700_000_600)),
            };
            assert_eq!(
                queue.observe_ai_event(&rl_evt).await,
                QuotaTransition::Changed
            );
            assert_eq!(budget_events.load(Ordering::SeqCst), 1_usize);
            assert_eq!(
                queue.observe_ai_event(&rl_evt).await,
                QuotaTransition::Unchanged
            );
            assert_eq!(budget_events.load(Ordering::SeqCst), 1_usize);
            let rl_blocked = queue.request_scheduled_slot("est").await.expect("rl");
            assert_eq!(admission_kind(&rl_blocked), AdmissionKind::RateLimited);
            let _advanced = clock.advance_by(span_secs(600_u64)).expect("advance");
            assert_eq!(queue.refresh_budget().await, QuotaTransition::Changed);
            assert_eq!(budget_events.load(Ordering::SeqCst), 2_usize, "resumed");
            assert_eq!(
                queue.snapshot().await.budget_summary,
                BudgetSummaryKind::Normal
            );
            let _ok = expect_admitted(queue.request_scheduled_slot("est").await.expect("ok"));
        });
    }

    #[test]
    fn concurrency_cap_chat_queue_port_and_unique_credentials() {
        let dir = TempDirGuard::new("creds");
        let provider = make_provider(dir.path(), dir.path().join("unused"));
        let clock = Arc::new(SteppedClock::new(ts(1_700_000_000)));
        let (bus, _events) = make_budget_bus();
        let queue =
            AiRunnerQueue::with_default_policy(provider, dyn_clock(&clock), bus).expect("q");
        run_async(async {
            let r1 = AiRunId::try_new("chat-port-1").expect("r1");
            let r2 = AiRunId::try_new("chat-port-2").expect("r2");
            assert_eq!(
                queue.acquire_chat(&r1).await.expect("acq1"),
                ChatQueueAcquisition::Acquired {
                    preempted_background: BackgroundPreemption::NoneRunning
                }
            );
            assert_eq!(
                queue.acquire_chat(&r2).await.expect("acq2"),
                ChatQueueAcquisition::Busy
            );
            let _rel_chat = queue.release_chat(&r1).await.expect("rel1");
            let s1 = expect_admitted(queue.request_scheduled_slot("j1").await.expect("s1"));
            let s2 = queue.request_scheduled_slot("j2").await.expect("s2");
            assert_eq!(admission_kind(&s2), AdmissionKind::Busy);
            let _rel_sched = queue
                .release_scheduled_slot(s1.credentials.run_id(), ScheduledReleaseOutcome::Completed)
                .await
                .expect("rel s1");
        });
        let c1 = RunCredentials::issue("run").expect("c1");
        let c2 = RunCredentials::issue("run").expect("c2");
        assert_ne!(c1.run_id(), c2.run_id(), "unique run id");
        assert_ne!(c1.session_id(), c2.session_id(), "unique session id");
        assert_ne!(c1.run_id(), c1.session_id(), "distinct session id");
        let t1 = c1.token().as_exposed_secret();
        let t2 = c2.token().as_exposed_secret();
        assert_eq!(t1.len(), 64_usize, "256-bit hex token");
        assert_eq!(t2.len(), 64_usize, "256-bit hex token");
        assert_ne!(t1, t2, "unique random tokens");
        assert!(!format!("{c1:?}").contains(std::str::from_utf8(t1).expect("utf8")));
    }
}
