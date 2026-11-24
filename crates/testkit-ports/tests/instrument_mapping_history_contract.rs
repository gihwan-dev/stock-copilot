#[cfg(test)]
mod tests {
    use application::{InstrumentMappingHistoryRepository, SystemPortError};
    use domain::{
        InstrumentId, InstrumentMappingRecord, InstrumentMappingStatus, InstrumentSource, IsinCode,
        KnownMarket, MappingHistoryEvent, MappingLedgerIntegrityError, Symbol,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        contracts::instrument_mapping::{
            empty_history_contract, history_contract_dataset, populated_history_contract,
        },
        memory::{MemoryInstrumentMappingHistoryStore, MemoryInstrumentMappingRepository},
    };

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
    fn memory_repository_passes_shared_empty_history_contract() {
        let repo = MemoryInstrumentMappingRepository::default();
        run(empty_history_contract(&repo, &repo));
    }

    #[test]
    fn memory_history_store_passes_shared_populated_history_contract_and_rejects_inconsistent_seed()
    {
        let dataset = history_contract_dataset().expect("공유 계약 데이터셋");
        let store = MemoryInstrumentMappingHistoryStore::try_from_verified(
            &dataset.records,
            dataset.events.clone(),
        )
        .expect("검증된 이력 저장소");
        run(populated_history_contract(&store, &dataset));

        let orphan = InstrumentMappingRecord::new(
            InstrumentSource::new(
                KnownMarket::Kr,
                Symbol::try_new("005930").expect("심볼"),
                IsinCode::try_new("KR7005930003").expect("ISIN"),
            ),
            InstrumentId::try_new("inst-0000000000000099").expect("ID"),
            InstrumentMappingStatus::Superseded,
        );
        assert_eq!(
            MemoryInstrumentMappingHistoryStore::try_from_verified(&[orphan], Vec::new())
                .expect_err("고아 superseded 거부"),
            MappingLedgerIntegrityError::OrphanSupersededRecord
        );
    }

    struct ReversedHistoryFake {
        events: Vec<MappingHistoryEvent>,
    }

    impl InstrumentMappingHistoryRepository for ReversedHistoryFake {
        fn history(
            &self,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            let mut copy = self.events.clone();
            copy.reverse();
            std::future::ready(Ok(copy))
        }

        fn history_for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            let filtered = self
                .events
                .iter()
                .filter(|event| event.instrument_id() == id)
                .cloned()
                .collect();
            std::future::ready(Ok(filtered))
        }
    }

    struct UnfilteredHistoryFake {
        events: Vec<MappingHistoryEvent>,
    }

    impl InstrumentMappingHistoryRepository for UnfilteredHistoryFake {
        fn history(
            &self,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.events.clone()))
        }

        fn history_for_id(
            &self,
            _id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.events.clone()))
        }
    }

    struct GroupedByIdHistoryFake {
        events: Vec<MappingHistoryEvent>,
    }

    impl InstrumentMappingHistoryRepository for GroupedByIdHistoryFake {
        fn history(
            &self,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            let mut copy = self.events.clone();
            copy.sort_by(|left, right| {
                left.instrument_id()
                    .as_str()
                    .cmp(right.instrument_id().as_str())
            });
            std::future::ready(Ok(copy))
        }

        fn history_for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            let filtered = self
                .events
                .iter()
                .filter(|event| event.instrument_id() == id)
                .cloned()
                .collect();
            std::future::ready(Ok(filtered))
        }
    }

    fn event_ts(event: &MappingHistoryEvent) -> domain::Timestamp {
        match event {
            MappingHistoryEvent::IdentifierChanged { confirmed_at, .. } => *confirmed_at,
            MappingHistoryEvent::RetirementCorrected { corrected_at, .. }
            | MappingHistoryEvent::LinkCorrected { corrected_at, .. } => *corrected_at,
        }
    }

    struct TimestampSortedHistoryFake {
        events: Vec<MappingHistoryEvent>,
        descending: TimestampSortOrder,
    }

    #[derive(Clone, Copy)]
    enum TimestampSortOrder {
        Ascending,
        Descending,
    }

    impl TimestampSortedHistoryFake {
        fn sort_slice(&self, slice: &mut [MappingHistoryEvent]) {
            slice.sort_by(|left, right| match self.descending {
                TimestampSortOrder::Ascending => event_ts(left).cmp(&event_ts(right)),
                TimestampSortOrder::Descending => event_ts(right).cmp(&event_ts(left)),
            });
        }
    }

    impl InstrumentMappingHistoryRepository for TimestampSortedHistoryFake {
        fn history(
            &self,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            let mut copy = self.events.clone();
            self.sort_slice(&mut copy);
            std::future::ready(Ok(copy))
        }

        fn history_for_id(
            &self,
            id: &InstrumentId,
        ) -> impl Future<Output = Result<Vec<MappingHistoryEvent>, SystemPortError>> + Send
        {
            let mut filtered: Vec<_> = self
                .events
                .iter()
                .filter(|event| event.instrument_id() == id)
                .cloned()
                .collect();
            self.sort_slice(&mut filtered);
            std::future::ready(Ok(filtered))
        }
    }

    #[test]
    #[should_panic(expected = "순번 순서")]
    fn populated_history_contract_rejects_reversed_order_implementation() {
        let dataset = history_contract_dataset().expect("공유 계약 데이터셋");
        let broken = ReversedHistoryFake {
            events: dataset.events.clone(),
        };
        run(populated_history_contract(&broken, &dataset));
    }

    #[test]
    #[should_panic(expected = "해당 종목 사건만")]
    fn populated_history_contract_rejects_unfiltered_history_for_id_implementation() {
        let dataset = history_contract_dataset().expect("공유 계약 데이터셋");
        let broken = UnfilteredHistoryFake {
            events: dataset.events.clone(),
        };
        run(populated_history_contract(&broken, &dataset));
    }

    #[test]
    #[should_panic(expected = "순번 순서")]
    fn populated_history_contract_rejects_grouped_by_id_implementation() {
        let dataset = history_contract_dataset().expect("공유 계약 데이터셋");
        let broken = GroupedByIdHistoryFake {
            events: dataset.events.clone(),
        };
        run(populated_history_contract(&broken, &dataset));
    }

    #[test]
    #[should_panic(expected = "순번 순서")]
    fn populated_history_contract_rejects_timestamp_descending_implementation() {
        let dataset = history_contract_dataset().expect("공유 계약 데이터셋");
        let broken = TimestampSortedHistoryFake {
            events: dataset.events.clone(),
            descending: TimestampSortOrder::Descending,
        };
        run(populated_history_contract(&broken, &dataset));
    }

    #[test]
    #[should_panic(expected = "순번 순서")]
    fn populated_history_contract_rejects_timestamp_ascending_implementation() {
        let dataset = history_contract_dataset().expect("공유 계약 데이터셋");
        let broken = TimestampSortedHistoryFake {
            events: dataset.events.clone(),
            descending: TimestampSortOrder::Ascending,
        };
        run(populated_history_contract(&broken, &dataset));
    }
}
