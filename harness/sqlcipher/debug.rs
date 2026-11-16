#[cfg(test)]
mod tests {
    struct FixedClock;
    impl application::Clock for FixedClock {
        fn now(&self) -> domain::Timestamp {
            domain::Timestamp::from_parts(
                domain::UnixSeconds::new(1_723_456_789),
                domain::Nanosecond::new(0),
                domain::OffsetSeconds::new(0),
            )
            .expect("fixed timestamp")
        }
    }

    #[test]
    fn secret_debug_contract() {
        use application::{SecretValue, SystemFailure, SystemPort, SystemPortError};
        use store_sqlite::{Database, DatabasePath};
        std::fs::create_dir_all("target/S-01").expect("probe directory");
        let path = std::path::PathBuf::from("target/S-01/secret-debug-cargo.db");
        let key = SecretValue::new((1..=32).collect());
        {
            let database =
                Database::open(&DatabasePath::new(path.clone()), &key, &FixedClock).expect("open");
            assert_eq!(format!("{database:?}"), "Database(<redacted>)");
            assert_eq!(format!("{database:#?}"), "Database(<redacted>)");
        }
        let error = Database::open(
            &DatabasePath::new(path.clone()),
            &SecretValue::new(vec![99; 32]),
            &FixedClock,
        )
        .expect_err("wrong key");
        assert_eq!(
            format!("{error:?}"),
            "SystemPortError { port: Repository, failure: RepositoryIntegrity }"
        );
        assert_eq!(
            error,
            SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity)
        );
        assert_eq!(format!("{key:?}"), "SecretValue(<redacted>)");
        std::fs::remove_file(path).expect("cleanup");
    }
}
