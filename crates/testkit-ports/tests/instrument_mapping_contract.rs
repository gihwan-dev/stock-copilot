#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        contracts::instrument_mapping::{self, ContractRuntime},
        memory::{MemoryGlobalInstrumentIds, MemoryInstrumentMappingRepository},
    };

    struct ImmediateRuntime;

    impl ContractRuntime for ImmediateRuntime {
        fn block_on<F: Future>(&self, future: F) -> F::Output {
            let mut pinned = std::pin::pin!(future);
            match pinned
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                Poll::Ready(value) => value,
                Poll::Pending => panic!("memory repository future must resolve immediately"),
            }
        }
    }

    #[test]
    fn memory_repository_passes_shared_lifecycle_contract() {
        let repo = MemoryInstrumentMappingRepository::default();
        ImmediateRuntime.block_on(instrument_mapping::lifecycle_roundtrip(&repo));
    }

    #[test]
    fn memory_repository_passes_shared_overlap_and_case_contract() {
        let repo = MemoryInstrumentMappingRepository::default();
        ImmediateRuntime.block_on(instrument_mapping::uncertain_overlaps_and_case_variants(
            &repo,
        ));
    }

    #[test]
    fn memory_repository_passes_shared_collision_contract() {
        let repo = MemoryInstrumentMappingRepository::default();
        ImmediateRuntime.block_on(instrument_mapping::candidate_collisions_fail_closed(&repo));
    }

    #[test]
    fn memory_repository_passes_shared_unpolled_drop_contract() {
        let repo = MemoryInstrumentMappingRepository::default();
        ImmediateRuntime
            .block_on(instrument_mapping::dropping_unpolled_writes_does_not_mutate(&repo));
    }

    #[test]
    fn memory_repository_passes_shared_reference_model_contract() {
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
            let repo = MemoryInstrumentMappingRepository::default();
            let salt = u16::try_from(index).expect("sequence index");
            ImmediateRuntime.block_on(instrument_mapping::reference_model_sequence(
                &repo, salt, operations,
            ));
        }
    }

    #[test]
    fn memory_repository_passes_shared_concurrency_contracts() {
        let repo_same = MemoryInstrumentMappingRepository::default();
        instrument_mapping::concurrent_same_source_has_one_winner(
            &repo_same,
            &repo_same,
            &ImmediateRuntime,
        );

        let repo_distinct = MemoryInstrumentMappingRepository::default();
        instrument_mapping::concurrent_distinct_sources_same_candidate_has_one_winner(
            &repo_distinct,
            &repo_distinct,
            &ImmediateRuntime,
        );

        let repo_overlap = MemoryInstrumentMappingRepository::default();
        instrument_mapping::concurrent_overlapping_sources_has_one_winner(
            &repo_overlap,
            &repo_overlap,
            &ImmediateRuntime,
        );

        let repo_retire = MemoryInstrumentMappingRepository::default();
        instrument_mapping::concurrent_ensure_and_retire_never_reactivates(
            &repo_retire,
            &repo_retire,
            &ImmediateRuntime,
        );

        let repo_idempotent = MemoryInstrumentMappingRepository::default();
        instrument_mapping::concurrent_retire_is_idempotent(
            &repo_idempotent,
            &repo_idempotent,
            &ImmediateRuntime,
        );
    }

    #[test]
    fn memory_issuer_passes_shared_issuer_contracts() {
        let global = MemoryGlobalInstrumentIds::default();
        let issuer = global.issuer();
        let repo = global.repository();
        ImmediateRuntime.block_on(
            instrument_mapping::issuer_persists_uniqueness_and_skips_retired_and_existing(
                &issuer, &repo,
            ),
        );

        let concurrent_global = MemoryGlobalInstrumentIds::default();
        let left_issuer = concurrent_global.issuer();
        let right_issuer = concurrent_global.issuer();
        instrument_mapping::concurrent_issuer_never_duplicates(
            &left_issuer,
            &right_issuer,
            &ImmediateRuntime,
        );
    }

    struct AlwaysErrorRepo {
        error: application::MappingStoreError,
    }

    impl application::InstrumentMappingRepository for AlwaysErrorRepo {
        fn resolve(
            &self,
            _source: &domain::InstrumentSource,
        ) -> impl Future<Output = Result<domain::InstrumentId, application::MappingStoreError>>
        {
            std::future::ready(Err(self.error.clone()))
        }

        fn lookup(
            &self,
            _source: &domain::InstrumentSource,
        ) -> impl Future<Output = Result<domain::InstrumentMappingRecord, application::MappingStoreError>>
        {
            std::future::ready(Err(self.error.clone()))
        }

        fn for_id(
            &self,
            _id: &domain::InstrumentId,
        ) -> impl Future<
            Output = Result<Vec<domain::InstrumentMappingRecord>, application::MappingStoreError>,
        > {
            std::future::ready(Err(self.error.clone()))
        }

        fn ensure(
            &self,
            _source: &domain::InstrumentSource,
            _candidate: domain::IssuedInstrumentId,
        ) -> impl Future<Output = Result<domain::MappingAssignment, application::MappingStoreError>>
        {
            std::future::ready(Err(self.error.clone()))
        }

        fn retire(
            &self,
            _source: &domain::InstrumentSource,
        ) -> impl Future<Output = Result<domain::InstrumentId, application::MappingStoreError>>
        {
            std::future::ready(Err(self.error.clone()))
        }
    }

    #[test]
    #[should_panic(expected = "겹침 원본 경쟁은 Issued 1건과 IdentityUncertain 1건이어야 함")]
    fn overlapping_race_contract_rejects_double_uncertain_without_winner() {
        let broken = AlwaysErrorRepo {
            error: application::MappingStoreError::Mapping(
                domain::InstrumentMappingError::IdentityUncertain,
            ),
        };
        instrument_mapping::concurrent_overlapping_sources_has_one_winner(
            &broken,
            &broken,
            &ImmediateRuntime,
        );
    }

    #[test]
    #[should_panic(expected = "겹침 원본 경쟁은 Issued 1건과 IdentityUncertain 1건이어야 함")]
    fn overlapping_race_contract_rejects_uncertain_plus_storage_unavailable() {
        let left = AlwaysErrorRepo {
            error: application::MappingStoreError::Mapping(
                domain::InstrumentMappingError::IdentityUncertain,
            ),
        };
        let right = AlwaysErrorRepo {
            error: application::MappingStoreError::Storage(application::SystemPortError::new(
                application::SystemPort::Repository,
                application::SystemFailure::Unavailable(application::RetryDelay::Unknown),
            )),
        };
        instrument_mapping::concurrent_overlapping_sources_has_one_winner(
            &left,
            &right,
            &ImmediateRuntime,
        );
    }
}
