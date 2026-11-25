use super::support::ports;
use crate::build_app;
use crate::database::{
    DatabaseFactory, DatabaseKeyProvision, DatabaseStatus, KeyProvisioning,
    open_provisioned_database, provision_database_key,
};
use application::{SecretKey, SecretValue, SystemFailure, SystemPort, SystemPortError};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::Manager as _;
use tauri::test::{mock_builder, mock_context, noop_assets};
use testkit_ports::memory::MemoryReply;
use testkit_ports::system_fakes::{MemorySecretStore, SecretCall};

const FAKE_KEY: [u8; 32] = [0x5a; 32];

fn counting_provision(count: Arc<AtomicUsize>) -> DatabaseKeyProvision {
    DatabaseKeyProvision(Box::new(move || {
        count.fetch_add(1, Ordering::SeqCst);
        Ok(SecretValue::new(FAKE_KEY.to_vec()))
    }))
}

fn memory_store(loads: usize, stores: usize) -> MemorySecretStore {
    let store = MemorySecretStore::default();
    for _ in 0..loads {
        store.script_load().push(MemoryReply::Memory);
    }
    for _ in 0..stores {
        store.script_store().push(MemoryReply::Memory);
    }
    store
}

fn missing_path(name: &str) -> store_sqlite::DatabasePath {
    store_sqlite::DatabasePath::new(
        std::path::PathBuf::from("../../../target/fin-c")
            .join(format!("{name}-{}", std::process::id()))
            .join("absent.db"),
    )
}

fn recording_factory(keys: Arc<std::sync::RwLock<Vec<Vec<u8>>>>) -> Arc<DatabaseFactory> {
    Arc::new(DatabaseFactory(Box::new(move |_path, key| {
        match keys.write() {
            Ok(mut seen) => seen.push(key.as_exposed_secret().to_vec()),
            Err(poisoned) => poisoned.into_inner().push(key.as_exposed_secret().to_vec()),
        }
        Err(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::InvalidData,
        ))
    })))
}

#[test]
fn first_launch_creates_and_stores_a_key_then_opens_with_it() {
    tauri::async_runtime::block_on(async {
        let secrets = memory_store(2, 1);
        let count = Arc::new(AtomicUsize::new(0));
        let keys = Arc::new(std::sync::RwLock::new(Vec::new()));
        let state = open_provisioned_database(
            &secrets,
            &counting_provision(Arc::clone(&count)),
            recording_factory(Arc::clone(&keys)),
            missing_path("first-launch"),
        )
        .await;
        assert_eq!(
            state.status(),
            DatabaseStatus::Failed(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::InvalidData
            )),
            "팩토리까지 도달"
        );
        assert_eq!(count.load(Ordering::SeqCst), 1);
        let opened_with = keys.read().expect("기록").clone();
        assert_eq!(opened_with, vec![FAKE_KEY.to_vec()]);
        let calls = secrets.calls();
        assert_eq!(calls.len(), 3);
        match calls.as_slice() {
            [
                SecretCall::Load(SecretKey::DatabaseEncryptionKey),
                SecretCall::Store(SecretKey::DatabaseEncryptionKey, stored),
                SecretCall::Load(SecretKey::DatabaseEncryptionKey),
            ] => assert_eq!(stored.as_exposed_secret(), FAKE_KEY.as_slice()),
            other => panic!("호출 순서: {other:?}"),
        }
    });
}

#[test]
fn existing_key_is_reused_without_generating_a_new_one() {
    tauri::async_runtime::block_on(async {
        let secrets = MemorySecretStore::default();
        secrets
            .script_load()
            .push(MemoryReply::Return(Ok(Some(SecretValue::new(vec![7; 32])))));
        let count = Arc::new(AtomicUsize::new(0));
        let outcome = provision_database_key(
            &secrets,
            &counting_provision(Arc::clone(&count)),
            &missing_path("existing-key"),
        )
        .await;
        assert_eq!(outcome, Ok(KeyProvisioning::Existing));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        assert_eq!(secrets.calls().len(), 1);
    });
}

