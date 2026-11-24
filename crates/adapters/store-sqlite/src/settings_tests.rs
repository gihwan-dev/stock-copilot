use crate::sql::{settings as settings_sql, settings_tests as sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{Database, DatabasePath, SettingsStore, TossSpecHash, TossSpecHashError};
use application::{SettingsRepository, SystemFailure};
use domain::{
    HoursPermissionState, InvestorProfile, OrderPermission, RiskAppetite, RiskSettings,
    SavedMarketOrderSetting, WeightCaps,
};
use testkit_ports::memory::{MemoryReply, MemorySettingsRepository};

type Observed = (
    Option<InvestorProfile>,
    Option<RiskSettings>,
    Option<RiskSettings>,
    Option<InvestorProfile>,
);

fn risk() -> RiskSettings {
    RiskSettings::new(
        OrderPermission::Enabled,
        SavedMarketOrderSetting::Enabled,
        HoursPermissionState::Enabled,
        WeightCaps::default(),
        RiskAppetite::Conservative,
    )
}

async fn exercise<R: SettingsRepository>(repository: &R) -> Observed {
    assert_eq!(repository.profile().await.expect("empty"), None);
    assert_eq!(repository.risk_settings().await.expect("empty"), None);
    let profile = InvestorProfile::default().with_risk_appetite(RiskAppetite::Aggressive);
    repository.store_profile(&profile).await.expect("profile");
    let first_profile = repository.profile().await.expect("profile");
    let derived = repository.risk_settings().await.expect("risk");
    repository.store_risk_settings(&risk()).await.expect("risk");
    (
        first_profile,
        derived,
        repository.risk_settings().await.expect("risk"),
        repository.profile().await.expect("profile"),
    )
}

#[test]
fn settings_round_trip_and_match_the_memory_contract() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let stored = rt.block_on(exercise(&SettingsStore::new(&db)));
    let fake = MemorySettingsRepository::default();
    for _ in 0_u8..4 {
        fake.script_store_profile().push(MemoryReply::Memory);
        fake.script_store_risk_settings().push(MemoryReply::Memory);
        fake.script_profile().push(MemoryReply::Memory);
        fake.script_risk_settings().push(MemoryReply::Memory);
    }
    let memory = rt.block_on(exercise(&fake));
    assert_eq!(stored, memory);
    assert_eq!(stored.2, Some(risk()));
    assert_eq!(
        stored.3.expect("profile").risk_appetite(),
        RiskAppetite::Conservative
    );
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    let store = SettingsStore::new(&reopened);
    assert_eq!(
        rt.block_on(store.risk_settings()).expect("risk"),
        Some(risk())
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn toss_spec_hash_is_kept_and_validated() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = SettingsStore::new(&db);
    assert_eq!(rt.block_on(store.toss_spec_hash()).expect("empty"), None);
    for raw in ["a".repeat(64), "b".repeat(64)] {
        let hash = TossSpecHash::try_new(&raw).expect("hash");
        rt.block_on(store.store_toss_spec_hash(&hash))
            .expect("store");
        assert_eq!(
            rt.block_on(store.toss_spec_hash()).expect("load"),
            Some(hash)
        );
    }
    for bad in ["A".repeat(64), "a".repeat(63), String::new()] {
        assert_eq!(
            TossSpecHash::try_new(&bad),
            Err(TossSpecHashError::InvalidFormat)
        );
    }
    rt.block_on(db.close()).expect("close");
}

#[test]
fn unknown_versions_corrupt_payloads_and_deletes_are_rejected() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = SettingsStore::new(&db);
    rt.block_on(store.store_risk_settings(&risk()))
        .expect("risk");
    for statement in [sql::DELETE_ALL, sql::UNKNOWN_NAME] {
        assert!(
            rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
                .is_err(),
            "{statement}"
        );
    }
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::FUTURE_VERSION)))
        .expect("future");
    assert_eq!(
        rt.block_on(store.risk_settings())
            .expect_err("future version")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::CORRUPT_PROFILE)))
        .expect("corrupt");
    assert_eq!(
        rt.block_on(store.profile()).expect_err("corrupt").failure(),
        SystemFailure::RepositoryIntegrity
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn settings_sql_constants_match_the_migration_and_prepare() {
    assert_eq!(
        std::fs::read_to_string("migrations/0014_settings.sql").expect("file"),
        settings_sql::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [
        settings_sql::FIND,
        settings_sql::UPSERT,
        sql::FUTURE_VERSION,
        sql::CORRUPT_PROFILE,
        sql::DELETE_ALL,
        sql::UNKNOWN_NAME,
    ] {
        connection.prepare(statement).expect(statement);
    }
}
