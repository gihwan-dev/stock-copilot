use crate::sql::{status as status_sql, status_tests as sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{Database, DatabasePath, NewsDataSettingsStore, SourceStatusStore, SpecStatusStore};
use application::{
    AliasName, CollectionMinutes, CollectionTarget, CredentialSource, HttpLastModified,
    InformationError, NewsDataSettings, NewsDataSettingsChange, NewsDataSettingsRepository,
    ResetTime, ResponseMismatch, RetryDelay, SchemaReviewState, SecContactSetting,
    SourceAttemptOutcome, SourceState, SourceStatus, SourceStatusRepository, SourceSwitch,
    SpecChangeReason, SpecDocumentSnapshot, SpecDriftStatus, SpecFetchError, SpecHash,
    SpecStatusRecord, SpecVersionLabel, StockAlias, SystemFailure, TossSpecBaseline,
    TossSpecObservation, TossSpecStatusRepository,
};
use domain::{
    AiBodyReadToggle, DataSource, InstrumentId, MovePercent, Nanosecond, NewsBurstCount,
    NewsSource, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds, WakeThresholds,
};
use testkit_ports::spec_check::MemoryTossSpecStatusRepository;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(7),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn statuses() -> Vec<SourceStatus> {
    vec![
        SourceStatus::initial(DataSource::Fred),
        SourceStatus::initial(DataSource::KindDisclosure)
            .apply_attempt(at(10), SourceAttemptOutcome::Succeeded),
        SourceStatus::initial(DataSource::GoogleNews)
            .apply_attempt(at(11), SourceAttemptOutcome::NoData),
        SourceStatus::initial(DataSource::DartDisclosureApi)
            .apply_attempt(at(12), SourceAttemptOutcome::NotConfigured),
        SourceStatus::initial(DataSource::Yonhap)
            .apply_attempt(at(13), SourceAttemptOutcome::Succeeded)
            .apply_attempt(
                at(14),
                SourceAttemptOutcome::Failed(InformationError::Unavailable(ResetTime::Known(at(
                    99,
                )))),
            ),
        SourceStatus::initial(DataSource::SecEdgar).apply_attempt(
            at(15),
            SourceAttemptOutcome::Failed(InformationError::InvalidResponse(
                ResponseMismatch::Validation,
            )),
        ),
        SourceStatus::initial(DataSource::DartFundamentals).apply_attempt(
            at(16),
            SourceAttemptOutcome::Failed(InformationError::InvalidCredential(
                CredentialSource::Dart,
            )),
        ),
        SourceStatus::initial(DataSource::UsTreasury).apply_attempt(
            at(17),
            SourceAttemptOutcome::Failed(InformationError::Unavailable(ResetTime::NotProvided)),
        ),
    ]
}

#[test]
fn source_statuses_round_trip_replace_and_survive_reopen() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = SourceStatusStore::new(&db);
    assert_eq!(
        rt.block_on(store.find(DataSource::Fred)).expect("empty"),
        None
    );
    assert_eq!(rt.block_on(store.list_all()).expect("empty"), Vec::new());
    for status in statuses() {
        rt.block_on(store.save(&status)).expect("save");
    }
    let replaced = SourceStatus::initial(DataSource::KindDisclosure)
        .apply_attempt(at(10), SourceAttemptOutcome::Succeeded)
        .apply_attempt(
            at(20),
            SourceAttemptOutcome::Failed(InformationError::InvalidResponse(
                ResponseMismatch::Schema,
            )),
        );
    rt.block_on(store.save(&replaced)).expect("replace");
    let mut expected = statuses();
    expected[1] = replaced;
    expected.sort_by_key(SourceStatus::source);
    assert_eq!(rt.block_on(store.list_all()).expect("all"), expected);
    assert_eq!(
        rt.block_on(store.find(DataSource::KindDisclosure))
            .expect("find")
            .map(|status| status.state()),
        Some(SourceState::Failed(InformationError::InvalidResponse(
            ResponseMismatch::Schema
        )))
    );
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    assert_eq!(
        rt.block_on(SourceStatusStore::new(&reopened).list_all())
            .expect("all"),
        expected
    );
    rt.block_on(reopened.close()).expect("close");
}

