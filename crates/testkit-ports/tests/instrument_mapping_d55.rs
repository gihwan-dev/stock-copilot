#[cfg(test)]
mod tests {
    use application::{InstrumentIdIssuer, InstrumentMappingRepository, MappingStoreError};
    use domain::{
        IdAdoption, IndependentInstrumentIdIssuer, InstrumentId, InstrumentMappingError,
        InstrumentSource, IsinCode, KnownMarket, MappingAssignment, ReservationStatus, Symbol,
    };
    use std::{
        collections::BTreeSet,
        future::Future,
        sync::{Arc, Barrier},
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::{
        MemoryGlobalInstrumentIds, MemoryInstrumentIdIssuer, MemoryInstrumentMappingRepository,
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

    fn source(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
        InstrumentSource::new(
            market,
            Symbol::try_new(symbol).expect("심볼"),
            IsinCode::try_new(isin).expect("ISIN"),
        )
    }

    #[test]
    fn global_registry_adopts_existing_ids_and_prevents_cross_ledger_collisions() {
        let legacy = InstrumentId::try_new("legacy-existing-7").expect("기존 ID");
        let seq_one = InstrumentId::try_new("inst-0000000000000001").expect("첫 순번 ID");
        let global = MemoryGlobalInstrumentIds::with_existing(&[legacy.clone(), seq_one.clone()]);
        assert_eq!(global.reservation(&legacy), ReservationStatus::Reserved);
        assert_eq!(global.reservation(&seq_one), ReservationStatus::Reserved);
        assert_eq!(global.adopt_existing(legacy), IdAdoption::AlreadyKnown);

        let issuer = global.issuer();
        let repo_a = global.repository();
        let repo_b = global.repository();
        let first_issued = run(issuer.issue()).expect("발급");
        assert_eq!(
            first_issued.as_instrument_id().as_str(),
            "inst-0000000000000002"
        );

        let mut rogue = IndependentInstrumentIdIssuer::new();
        let rogue_one = rogue.issue().expect("미연동 첫 후보");
        let rogue_two = rogue.issue().expect("미연동 둘째 후보");
        let stock_a = source(KnownMarket::Kr, "005930", "KR7005930003");
        let stock_b = source(KnownMarket::Us, "AAPL", "US0378331005");
        assert_eq!(
            run(repo_a.ensure(&stock_a, rogue_one)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdAlreadyUsed
            ))
        );
        assert_eq!(
            run(repo_a.ensure(&stock_a, first_issued)),
            Ok(MappingAssignment::Issued(
                InstrumentId::try_new("inst-0000000000000002").expect("발급 ID")
            ))
        );
        assert_eq!(
            run(repo_b.ensure(&stock_b, rogue_two)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdAlreadyUsed
            ))
        );
    }

    #[test]
    fn retired_id_stays_reserved_globally_while_next_issued_id_succeeds() {
        let global = MemoryGlobalInstrumentIds::default();
        let issuer = global.issuer();
        let repo_a = global.repository();
        let repo_b = global.repository();
        let stock_a = source(KnownMarket::Kr, "005930", "KR7005930003");
        let stock_b = source(KnownMarket::Us, "AAPL", "US0378331005");

        let first = run(issuer.issue()).expect("첫 발급");
        run(repo_a.ensure(&stock_a, first)).expect("등록");
        run(repo_a.retire(&stock_a)).expect("폐지");

        let mut rogue = IndependentInstrumentIdIssuer::new();
        let colliding = rogue.issue().expect("폐지 후 충돌 후보");
        assert_eq!(
            run(repo_b.ensure(&stock_b, colliding)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdAlreadyUsed
            ))
        );

