#[cfg(test)]
mod tests {
    use application::{InstrumentMappingRepository, MappingStoreError};
    use domain::{
        InstrumentId, InstrumentMappingError, InstrumentMappingStatus, InstrumentSource, IsinCode,
        KnownMarket, Symbol,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::MemoryInstrumentMappingRepository;

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
    #[test]
    fn repository_reads_retired_history_without_reactivating_or_writing() {
        let repository = MemoryInstrumentMappingRepository::default();
        let source = InstrumentSource::new(
            KnownMarket::Us,
            Symbol::try_new("AAPL").expect("symbol"),
            IsinCode::try_new("US0378331005").expect("isin"),
        );
        let issued = domain::IndependentInstrumentIdIssuer::new()
            .issue()
            .expect("후보");
        let id: InstrumentId = issued.as_instrument_id().clone();
        assert_eq!(
            run(repository.for_id(&id)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        assert_eq!(
            run(repository.lookup(&source)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        run(repository.ensure(&source, issued)).expect("발급");
        let ambiguous = InstrumentSource::new(
            KnownMarket::Us,
            Symbol::try_new("AAPL").expect("symbol"),
            IsinCode::try_new("US0000000002").expect("isin"),
        );
        assert_eq!(
            run(repository.lookup(&ambiguous)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdentityUncertain
            ))
        );
        let active = run(repository.lookup(&source)).expect("조회");
        assert_eq!(active.id(), &id);
        assert_eq!(active.source(), &source);
        assert_eq!(active.status(), InstrumentMappingStatus::Active);
        assert_eq!(run(repository.for_id(&id)), Ok(vec![active]));
        run(repository.retire(&source)).expect("폐지");
        let retired = run(repository.lookup(&source)).expect("이력");
        assert_eq!(retired.id(), &id);
        assert_eq!(retired.source(), &source);
        assert_eq!(retired.status(), InstrumentMappingStatus::Retired);
        assert_eq!(run(repository.for_id(&id)), Ok(vec![retired]));
        assert_eq!(
            run(repository.resolve(&source)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
        );
    }
}
