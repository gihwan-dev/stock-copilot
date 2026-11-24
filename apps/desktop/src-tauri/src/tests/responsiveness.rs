use crate::lifecycle::Lifecycle;
use crate::platform::background::{
    BackgroundRunState, BackgroundRuntime, CollectorFuture, CollectorWorker,
};
use domain::{Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds};
use scheduler::{
    DisabledAiJobQueue, JobConfig, JobEnablement, JobFuture, JobKind, JobRegistry, JobRunDetail,
    LoopControl, MissedPolicy, ScheduleRule, ScheduledJob, ScheduledJobId, Scheduler,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tauri::Manager;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use testkit_ports::SteppedClock;
use testkit_ports::memory::MemoryReply;
use tokio::sync::{Notify, watch};

struct BlockingCollector {
    started: Arc<Notify>,
    release: Arc<Notify>,
    entered: Arc<AtomicUsize>,
    completed: Arc<AtomicUsize>,
}

impl CollectorWorker for BlockingCollector {
    fn run(&self, mut shutdown: watch::Receiver<LoopControl>) -> CollectorFuture {
        let started = Arc::clone(&self.started);
        let release = Arc::clone(&self.release);
        let entered = Arc::clone(&self.entered);
        let completed = Arc::clone(&self.completed);
        Box::pin(async move {
            entered.fetch_add(1, Ordering::SeqCst);
            started.notify_waiters();
            let release_wait = std::pin::pin!(release.notified());
            let shutdown_wait = std::pin::pin!(wait_stop(&mut shutdown));
            wait_either(release_wait, shutdown_wait).await;
            completed.fetch_add(1, Ordering::SeqCst);
        })
    }
}

async fn wait_stop(shutdown: &mut watch::Receiver<LoopControl>) {
    while *shutdown.borrow() != LoopControl::Stop {
        if shutdown.changed().await.is_err() {
            break;
        }
    }
}

async fn wait_either<F1: Future<Output = ()>, F2: Future<Output = ()>>(
    mut first: std::pin::Pin<&mut F1>,
    mut second: std::pin::Pin<&mut F2>,
) {
    std::future::poll_fn(|cx| {
        if first.as_mut().poll(cx).is_ready() || second.as_mut().poll(cx).is_ready() {
            return std::task::Poll::Ready(());
        }
        std::task::Poll::Pending
    })
    .await;
}

struct BlockingScheduledJob {
    started: Arc<Notify>,
    release: Arc<Notify>,
    entered: Arc<AtomicUsize>,
    completed: Arc<AtomicUsize>,
}

impl ScheduledJob for BlockingScheduledJob {
    fn id(&self) -> ScheduledJobId {
        ScheduledJobId::NewsCollection
    }

    fn default_config(&self) -> JobConfig {
        JobConfig {
            kind: JobKind::CodeOnly,
            schedule: ScheduleRule::Interval(
                TimeSpan::try_new(SpanSeconds::new(1), Nanosecond::new(0)).expect("간격"),
            ),
            missed_policy: MissedPolicy::CatchUpOnce,
            enabled: JobEnablement::Enabled,
        }
    }

    fn run(&self, _now: Timestamp) -> JobFuture<'_> {
        let started = Arc::clone(&self.started);
        let release = Arc::clone(&self.release);
        let entered = Arc::clone(&self.entered);
        let completed = Arc::clone(&self.completed);
        Box::pin(async move {
            entered.fetch_add(1, Ordering::SeqCst);
            started.notify_waiters();
            release.notified().await;
            completed.fetch_add(1, Ordering::SeqCst);
            Ok(JobRunDetail::new("blocked-job-finished", 1))
        })
    }
}

