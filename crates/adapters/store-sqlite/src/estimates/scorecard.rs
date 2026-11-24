use super::{EstimateStore, codec::decode_outcome, codec::invalid, decode_joined_record};
use crate::sql::estimates as sql;
use application::{ScorecardObservationRepository, SystemPortError};
use domain::ScorecardObservation;
use rusqlite::Connection;
use std::future::Future;

fn load_observations(connection: &Connection) -> rusqlite::Result<Vec<ScorecardObservation>> {
    let mut statement = connection.prepare(sql::RESOLVED_SUBMISSION_RECORDS)?;
    let rows = statement.query_map([], |row| {
        let record = decode_joined_record(row)?;
        let outcome = decode_outcome(&row.get::<_, String>(5)?)?;
        ScorecardObservation::try_new(
            record.task_id().as_estimate_id().clone(),
            record.submission().calibration_key().clone(),
            record.metadata().prompt.id.clone(),
            record.submission().probability(),
            outcome,
        )
        .map_err(invalid)
    })?;
    rows.collect()
}

impl ScorecardObservationRepository for EstimateStore<'_> {
    fn scorecard_observations(
        &self,
    ) -> impl Future<Output = Result<Vec<ScorecardObservation>, SystemPortError>> + Send {
        self.database.read(load_observations)
    }
}
