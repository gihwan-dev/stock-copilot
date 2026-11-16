use super::support::{failure, ports};
use crate::database::{DatabaseState, open_database};
use application::{SecretKey, SystemFailure};
use std::sync::Arc;
use testkit_ports::{memory::MemoryReply, system_fakes::SecretCall};

#[test]
fn missing_key_skips_database_open_and_preserves_not_configured() {
    tauri::async_runtime::block_on(async {
        let ports = ports();
        ports
            .secrets
            .script_load()
            .push(MemoryReply::Return(Ok(None)));
        let state = open_database(ports.secrets.as_ref(), Arc::clone(&ports.database), || {
            Ok(store_sqlite::DatabasePath::new("unused.db".into()))
        })
        .await;
        match state {
            DatabaseState::NotConfigured => {}
            DatabaseState::Ready(_) | DatabaseState::Failed(_) => panic!("설정 안 됨"),
        }
        let calls = ports.secrets.calls();
        assert_eq!(calls.len(), 1);
        match calls.first().expect("키 조회") {
            SecretCall::Load(key) => assert_eq!(*key, SecretKey::DatabaseEncryptionKey),
            SecretCall::Store(_, _) | SecretCall::Remove(_) => panic!("조회만"),
        }
    });
}

#[test]
fn key_load_failure_is_preserved_without_database_open() {
    tauri::async_runtime::block_on(async {
        let ports = ports();
        let expected = failure(SystemFailure::PermissionDenied);
        ports
            .secrets
            .script_load()
            .push(MemoryReply::Return(Err(expected)));
        let state = open_database(ports.secrets.as_ref(), Arc::clone(&ports.database), || {
            Ok(store_sqlite::DatabasePath::new("unused.db".into()))
        })
        .await;
        match state {
            DatabaseState::Failed(error) => assert_eq!(error, expected),
            DatabaseState::NotConfigured | DatabaseState::Ready(_) => panic!("실패 보존"),
        }
    });
}

#[test]
fn configured_key_reaches_the_factory_and_keeps_its_error() {
    tauri::async_runtime::block_on(async {
        let ports = ports();
        ports
            .secrets
            .script_load()
            .push(MemoryReply::Return(Ok(Some(
                application::SecretValue::new(vec![71; 32]),
            ))));
        let state = open_database(ports.secrets.as_ref(), Arc::clone(&ports.database), || {
            Ok(store_sqlite::DatabasePath::new("unused.db".into()))
        })
        .await;
        match state {
            DatabaseState::Failed(error) => assert_eq!(error, failure(SystemFailure::InvalidData)),
            DatabaseState::NotConfigured | DatabaseState::Ready(_) => panic!("열기 실패 보존"),
        }
    });
}

#[test]
fn directory_preparation_creates_nested_parents_and_preserves_existing_files() {
    let root = directory("created");
    std::fs::create_dir_all(&root).expect("폴더");
    let sentinel = root.join("keep");
    std::fs::write(&sentinel, b"preserved").expect("기존 자료");
    let path = store_sqlite::DatabasePath::new(root.join("a").join("b").join("store.db"));
    crate::database::prepare_directory(&path).expect("하위 폴더 생성");
    assert!(root.join("a").join("b").is_dir());
    assert_eq!(std::fs::read(&sentinel).expect("기존 자료"), b"preserved");
    std::fs::remove_dir_all(root).expect("정리");
}

#[test]
fn directory_preparation_keeps_permission_and_other_failures_distinct() {
    use std::os::unix::fs::PermissionsExt;
    let root = directory("failures");
    std::fs::create_dir_all(&root).expect("폴더");
    let denied = store_sqlite::DatabasePath::new(root.join("child").join("store.db"));
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o000)).expect("권한 차단");
    let denied_result = crate::database::prepare_directory(&denied);
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).expect("권한 복원");
    let parent_file = root.join("file");
    std::fs::write(&parent_file, b"file").expect("일반 파일");
    let invalid = store_sqlite::DatabasePath::new(parent_file.join("store.db"));
    let invalid_result = crate::database::prepare_directory(&invalid);
    std::fs::remove_dir_all(root).expect("정리");
    assert_eq!(denied_result, Err(failure(SystemFailure::PermissionDenied)));
    assert_eq!(
        invalid_result,
        Err(failure(SystemFailure::Unavailable(
            application::RetryDelay::Unknown
        )))
    );
}

fn directory(name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from("../../../target/U-01")
        .join(format!("directory-{name}-{}", std::process::id()))
}