fn spec_hash(fill: char) -> SpecHash {
    SpecHash::try_new(&fill.to_string().repeat(64)).expect("hash")
}

fn baseline() -> TossSpecBaseline {
    TossSpecBaseline::new(
        spec_hash('a'),
        spec_hash('b'),
        SpecVersionLabel::try_new("v1.2").expect("label"),
    )
    .with_versions_hash(spec_hash('c'))
    .with_spec_last_modified(
        HttpLastModified::try_new("Tue, 24 Nov 2026 10:00:00 GMT").expect("modified"),
    )
}

fn observation() -> TossSpecObservation {
    TossSpecObservation::new(
        SpecDocumentSnapshot::new(
            spec_hash('d'),
            Some(SpecVersionLabel::try_new("v1.3").expect("label")),
            None,
        ),
        SpecDocumentSnapshot::new(
            spec_hash('e'),
            None,
            Some(HttpLastModified::try_new("Wed, 25 Nov 2026 01:00:00 GMT").expect("modified")),
        ),
    )
}

fn spec_records() -> Vec<SpecStatusRecord> {
    let span = TimeSpan::try_new(SpanSeconds::new(30), Nanosecond::new(5)).expect("span");
    vec![
        SpecStatusRecord::new(
            at(1),
            baseline(),
            Some(observation()),
            SpecDriftStatus::Aligned,
        ),
        SpecStatusRecord::new(
            at(2),
            baseline(),
            Some(observation()),
            SpecDriftStatus::DriftDetected {
                reasons: vec![
                    SpecChangeReason::NormalizedHash,
                    SpecChangeReason::VersionsCatalog,
                    SpecChangeReason::LastModified,
                ],
                review: SchemaReviewState::PendingTypeDiffReview,
            },
        ),
        SpecStatusRecord::new(
            at(3),
            baseline(),
            None,
            SpecDriftStatus::CheckFailed(SpecFetchError::RateLimited(RetryDelay::Known(span))),
        ),
        SpecStatusRecord::new(
            at(4),
            TossSpecBaseline::new(
                spec_hash('1'),
                spec_hash('2'),
                SpecVersionLabel::try_new("v2").expect("label"),
            ),
            None,
            SpecDriftStatus::CheckFailed(SpecFetchError::ResponseTooLarge),
        ),
    ]
}

#[test]
fn spec_status_records_append_and_match_the_memory_contract() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = SpecStatusStore::new(&db);
    let fake = MemoryTossSpecStatusRepository::default();
    assert_eq!(rt.block_on(store.latest()).expect("empty"), None);
    for record in spec_records() {
        rt.block_on(store.record(&record)).expect("record");
        rt.block_on(fake.record(&record)).expect("fake");
        assert_eq!(
            rt.block_on(store.latest()).expect("latest"),
            rt.block_on(fake.latest()).expect("fake latest")
        );
    }
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    let read = |statement: &'static str| {
        rt.block_on(reopened.read(move |connection| {
            connection.query_row(statement, [], |row| row.get::<_, i64>(0))
        }))
        .expect("count")
    };
    assert_eq!(read(sql::SPEC_COUNT), 4);
    assert_eq!(
        rt.block_on(SpecStatusStore::new(&reopened).latest())
            .expect("latest"),
        spec_records().pop()
    );
    rt.block_on(reopened.close()).expect("close");
}

