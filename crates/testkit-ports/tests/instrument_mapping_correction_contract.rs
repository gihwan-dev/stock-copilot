#[cfg(test)]
mod tests {
    use application::{
        ConfirmInstrumentIdentity, IdentifierChangePrompt, IdentityConfirmationDecision,
        IdentityConfirmationOutcome, InstrumentMappingCorrectionRepository,
        InstrumentMappingHistoryRepository, InstrumentMappingRepository, LinkCorrectionPrompt,
        MappingStoreError, MappingTransitionStoreError, RetirementCorrectionPrompt,
        SystemPortError,
    };
    use domain::{
        ConfirmedIdentifierChange, ConfirmedLinkCorrection, ConfirmedRetirementCorrection,
        InstrumentId, InstrumentMappingRecord, InstrumentSource, IssuedInstrumentId,
        MappingAssignment, MappingHistoryEvent, MappingTransitionError,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        ScriptedIdentityConfirmation, SteppedClock,
        contracts::{
            instrument_mapping::ContractRuntime,
            instrument_mapping_correction::{
                concurrent_cross_id_overlapping_next_link_has_one_winner,
                concurrent_cross_id_same_next_link_has_one_winner,
                concurrent_double_link_correction_has_one_winner,
                concurrent_double_retirement_correction_has_one_winner,
                concurrent_link_and_retire_serializes_cleanly,
                concurrent_link_correction_vs_chained_link_serializes_cleanly,
                concurrent_same_previous_link_has_one_winner,
                correction_lifecycle_and_conflict_rules_contract,
                unpolled_correction_writes_do_not_mutate_contract,
            },
        },
        memory::{MemoryInstrumentMappingRepository, MemoryInstrumentMappingStore},
    };

    struct PollRuntime;