        let next_issued = run(issuer.issue()).expect("다음 발급");
        assert_eq!(
            next_issued.as_instrument_id().as_str(),
            "inst-0000000000000002"
        );
        assert_eq!(
            run(repo_b.ensure(&stock_b, next_issued)),
            Ok(MappingAssignment::Issued(
                InstrumentId::try_new("inst-0000000000000002").expect("둘째 ID")
            ))
        );
    }

    #[test]
    fn failed_ensure_does_not_reserve_candidate_globally() {
        let global = MemoryGlobalInstrumentIds::default();
        let repo_a = global.repository();
        let repo_b = global.repository();
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let ambiguous = source(KnownMarket::Kr, "005930", "KR7005930099");
        let unrelated = source(KnownMarket::Us, "MSFT", "US5949181045");

        let mut issuer_a = IndependentInstrumentIdIssuer::new();
        let first = issuer_a.issue().expect("첫 후보");
        let second = issuer_a.issue().expect("둘째 후보");
        run(repo_a.ensure(&original, first)).expect("등록");
        assert_eq!(
            run(repo_a.ensure(&ambiguous, second)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdentityUncertain
            ))
        );

        let mut issuer_b = IndependentInstrumentIdIssuer::new();
        let _skip = issuer_b.issue().expect("건너뛰기");
        let second_retry = issuer_b.issue().expect("둘째 재시도");
        assert_eq!(
            run(repo_b.ensure(&unrelated, second_retry)),
            Ok(MappingAssignment::Issued(
                InstrumentId::try_new("inst-0000000000000002").expect("둘째 ID")
            ))
        );
    }

    #[test]
    fn concurrent_cross_ledger_writes_preserve_global_uniqueness() {
        let global = MemoryGlobalInstrumentIds::default();
        let repo_a = Arc::new(global.repository());
        let repo_b = Arc::new(global.repository());
        let barrier = Arc::new(Barrier::new(3));
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let second = source(KnownMarket::Us, "AAPL", "US0378331005");

        let (left, right) = std::thread::scope(|scope| {
            let ra = Arc::clone(&repo_a);
            let ba = Arc::clone(&barrier);
            let sa = first.clone();
            let t1 = scope.spawn(move || {
                let mut local = IndependentInstrumentIdIssuer::new();
                let cand = local.issue().expect("후보 1");
                ba.wait();
                run(ra.ensure(&sa, cand))
            });
            let rb = Arc::clone(&repo_b);
            let bb = Arc::clone(&barrier);
            let sb = second.clone();
            let t2 = scope.spawn(move || {
                let mut local = IndependentInstrumentIdIssuer::new();
                let cand = local.issue().expect("후보 2");
                bb.wait();
                run(rb.ensure(&sb, cand))
            });
            barrier.wait();
            (t1.join().expect("스레드 1"), t2.join().expect("스레드 2"))
        });

        let expected_ok = Ok(MappingAssignment::Issued(
            InstrumentId::try_new("inst-0000000000000001").expect("ID"),
        ));
        let expected_err = Err(MappingStoreError::Mapping(
            InstrumentMappingError::IdAlreadyUsed,
        ));
        assert!(
            (left == expected_ok && right == expected_err)
                || (left == expected_err && right == expected_ok)
        );
    }

    fn issue_batch(issuer: &MemoryInstrumentIdIssuer) -> Vec<InstrumentId> {
        (0_usize..15_usize)
            .map(|_| {
                run(issuer.issue())
                    .expect("동시 발급")
                    .as_instrument_id()
                    .clone()
            })
            .collect()
    }

    #[test]
    fn concurrent_shared_issuer_never_duplicates_or_collides_with_adopted() {
        let shared_issuer = Arc::new(MemoryInstrumentIdIssuer::with_existing(&[
            InstrumentId::try_new("inst-0000000000000002").expect("예약"),
        ]));
        let batches: [Vec<InstrumentId>; 4] = std::thread::scope(|scope| {
            let handles = [0_usize, 1_usize, 2_usize, 3_usize].map(|_| {
                let handle_issuer = Arc::clone(&shared_issuer);
                scope.spawn(move || issue_batch(&handle_issuer))
            });
            handles.map(|handle| handle.join().expect("발급 스레드"))
        });
        let mut all = BTreeSet::new();
        for id in batches.into_iter().flatten() {
            assert_ne!(id.as_str(), "inst-0000000000000002");
            assert!(all.insert(id), "중복 발급 금지");
        }
        assert_eq!(all.len(), 60);
    }

    #[test]
    fn repository_with_existing_ids_blocks_colliding_candidate_and_adopts_idempotently() {
        let first_seq = InstrumentId::try_new("inst-0000000000000001").expect("첫 순번 ID");
        let legacy = InstrumentId::try_new("legacy-99").expect("기존 ID");
        let repo =
            MemoryInstrumentMappingRepository::with_existing_ids(&[first_seq.clone(), legacy]);
        assert_eq!(repo.adopt_existing_id(first_seq), IdAdoption::AlreadyKnown);
        let extra = InstrumentId::try_new("legacy-100").expect("추가 기존 ID");
        assert_eq!(repo.adopt_existing_id(extra), IdAdoption::Adopted);

        let mut rogue = IndependentInstrumentIdIssuer::new();
        let colliding = rogue.issue().expect("충돌 후보");
        let fresh = rogue.issue().expect("비충돌 후보");
        let stock = source(KnownMarket::Kr, "005930", "KR7005930003");
        assert_eq!(
            run(repo.ensure(&stock, colliding)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdAlreadyUsed
            ))
        );
        assert_eq!(
            run(repo.ensure(&stock, fresh)),
            Ok(MappingAssignment::Issued(
                InstrumentId::try_new("inst-0000000000000002").expect("둘째 ID")
            ))
        );
    }

    #[test]
    fn dropping_unpolled_issuer_and_repository_futures_preserves_state() {
        let global = MemoryGlobalInstrumentIds::default();
        let issuer = global.issuer();
        let repo = global.repository();
        let stock = source(KnownMarket::Kr, "005930", "KR7005930003");
        {
            let _unpolled_issue = issuer.issue();
        }
        let first = run(issuer.issue()).expect("첫 발급");
        assert_eq!(first.as_instrument_id().as_str(), "inst-0000000000000001");
        {
            let _unpolled_ensure = repo.ensure(&stock, first);
        }
        assert_eq!(
            run(repo.resolve(&stock)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Missing))
        );
        let mut rogue = IndependentInstrumentIdIssuer::new();
        let same_first = rogue.issue().expect("미예약 첫 후보");
        assert_eq!(
            run(repo.ensure(&stock, same_first)),
            Ok(MappingAssignment::Issued(
                InstrumentId::try_new("inst-0000000000000001").expect("첫 ID")
            ))
        );
        let fresh_global = MemoryGlobalInstrumentIds::default();
        let fresh_repo = fresh_global.repository();
        let fresh_issuer = fresh_global.issuer();
        let mut rogue_two = IndependentInstrumentIdIssuer::new();
        let preassigned = rogue_two.issue().expect("선점 후보");
        assert_eq!(
            run(fresh_repo.ensure(&stock, preassigned)),
            Ok(MappingAssignment::Issued(
                InstrumentId::try_new("inst-0000000000000001").expect("선점 ID")
            ))
        );
        let next_from_global = run(fresh_issuer.issue()).expect("전역 다음 발급");
        assert_eq!(
            next_from_global.as_instrument_id().as_str(),
            "inst-0000000000000002"
        );
    }
}
