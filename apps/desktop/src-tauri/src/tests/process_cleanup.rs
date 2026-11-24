use crate::database::DatabaseState;
use crate::lifecycle::Lifecycle;
use crate::platform::process_cleanup::{
    AuditDrainStatus, AuditDrainTracker, ChildProcessManager, CleanupFuture, CleanupReport,
    CommandProcessSignaler, InMemoryProcessLedger, ProcessCleanupError, ProcessGroupId,
    ProcessGroupRecord, ProcessLedgerPort, ProcessLiveness, ProcessRunId, ProcessRuntime,
    ProcessSignaler, SignalOutcome, TerminationSignal,
};
use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tauri::Manager;
use tauri::test::{mock_builder, mock_context, noop_assets};
use testkit_ports::memory::MemoryReply;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FakeProcessBehavior {
    ExitsOnTerm,
    IgnoresTermRequiresKill,
    AlreadyGone,
}

#[derive(Debug, Default)]
struct ScriptedSignaler {
    behaviors: RwLock<BTreeMap<ProcessGroupId, FakeProcessBehavior>>,
    alive: RwLock<BTreeMap<ProcessGroupId, ProcessLiveness>>,
    events: RwLock<Vec<(ProcessGroupId, TerminationSignal)>>,
}

impl ScriptedSignaler {
    fn register_process(&self, pgid: ProcessGroupId, behavior: FakeProcessBehavior) {
        self.behaviors
            .write()
            .expect("동작 등록")
            .insert(pgid, behavior);
        let initial = match behavior {
            FakeProcessBehavior::AlreadyGone => ProcessLiveness::Exited,
            FakeProcessBehavior::ExitsOnTerm | FakeProcessBehavior::IgnoresTermRequiresKill => {
                ProcessLiveness::Alive
            }
        };
        self.alive.write().expect("상태 등록").insert(pgid, initial);
    }

    fn events(&self) -> Vec<(ProcessGroupId, TerminationSignal)> {
        self.events.read().expect("시그널 이력").clone()
    }
}

impl ProcessSignaler for ScriptedSignaler {
    fn send_signal(
        &self,
        pgid: ProcessGroupId,
        signal: TerminationSignal,
    ) -> CleanupFuture<'_, Result<SignalOutcome, ProcessCleanupError>> {
        let result = (|| {
            let mut events = self
                .events
                .write()
                .map_err(|_error| ProcessCleanupError::SignalFailed(String::from("lock")))?;
            events.push((pgid, signal));
            let behavior = self
                .behaviors
                .read()
                .map_err(|_error| ProcessCleanupError::SignalFailed(String::from("lock")))?
                .get(&pgid)
                .copied()
                .unwrap_or(FakeProcessBehavior::AlreadyGone);
            let mut alive = self
                .alive
                .write()
                .map_err(|_error| ProcessCleanupError::SignalFailed(String::from("lock")))?;
            match (behavior, signal) {
                (FakeProcessBehavior::AlreadyGone, _) => Ok(SignalOutcome::AlreadyExited),
                (FakeProcessBehavior::ExitsOnTerm, TerminationSignal::Term)
                | (FakeProcessBehavior::IgnoresTermRequiresKill, TerminationSignal::Kill) => {
                    alive.insert(pgid, ProcessLiveness::Exited);
                    Ok(SignalOutcome::Signalled)
                }
                (FakeProcessBehavior::ExitsOnTerm, TerminationSignal::Kill) => {
                    Ok(SignalOutcome::AlreadyExited)
                }
                (FakeProcessBehavior::IgnoresTermRequiresKill, TerminationSignal::Term) => {
                    Ok(SignalOutcome::Signalled)
                }
            }
        })();
        Box::pin(std::future::ready(result))
    }

    fn probe(
        &self,
        pgid: ProcessGroupId,
    ) -> CleanupFuture<'_, Result<ProcessLiveness, ProcessCleanupError>> {
        let result = self
            .alive
            .read()
            .map_err(|_error| ProcessCleanupError::SignalFailed(String::from("lock")))
            .map(|table| table.get(&pgid).copied().unwrap_or(ProcessLiveness::Exited));
        Box::pin(std::future::ready(result))
    }
}

fn pgid(raw: u32) -> ProcessGroupId {
    ProcessGroupId::try_new(raw).expect("유효한 PGID")
}

