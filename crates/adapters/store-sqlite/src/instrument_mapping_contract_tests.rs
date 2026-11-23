use crate::{
    Database, DatabasePath, InstrumentMappingStore,
    tests::{Fixture, clock, key, runtime},
};
use application::InstrumentMappingRepository;
use domain::{
    InstrumentId, InstrumentMappingStatus, InstrumentSource, IsinCode, KnownMarket,
    MappingAssignment, Symbol,
};
use std::future::Future;
use testkit_ports::contracts::instrument_mapping::{self, ContractRuntime};

struct TokioRuntime;

impl ContractRuntime for TokioRuntime {
    fn block_on<F: Future>(&self, future: F) -> F::Output {
        runtime().block_on(future)
    }
}

fn open_db(fixture: &Fixture) -> Database {
    Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open database")
}

#[derive(Clone, Copy)]
enum RaceKind {
    SameSource,
    SameCandidate,
    OverlappingSources,
    EnsureAndRetire,
    RetireIdempotent,
}

impl RaceKind {
    const ALL: [Self; 5] = [
        Self::SameSource,
        Self::SameCandidate,
        Self::OverlappingSources,
        Self::EnsureAndRetire,
        Self::RetireIdempotent,
    ];

    fn run(self, first: &InstrumentMappingStore<'_>, second: &InstrumentMappingStore<'_>) {
        match self {
            Self::SameSource => {
                instrument_mapping::concurrent_same_source_has_one_winner(
                    first,
                    second,
                    &TokioRuntime,
                );
            }
            Self::SameCandidate => {
                instrument_mapping::concurrent_distinct_sources_same_candidate_has_one_winner(
                    first,
                    second,
                    &TokioRuntime,
                );
            }
            Self::OverlappingSources => {
                instrument_mapping::concurrent_overlapping_sources_has_one_winner(
                    first,
                    second,
                    &TokioRuntime,
                );
            }
            Self::EnsureAndRetire => {
                instrument_mapping::concurrent_ensure_and_retire_never_reactivates(
                    first,
                    second,
                    &TokioRuntime,
                );
            }
            Self::RetireIdempotent => {
                instrument_mapping::concurrent_retire_is_idempotent(first, second, &TokioRuntime);
            }
        }
    }
}

fn run_single_connection_race(kind: RaceKind) {
    let fixture = Fixture::new();
    let db = open_db(&fixture);
    let store = InstrumentMappingStore::new(&db);
    kind.run(&store, &store);
    TokioRuntime.block_on(db.close()).expect("close");
}

fn run_two_connection_race(kind: RaceKind) {
    let fixture = Fixture::new();
    let first_db = open_db(&fixture);
    let second_db = open_db(&fixture);
    let first_store = InstrumentMappingStore::new(&first_db);
    let second_store = InstrumentMappingStore::new(&second_db);
    kind.run(&first_store, &second_store);
    TokioRuntime.block_on(first_db.close()).expect("close");
    TokioRuntime.block_on(second_db.close()).expect("close");
}

fn seed_mappings_for_restart(
    fixture: &Fixture,
    kr: &InstrumentSource,
    kr_id: &InstrumentId,
    us: &InstrumentSource,
    us_id: &InstrumentId,
) {
    let db = open_db(fixture);
    let store = InstrumentMappingStore::new(&db);
    assert_eq!(
        TokioRuntime.block_on(store.ensure(kr, kr_id)),
        Ok(MappingAssignment::Issued(kr_id.clone()))
    );
    assert_eq!(
        TokioRuntime.block_on(store.ensure(us, us_id)),
        Ok(MappingAssignment::Issued(us_id.clone()))
    );
    assert_eq!(TokioRuntime.block_on(store.retire(us)), Ok(us_id.clone()));
    TokioRuntime.block_on(db.close()).expect("close");
}

#[test]
fn sqlite_store_passes_shared_lifecycle_contract() {
    let fixture = Fixture::new();
    let database = open_db(&fixture);
    let store = InstrumentMappingStore::new(&database);
    TokioRuntime.block_on(instrument_mapping::lifecycle_roundtrip(&store));
    TokioRuntime.block_on(database.close()).expect("close");
}

#[test]
fn sqlite_store_passes_shared_overlap_and_case_contract() {
    let fixture = Fixture::new();
    let database = open_db(&fixture);
    let store = InstrumentMappingStore::new(&database);
    TokioRuntime.block_on(instrument_mapping::uncertain_overlaps_and_case_variants(
        &store,
    ));
    TokioRuntime.block_on(database.close()).expect("close");
}

#[test]
fn sqlite_store_passes_shared_collision_contract() {
    let fixture = Fixture::new();
    let database = open_db(&fixture);
    let store = InstrumentMappingStore::new(&database);
    TokioRuntime.block_on(instrument_mapping::candidate_collisions_fail_closed(&store));
    TokioRuntime.block_on(database.close()).expect("close");
}

