#[cfg(test)]
mod tests {
    use application::{
        RetryDelay, SecretKey, SecretStore, SecretValue, SystemFailure, SystemPort,
        SystemPortError, WriteCompleted,
    };
    use secrets_keychain::{
        AccessGroup, BuildKind, KeychainBackend, KeychainFailure, KeychainItem, KeychainKind,
        KeychainProfile, KeychainSecretStore, ServiceName, generate_database_key,
    };
    use std::future::Future;
    use std::sync::{PoisonError, RwLock, RwLockWriteGuard};
    use std::task::{Context, Poll, Waker};
    use testkit_ports::contracts;

    fn guard<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
        match lock.write() {
            Ok(guard) => guard,
            Err(poisoned) => PoisonError::into_inner(poisoned),
        }
    }

    const FAKE_SECRET: &[u8] = b"fake-secret-7f3a9c-never-log";

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Operation {
        Read,
        Write,
        Delete,
    }

    #[derive(Debug, Default)]
    struct MemoryKeychain {
        items: RwLock<Vec<(KeychainItem, Vec<u8>)>>,
        touched: RwLock<Vec<(Operation, KeychainItem)>>,
        failure: RwLock<Option<KeychainFailure>>,
    }

    impl MemoryKeychain {
        fn failing(failure: KeychainFailure) -> Self {
            let keychain = Self::default();
            *guard(&keychain.failure) = Some(failure);
            keychain
        }
        fn record(&self, operation: Operation, item: &KeychainItem) -> Result<(), KeychainFailure> {
            guard(&self.touched).push((operation, item.clone()));
            let scripted = *guard(&self.failure);
            match scripted {
                Some(failure) => Err(failure),
                None => Ok(()),
            }
        }
        fn touched(&self) -> Vec<(Operation, KeychainItem)> {
            guard(&self.touched).clone()
        }
    }

    impl KeychainBackend for MemoryKeychain {
        fn read(&self, item: &KeychainItem) -> Result<Option<Vec<u8>>, KeychainFailure> {
            self.record(Operation::Read, item)?;
            Ok(guard(&self.items)
                .iter()
                .find(|(stored, _)| stored == item)
                .map(|(_, value)| value.clone()))
        }
        fn write(&self, item: &KeychainItem, value: &[u8]) -> Result<(), KeychainFailure> {
            self.record(Operation::Write, item)?;
            let mut items = guard(&self.items);
            items.retain(|(stored, _)| stored != item);
            items.push((item.clone(), value.to_vec()));
            Ok(())
        }
        fn delete(&self, item: &KeychainItem) -> Result<(), KeychainFailure> {
            self.record(Operation::Delete, item)?;
            guard(&self.items).retain(|(stored, _)| stored != item);
            Ok(())
        }
    }

    const ALL_KEYS: [SecretKey; 7] = [
        SecretKey::TossClientId,
        SecretKey::TossClientSecret,
        SecretKey::TossAccessToken,
        SecretKey::DatabaseEncryptionKey,
        SecretKey::AuditHmacKey,
        SecretKey::DartApiKey,
        SecretKey::FredApiKey,
    ];

    #[test]
    fn store_load_replace_and_remove_follow_the_port_contract() {
        let store =
            KeychainSecretStore::new(KeychainProfile::development(), MemoryKeychain::default());
        run(contracts::secret_store_load(
            &store,
            SecretKey::TossClientSecret,
            Ok(None),
        ));
        run(contracts::secret_store_store(
            &store,
            SecretKey::TossClientSecret,
            SecretValue::new(FAKE_SECRET.to_vec()),
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_store(
            &store,
            SecretKey::DartApiKey,
            SecretValue::new(vec![1, 2, 3]),
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_load(
            &store,
            SecretKey::TossClientSecret,
            Ok(Some(SecretValue::new(FAKE_SECRET.to_vec()))),
        ));
        run(contracts::secret_store_store(
            &store,
            SecretKey::TossClientSecret,
            SecretValue::new(vec![9]),
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_load(
            &store,
            SecretKey::TossClientSecret,
            Ok(Some(SecretValue::new(vec![9]))),
        ));
        run(contracts::secret_store_remove(
            &store,
            SecretKey::TossClientSecret,
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_remove(
            &store,
            SecretKey::TossClientSecret,
            Ok(WriteCompleted),
        ));
        run(contracts::secret_store_load(
            &store,
            SecretKey::TossClientSecret,
            Ok(None),
        ));
        run(contracts::secret_store_load(
            &store,
            SecretKey::DartApiKey,
            Ok(Some(SecretValue::new(vec![1, 2, 3]))),
        ));
    }

    #[test]
    fn every_secret_key_has_its_own_account_under_one_service() {
        let profile = KeychainProfile::product();
        let mut accounts: Vec<&str> = ALL_KEYS
            .iter()
            .map(|key| profile.item(*key).account())
            .collect();
        accounts.sort_unstable();
        accounts.dedup();
        assert_eq!(accounts.len(), ALL_KEYS.len());
        for key in ALL_KEYS {
            assert_eq!(profile.item(key).service(), &ServiceName::PRODUCT);
        }
    }

    #[test]
    fn development_profile_never_touches_the_product_service() {
        assert_ne!(ServiceName::DEVELOPMENT, ServiceName::PRODUCT);
        assert!(
            !ServiceName::DEVELOPMENT
                .as_str()
                .eq_ignore_ascii_case(ServiceName::PRODUCT.as_str())
        );
        let backend = MemoryKeychain::default();
        let store = KeychainSecretStore::new(KeychainProfile::development(), backend);
        for key in ALL_KEYS {
            let _stored = run(store.store(key, SecretValue::new(FAKE_SECRET.to_vec())));
            let _loaded = run(store.load(key));
            let _removed = run(store.remove(key));
        }
        let touched = store.backend().touched();
        assert_eq!(touched.len(), ALL_KEYS.len() * 3);
        for (_operation, item) in touched {
            assert_eq!(item.service(), &ServiceName::DEVELOPMENT);
            assert_eq!(item.kind(), KeychainKind::Login);
        }
    }

    #[test]
    fn development_builds_use_the_development_profile_and_release_uses_data_protection() {
        assert_eq!(
            KeychainProfile::for_build(BuildKind::Development),
            KeychainProfile::development()
        );
        assert_eq!(
            KeychainProfile::for_build(BuildKind::Release),
            KeychainProfile::product()
        );
        let product = KeychainProfile::product();
        assert_eq!(product.kind(), KeychainKind::DataProtection);
        assert_eq!(product.access_group(), &AccessGroup::EntitlementDefault);
        let named = KeychainProfile::product()
            .with_access_group(AccessGroup::Named("TEAMID.group".to_owned()));
        assert_eq!(
            named.item(SecretKey::DatabaseEncryptionKey).access_group(),
            &AccessGroup::Named("TEAMID.group".to_owned())
        );
    }

    #[test]
    fn backend_failures_map_to_secret_store_errors_without_values() {
        let cases = [
            (
                KeychainFailure::MissingEntitlement,
                SystemFailure::PermissionDenied,
            ),
            (KeychainFailure::Denied, SystemFailure::PermissionDenied),
            (
                KeychainFailure::Locked,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            ),
            (
                KeychainFailure::Unavailable,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            ),
            (KeychainFailure::Malformed, SystemFailure::SecretIntegrity),
        ];
        for (failure, expected) in cases {
            let store = KeychainSecretStore::new(
                KeychainProfile::development(),
                MemoryKeychain::failing(failure),
            );
            let error = SystemPortError::new(SystemPort::SecretStore, expected);
            run(contracts::secret_store_load(
                &store,
                SecretKey::TossAccessToken,
                Err(error),
            ));
            let stored = run(store.store(
                SecretKey::TossAccessToken,
                SecretValue::new(FAKE_SECRET.to_vec()),
            ));
            assert_eq!(stored, Err(error));
            run(contracts::secret_store_remove(
                &store,
                SecretKey::TossAccessToken,
                Err(error),
            ));
            assert_no_secret(&format!("{stored:?} {store:?}"));
        }
    }

    #[test]
    fn debug_output_of_store_and_loaded_value_hides_the_secret() {
        let store =
            KeychainSecretStore::new(KeychainProfile::development(), MemoryKeychain::default());
        let _stored = run(store.store(
            SecretKey::TossClientSecret,
            SecretValue::new(FAKE_SECRET.to_vec()),
        ));
        let loaded = run(store.load(SecretKey::TossClientSecret));
        assert_no_secret(&format!("{loaded:?}"));
        assert_no_secret(&format!("{:?}", store.profile()));
    }

    #[test]
    fn generated_database_keys_are_32_random_bytes() {
        let first = generate_database_key().expect("난수");
        let second = generate_database_key().expect("난수");
        assert_eq!(first.as_exposed_secret().len(), 32);
        assert_ne!(first.as_exposed_secret(), second.as_exposed_secret());
        assert_no_secret(&format!("{first:?}"));
    }

    fn assert_no_secret(text: &str) {
        let needle = String::from_utf8_lossy(FAKE_SECRET);
        assert!(!text.contains(needle.as_ref()), "비밀 노출: {text}");
    }
}