fn news_settings() -> NewsDataSettings {
    let mut settings = NewsDataSettings::initial();
    for change in [
        NewsDataSettingsChange::Source(DataSource::GoogleNews, SourceSwitch::Off),
        NewsDataSettingsChange::Source(DataSource::Fred, SourceSwitch::Off),
        NewsDataSettingsChange::BodyRead(NewsSource::Hankyung, AiBodyReadToggle::Enabled),
        NewsDataSettingsChange::BodyRead(NewsSource::YahooFinance, AiBodyReadToggle::Enabled),
        NewsDataSettingsChange::SecContact(Some(
            SecContactSetting::try_new("Stock Copilot dev@example.com").expect("contact"),
        )),
        NewsDataSettingsChange::Cadence(
            CollectionTarget::PressRss,
            Some(CollectionMinutes::try_new(45).expect("minutes")),
        ),
        NewsDataSettingsChange::Wake(WakeThresholds {
            daily_move: MovePercent::try_new(7).expect("move"),
            news_per_hour: NewsBurstCount::try_new(12).expect("burst"),
        }),
        NewsDataSettingsChange::AddAlias(StockAlias {
            instrument: InstrumentId::try_new("instrument-b").expect("id"),
            alias: AliasName::try_new("삼전").expect("alias"),
        }),
        NewsDataSettingsChange::AddAlias(StockAlias {
            instrument: InstrumentId::try_new("instrument-a").expect("id"),
            alias: AliasName::try_new("하닉").expect("alias"),
        }),
    ] {
        settings.apply(change).expect("change");
    }
    settings
}

#[test]
fn news_data_settings_round_trip_replace_and_survive_reopen() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = NewsDataSettingsStore::new(&db);
    assert_eq!(rt.block_on(store.load()).expect("empty"), None);
    rt.block_on(store.store(&NewsDataSettings::initial()))
        .expect("initial");
    assert_eq!(
        rt.block_on(store.load()).expect("initial"),
        Some(NewsDataSettings::initial())
    );
    rt.block_on(store.store(&news_settings())).expect("store");
    assert_eq!(
        rt.block_on(store.load()).expect("load"),
        Some(news_settings())
    );
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    let loaded = rt
        .block_on(NewsDataSettingsStore::new(&reopened).load())
        .expect("load")
        .expect("settings");
    assert_eq!(loaded, news_settings());
    assert_eq!(
        loaded
            .aliases()
            .iter()
            .map(|alias| alias.alias.as_str())
            .collect::<Vec<_>>(),
        vec!["삼전", "하닉"]
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn status_tables_reject_deletes_rewrites_and_unknown_rows() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    rt.block_on(SourceStatusStore::new(&db).save(&statuses()[1]))
        .expect("source");
    rt.block_on(SpecStatusStore::new(&db).record(&spec_records()[0]))
        .expect("spec");
    rt.block_on(NewsDataSettingsStore::new(&db).store(&news_settings()))
        .expect("news");
    for statement in [
        sql::DELETE_SOURCE,
        sql::UNKNOWN_SOURCE,
        sql::UPDATE_SPEC,
        sql::DELETE_SPEC,
        sql::DELETE_NEWS,
        sql::SECOND_NEWS,
    ] {
        assert!(
            rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
                .is_err(),
            "{statement}"
        );
    }
    for statement in [sql::CORRUPT_SOURCE, sql::FUTURE_NEWS] {
        rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
            .expect(statement);
    }
    assert_eq!(
        rt.block_on(SourceStatusStore::new(&db).list_all())
            .map_err(application::SystemPortError::failure),
        Err(SystemFailure::RepositoryIntegrity)
    );
    assert_eq!(
        rt.block_on(NewsDataSettingsStore::new(&db).load())
            .map_err(application::SystemPortError::failure),
        Err(SystemFailure::RepositoryIntegrity)
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn status_sql_constants_match_the_migration_and_prepare() {
    assert_eq!(
        std::fs::read_to_string("migrations/0016_source_and_spec_status.sql").expect("file"),
        status_sql::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [
        status_sql::FIND_SOURCE,
        status_sql::ALL_SOURCES,
        status_sql::UPSERT_SOURCE,
        status_sql::INSERT_SPEC,
        status_sql::LATEST_SPEC,
        status_sql::FIND_NEWS,
        status_sql::UPSERT_NEWS,
        sql::SPEC_COUNT,
    ] {
        connection.prepare(statement).expect(statement);
    }
}
