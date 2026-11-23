#[cfg(test)]
mod tests {
    use domain::{
        IndependentInstrumentIdIssuer, InstrumentId, InstrumentIdIssueError,
        InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus,
        InstrumentMappings, InstrumentMappingsRestoreError, InstrumentSource, IsinCode,
        IssuerSequence, IssuerSequenceError, KnownMarket, MappingAssignment, PersistedIssuerCursor,
        ReservationStatus, Symbol,
    };

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
    fn issuer_sequence_and_cursor_restore_advance_and_exhaust_cleanly() {
        assert_eq!(IssuerSequence::try_new(0), Err(IssuerSequenceError::Zero));
        let seq_one = IssuerSequence::try_new(1).expect("순번 1");
        assert_eq!(*seq_one.as_u64(), 1);

        let mut initial =
            IndependentInstrumentIdIssuer::restore(PersistedIssuerCursor::Initial, &[]);
        assert_eq!(initial.last_issued(), PersistedIssuerCursor::Initial);
        let first = initial.issue().expect("첫 발급");
        assert_eq!(first.as_instrument_id().as_str(), "inst-0000000000000001");
        assert_eq!(
            initial.last_issued(),
            PersistedIssuerCursor::LastIssued(seq_one)
        );

        let seq_two = IssuerSequence::try_new(2).expect("순번 2");
        let existing_three = id("inst-0000000000000003");
        let legacy = id("legacy-77");
        let mut restored = IndependentInstrumentIdIssuer::restore(
            PersistedIssuerCursor::LastIssued(seq_two),
            &[existing_three.clone(), legacy.clone()],
        );
        assert_eq!(
            restored.last_issued(),
            PersistedIssuerCursor::LastIssued(seq_two)
        );
        assert_eq!(
            restored.reservation(&existing_three),
            ReservationStatus::Reserved
        );
        assert_eq!(restored.reservation(&legacy), ReservationStatus::Reserved);

        let fourth = restored.issue().expect("기존 3 건너뛰고 4 발급");
        assert_eq!(fourth.as_instrument_id().as_str(), "inst-0000000000000004");
        assert_eq!(
            restored.last_issued(),
            PersistedIssuerCursor::LastIssued(IssuerSequence::try_new(4).expect("순번 4"))
        );

        let max_seq = IssuerSequence::try_new(u64::MAX).expect("최대 순번");
        let mut exhausted =
            IndependentInstrumentIdIssuer::restore(PersistedIssuerCursor::LastIssued(max_seq), &[]);
        assert_eq!(
            exhausted.last_issued(),
            PersistedIssuerCursor::LastIssued(max_seq)
        );
        assert_eq!(exhausted.issue(), Err(InstrumentIdIssueError::Exhausted));

        let almost_max = IssuerSequence::try_new(u64::MAX - 1).expect("최대-1 순번");
        let mut last_one = IndependentInstrumentIdIssuer::restore(
            PersistedIssuerCursor::LastIssued(almost_max),
            &[],
        );
        let issued_max = last_one.issue().expect("u64::MAX 발급");
        assert_eq!(
            issued_max.as_instrument_id().as_str(),
            "inst-ffffffffffffffff"
        );
        assert_eq!(
            last_one.last_issued(),
            PersistedIssuerCursor::LastIssued(max_seq)
        );
        assert_eq!(last_one.issue(), Err(InstrumentIdIssueError::Exhausted));
    }

    fn seeded_three_status_ledger() -> (
        InstrumentMappings,
        InstrumentSource,
        InstrumentSource,
        InstrumentSource,
    ) {
        let active_src = source(KnownMarket::Kr, "005930", "KR7005930003");
        let superseded_src = source(KnownMarket::Us, "FB", "US30303M1027");
        let retired_src = source(KnownMarket::Us, "TWTR", "US90184L1026");
        let ledger = InstrumentMappings::restore(vec![
            InstrumentMappingRecord::new(
                active_src.clone(),
                id("inst-0000000000000001"),
                InstrumentMappingStatus::Active,
            ),
            InstrumentMappingRecord::new(
                superseded_src.clone(),
                id("inst-0000000000000002"),
                InstrumentMappingStatus::Superseded,
            ),
            InstrumentMappingRecord::new(
                retired_src.clone(),
                id("inst-0000000000000003"),
                InstrumentMappingStatus::Retired,
            ),
        ])
        .expect("정상 레코드 복원");
        (ledger, active_src, superseded_src, retired_src)
    }

