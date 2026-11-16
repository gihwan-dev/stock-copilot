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