#[test]
fn missing_key_with_an_existing_database_file_is_key_loss_and_creates_nothing() {
    tauri::async_runtime::block_on(async {
        let directory = std::path::PathBuf::from("../../../target/fin-c")
            .join(format!("key-loss-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("폴더");
        let file = directory.join("stock-copilot.db");
        std::fs::write(&file, b"encrypted-bytes").expect("기존 DB");
        let secrets = memory_store(1, 0);
        let count = Arc::new(AtomicUsize::new(0));
        let keys = Arc::new(std::sync::RwLock::new(Vec::new()));
        let state = open_provisioned_database(
            &secrets,
            &counting_provision(Arc::clone(&count)),
            recording_factory(Arc::clone(&keys)),
            store_sqlite::DatabasePath::new(file.clone()),
        )
        .await;
        assert_eq!(
            state.status(),
            DatabaseStatus::Failed(SystemPortError::new(
                SystemPort::SecretStore,
                SystemFailure::SecretIntegrity
            ))
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
        assert!(keys.read().expect("기록").is_empty(), "DB를 열지 않음");
        assert_eq!(secrets.calls().len(), 1, "키를 저장하지 않음");
        assert_eq!(std::fs::read(&file).expect("파일"), b"encrypted-bytes");
        std::fs::remove_dir_all(directory).expect("정리");
    });
}

#[test]
fn key_read_and_store_failures_are_preserved_without_opening() {
    tauri::async_runtime::block_on(async {
        let denied = SystemPortError::new(SystemPort::SecretStore, SystemFailure::PermissionDenied);
        let unreadable = MemorySecretStore::default();
        unreadable
            .script_load()
            .push(MemoryReply::Return(Err(denied)));
        let count = Arc::new(AtomicUsize::new(0));
        let keys = Arc::new(std::sync::RwLock::new(Vec::new()));
        let state = open_provisioned_database(
            &unreadable,
            &counting_provision(Arc::clone(&count)),
            recording_factory(Arc::clone(&keys)),
            missing_path("unreadable"),
        )
        .await;
        assert_eq!(state.status(), DatabaseStatus::Failed(denied));
        assert_eq!(count.load(Ordering::SeqCst), 0, "읽기 실패는 분실이 아님");

        let unwritable = memory_store(1, 0);
        unwritable
            .script_store()
            .push(MemoryReply::Return(Err(denied)));
        let store_failed = open_provisioned_database(
            &unwritable,
            &counting_provision(Arc::clone(&count)),
            recording_factory(Arc::clone(&keys)),
            missing_path("unwritable"),
        )
        .await;
        assert_eq!(store_failed.status(), DatabaseStatus::Failed(denied));
        assert!(keys.read().expect("기록").is_empty());
    });
}

fn startup_ready_ports() -> crate::ports::Ports<super::support::TestPorts> {
    let ports = ports();
    for _ in 0..2_usize {
        ports.secrets.script_load().push(MemoryReply::Memory);
    }
    ports.secrets.script_store().push(MemoryReply::Memory);
    ports
        .settings
        .script_profile()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .settings
        .script_risk_settings()
        .push(MemoryReply::Return(Ok(None)));
    ports
        .broker_read
        .script_accounts()
        .push(Ok(domain::BrokerAccountRows::try_new(vec![]).expect("계좌")));
    ports
}

fn observe_database_status(app: tauri::App<tauri::test::MockRuntime>) -> Option<DatabaseStatus> {
    let observed = Arc::new(std::sync::RwLock::new(None));
    let seen = Arc::clone(&observed);
    let code = app.run_return(move |handle, event| match event {
        tauri::RunEvent::Ready => {
            if let Ok(mut slot) = seen.write() {
                *slot = Some(*handle.state::<DatabaseStatus>());
            }
            tauri::async_runtime::block_on(
                handle.state::<crate::lifecycle::Lifecycle>().shutdown(),
            );
            handle
                .get_webview_window("main")
                .expect("주 창")
                .close()
                .expect("창 닫기");
        }
        tauri::RunEvent::ExitRequested { .. } => {
            assert!(
                handle
                    .state::<crate::lifecycle::Lifecycle>()
                    .outcome()
                    .try_lock()
                    .expect("종료 기록")
                    .is_some(),
                "정리한 앱을 종료한다"
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
    assert_eq!(code, 0_i32, "조립 종료");
    *observed.read().expect("관측")
}

#[test]
fn build_app_provisions_the_key_when_the_composition_root_manages_a_provision() {
    let ports = startup_ready_ports();
    let secrets = Arc::clone(&ports.secrets);
    let count = Arc::new(AtomicUsize::new(0));
    let mut context = mock_context(noop_assets());
    context
        .config_mut()
        .app
        .windows
        .push(tauri::utils::config::WindowConfig::default());
    let app = build_app(
        mock_builder().manage(counting_provision(Arc::clone(&count))),
        ports,
        context,
    )
    .expect("모의 앱");
    assert_eq!(
        observe_database_status(app),
        Some(DatabaseStatus::Failed(SystemPortError::new(
            SystemPort::Repository,
            SystemFailure::InvalidData
        ))),
        "지원 팩토리까지 도달"
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    match secrets.calls().get(1) {
        Some(SecretCall::Store(SecretKey::DatabaseEncryptionKey, _)) => {}
        other => panic!("키 저장 필요: {other:?}"),
    }
}
