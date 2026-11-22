use crate::{
    Database, DatabasePath, ProposalConsumptionStore,
    tests::{Fixture, clock, key, runtime},
};
use application::{ConsumptionOutcome, ConsumptionStatus, ProposalConsumptionRepository};
use domain::{ClientOrderId, ProposalId};
use std::sync::{Arc, Barrier};

fn proposal(value: &str) -> ProposalId {
    ProposalId::new(ClientOrderId::try_new(value).expect("proposal id"))
}

#[test]
fn consumption_is_unique_and_isolated_by_proposal() {
    let fixture = Fixture::new();
    let rt = runtime();
    let path = DatabasePath::new(fixture.path());
    let first = proposal("proposal-1");
    let second = proposal("proposal-2");
    let close = {
        let database = Database::open(&path, &key(), &clock()).expect("open");
        let store = ProposalConsumptionStore::new(&database);
        assert_eq!(
            rt.block_on(store.status(&first)),
            Ok(ConsumptionStatus::NotConsumed)
        );
        assert_eq!(
            rt.block_on(store.consume(&first)),
            Ok(ConsumptionOutcome::Recorded)
        );
        assert_eq!(
            rt.block_on(store.consume(&first)),
            Ok(ConsumptionOutcome::AlreadyConsumed)
        );
        rt.block_on(database.write(|tx| {
            tx.execute(crate::sql::proposal::INSERT, ["proposal-1"])?;
            Ok(())
        }))
        .expect_err("database rejects duplicate");
        assert_eq!(
            rt.block_on(store.status(&second)),
            Ok(ConsumptionStatus::NotConsumed)
        );
        rt.block_on(database.close())
    };
    close.expect("close");
    let database = Database::open(&path, &key(), &clock()).expect("reopen");
    let store = ProposalConsumptionStore::new(&database);
    assert_eq!(
        rt.block_on(store.status(&first)),
        Ok(ConsumptionStatus::Consumed)
    );
    assert_eq!(
        rt.block_on(store.status(&second)),
        Ok(ConsumptionStatus::NotConsumed)
    );
}

#[test]
fn two_database_connections_cannot_consume_one_proposal_twice() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let first = Database::open(&path, &key(), &clock()).expect("first connection");
    let second = Database::open(&path, &key(), &clock()).expect("second connection");
    let barrier = Arc::new(Barrier::new(2));
    let results = std::thread::scope(|scope| {
        let left_barrier = Arc::clone(&barrier);
        let left_db = &first;
        let right_db = &second;
        let left = scope.spawn(move || {
            let store = ProposalConsumptionStore::new(left_db);
            left_barrier.wait();
            runtime().block_on(store.consume(&proposal("race-1")))
        });
        let right = scope.spawn(move || {
            let store = ProposalConsumptionStore::new(right_db);
            barrier.wait();
            runtime().block_on(store.consume(&proposal("race-1")))
        });
        (left.join().expect("thread"), right.join().expect("thread"))
    });
    match results {
        (Ok(ConsumptionOutcome::Recorded), Ok(ConsumptionOutcome::AlreadyConsumed))
        | (Ok(ConsumptionOutcome::AlreadyConsumed), Ok(ConsumptionOutcome::Recorded)) => {}
        other => panic!("expected exactly one success, got {other:?}"),
    }
    let store = ProposalConsumptionStore::new(&first);
    assert_eq!(
        runtime().block_on(store.status(&proposal("race-1"))),
        Ok(ConsumptionStatus::Consumed)
    );
}

#[test]
fn consumption_records_cannot_be_changed_or_deleted() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ProposalConsumptionStore::new(&db);
    let id = proposal("proposal-1");
    rt.block_on(store.consume(&id)).expect("consume");
    for statement in [
        crate::sql::proposal_tests::UPDATE_CONSUMPTION,
        crate::sql::proposal_tests::DELETE_CONSUMPTION,
    ] {
        rt.block_on(db.write(move |tx| {
            tx.execute(statement, ["proposal-1"])?;
            Ok(())
        }))
        .expect_err("append only");
    }
    assert_eq!(
        rt.block_on(store.status(&id)),
        Ok(ConsumptionStatus::Consumed)
    );
}

#[test]
fn proposal_sql_statements_prepare_on_migrated_database() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("migration");
    for statement in [
        crate::sql::proposal::INSERT,
        crate::sql::proposal::FIND,
        crate::sql::proposal::INSERT_REPORT,
        crate::sql::proposal::REPORTS,
    ] {
        connection.prepare(statement).expect("prepare");
    }
}

#[test]
fn unrelated_database_failures_are_not_reported_as_consumption() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ProposalConsumptionStore::new(&database);
    rt.block_on(database.write(|tx| tx.execute_batch(crate::sql::proposal_tests::OTHER_UNIQUE)))
        .expect("index fixture");
    let first = proposal("same-a");
    let second = proposal("same-b");
    assert_eq!(
        rt.block_on(store.consume(&first)),
        Ok(ConsumptionOutcome::Recorded)
    );
    rt.block_on(store.consume(&second))
        .expect_err("unrelated uniqueness failure");
    assert_eq!(
        rt.block_on(store.status(&second)),
        Ok(ConsumptionStatus::NotConsumed)
    );
}

#[test]
fn trigger_failure_does_not_record_consumption() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ProposalConsumptionStore::new(&database);
    let id = proposal("proposal-1");
    rt.block_on(
        database.write(|tx| tx.execute_batch(crate::sql::proposal_tests::FAIL_CONSUMPTION)),
    )
    .expect("trigger fixture");
    rt.block_on(store.consume(&id))
        .expect_err("trigger failure");
    assert_eq!(
        rt.block_on(store.status(&id)),
        Ok(ConsumptionStatus::NotConsumed)
    );
}

#[test]
fn sqlite_rejects_conflict_replacement_of_consumption() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ProposalConsumptionStore::new(&database);
    let id = proposal("proposal-1");
    rt.block_on(store.consume(&id)).expect("consume");
    rt.block_on(database.write(|tx| {
        tx.execute(
            crate::sql::proposal_tests::REPLACE_CONSUMPTION,
            ["proposal-1"],
        )?;
        Ok(())
    }))
    .expect_err("consumption cannot be replaced");
    assert_eq!(
        rt.block_on(store.status(&id)),
        Ok(ConsumptionStatus::Consumed)
    );
}
