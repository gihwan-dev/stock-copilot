#[cfg(test)]
mod tests {
    use application::{
        ConsumptionOutcome, ConsumptionStatus, ProposalConsumptionRepository, SystemFailure,
        SystemPort, SystemPortError, ValidationReportRepository, WriteCompleted,
    };
    use domain::{
        CheckResult, ClientOrderId, ProposalId, ValidationReport, ValidationReportResults,
    };
    use std::future::Future;
    use std::sync::{Arc, Barrier};
    use testkit_ports::{
        memory::MemoryReply,
        proposal_memory::{MemoryProposalConsumptionRepository, MemoryValidationReportRepository},
    };

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
        {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn id(value: &str) -> ProposalId {
        ProposalId::new(ClientOrderId::try_new(value).expect("id"))
    }
    fn report(instrument: CheckResult) -> ValidationReport {
        ValidationReport::from_results(ValidationReportResults {
            instrument,
            market_hours: CheckResult::Unknown,
            available_funds_or_quantity: CheckResult::Unknown,
            opposite_pending_order: CheckResult::Unknown,
            buy_warnings: CheckResult::Unknown,
            principles: CheckResult::Unknown,
            order_price: CheckResult::Unknown,
            market_order_permission: CheckResult::Unknown,
            account_match: CheckResult::Unknown,
            price_observation: CheckResult::Unknown,
            expiry: CheckResult::Unknown,
            price_range: CheckResult::Unknown,
        })
    }
    #[test]
    fn memory_consumption_distinguishes_first_duplicate_and_scripted_error() {
        let store = MemoryProposalConsumptionRepository::default();
        let first = id("first");
        let other = id("other");
        for _ in 0_u8..2 {
            store.script_consume().push(MemoryReply::Memory);
        }
        for _ in 0_u8..2 {
            store.script_status().push(MemoryReply::Memory);
        }
        assert_eq!(run(store.consume(&first)), Ok(ConsumptionOutcome::Recorded));
        assert_eq!(
            run(store.consume(&first)),
            Ok(ConsumptionOutcome::AlreadyConsumed)
        );
        assert_eq!(run(store.status(&first)), Ok(ConsumptionStatus::Consumed));
        assert_eq!(
            run(store.status(&other)),
            Ok(ConsumptionStatus::NotConsumed)
        );
        let failure = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        store
            .script_consume()
            .push(MemoryReply::Return(Err(failure)));
        assert_eq!(run(store.consume(&other)), Err(failure));
        store.script_status().push(MemoryReply::Memory);
        assert_eq!(
            run(store.status(&other)),
            Ok(ConsumptionStatus::NotConsumed)
        );
    }
    #[test]
    fn memory_consumption_is_atomic_across_threads() {
        let store = Arc::new(MemoryProposalConsumptionRepository::default());
        store.script_consume().push(MemoryReply::Memory);
        store.script_consume().push(MemoryReply::Memory);
        let barrier = Arc::new(Barrier::new(2));
        let pair = std::thread::scope(|scope| {
            let left_store = Arc::clone(&store);
            let left_barrier = Arc::clone(&barrier);
            let left = scope.spawn(move || {
                left_barrier.wait();
                run(left_store.consume(&id("same")))
            });
            let right = scope.spawn(move || {
                barrier.wait();
                run(store.consume(&id("same")))
            });
            (left.join().expect("left"), right.join().expect("right"))
        });
        match pair {
            (Ok(ConsumptionOutcome::Recorded), Ok(ConsumptionOutcome::AlreadyConsumed))
            | (Ok(ConsumptionOutcome::AlreadyConsumed), Ok(ConsumptionOutcome::Recorded)) => {}
            other => panic!("unexpected outcomes {other:?}"),
        }
    }
    #[test]
    fn memory_reports_preserve_order_scope_and_scripted_errors() {
        let store = MemoryValidationReportRepository::default();
        let first = id("first");
        let other = id("other");
        let reports = [report(CheckResult::Failed), report(CheckResult::Passed)];
        store.script_append().push(MemoryReply::Memory);
        store.script_history().push(MemoryReply::Memory);
        store.script_history().push(MemoryReply::Memory);
        assert_eq!(run(store.append(&first, &reports)), Ok(WriteCompleted));
        assert_eq!(run(store.history(&first)), Ok(reports.to_vec()));
        assert_eq!(run(store.history(&other)), Ok(vec![]));
        let failure = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        store
            .script_append()
            .push(MemoryReply::Return(Err(failure)));
        assert_eq!(run(store.append(&first, &reports)), Err(failure));
        store.script_history().push(MemoryReply::Memory);
        assert_eq!(run(store.history(&first)), Ok(reports.to_vec()));
    }
}
