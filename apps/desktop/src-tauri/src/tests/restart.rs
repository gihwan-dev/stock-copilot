use crate::platform::single_instance::{
    InstanceAcquisition, InstanceRelease, InstanceSlot, SingleInstanceGuard, SocketPath, acquire,
    release_for_exit,
};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;
use tauri::test::{mock_builder, mock_context, noop_assets};

fn socket_dir(name: &str) -> PathBuf {
    let directory =
        PathBuf::from("../../../target/U-18").join(format!("{name}-{}", std::process::id()));
    fs::create_dir_all(&directory).expect("임시 소켓 폴더");
    directory
}

fn primary(path: &Path) -> SingleInstanceGuard {
    match acquire(
        SocketPath::try_new(path.to_path_buf()).expect("소켓 경로"),
        || {},
    )
    .expect("확보")
    {
        InstanceAcquisition::Primary(guard) => guard,
        InstanceAcquisition::SecondaryNotified => panic!("1차 인스턴스여야 한다"),
    }
}

fn relaunch_once(path: &Path, slot: &InstanceSlot) {
    let relaunched = primary(path);
    assert_eq!(relaunched.socket_path(), path);
    assert_eq!(slot.release(), InstanceRelease::NotHeld);
}

#[test]
fn relaunched_instance_becomes_primary_after_exit_releases_the_socket() {
    let directory = socket_dir("restart-release");
    let path = directory.join("app.sock");
    let slot = InstanceSlot::holding(primary(&path));
    let before = acquire(SocketPath::try_new(path.clone()).expect("경로"), || {}).expect("통지");
    match before {
        InstanceAcquisition::SecondaryNotified => {}
        InstanceAcquisition::Primary(_guard) => {
            panic!("해제 전에는 새 프로세스가 기존 인스턴스로 넘긴다")
        }
    }
    assert_eq!(slot.release(), InstanceRelease::Released);
    assert!(!path.exists(), "해제하면 소켓 파일이 사라진다");
    relaunch_once(&path, &slot);
    assert!(!path.exists());
    fs::remove_dir_all(directory).expect("정리");
}

#[test]
fn exit_event_release_uses_the_managed_slot() {
    let directory = socket_dir("restart-managed");
    let path = directory.join("app.sock");
    let app = mock_builder()
        .build(mock_context(noop_assets()))
        .expect("앱");
    assert_eq!(release_for_exit(app.handle()), InstanceRelease::NotHeld);
    let _managed = app.manage(InstanceSlot::holding(primary(&path)));
    assert_eq!(release_for_exit(app.handle()), InstanceRelease::Released);
    assert!(!path.exists());
    assert_eq!(release_for_exit(app.handle()), InstanceRelease::NotHeld);
    fs::remove_dir_all(directory).expect("정리");
}