    impl ContractRuntime for PollRuntime {
        fn block_on<F: Future>(&self, future: F) -> F::Output {
            let mut pinned = std::pin::pin!(future);
            match pinned
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                Poll::Ready(value) => value,
                Poll::Pending => panic!("메모리 가짜는 즉시 완료되어야 함"),
            }
        }
    }

    #[test]
    fn memory_store_passes_shared_correction_lifecycle_and_conflict_rules() {
        let repo = MemoryInstrumentMappingRepository::default();
        let store = MemoryInstrumentMappingStore::new(&repo);
        PollRuntime.block_on(correction_lifecycle_and_conflict_rules_contract(&store));
    }

    #[test]
    fn memory_store_passes_shared_unpolled_correction_write_drop_contract() {
        let repo = MemoryInstrumentMappingRepository::default();
        let store = MemoryInstrumentMappingStore::new(&repo);
        PollRuntime.block_on(unpolled_correction_writes_do_not_mutate_contract(&store));
    }

    fn run_memory_race<F>(race: F)
    where
        F: for<'a> FnOnce(
            &MemoryInstrumentMappingStore<'a>,
            &MemoryInstrumentMappingStore<'a>,
            &PollRuntime,
        ),
    {
        let repo = MemoryInstrumentMappingRepository::default();
        let store = MemoryInstrumentMappingStore::new(&repo);
        race(&store, &store, &PollRuntime);
    }

    #[test]
    fn memory_store_passes_all_seven_shared_concurrent_correction_races() {
        run_memory_race(|left, right, rt| {
            concurrent_same_previous_link_has_one_winner(left, right, rt);
        });
        run_memory_race(|left, right, rt| {
            concurrent_cross_id_same_next_link_has_one_winner(left, right, rt);
        });
        run_memory_race(|left, right, rt| {
            concurrent_cross_id_overlapping_next_link_has_one_winner(left, right, rt);
        });
        run_memory_race(|left, right, rt| {
            concurrent_link_and_retire_serializes_cleanly(left, right, rt);
        });
        run_memory_race(|left, right, rt| {
            concurrent_double_retirement_correction_has_one_winner(left, right, rt);
        });
        run_memory_race(|left, right, rt| {
            concurrent_double_link_correction_has_one_winner(left, right, rt);
        });
        run_memory_race(|left, right, rt| {
            concurrent_link_correction_vs_chained_link_serializes_cleanly(left, right, rt);
        });
    }

    struct DropsRetirementHistoryStore {
        inner: MemoryInstrumentMappingRepository,
    }

    impl InstrumentMappingRepository for DropsRetirementHistoryStore {
        fn resolve(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.inner.resolve(source)
        }
        fn lookup(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentMappingRecord, MappingStoreError>> + Send
        {
            self.inner.lookup(source)
        }
        fn for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<InstrumentMappingRecord>, MappingStoreError>> + Send
        {
            self.inner.for_id(id)
        }
        fn ensure(
            &self,
            source: &InstrumentSource,
            candidate: IssuedInstrumentId,
        ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send {
            self.inner.ensure(source, candidate)
        }
        fn retire(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.inner.retire(source)
        }
    }

    impl InstrumentMappingHistoryRepository for DropsRetirementHistoryStore {
        fn history(
            &self,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            let filtered: Vec<_> = self
                .inner
                .history()
                .into_iter()
                .filter(|event| match event {
                    MappingHistoryEvent::RetirementCorrected { .. } => false,
                    MappingHistoryEvent::IdentifierChanged { .. }
                    | MappingHistoryEvent::LinkCorrected { .. } => true,
                })
                .collect();
            std::future::ready(Ok(filtered))
        }
        fn history_for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.inner.history_for_id(id)))
        }
    }

    impl InstrumentMappingCorrectionRepository for DropsRetirementHistoryStore {
        async fn link_confirmed_change(
            &self,
            change: ConfirmedIdentifierChange,
        ) -> Result<InstrumentId, MappingTransitionStoreError> {
            MemoryInstrumentMappingStore::new(&self.inner)
                .link_confirmed_change(change)
                .await
        }
        async fn correct_retirement(
            &self,
            correction: ConfirmedRetirementCorrection,
        ) -> Result<InstrumentId, MappingTransitionStoreError> {
            MemoryInstrumentMappingStore::new(&self.inner)
                .correct_retirement(correction)
                .await
        }
        async fn correct_link(
            &self,
            correction: ConfirmedLinkCorrection,
        ) -> Result<InstrumentId, MappingTransitionStoreError> {
            MemoryInstrumentMappingStore::new(&self.inner)
                .correct_link(correction)
                .await
        }
    }

    #[test]
    #[should_panic(expected = "재폐지 후 새 사건 없음")]
    fn lifecycle_contract_rejects_store_dropping_retirement_history() {
        let broken = DropsRetirementHistoryStore {
            inner: MemoryInstrumentMappingRepository::default(),
        };
        PollRuntime.block_on(correction_lifecycle_and_conflict_rules_contract(&broken));
    }

    struct EagerCorrectionStore {
        inner: MemoryInstrumentMappingRepository,
    }

    impl InstrumentMappingRepository for EagerCorrectionStore {
        fn resolve(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.inner.resolve(source)
        }
        fn lookup(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentMappingRecord, MappingStoreError>> + Send
        {
            self.inner.lookup(source)
        }
        fn for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<InstrumentMappingRecord>, MappingStoreError>> + Send
        {
            self.inner.for_id(id)
        }
        fn ensure(
            &self,
            source: &InstrumentSource,
            candidate: IssuedInstrumentId,
        ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send {
            self.inner.ensure(source, candidate)
        }
        fn retire(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.inner.retire(source)
        }
    }

    impl InstrumentMappingHistoryRepository for EagerCorrectionStore {
        fn history(
            &self,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.inner.history()))
        }
        fn history_for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.inner.history_for_id(id)))
        }
    }

    impl InstrumentMappingCorrectionRepository for EagerCorrectionStore {
        fn link_confirmed_change(
            &self,
            change: ConfirmedIdentifierChange,
        ) -> impl Future<Output = Result<InstrumentId, MappingTransitionStoreError>> + Send
        {
            let outcome = self
                .inner
                .link_confirmed_change(change)
                .map_err(MappingTransitionStoreError::Transition);
            std::future::ready(outcome)
        }
        fn correct_retirement(
            &self,
            correction: ConfirmedRetirementCorrection,
        ) -> impl Future<Output = Result<InstrumentId, MappingTransitionStoreError>> + Send
        {
            let outcome = self
                .inner
                .correct_retirement(correction)
                .map_err(MappingTransitionStoreError::Transition);
            std::future::ready(outcome)
        }
        fn correct_link(
            &self,
            correction: ConfirmedLinkCorrection,
        ) -> impl Future<Output = Result<InstrumentId, MappingTransitionStoreError>> + Send
        {
            let outcome = self
                .inner
                .correct_link(correction)
                .map_err(MappingTransitionStoreError::Transition);
            std::future::ready(outcome)
        }
    }

    #[test]
    #[should_panic(expected = "미폴링 연결 무변경")]
    fn unpolled_contract_rejects_eager_correction_store() {
        let broken = EagerCorrectionStore {
            inner: MemoryInstrumentMappingRepository::default(),
        };
        PollRuntime.block_on(unpolled_correction_writes_do_not_mutate_contract(&broken));
    }

    struct AlwaysRejectLinkStore {
        inner: MemoryInstrumentMappingRepository,
    }

    impl InstrumentMappingRepository for AlwaysRejectLinkStore {
        fn resolve(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.inner.resolve(source)
        }
        fn lookup(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentMappingRecord, MappingStoreError>> + Send
        {
            self.inner.lookup(source)
        }
        fn for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<InstrumentMappingRecord>, MappingStoreError>> + Send
        {
            self.inner.for_id(id)
        }
        fn ensure(
            &self,
            source: &InstrumentSource,
            candidate: IssuedInstrumentId,
        ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send {
            self.inner.ensure(source, candidate)
        }
        fn retire(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.inner.retire(source)
        }
    }

    impl InstrumentMappingHistoryRepository for AlwaysRejectLinkStore {
        fn history(
            &self,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.inner.history()))
        }
        fn history_for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.inner.history_for_id(id)))
        }
    }

    impl InstrumentMappingCorrectionRepository for AlwaysRejectLinkStore {
        fn link_confirmed_change(
            &self,
            _change: ConfirmedIdentifierChange,
        ) -> impl Future<Output = Result<InstrumentId, MappingTransitionStoreError>> + Send
        {
            std::future::ready(Err(MappingTransitionStoreError::Transition(
                MappingTransitionError::SourceNotActive,
            )))
        }
        async fn correct_retirement(
            &self,
            correction: ConfirmedRetirementCorrection,
        ) -> Result<InstrumentId, MappingTransitionStoreError> {
            MemoryInstrumentMappingStore::new(&self.inner)
                .correct_retirement(correction)
                .await
        }
        async fn correct_link(
            &self,
            correction: ConfirmedLinkCorrection,
        ) -> Result<InstrumentId, MappingTransitionStoreError> {
            MemoryInstrumentMappingStore::new(&self.inner)
                .correct_link(correction)
                .await
        }
    }

    #[test]
    #[should_panic(expected = "승자 1건과 SourceNotActive 1건")]
    fn concurrent_race_contract_rejects_zero_winner_store() {
        let broken = AlwaysRejectLinkStore {
            inner: MemoryInstrumentMappingRepository::default(),
        };
        concurrent_same_previous_link_has_one_winner(&broken, &broken, &PollRuntime);
    }

    struct AcceptsNonLatestLinkStore {
        inner: MemoryInstrumentMappingRepository,
    }

    impl InstrumentMappingRepository for AcceptsNonLatestLinkStore {
        fn resolve(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.inner.resolve(source)
        }
        fn lookup(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentMappingRecord, MappingStoreError>> + Send
        {
            self.inner.lookup(source)
        }
        fn for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<InstrumentMappingRecord>, MappingStoreError>> + Send
        {
            self.inner.for_id(id)
        }
        fn ensure(
            &self,
            source: &InstrumentSource,
            candidate: IssuedInstrumentId,
        ) -> impl Future<Output = Result<MappingAssignment, MappingStoreError>> + Send {
            self.inner.ensure(source, candidate)
        }
        fn retire(
            &self,
            source: &InstrumentSource,
        ) -> impl Future<Output = Result<InstrumentId, MappingStoreError>> + Send {
            self.inner.retire(source)
        }
    }

    impl InstrumentMappingHistoryRepository for AcceptsNonLatestLinkStore {
        fn history(
            &self,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.inner.history()))
        }
        fn history_for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.inner.history_for_id(id)))
        }
    }

    impl InstrumentMappingCorrectionRepository for AcceptsNonLatestLinkStore {
        async fn link_confirmed_change(
            &self,
            change: ConfirmedIdentifierChange,
        ) -> Result<InstrumentId, MappingTransitionStoreError> {
            MemoryInstrumentMappingStore::new(&self.inner)
                .link_confirmed_change(change)
                .await
        }
        async fn correct_retirement(
            &self,
            correction: ConfirmedRetirementCorrection,
        ) -> Result<InstrumentId, MappingTransitionStoreError> {
            MemoryInstrumentMappingStore::new(&self.inner)
                .correct_retirement(correction)
                .await
        }
        fn correct_link(
            &self,
            correction: ConfirmedLinkCorrection,
        ) -> impl Future<Output = Result<InstrumentId, MappingTransitionStoreError>> + Send
        {
            let target = correction.instrument_id().clone();
            match self.inner.correct_link(correction) {
                Ok(id) => std::future::ready(Ok(id)),
                Err(MappingTransitionError::LinkNotLatest) => std::future::ready(Ok(target)),
                Err(other) => {
                    std::future::ready(Err(MappingTransitionStoreError::Transition(other)))
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "비최신·충돌 연결 정정")]
    fn lifecycle_contract_rejects_store_ignoring_link_not_latest() {
        let broken = AcceptsNonLatestLinkStore {
            inner: MemoryInstrumentMappingRepository::default(),
        };
        PollRuntime.block_on(correction_lifecycle_and_conflict_rules_contract(&broken));
    }

    #[derive(Clone, Copy)]
    enum NonConfirmed {
        Denied,
        Unavailable,
    }

    const fn expected_outcome<T>(kind: NonConfirmed) -> IdentityConfirmationOutcome<T> {
        match kind {
            NonConfirmed::Denied => IdentityConfirmationOutcome::Denied,
            NonConfirmed::Unavailable => IdentityConfirmationOutcome::Unavailable,
        }
    }

    fn assert_non_confirmed_round(
        use_case: &ConfirmInstrumentIdentity<'_, ScriptedIdentityConfirmation, SteppedClock>,
        id: &InstrumentId,
        pair: (&InstrumentSource, &InstrumentSource),
        kind: NonConfirmed,
    ) {
        let change_req = IdentifierChangePrompt::new(
            id.clone(),
            pair.0.clone(),
            pair.1.clone(),
            domain::IdentityChangeEvidence::None,
        );
        assert_eq!(
            PollRuntime.block_on(use_case.confirm_identifier_change(change_req)),
            expected_outcome(kind)
        );
        let retire_req = RetirementCorrectionPrompt::new(
            id.clone(),
            pair.0.clone(),
            domain::MappingCorrectionEvidence::None,
        );
        assert_eq!(
            PollRuntime.block_on(use_case.confirm_retirement_correction(retire_req)),
            expected_outcome(kind)
        );
        let link_req = LinkCorrectionPrompt::new(
            id.clone(),
            pair.0.clone(),
            pair.1.clone(),
            domain::MappingCorrectionEvidence::None,
        );
        assert_eq!(
            PollRuntime.block_on(use_case.confirm_link_correction(link_req)),
            expected_outcome(kind)
        );
    }

    #[test]
    fn denied_and_unavailable_confirmations_cannot_reach_correction_port() {
        let prompt = ScriptedIdentityConfirmation::new(
            [
                IdentityConfirmationDecision::Denied,
                IdentityConfirmationDecision::Unavailable,
            ],
            [
                IdentityConfirmationDecision::Denied,
                IdentityConfirmationDecision::Unavailable,
            ],
            [
                IdentityConfirmationDecision::Denied,
                IdentityConfirmationDecision::Unavailable,
            ],
        );
        let clock = SteppedClock::new(
            domain::Timestamp::from_parts(
                domain::UnixSeconds::new(1_764_003_000),
                domain::Nanosecond::new(0),
                domain::OffsetSeconds::new(0),
            )
            .expect("시각"),
        );
        let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);
        let id = InstrumentId::try_new("inst-0000000000000001").expect("ID");
        let s1 = InstrumentSource::new(
            domain::KnownMarket::Us,
            domain::Symbol::try_new("FB").expect("심볼"),
            domain::IsinCode::try_new("US30303M1027").expect("ISIN"),
        );
        let s2 = InstrumentSource::new(
            domain::KnownMarket::Us,
            domain::Symbol::try_new("META").expect("심볼"),
            domain::IsinCode::try_new("US30303M1027").expect("ISIN"),
        );
        for kind in [NonConfirmed::Denied, NonConfirmed::Unavailable] {
            assert_non_confirmed_round(&use_case, &id, (&s1, &s2), kind);
        }
        assert!(clock.calls().is_empty(), "거부·불가 시 시계 미호출");
    }
}
