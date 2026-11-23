#[cfg(test)]
mod tests {
    use domain::{
        IndependentInstrumentIdIssuer, InstrumentId, InstrumentIdIssueError, IssuerSequence,
        IssuerSequenceError, PersistedIssuerCursor, ReservationStatus,
    };

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
    }

    #[test]
    fn issuer_handles_u64_max_boundary_without_mutating_cursor_on_exhaustion() {
        let max_seq = IssuerSequence::try_new(u64::MAX).expect("최대 순번");
        let mut exhausted =
            IndependentInstrumentIdIssuer::restore(PersistedIssuerCursor::LastIssued(max_seq), &[]);
        assert_eq!(
            exhausted.last_issued(),
            PersistedIssuerCursor::LastIssued(max_seq)
        );
        assert_eq!(exhausted.issue(), Err(InstrumentIdIssueError::Exhausted));
        assert_eq!(
            exhausted.last_issued(),
            PersistedIssuerCursor::LastIssued(max_seq)
        );

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

        let mut blocked = IndependentInstrumentIdIssuer::restore(
            PersistedIssuerCursor::LastIssued(almost_max),
            &[id("inst-ffffffffffffffff")],
        );
        assert_eq!(blocked.issue(), Err(InstrumentIdIssueError::Exhausted));
        assert_eq!(
            blocked.last_issued(),
            PersistedIssuerCursor::LastIssued(almost_max)
        );
    }
}