fn fixed_time(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn app_version_invoke() -> tauri::webview::InvokeRequest {
    tauri::webview::InvokeRequest {
        cmd: "app_version".into(),
        callback: tauri::ipc::CallbackFn(101),
        error: tauri::ipc::CallbackFn(102),
        url: "tauri://localhost".parse().expect("로컬 출처"),
        body: tauri::ipc::InvokeBody::Json(serde_json::json!({"request":{"query":"current"}})),
        headers: tauri::http::HeaderMap::new(),
        invoke_key: tauri::test::INVOKE_KEY.into(),
    }
}

fn ready_app_with_background(background: Arc<BackgroundRuntime>) -> tauri::App<MockRuntime> {
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
    crate::build_app(mock_builder().manage(background), ports, context).expect("제품 조립")
}

struct BlockedWorkFixture {
    collector_entered: Arc<AtomicUsize>,
    collector_completed: Arc<AtomicUsize>,
    job_entered: Arc<AtomicUsize>,
    job_completed: Arc<AtomicUsize>,
    collector_release: Arc<Notify>,
    job_release: Arc<Notify>,
    background: Arc<BackgroundRuntime>,
}

fn spawn_blocked_background_work() -> BlockedWorkFixture {
    let collector_started = Arc::new(Notify::new());
    let collector_release = Arc::new(Notify::new());
    let collector_entered = Arc::new(AtomicUsize::new(0));
    let collector_completed = Arc::new(AtomicUsize::new(0));
    let job_started = Arc::new(Notify::new());
    let job_release = Arc::new(Notify::new());
    let job_entered = Arc::new(AtomicUsize::new(0));
    let job_completed = Arc::new(AtomicUsize::new(0));

    let clock = Arc::new(SteppedClock::new(fixed_time(1_760_000_000)));
    let mut registry = JobRegistry::new();
    registry
        .register(BlockingScheduledJob {
            started: Arc::clone(&job_started),
            release: Arc::clone(&job_release),
            entered: Arc::clone(&job_entered),
            completed: Arc::clone(&job_completed),
        })
        .expect("작업 등록");
    let scheduler = Arc::new(
        Scheduler::new(Arc::clone(&clock), Arc::new(DisabledAiJobQueue), registry)
            .expect("스케줄러"),
    );
    let collector: Arc<dyn CollectorWorker> = Arc::new(BlockingCollector {
        started: Arc::clone(&collector_started),
        release: Arc::clone(&collector_release),
        entered: Arc::clone(&collector_entered),
        completed: Arc::clone(&collector_completed),
    });

    clock
        .advance_to(fixed_time(1_760_000_005))
        .expect("예약 시각 도달");
    let background = Arc::new(BackgroundRuntime::spawn(
        scheduler,
        Duration::from_millis(10),
        collector,
    ));
    wait_both_entered(
        &collector_started,
        &collector_entered,
        &job_started,
        &job_entered,
    );
    BlockedWorkFixture {
        collector_entered,
        collector_completed,
        job_entered,
        job_completed,
        collector_release,
        job_release,
        background,
    }
}

async fn wait_entered(notify: &Notify, counter: &AtomicUsize) {
    loop {
        let notified = notify.notified();
        let pinned = std::pin::pin!(notified);
        if counter.load(Ordering::SeqCst) > 0 {
            break;
        }
        pinned.await;
    }
}

fn wait_both_entered(
    collector_started: &Notify,
    collector_entered: &AtomicUsize,
    job_started: &Notify,
    job_entered: &AtomicUsize,
) {
    tauri::async_runtime::block_on(async {
        tokio::time::timeout(Duration::from_secs(2), async {
            wait_entered(collector_started, collector_entered).await;
            wait_entered(job_started, job_entered).await;
        })
        .await
        .expect("수집·예약 작업 진입 대기");
    });
}

fn verify_ipc_while_blocked_and_shutdown(
    handle: &tauri::AppHandle<MockRuntime>,
    fixture: &BlockedWorkFixture,
) {
    assert_eq!(
        fixture.collector_entered.load(Ordering::SeqCst),
        1,
        "수집 작업이 백그라운드에서 실행 중이어야 한다"
    );
    assert_eq!(
        fixture.job_entered.load(Ordering::SeqCst),
        1,
        "예약 작업이 백그라운드에서 실행 중이어야 한다"
    );
    let window = handle.get_webview_window("main").expect("주 창");
    for _ in 0_u8..5 {
        let response = tauri::test::get_ipc_response(&window, app_version_invoke())
            .expect("수집·예약 작업이 막혀 있어도 IPC 조회가 즉시 응답해야 한다");
        assert_eq!(
            response
                .deserialize::<serde_json::Value>()
                .expect("응답 JSON"),
            serde_json::json!({"version":"0.1.0"}),
            "응답 값 일치"
        );
    }
    assert_eq!(
        fixture.collector_completed.load(Ordering::SeqCst),
        0,
        "IPC 응답 완료 시점에도 수집 작업은 여전히 블로킹 상태여야 한다"
    );
    assert_eq!(
        fixture.job_completed.load(Ordering::SeqCst),
        0,
        "IPC 응답 완료 시점에도 예약 작업은 여전히 블로킹 상태여야 한다"
    );
    fixture.collector_release.notify_waiters();
    fixture.job_release.notify_waiters();
    tauri::async_runtime::block_on(handle.state::<Lifecycle>().shutdown());
    assert_eq!(
        fixture.background.state(),
        BackgroundRunState::Stopped,
        "종료 시 백그라운드 런타임이 정리되어야 한다"
    );
    window.close().expect("창 닫기");
}

#[test]
fn ipc_queries_respond_while_collection_and_scheduled_jobs_are_blocked() {
    let fixture = Arc::new(spawn_blocked_background_work());
    let fixture_ref = Arc::clone(&fixture);
    let code = ready_app_with_background(Arc::clone(&fixture.background)).run_return(
        move |handle, event| match event {
            tauri::RunEvent::Ready => {
                verify_ipc_while_blocked_and_shutdown(handle, &fixture_ref);
            }
            tauri::RunEvent::ExitRequested { .. } => {
                assert!(
                    handle
                        .state::<Lifecycle>()
                        .outcome()
                        .try_lock()
                        .expect("종료 기록")
                        .is_some(),
                    "백그라운드 정리 후 종료"
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
        },
    );
    assert_eq!(code, 0_i32, "비차단 응답성 검증 종료 코드 0");
}
