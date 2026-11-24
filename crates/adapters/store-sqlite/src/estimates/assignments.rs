use super::{EstimateStore, codec::invalid, issue_task_tx, load_assignments};
use crate::sql::estimates as sql;
use application::{
    AiRunId, EstimateAssignmentRepository, EstimateCommitOutcome, RecordOutcome, SystemPortError,
};
use domain::{EstimateAssignments, EstimateConsumption, IssuedEstimateTask};
use rusqlite::Transaction;
use std::future::Future;

fn consume(
    transaction: &Transaction<'_>,
    entry: &IssuedEstimateTask<AiRunId>,
) -> rusqlite::Result<()> {
    match transaction.execute(sql::CONSUME_TASK, [entry.id().as_estimate_id().as_str()])? {
        1 => Ok(()),
        _ => Err(invalid(())),
    }
}

fn commit_assignments_tx(
    transaction: &Transaction<'_>,
    previous: &EstimateAssignments<AiRunId>,
    next: &EstimateAssignments<AiRunId>,
) -> rusqlite::Result<EstimateCommitOutcome> {
    if &load_assignments(transaction)? != previous {
        return Ok(EstimateCommitOutcome::Conflict);
    }
    let (kept, issued) = next
        .entries()
        .split_at_checked(previous.entries().len())
        .ok_or_else(|| invalid(()))?;
    for (prior, updated) in previous.entries().iter().zip(kept) {
        if prior.id() != updated.id()
            || prior.run() != updated.run()
            || prior.task() != updated.task()
        {
            return Err(invalid(()));
        }
        match (prior.consumption(), updated.consumption()) {
            (EstimateConsumption::Available, EstimateConsumption::Consumed) => {
                consume(transaction, updated)?;
            }
            (EstimateConsumption::Available, EstimateConsumption::Available)
            | (EstimateConsumption::Consumed, EstimateConsumption::Consumed) => {}
            (EstimateConsumption::Consumed, EstimateConsumption::Available) => {
                return Err(invalid(()));
            }
        }
    }
    for entry in issued {
        match issue_task_tx(
            transaction,
            entry.id().as_estimate_id().as_str(),
            entry.run(),
            entry.task(),
        )? {
            RecordOutcome::Recorded => {}
            RecordOutcome::AlreadyRecorded => return Err(invalid(())),
        }
        if entry.consumption() == EstimateConsumption::Consumed {
            consume(transaction, entry)?;
        }
    }
    if &load_assignments(transaction)? != next {
        return Err(invalid(()));
    }
    Ok(EstimateCommitOutcome::Committed)
}

impl EstimateAssignmentRepository for EstimateStore<'_> {
    fn load_assignments(
        &self,
    ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send {
        self.database.read(load_assignments)
    }

    fn commit_assignments(
        &self,
        previous: &EstimateAssignments<AiRunId>,
        next: &EstimateAssignments<AiRunId>,
    ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send {
        let expected = previous.clone();
        let replacement = next.clone();
        self.database
            .write(move |transaction| commit_assignments_tx(transaction, &expected, &replacement))
    }
}