#[test]
fn sqlite_store_passes_shared_unpolled_drop_contract() {
    let fixture = Fixture::new();
    let database = open_db(&fixture);
    let store = InstrumentMappingStore::new(&database);
    TokioRuntime.block_on(instrument_mapping::dropping_unpolled_writes_does_not_mutate(&store));
    TokioRuntime.block_on(database.close()).expect("close");
}

#[test]
fn sqlite_store_passes_shared_reference_model_contract_and_reopen() {
    let sequences: &[&[(u8, u8, u8)]] = &[
        &[
            (0, 0, 0),
            (0, 2, 1),
            (0, 3, 1),
            (0, 5, 1),
            (0, 4, 1),
            (0, 1, 0),
            (0, 1, 2),
            (1, 0, 0),
            (0, 0, 2),
            (1, 2, 0),
            (1, 0, 0),
            (2, 0, 0),
            (2, 4, 1),
            (2, 1, 2),
        ],
        &[
            (1, 0, 0),
            (0, 5, 2),
            (0, 0, 1),
            (0, 2, 1),
            (0, 4, 1),
            (1, 5, 2),
            (0, 5, 0),
            (0, 3, 0),
            (2, 5, 2),
            (2, 3, 0),
        ],
    ];
    for (index, operations) in sequences.iter().enumerate() {
        let fixture = Fixture::new();
        let database = open_db(&fixture);
        let store = InstrumentMappingStore::new(&database);
        let salt = u16::try_from(index).expect("sequence index");
        TokioRuntime.block_on(instrument_mapping::reference_model_sequence(
            &store, salt, operations,
        ));
        TokioRuntime.block_on(database.close()).expect("close");
        let reopened = open_db(&fixture);
        let reopened_store = InstrumentMappingStore::new(&reopened);
        let check_source = InstrumentSource::new(
            KnownMarket::Kr,
            Symbol::try_new(&["S", &salt.to_string(), "A"].join("")).expect("symbol"),
            IsinCode::try_new(&["I", &salt.to_string(), "D"].join("")).expect("isin"),
        );
        let expected_id =
            InstrumentId::try_new(&["cand-", &salt.to_string(), "-1"].join("")).expect("id");
        assert_eq!(
            TokioRuntime.block_on(reopened_store.resolve(&check_source)),
            Ok(expected_id)
        );
        TokioRuntime.block_on(reopened.close()).expect("close");
    }
}

#[test]
fn sqlite_store_passes_shared_concurrency_contracts_on_single_connection() {
    for kind in RaceKind::ALL {
        run_single_connection_race(kind);
    }
}

#[test]
fn sqlite_store_passes_shared_concurrency_contracts_across_two_connections() {
    for _ in 0_u8..4 {
        for kind in RaceKind::ALL {
            run_two_connection_race(kind);
        }
    }
}

#[test]
fn sqlite_store_roundtrips_active_and_retired_mappings_across_restart() {
    let fixture = Fixture::new();
    let kr = InstrumentSource::new(
        KnownMarket::Kr,
        Symbol::try_new("005930").expect("kr symbol"),
        IsinCode::try_new("KR7005930003").expect("kr isin"),
    );
    let us = InstrumentSource::new(
        KnownMarket::Us,
        Symbol::try_new("brk.B").expect("us symbol"),
        IsinCode::try_new("us0846707026").expect("us isin"),
    );
    let kr_id = InstrumentId::try_new("inst-kr-001").expect("kr id");
    let us_id = InstrumentId::try_new("inst-us-002").expect("us id");

    seed_mappings_for_restart(&fixture, &kr, &kr_id, &us, &us_id);

    let reopened = open_db(&fixture);
    let store = InstrumentMappingStore::new(&reopened);
    assert_eq!(TokioRuntime.block_on(store.resolve(&kr)), Ok(kr_id.clone()));
    let kr_record = TokioRuntime.block_on(store.lookup(&kr)).expect("kr record");
    assert_eq!(kr_record.source(), &kr);
    assert_eq!(kr_record.id(), &kr_id);
    assert_eq!(kr_record.status(), InstrumentMappingStatus::Active);
    assert_eq!(
        TokioRuntime.block_on(store.for_id(&kr_id)),
        Ok(vec![kr_record])
    );

    let us_record = TokioRuntime.block_on(store.lookup(&us)).expect("us record");
    assert_eq!(us_record.source(), &us);
    assert_eq!(us_record.id(), &us_id);
    assert_eq!(us_record.status(), InstrumentMappingStatus::Retired);
    assert_eq!(
        TokioRuntime.block_on(store.for_id(&us_id)),
        Ok(vec![us_record])
    );
    TokioRuntime.block_on(reopened.close()).expect("close");
}
