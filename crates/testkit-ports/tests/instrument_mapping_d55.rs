#[cfg(test)]
mod tests {
    use application::{InstrumentIdIssuer, InstrumentMappingRepository, MappingStoreError};
    use domain::{
        CalendarDate, ConfirmedIdentifierChange, ConfirmedLinkCorrection,
        ConfirmedRetirementCorrection, CorporateAction, CorporateActionId, CorporateActionKind,
        DartReceiptNo, Disclosure, IdAdoption, IdentityChangeConfirmation, IdentityChangeEvidence,
        IdentityConfirmationOrigin, IndependentInstrumentIdIssuer, InstrumentId,
        InstrumentMappingError, InstrumentSource, IsinCode, KindReceiptNo, KnownMarket,
        MappingAssignment, MappingCorrectionConfirmation, MappingCorrectionEvidence,
        MappingTransitionError, Nanosecond, NormalizedUrl, OffsetSeconds, ReceiptNo,
        ReservationStatus, Symbol, Timestamp, UnixSeconds, UntrustedText, UserReason,
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

    fn timestamp(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각")
    }

    fn note(text: &str) -> UserReason {
        UserReason::try_new(UntrustedText::try_new(text).expect("문구")).expect("사유")
    }

    fn symbol_change_request(
        id: &InstrumentId,
        original: &InstrumentSource,
        renamed: &InstrumentSource,
    ) -> ConfirmedIdentifierChange {
        let disclosure = Disclosure::new(
            ReceiptNo::Dart(DartReceiptNo::try_new("20261121000001").expect("DART")),
            NormalizedUrl::try_new("https://dart.fss.or.kr/dsaf001/main.do?rcpNo=2").expect("URL"),
            UntrustedText::try_new("코드 변경 공시").expect("제목"),
            None,
        )
        .key()
        .clone();
        let action = CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            CalendarDate::from_parts(
                domain::CalendarYear::new(2026),
                domain::MonthNumber::new(11),
                domain::DayNumber::new(21),
            )
            .expect("날짜"),
            CorporateActionKind::SymbolChange {
                from: original.symbol().clone(),
                to: renamed.symbol().clone(),
            },
        )
        .expect("기업행위");
        ConfirmedIdentifierChange::new(
            id.clone(),
            original.clone(),
            renamed.clone(),
            IdentityChangeConfirmation::new(
                IdentityConfirmationOrigin::UserConfirmed,
                IdentityChangeEvidence::DisclosureAndAction {
                    disclosure,
                    action,
                    note: note("코드 변경 확인"),
                },
                timestamp(1_700_010_000),
            ),
        )
    }

    fn kind_correction(reason: &str, at: i64) -> MappingCorrectionConfirmation {
        let disclosure = Disclosure::new(
            ReceiptNo::Kind(KindReceiptNo::try_new("20261121000002").expect("KIND")),
            NormalizedUrl::try_new("https://kind.krx.co.kr/common/disclsviewer.do?2").expect("URL"),
            UntrustedText::try_new("정정 공시").expect("제목"),
            None,
        )
        .key()
        .clone();
        MappingCorrectionConfirmation::new(
            IdentityConfirmationOrigin::UserConfirmed,
            MappingCorrectionEvidence::DisclosureAndNote {
                disclosure,
                note: note(reason),
            },
            timestamp(at),
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
    fn memory_repository_supports_confirmed_changes_and_lazy_drops() {
        let repo = MemoryInstrumentMappingRepository::with_existing_ids(&[InstrumentId::try_new(
            "legacy-99",
        )
        .expect("기존 ID")]);
        let issuer = MemoryInstrumentIdIssuer::default();
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let renamed = source(KnownMarket::Kr, "005931", "KR7005930003");
        let MappingAssignment::Issued(id) =
            run(repo.ensure(&original, run(issuer.issue()).expect("발급"))).expect("등록")
        else {
            panic!("발급");
        };
        let change = symbol_change_request(&id, &original, &renamed);
        {
            let _dropped = repo.link_confirmed_change(&change);
        }
        assert_eq!(run(repo.resolve(&original)), Ok(id.clone()));
        assert_eq!(run(repo.link_confirmed_change(&change)), Ok(id.clone()));
        assert_eq!(run(repo.resolve(&renamed)), Ok(id));
        assert_eq!(
            run(repo.resolve(&original)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::Superseded
            ))
        );
    }

    #[test]
    fn memory_repository_supports_link_and_retirement_corrections() {
        let repo = MemoryInstrumentMappingRepository::default();
        let issuer = MemoryInstrumentIdIssuer::default();
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let renamed = source(KnownMarket::Kr, "005931", "KR7005930003");
        let MappingAssignment::Issued(id) =
            run(repo.ensure(&original, run(issuer.issue()).expect("발급"))).expect("등록")
        else {
            panic!("발급");
        };
        run(repo.link_confirmed_change(&symbol_change_request(&id, &original, &renamed)))
            .expect("연결");

        let link_corr = ConfirmedLinkCorrection::new(
            id.clone(),
            original.clone(),
            renamed.clone(),
            kind_correction("연결 오기록 정정", 1_700_010_100),
        );
        assert_eq!(run(repo.correct_link(&link_corr)), Ok(id.clone()));
        assert_eq!(run(repo.resolve(&original)), Ok(id.clone()));
        assert_eq!(
            run(repo.resolve(&renamed)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdentityUncertain
            ))
        );

        run(repo.retire(&original)).expect("폐지");
        let retire_corr = ConfirmedRetirementCorrection::new(
            id.clone(),
            original.clone(),
            kind_correction("폐지 오기록 정정", 1_700_010_200),
        );
        assert_eq!(run(repo.correct_retirement(&retire_corr)), Ok(id.clone()));
        assert_eq!(run(repo.resolve(&original)), Ok(id.clone()));
        assert_eq!(run(repo.history_for_id(&id)).len(), 3);
        assert_eq!(run(repo.history()).len(), 3);
        assert_eq!(
            run(repo.correct_link(&link_corr)),
            Err(MappingTransitionError::IdentityUncertain)
        );
    }
}
