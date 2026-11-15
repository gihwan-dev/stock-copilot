use super::{Fixture, key};
use crate::{Database, DatabasePath};
use application::SystemFailure;
use std::os::unix::fs::PermissionsExt;

#[test]
fn readonly_parent_rejects_new_and_existing_database_with_permission_action() {
    let fixture = Fixture::new();
    let parent = fixture.0.join("readonly-parent");
    std::fs::create_dir_all(&parent).expect("parent");
    let path = DatabasePath::new(parent.join("store.db"));
    for state in ["new", "existing"] {
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o500))
            .expect("deny creation");
        let result = Database::open(&path, &key());
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700))
            .expect("restore before assertion");
        assert_eq!(
            result.expect_err(state).failure(),
            SystemFailure::PermissionDenied
        );
        let _database = Database::open(&path, &key()).expect("create writable database");
    }
}

#[test]
fn readonly_database_file_is_rejected_before_a_successful_health_result() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    {
        let _database = Database::open(&path, &key()).expect("create database");
    }
    std::fs::set_permissions(path.as_path(), std::fs::Permissions::from_mode(0o400))
        .expect("read only");
    let result = Database::open(&path, &key());
    std::fs::set_permissions(path.as_path(), std::fs::Permissions::from_mode(0o600))
        .expect("restore before assertion");
    assert_eq!(
        result.expect_err("writer must be writable").failure(),
        SystemFailure::PermissionDenied
    );
}

#[test]
fn a_directory_or_non_directory_parent_is_a_storage_layout_failure() {
    let fixture = Fixture::new();
    assert_eq!(
        Database::open(&DatabasePath::new(fixture.0.clone()), &key())
            .expect_err("directory is not a database file")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    let component = fixture.0.join("file-parent");
    std::fs::write(&component, b"not a directory").expect("file");
    assert_eq!(
        Database::open(&DatabasePath::new(component.join("store.db")), &key())
            .expect_err("parent is not a directory")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
}

#[test]
fn invalid_key_length_creates_no_database_file() {
    let fixture = Fixture::new();
    assert_eq!(
        Database::open(
            &DatabasePath::new(fixture.path()),
            &application::SecretValue::new(vec![9; 31]),
        )
        .expect_err("invalid key")
        .failure(),
        SystemFailure::SecretIntegrity
    );
    assert!(!fixture.path().exists());
}

#[test]
fn file_preparation_is_owner_only_and_preserves_existing_encrypted_data() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let created_mode = {
        let database = Database::open(&path, &key()).expect("new database");
        super::runtime()
            .block_on(database.write(|transaction| {
                transaction.execute_batch(crate::sql::tests::CREATE_TEST)?;
                transaction.execute(
                    crate::sql::tests::INSERT_TEST,
                    ["s01-reopen-preserved-41789"],
                )?;
                Ok(())
            }))
            .expect("write marker");
        std::fs::metadata(path.as_path())
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777_u32
    };
    assert_eq!(created_mode, 0o600_u32);
    let database = Database::open(&path, &key()).expect("existing database");
    assert_eq!(
        super::runtime()
            .block_on(database.read(|connection| {
                connection.query_row(crate::sql::tests::VALUE_TEST, [], |row| {
                    row.get::<_, String>(0)
                })
            }))
            .expect("preserved marker"),
        "s01-reopen-preserved-41789"
    );
}
