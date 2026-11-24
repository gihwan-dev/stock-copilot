use crate::estimates::codec::decode_outcome;
use crate::estimates_tests::{cal_key, seed_resolved_and_append_only_rows};
use crate::sql::estimates as sql;
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{Database, DatabasePath, EstimateStore};
use application::{EstimateSubmissionRepository, ScorecardObservationRepository};
use domain::{CalibrationEstimateId, InstrumentId, ScorecardObservation, ScorecardPromptId};

#[test]
fn scorecard_observations_come_from_resolved_submission_records() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = EstimateStore::new(&db);
    assert_eq!(
        rt.block_on(store.scorecard_observations()).expect("empty"),
        Vec::new()
    );
    let task_id = CalibrationEstimateId::try_new("scorecard-task").expect("과제");
    seed_resolved_and_append_only_rows(&rt, &db, &task_id);
    let records = rt
        .block_on(store.submission_records(&InstrumentId::try_new("est-e2e-a").expect("종목")))
        .expect("records");
    let record = records.first().expect("record");
    let outcome_payload = rt
        .block_on(db.read(|connection| {
            connection.query_row(sql::TASK_RESOLUTION_BY_ID, ["scorecard-task"], |row| {
                row.get::<_, String>(0)
            })
        }))
        .expect("outcome");
    let expected = ScorecardObservation::try_new(
        task_id,
        cal_key("e2e-model"),
        ScorecardPromptId::try_new("p").expect("프롬프트"),
        record.submission().probability(),
        decode_outcome(&outcome_payload).expect("decode"),
    )
    .expect("observation");
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    assert_eq!(
        rt.block_on(EstimateStore::new(&reopened).scorecard_observations())
            .expect("observations"),
        vec![expected]
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn scorecard_sql_prepares_on_the_migrated_database() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    connection
        .prepare(sql::RESOLVED_SUBMISSION_RECORDS)
        .expect("prepare");
}
