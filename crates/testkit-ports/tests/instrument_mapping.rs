#[cfg(test)]
mod tests {
    use application::{InstrumentMappingRepository, MappingStoreError};
    use domain::{
        InstrumentId, InstrumentMappingError, InstrumentSource, IsinCode, KnownMarket,
        MappingAssignment, Symbol,
    };
    use std::{
        future::Future,
        sync::{Arc, Barrier},
        task::{Context, Poll, Waker},
    };
    use testkit_ports::instrument_mapping::MemoryInstrumentMappingRepository;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }

    fn source(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
        InstrumentSource::new(
            market,
            Symbol::try_new(symbol).expect("심볼"),
            IsinCode::try_new(isin).expect("ISIN"),
        )
    }

    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("내부 ID")
    }

    #[test]
    fn repository_retains_mapping_and_retirement_across_calls() {
        let repo = MemoryInstrumentMappingRepository::default();
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let unknown = source(KnownMarket::Us, "AAPL", "US0378331005");
        assert_eq!(
            run(repo.resolve(&unknown)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        assert_eq!(
            run(repo.ensure(&original, &id("opaque-1"))),
            Ok(MappingAssignment::Issued(id("opaque-1")))
        );
        assert_eq!(run(repo.resolve(&original)), Ok(id("opaque-1")));
        assert_eq!(
            run(repo.ensure(&original, &id("opaque-2"))),
            Ok(MappingAssignment::Existing(id("opaque-1")))
        );
        assert_eq!(run(repo.retire(&original)), Ok(id("opaque-1")));
        assert_eq!(
            run(repo.resolve(&original)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
        );
        assert_eq!(
            run(repo.ensure(&original, &id("opaque-2"))),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
        );
    }

    #[test]
    fn concurrent_issuance_for_same_source_has_one_winner() {
        let repo = Arc::new(MemoryInstrumentMappingRepository::default());
        let barrier = Arc::new(Barrier::new(3));
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let (first, second) = std::thread::scope(|scope| {
            let first_repo = Arc::clone(&repo);
            let first_barrier = Arc::clone(&barrier);
            let first_source = original.clone();
            let first = scope.spawn(move || {
                first_barrier.wait();
                run(first_repo.ensure(&first_source, &id("opaque-1"))).expect("첫 발급")
            });
            let second_repo = Arc::clone(&repo);
            let second_barrier = Arc::clone(&barrier);
            let second_source = original.clone();
            let second = scope.spawn(move || {
                second_barrier.wait();
                run(second_repo.ensure(&second_source, &id("opaque-2"))).expect("둘째 발급")
            });
            barrier.wait();
            (
                first.join().expect("첫 스레드"),
                second.join().expect("둘째 스레드"),
            )
        });
        let winner = match (first, second) {
            (MappingAssignment::Issued(issued_id), MappingAssignment::Existing(existing_id))
            | (MappingAssignment::Existing(existing_id), MappingAssignment::Issued(issued_id)) => {
                assert_eq!(issued_id, existing_id);
                issued_id
            }
            other => panic!("한 번만 발급해야 한다: {other:?}"),
        };
        assert_eq!(run(repo.resolve(&original)), Ok(winner));
    }

    #[test]
    fn candidate_collisions_across_distinct_sources_fail_closed() {
        let repo = MemoryInstrumentMappingRepository::default();
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let second = source(KnownMarket::Us, "AAPL", "US0378331005");
        run(repo.ensure(&first, &id("opaque-1"))).expect("발급");
        assert_eq!(
            run(repo.ensure(&second, &id("opaque-1"))),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdAlreadyUsed
            ))
        );
        assert_eq!(
            run(repo.resolve(&second)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        assert_eq!(
            run(repo.ensure(&second, &id("opaque-2"))),
            Ok(MappingAssignment::Issued(id("opaque-2")))
        );
    }

    #[test]
    fn concurrent_distinct_sources_cannot_receive_the_same_candidate() {
        let repo = Arc::new(MemoryInstrumentMappingRepository::default());
        let barrier = Arc::new(Barrier::new(3));
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let second = source(KnownMarket::Us, "AAPL", "US0378331005");
        let (left, right) = std::thread::scope(|scope| {
            let first_repo = Arc::clone(&repo);
            let first_barrier = Arc::clone(&barrier);
            let first_source = first.clone();
            let left = scope.spawn(move || {
                first_barrier.wait();
                run(first_repo.ensure(&first_source, &id("opaque-1")))
            });
            let second_repo = Arc::clone(&repo);
            let second_barrier = Arc::clone(&barrier);
            let second_source = second.clone();
            let right = scope.spawn(move || {
                second_barrier.wait();
                run(second_repo.ensure(&second_source, &id("opaque-1")))
            });
            barrier.wait();
            (
                left.join().expect("첫 스레드"),
                right.join().expect("둘째 스레드"),
            )
        });
        let issued = Ok(MappingAssignment::Issued(id("opaque-1")));
        let collision = Err(MappingStoreError::Mapping(
            InstrumentMappingError::IdAlreadyUsed,
        ));
        assert!((left == issued && right == collision) || (left == collision && right == issued));
        let lookups = [run(repo.resolve(&first)), run(repo.resolve(&second))];
        assert_eq!(
            lookups
                .iter()
                .filter(|entry| **entry == Ok(id("opaque-1")))
                .count(),
            1
        );
        assert_eq!(
            lookups
                .iter()
                .filter(|entry| **entry
                    == Err(MappingStoreError::Mapping(InstrumentMappingError::Missing)))
                .count(),
            1
        );
    }

    #[test]
    fn concurrent_retirement_never_reactivates_a_source() {
        let repo = Arc::new(MemoryInstrumentMappingRepository::default());
        let barrier = Arc::new(Barrier::new(3));
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        run(repo.ensure(&original, &id("opaque-1"))).expect("발급");
        let (ensured, retired) = std::thread::scope(|scope| {
            let first_repo = Arc::clone(&repo);
            let first_barrier = Arc::clone(&barrier);
            let first_source = original.clone();
            let ensure = scope.spawn(move || {
                first_barrier.wait();
                run(first_repo.ensure(&first_source, &id("opaque-2")))
            });
            let second_repo = Arc::clone(&repo);
            let second_barrier = Arc::clone(&barrier);
            let second_source = original.clone();
            let retire = scope.spawn(move || {
                second_barrier.wait();
                run(second_repo.retire(&second_source))
            });
            barrier.wait();
            (
                ensure.join().expect("발급 스레드"),
                retire.join().expect("폐지 스레드"),
            )
        });
        assert!(
            ensured == Ok(MappingAssignment::Existing(id("opaque-1")))
                || ensured == Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
        );
        assert_eq!(retired, Ok(id("opaque-1")));
        assert_eq!(
            run(repo.resolve(&original)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
        );
    }

    #[test]
    fn dropping_unpolled_writes_does_not_change_the_mapping() {
        let repo = MemoryInstrumentMappingRepository::default();
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let candidate = id("opaque-1");
        {
            let _pending = repo.ensure(&original, &candidate);
        }
        assert_eq!(
            run(repo.resolve(&original)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        run(repo.ensure(&original, &candidate)).expect("발급");
        {
            let _pending = repo.retire(&original);
        }
        assert_eq!(run(repo.resolve(&original)), Ok(candidate));
    }
}