fn record(run: &str, raw_pgid: u32) -> ProcessGroupRecord {
    ProcessGroupRecord {
        run_id: ProcessRunId::try_new(run).expect("실행 ID"),
        pgid: pgid(raw_pgid),
        started_at: Timestamp::from_parts(
            UnixSeconds::new(1_760_000_000),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각"),
    }
}

#[test]
fn startup_cleanup_terminates_orphaned_groups_with_term_then_kill_and_clears_ledger() {
    tauri::async_runtime::block_on(async {
        let ledger = Arc::new(InMemoryProcessLedger::new());
        let signaler = Arc::new(ScriptedSignaler::default());
        let manager = ChildProcessManager::new(Arc::clone(&ledger), Arc::clone(&signaler));

        let graceful = pgid(41_001);
        let stubborn = pgid(41_002);
        let gone = pgid(41_003);
        signaler.register_process(graceful, FakeProcessBehavior::ExitsOnTerm);
        signaler.register_process(stubborn, FakeProcessBehavior::IgnoresTermRequiresKill);
        signaler.register_process(gone, FakeProcessBehavior::AlreadyGone);
        manager
            .register_group(record("run-graceful", 41_001))
            .await
            .expect("등록 1");
        manager
            .register_group(record("run-stubborn", 41_002))
            .await
            .expect("등록 2");
        manager
            .register_group(record("run-gone", 41_003))
            .await
            .expect("등록 3");

        let report = manager
            .cleanup_orphans_on_startup(Duration::from_millis(25))
            .await
            .expect("고아 정리");

        assert_eq!(
            report,
            CleanupReport {
                terminated: vec![graceful],
                force_killed: vec![stubborn],
                already_exited: vec![gone],
            },
            "정상 종료·강제 종료·이미 종료된 그룹이 구분되어야 한다"
        );
        assert_eq!(
            signaler.events(),
            vec![
                (graceful, TerminationSignal::Term),
                (stubborn, TerminationSignal::Term),
                (gone, TerminationSignal::Term),
                (stubborn, TerminationSignal::Kill),
            ],
            "SIGTERM 뒤 살아남은 그룹에만 SIGKILL이 전송되어야 한다"
        );
        assert!(
            ledger.list_active().await.expect("원장 조회").is_empty(),
            "정리 완료 뒤 원장에 남은 프로세스 그룹이 없어야 한다"
        );
    });
}

async fn assert_shutdown_drained(lifecycle: &Lifecycle, expected_target: ProcessGroupId) {
    let recorded = lifecycle.outcome().lock().await;
    let summary = recorded
        .as_ref()
        .expect("종료 결과")
        .processes
        .as_ref()
        .expect("프로세스 정리 결과");
    assert_eq!(
        summary.audit_drain,
        AuditDrainStatus::Drained,
        "감사 로그 쓰기가 정상 드레인되어야 한다"
    );
    assert_eq!(
        summary
            .process_cleanup
            .as_ref()
            .expect("정리 성공")
            .terminated,
        vec![expected_target],
        "활성 자식 그룹이 정리되어야 한다"
    );
}

#[test]
fn exit_shutdown_waits_for_in_flight_audit_write_before_completing() {
    tauri::async_runtime::block_on(async {
        let ledger = Arc::new(InMemoryProcessLedger::new());
        let signaler = Arc::new(ScriptedSignaler::default());
        let target = pgid(42_001);
        signaler.register_process(target, FakeProcessBehavior::ExitsOnTerm);
        ledger.seed(record("run-active", 42_001)).expect("시드");

        let manager = Arc::new(ChildProcessManager::new(ledger, signaler));
        let audit_drain = AuditDrainTracker::new();
        let guard = audit_drain.begin_write();
        assert_eq!(audit_drain.in_flight_count(), 1, "진행 중 감사 쓰기 1건");

        let runtime = ProcessRuntime::new(
            manager,
            audit_drain,
            Duration::from_millis(20),
            Duration::from_secs(2),
        );
        let lifecycle = Arc::new(Lifecycle::with_shutdown_hook(
            tauri::async_runtime::spawn(std::future::pending()),
            DatabaseState::NotConfigured,
            runtime.shutdown_hook(),
        ));
        let shutdown_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done_flag = Arc::clone(&shutdown_done);
        let worker_lifecycle = Arc::clone(&lifecycle);
        let handle = tauri::async_runtime::spawn(async move {
            worker_lifecycle.shutdown().await;
            done_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(
            !shutdown_done.load(std::sync::atomic::Ordering::SeqCst),
            "감사 로그 쓰기가 진행 중이면 종료가 완료되지 않아야 한다"
        );
        {
            let _release_audit = guard;
        }
        handle.await.expect("종료 완료");
        assert!(
            shutdown_done.load(std::sync::atomic::Ordering::SeqCst),
            "감사 로그 쓰기 종료 뒤 수명 주기 종료가 완료되어야 한다"
        );
        assert_shutdown_drained(&lifecycle, target).await;
    });
}

#[test]
fn command_signaler_refuses_to_signal_current_process_group() {
    tauri::async_runtime::block_on(async {
        let signaler = CommandProcessSignaler;
        let self_pgid = pgid(std::process::id());
        let error = signaler
            .send_signal(self_pgid, TerminationSignal::Term)
            .await
            .expect_err("자기 PID 보호");
        assert_eq!(
            error,
            ProcessCleanupError::ProtectedSelfGroup(self_pgid),
            "자기 프로세스에 시그널을 보내지 않아야 한다"
        );
    });
}

fn ready_app_with_runtime(runtime: ProcessRuntime) -> tauri::App<tauri::test::MockRuntime> {
    let ports = super::support::ports();
    ports
        .settings
        .script_profile()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .settings
        .script_risk_settings()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .secrets
        .script_load()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .broker_read
        .script_accounts()
        .push(Ok(domain::BrokerAccountRows::try_new(vec![]).expect("계좌")));
    let mut context = mock_context(noop_assets());
    context
        .config_mut()
        .app
        .windows
        .push(tauri::utils::config::WindowConfig::default());
    crate::build_app(mock_builder().manage(runtime), ports, context).expect("제품 조립")
}

fn verify_ready_and_shutdown(
    handle: &tauri::AppHandle<tauri::test::MockRuntime>,
    ledger: &InMemoryProcessLedger,
    signaler: &ScriptedSignaler,
    orphan: ProcessGroupId,
) {
    assert_eq!(
        signaler.events(),
        vec![(orphan, TerminationSignal::Term)],
        "build_app 시작 시 고아 프로세스 그룹에 SIGTERM이 전송되어야 한다"
    );
    assert!(
        ledger.snapshot().expect("원장 스냅샷").is_empty(),
        "시작 시 고아 그룹이 원장에서 제거되어야 한다"
    );

    let live_child = pgid(43_002);
    signaler.register_process(live_child, FakeProcessBehavior::IgnoresTermRequiresKill);
    ledger
        .seed(record("run-live", 43_002))
        .expect("실행 중 자식 시드");
    tauri::async_runtime::block_on(handle.state::<Lifecycle>().shutdown());
    assert_eq!(
        signaler.events(),
        vec![
            (orphan, TerminationSignal::Term),
            (live_child, TerminationSignal::Term),
            (live_child, TerminationSignal::Kill),
        ],
        "종료 시 활성 자식 그룹이 SIGTERM 후 SIGKILL로 정리되어야 한다"
    );
    handle
        .get_webview_window("main")
        .expect("주 창")
        .close()
        .expect("창 닫기");
}

#[test]
fn build_app_runs_orphan_cleanup_on_startup_and_wires_exit_hook() {
    let ledger = Arc::new(InMemoryProcessLedger::new());
    let signaler = Arc::new(ScriptedSignaler::default());
    let orphan = pgid(43_001);
    signaler.register_process(orphan, FakeProcessBehavior::ExitsOnTerm);
    ledger
        .seed(record("run-orphan", 43_001))
        .expect("고아 시드");

    let manager = Arc::new(ChildProcessManager::new(
        Arc::clone(&ledger),
        Arc::clone(&signaler),
    ));
    let runtime = ProcessRuntime::new(
        manager,
        AuditDrainTracker::new(),
        Duration::from_millis(20),
        Duration::from_millis(200),
    );
    let ledger_ref = Arc::clone(&ledger);
    let signaler_ref = Arc::clone(&signaler);
    let code = ready_app_with_runtime(runtime).run_return(move |handle, event| match event {
        tauri::RunEvent::Ready => {
            verify_ready_and_shutdown(handle, &ledger_ref, &signaler_ref, orphan);
        }
        tauri::RunEvent::ExitRequested { .. } => {
            assert!(
                handle
                    .state::<Lifecycle>()
                    .outcome()
                    .try_lock()
                    .expect("종료 기록")
                    .is_some(),
                "정리 후 종료"
            );
        }
        tauri::RunEvent::Exit
        | tauri::RunEvent::WindowEvent { .. }
        | tauri::RunEvent::WebviewEvent { .. }
        | tauri::RunEvent::Resumed
        | tauri::RunEvent::MainEventsCleared
        | tauri::RunEvent::Opened { .. }
        | tauri::RunEvent::MenuEvent(_)
        | tauri::RunEvent::Reopen { .. }
        | _ => {}
    });
    assert_eq!(code, 0_i32, "제품 조립 종료 코드 0");
}
