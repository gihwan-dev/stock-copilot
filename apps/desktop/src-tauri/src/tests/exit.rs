use crate::{
    database::DatabaseState,
    exit::{ExitCode, ExitRequest},
    lifecycle::{ExitDecision, Lifecycle},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tauri::{AppHandle, Manager, test::MockRuntime};

#[derive(Default)]
struct Request(AtomicUsize);
impl ExitRequest for Request {
    fn prevent(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn requested_codes_preserve_nonzero_negative_and_absent_values() {
    for (requested, expected) in [
        (Some(73_i32), 73_i32),
        (Some(-41_i32), -41_i32),
        (None, 0_i32),
    ] {
        assert_eq!(ExitCode::from_request(requested).as_i32(), expected);
    }
}

#[test]
fn the_product_exit_handler_defers_cleans_once_and_allows_reentry() {
    let app = tauri::test::mock_app();
    app.manage(Lifecycle::new(
        tauri::async_runtime::spawn(std::future::pending()),
        DatabaseState::NotConfigured,
    ));
    let request = Request::default();
    let (send, receive) = std::sync::mpsc::channel();
    let finish = Arc::new(move |handle: &AppHandle<MockRuntime>, code: ExitCode| {
        assert_eq!(
            handle.state::<Lifecycle>().exit_decision(),
            ExitDecision::Allow
        );
        assert!(
            handle
                .state::<Lifecycle>()
                .outcome()
                .try_lock()
                .expect("종료 상태")
                .is_some()
        );
        send.send(code).expect("종료 요청 기록");
    });
    crate::exit::request(
        app.handle(),
        &request,
        ExitCode::new(73),
        Arc::clone(&finish),
    );
    assert_eq!(request.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        receive
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("종료 완료"),
        ExitCode::new(73)
    );
    crate::exit::request(app.handle(), &request, ExitCode::new(91), finish);
    assert_eq!(request.0.load(Ordering::SeqCst), 1);
    receive.try_recv().expect_err("중복 종료 요청 없음");
}

#[test]
fn a_duplicate_exit_request_defers_without_starting_another_cleanup() {
    let app = tauri::test::mock_app();
    app.manage(Lifecycle::new(
        tauri::async_runtime::spawn(std::future::pending()),
        DatabaseState::NotConfigured,
    ));
    assert_eq!(
        app.state::<Lifecycle>().exit_decision(),
        ExitDecision::Start
    );
    let request = Request::default();
    crate::exit::request(
        app.handle(),
        &request,
        ExitCode::new(19),
        Arc::new(|_handle: &AppHandle<MockRuntime>, _code: ExitCode| panic!("중복 정리 금지")),
    );
    assert_eq!(request.0.load(Ordering::SeqCst), 1);
    tauri::async_runtime::block_on(app.state::<Lifecycle>().shutdown());
    assert_eq!(
        app.state::<Lifecycle>().exit_decision(),
        ExitDecision::Allow
    );
}

#[test]
fn run_app_dispatches_the_real_mock_exit_event_and_waits_for_cleanup() {
    let app = ready_app();
    let trigger = app.handle().clone();
    let closer = std::thread::spawn(move || {
        while trigger.try_state::<Lifecycle>().is_none() {
            std::thread::yield_now();
        }
        let closing = trigger.clone();
        trigger
            .run_on_main_thread(move || {
                crate::platform::residency::begin_quit(&closing);
                closing
                    .get_webview_window("main")
                    .expect("주 창")
                    .close()
                    .expect("종료 이벤트");
            })
            .expect("주 스레드");
    });
    let recorded = Arc::new(std::sync::RwLock::new(Vec::new()));
    let observe = Arc::clone(&recorded);
    crate::run_app(app, move |handle, requested| {
        observe.write().expect("기록").push(requested);
        assert!(
            handle
                .state::<Lifecycle>()
                .outcome()
                .try_lock()
                .expect("정리 결과")
                .is_some()
        );
        let finishing = handle.clone();
        handle
            .run_on_main_thread(move || {
                tauri::WebviewWindowBuilder::new(
                    &finishing,
                    "exit-reentry",
                    tauri::WebviewUrl::default(),
                )
                .build()
                .expect("재진입 창")
                .close()
                .expect("재진입 종료");
            })
            .expect("종료 재진입");
    });
    closer.join().expect("종료 요청 스레드");
    assert_eq!(*recorded.read().expect("종료 기록"), vec![ExitCode::new(0)]);
}

fn ready_app() -> tauri::App<MockRuntime> {
    use testkit_ports::memory::MemoryReply;
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
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context
        .config_mut()
        .app
        .windows
        .push(tauri::utils::config::WindowConfig::default());
    crate::build_app(tauri::test::mock_builder(), ports, context).expect("제품 조립")
}