    #[test]
    fn restore_preserves_active_superseded_retired_and_blocks_reserved_ids() {
        let (mut ledger, active_src, superseded_src, retired_src) = seeded_three_status_ledger();
        let fresh_src = source(KnownMarket::Us, "MSFT", "US5949181045");
        let superseded_id = id("inst-0000000000000002");

        assert_eq!(ledger.resolve(&active_src), Ok(id("inst-0000000000000001")));
        assert_eq!(
            ledger.resolve(&superseded_src),
            Err(InstrumentMappingError::Superseded)
        );
        assert_eq!(
            ledger.resolve(&retired_src),
            Err(InstrumentMappingError::Retired)
        );
        assert_eq!(
            ledger.retire(&superseded_src),
            Err(InstrumentMappingError::Superseded)
        );
        let expected_superseded = InstrumentMappingRecord::new(
            superseded_src.clone(),
            superseded_id.clone(),
            InstrumentMappingStatus::Superseded,
        );
        assert_eq!(
            ledger.lookup(&superseded_src),
            Ok(expected_superseded.clone())
        );
        assert_eq!(ledger.for_id(&superseded_id), Ok(vec![expected_superseded]));

        let mut issuer = IndependentInstrumentIdIssuer::new();
        for _ in 0_u8..3_u8 {
            let colliding = issuer.issue().expect("충돌 후보");
            assert_eq!(
                ledger.ensure(&fresh_src, colliding),
                Err(InstrumentMappingError::IdAlreadyUsed)
            );
        }
        assert_eq!(
            ledger.ensure(&superseded_src, issuer.issue().expect("4")),
            Err(InstrumentMappingError::Superseded)
        );
        assert_eq!(
            ledger.ensure(&fresh_src, issuer.issue().expect("5")),
            Ok(MappingAssignment::Issued(id("inst-0000000000000005")))
        );
    }

    #[test]
    fn restore_rejects_shared_ids_exact_duplicates_and_overlapping_sources() {
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let second = source(KnownMarket::Us, "AAPL", "US0378331005");
        let overlap_symbol = source(KnownMarket::Us, "aapl", "US0000000009");
        let overlap_isin = source(KnownMarket::Kr, "000660", "us0378331005");

        assert_eq!(
            InstrumentMappings::restore(vec![
                InstrumentMappingRecord::new(
                    first.clone(),
                    id("inst-shared"),
                    InstrumentMappingStatus::Active,
                ),
                InstrumentMappingRecord::new(
                    second.clone(),
                    id("inst-shared"),
                    InstrumentMappingStatus::Retired,
                ),
            ]),
            Err(InstrumentMappingsRestoreError::SharedInstrumentId)
        );

        assert_eq!(
            InstrumentMappings::restore(vec![
                InstrumentMappingRecord::new(
                    first.clone(),
                    id("inst-a"),
                    InstrumentMappingStatus::Active,
                ),
                InstrumentMappingRecord::new(first, id("inst-b"), InstrumentMappingStatus::Retired),
            ]),
            Err(InstrumentMappingsRestoreError::DuplicateRecord)
        );

        for conflicting in [overlap_symbol, overlap_isin] {
            assert_eq!(
                InstrumentMappings::restore(vec![
                    InstrumentMappingRecord::new(
                        second.clone(),
                        id("inst-a"),
                        InstrumentMappingStatus::Active,
                    ),
                    InstrumentMappingRecord::new(
                        conflicting,
                        id("inst-b"),
                        InstrumentMappingStatus::Active,
                    ),
                ]),
                Err(InstrumentMappingsRestoreError::ConflictingSources)
            );
        }
    }
}
