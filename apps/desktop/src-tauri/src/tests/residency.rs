use crate::exit::ExitCode;
use crate::lifecycle::{Lifecycle, LifecyclePhase};
use crate::platform::residency::{
    MainWindowVisibility, ResidencyState, VisibleWindowPresence, begin_quit, handle_reopen,
    handle_tray_action,
};
use crate::platform::single_instance::{
    InstanceAcquisition, SingleInstanceError, SocketPath, acquire, attach_to_app,
};
use crate::platform::tray::{
    TrayController, TrayError, TrayIconState, TrayMenuAction, TrayMenuItem,
    UnavailableTrayController, default_tray_menu,
};
use std::fs;
use std::path::PathBuf;
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Duration;
use tauri::Manager;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};
use testkit_ports::memory::MemoryReply;

fn ready_app() -> tauri::App<MockRuntime> {
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
    crate::build_app(mock_builder(), ports, context).expect("제품 조립")
}

fn temp_socket_dir(name: &str) -> PathBuf {
    let directory =
        PathBuf::from("../../../target/U-18").join(format!("{name}-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("임시 소켓 폴더");
    directory
}

fn finish_mock_exit(handle: &tauri::AppHandle<MockRuntime>) {
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
}

struct FakeTrayController {
    state: RwLock<TrayIconState>,
    installed: RwLock<Vec<TrayMenuItem>>,
}

impl FakeTrayController {
    fn new() -> Self {
        Self {
            state: RwLock::new(TrayIconState::Hidden),
            installed: RwLock::new(Vec::new()),
        }
    }
}

impl TrayController for FakeTrayController {
    fn install(&self, items: &[TrayMenuItem]) -> Result<TrayIconState, TrayError> {
        let mut installed = self
            .installed
            .write()
            .map_err(|_error| TrayError::Unavailable)?;
        *installed = items.to_vec();
        let mut state = self
            .state
            .write()
            .map_err(|_error| TrayError::Unavailable)?;
        *state = TrayIconState::Visible;
        Ok(TrayIconState::Visible)
    }

    fn state(&self) -> TrayIconState {
        *self.state.read().expect("상태 조회")
    }
}

fn drive_hide_reopen_and_quit(handle: &tauri::AppHandle<MockRuntime>) {
    while handle.try_state::<ResidencyState>().is_none() {
        std::thread::yield_now();
    }
    let close_handle = handle.clone();
    handle
        .run_on_main_thread(move || {
            let main_window = close_handle.get_webview_window("main").expect("주 창");
            assert!(
                main_window.is_visible().expect("표시 상태"),
                "시작 시 준비된 주 창이 표시되어야 한다"
            );
            main_window.close().expect("닫기 요청");
        })
        .expect("닫기 실행");
    while handle.state::<ResidencyState>().close_intercepted_count() == 0 {
        std::thread::yield_now();
    }
    assert_eq!(
        handle.state::<ResidencyState>().main_visibility(),
        MainWindowVisibility::Hidden,
        "주 창 닫기는 숨김 상태로 전환되어야 한다"
    );
    assert_eq!(
        handle.state::<Lifecycle>().phase(),
        LifecyclePhase::Running,
        "주 창 숨김 뒤에도 앱은 계속 실행 중이어야 한다"
    );
    let reopen_handle = handle.clone();
    handle
        .run_on_main_thread(move || {
            let main_window = reopen_handle.get_webview_window("main").expect("주 창");
            assert_eq!(
                reopen_handle.state::<ResidencyState>().main_visibility(),
                MainWindowVisibility::Hidden,
                "주 창이 숨김 상태로 유지되어야 한다"
            );
            handle_reopen(&reopen_handle, VisibleWindowPresence::NoneVisible).expect("재활성화");
            assert_eq!(
                reopen_handle.state::<ResidencyState>().main_visibility(),
                MainWindowVisibility::Visible,
                "재활성화하면 주 창 상태가 표시로 바뀌어야 한다"
            );
            assert!(
                main_window.is_visible().expect("재표시 확인"),
                "재활성화하면 주 창이 다시 표시되어야 한다"
            );
            handle_tray_action(&reopen_handle, TrayMenuAction::OpenMain).expect("트레이 열기");
            begin_quit(&reopen_handle);
            main_window.close().expect("명시적 종료");
        })
        .expect("재표시 및 종료");
}

#[test]
fn closing_main_window_hides_and_keeps_running_until_explicit_quit() {
    let app = ready_app();
    let trigger = app.handle().clone();
    let driver = std::thread::spawn(move || drive_hide_reopen_and_quit(&trigger));
    let recorded = Arc::new(RwLock::new(Vec::new()));
    let observe = Arc::clone(&recorded);
    crate::run_app(app, move |handle, code| {
        observe.write().expect("기록").push(code);
        finish_mock_exit(handle);
    });
    driver.join().expect("드라이버 완료");
    assert_eq!(
        *recorded.read().expect("종료 코드"),
        vec![ExitCode::new(0)],
        "명시적 종료 뒤 정상 종료 코드"
    );
}

#[test]
fn second_instance_notifies_primary_and_stale_socket_recovers() {
    let directory = temp_socket_dir("single-instance");
    let path = directory.join("app.sock");
    fs::write(&path, b"stale").expect("낡은 소켓 자리");

    let activations = Arc::new(AtomicUsize::new(0));
    let (sender, receiver) = mpsc::channel();
    let count_ref = Arc::clone(&activations);
    let primary = acquire(
        SocketPath::try_new(path.clone()).expect("소켓 경로"),
        move || {
            let next = count_ref.fetch_add(1, Ordering::SeqCst);
            sender.send(next).expect("활성화 알림");
        },
    )
    .expect("1차 인스턴스 확보");
    let guard = match primary {
        InstanceAcquisition::Primary(guard) => guard,
        InstanceAcquisition::SecondaryNotified => panic!("1차 인스턴스여야 한다"),
    };
    assert_eq!(guard.socket_path(), path.as_path(), "소켓 경로 일치");

    let secondary = acquire(
        SocketPath::try_new(path.clone()).expect("2차 경로"),
        || {
            panic!("2차 인스턴스 콜백은 호출되지 않아야 한다");
        },
    )
    .expect("2차 인스턴스 통지");
    match secondary {
        InstanceAcquisition::SecondaryNotified => {}
        InstanceAcquisition::Primary(_) => panic!("2차 인스턴스는 기존 인스턴스에 통지해야 한다"),
    }
    assert_eq!(
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("1차 활성화 수신"),
        0,
        "1차 활성화 콜백이 호출되어야 한다"
    );
    assert_eq!(
        activations.load(Ordering::SeqCst),
        1,
        "활성화 콜백 1회 호출"
    );
    {
        let _release_guard = guard;
    }
    assert!(!path.exists(), "가드 해제 후 소켓 파일 정리");
    fs::remove_dir_all(directory).expect("임시 폴더 정리");
}

fn drive_secondary_activation(
    handle: &tauri::AppHandle<MockRuntime>,
    secondary_path: PathBuf,
    raised: &AtomicUsize,
) {
    while handle.try_state::<ResidencyState>().is_none() {
        std::thread::yield_now();
    }
    let close_handle = handle.clone();
    handle
        .run_on_main_thread(move || {
            close_handle
                .get_webview_window("main")
                .expect("주 창")
                .close()
                .expect("창 숨김");
        })
        .expect("창 숨김 지시");
    while handle.state::<ResidencyState>().main_visibility() != MainWindowVisibility::Hidden {
        std::thread::yield_now();
    }
    let notified = acquire(
        SocketPath::try_new(secondary_path).expect("2차 경로"),
        || {},
    )
    .expect("2차 실행");
    match notified {
        InstanceAcquisition::SecondaryNotified => {}
        InstanceAcquisition::Primary(_) => panic!("2차 실행은 통지 후 종료"),
    }
    while handle.state::<ResidencyState>().main_visibility() != MainWindowVisibility::Visible {
        std::thread::yield_now();
    }
    raised.fetch_add(1, Ordering::SeqCst);
    let quit_handle = handle.clone();
    handle
        .run_on_main_thread(move || {
            let main_window = quit_handle.get_webview_window("main").expect("주 창");
            assert!(
                main_window.is_visible().expect("재표시 확인"),
                "2차 실행이 숨겨진 주 창을 다시 표시해야 한다"
            );
            begin_quit(&quit_handle);
            main_window.close().expect("명시적 종료");
        })
        .expect("종료 지시");
}

#[test]
fn second_instance_raises_hidden_main_window_on_primary_app() {
    let directory = temp_socket_dir("attach-app");
    let path = directory.join("app.sock");
    let app = ready_app();
    let primary = attach_to_app(
        app.handle(),
        SocketPath::try_new(path.clone()).expect("소켓 경로"),
    )
    .expect("1차 앱 연결");
    let guard = match primary {
        InstanceAcquisition::Primary(guard) => guard,
        InstanceAcquisition::SecondaryNotified => panic!("1차 인스턴스여야 한다"),
    };
    let raised = Arc::new(AtomicUsize::new(0));
    let raised_ref = Arc::clone(&raised);
    let trigger = app.handle().clone();
    let worker_path = path.clone();
    let driver = std::thread::spawn(move || {
        drive_secondary_activation(&trigger, worker_path, &raised_ref);
    });
    crate::run_app(app, |handle, _code| {
        finish_mock_exit(handle);
    });
    driver.join().expect("드라이버 완료");
    assert_eq!(
        raised.load(Ordering::SeqCst),
        1,
        "2차 실행으로 숨겨진 주 창 복원 완료"
    );
    {
        let _release_guard = guard;
    }
    assert!(!path.exists(), "앱 연결 가드 해제 후 소켓 파일 정리");
    fs::remove_dir_all(directory).expect("임시 폴더 정리");
}

#[test]
fn socket_path_rejects_paths_exceeding_104_bytes() {
    let too_long = PathBuf::from(format!("/tmp/{}", "a".repeat(110)));
    let error = SocketPath::try_new(too_long).expect_err("104바이트 초과 거부");
    match error {
        SingleInstanceError::PathTooLong { length } => {
            assert!(length > 104, "초과 길이 보존");
        }
        SingleInstanceError::MissingParentDirectory
        | SingleInstanceError::DirectorySetup(_)
        | SingleInstanceError::BindFailed(_)
        | SingleInstanceError::SignalFailed(_) => panic!("PathTooLong 오류여야 한다"),
    }
}

#[test]
fn tray_menu_defaults_and_controller_contract_work() {
    let items = default_tray_menu();
    assert_eq!(items.len(), 3, "기본 트레이 메뉴 3개");
    assert_eq!(items[0].action, TrayMenuAction::OpenMain, "첫 항목 열기");
    assert_eq!(items[0].label, "열기", "한국어 라벨");
    assert_eq!(
        items[1].action,
        TrayMenuAction::OpenSettings,
        "둘째 항목 설정"
    );
    assert_eq!(items[1].label, "설정", "한국어 라벨");
    assert_eq!(items[2].action, TrayMenuAction::Quit, "셋째 항목 종료");
    assert_eq!(items[2].label, "종료", "한국어 라벨");

    let unavailable = UnavailableTrayController;
    assert_eq!(
        unavailable.state(),
        TrayIconState::Hidden,
        "미지원 어댑터 초기 상태"
    );
    assert_eq!(
        unavailable.install(&items),
        Err(TrayError::Unavailable),
        "잠금 파일에 tray-icon 부재 시 명시적 불가"
    );

    let fake = FakeTrayController::new();
    assert_eq!(
        fake.install(&items),
        Ok(TrayIconState::Visible),
        "가짜 트레이 설치 성공"
    );
    assert_eq!(fake.state(), TrayIconState::Visible, "설치 후 표시 상태");
    assert_eq!(
        fake.installed.read().expect("기록").as_slice(),
        &items,
        "메뉴 항목 보존"
    );
}
