#[cfg(test)]
mod tests {
    use domain::{
        CalendarDate, ConfirmedIdentifierChange, ConfirmedLinkCorrection,
        ConfirmedRetirementCorrection, CorporateAction, CorporateActionId, CorporateActionKind,
        DartReceiptNo, Disclosure, DisclosureKey, IdAdoption, IdentityChangeConfirmation,
        IdentityChangeEvidence, IdentityConfirmationOrigin, IndependentInstrumentIdIssuer,
        InstrumentId, InstrumentMappingError, InstrumentMappingStatus, InstrumentMappings,
        InstrumentMaster, InstrumentMasterEntry, InstrumentMasterError, InstrumentSource, IsinCode,
        KindReceiptNo, KnownMarket, MappingAssignment, MappingCorrectionConfirmation,
        MappingCorrectionEvidence, MappingHistoryEvent, MappingTransitionError, Nanosecond,
        NormalizedUrl, OffsetSeconds, ReceiptNo, ReservationStatus, SecEdgarReceiptNo, ShareCount,
        ShareRatio, Symbol, Timestamp, UnixSeconds, UntrustedText, UserReason,
    };
    use proptest::{
        collection::vec,
        prop_assert, prop_assert_eq,
        test_runner::{Config, TestCaseError, TestRunner},
    };
    use std::collections::{BTreeMap, BTreeSet};

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

    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            domain::CalendarYear::new(2026),
            domain::MonthNumber::new(11),
            domain::DayNumber::new(day),
        )
        .expect("날짜")
    }

    fn note(text: &str) -> UserReason {
        UserReason::try_new(UntrustedText::try_new(text).expect("문구")).expect("사유")
    }

    fn dart_disclosure(receipt: &str) -> DisclosureKey {
        Disclosure::new(
            ReceiptNo::Dart(DartReceiptNo::try_new(receipt).expect("DART")),
            NormalizedUrl::try_new("https://dart.fss.or.kr/dsaf001/main.do?rcpNo=1").expect("URL"),
            UntrustedText::try_new("액면분할 공시").expect("제목"),
            None,
        )
        .key()
        .clone()
    }

    fn kind_disclosure(receipt: &str) -> DisclosureKey {
        Disclosure::new(
            ReceiptNo::Kind(KindReceiptNo::try_new(receipt).expect("KIND")),
            NormalizedUrl::try_new("https://kind.krx.co.kr/common/disclsviewer.do").expect("URL"),
            UntrustedText::try_new("종목명 변경 공시").expect("제목"),
            None,
        )
        .key()
        .clone()
    }

    fn sec_disclosure(receipt: &str) -> DisclosureKey {
        Disclosure::new(
            ReceiptNo::SecEdgar(SecEdgarReceiptNo::try_new(receipt).expect("SEC")),
            NormalizedUrl::try_new("https://www.sec.gov/Archives/edgar/data/1/index.htm")
                .expect("URL"),
            UntrustedText::try_new("Ticker change filing").expect("제목"),
            None,
        )
        .key()
        .clone()
    }

    fn split_action(id: &InstrumentId) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            date(10),
            CorporateActionKind::Split(ShareRatio::new(
                ShareCount::try_new(1).expect("구주"),
                ShareCount::try_new(5).expect("신주"),
            )),
        )
        .expect("분할")
    }

    fn symbol_change_action(id: &InstrumentId, from: &str, to: &str) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            date(11),
            CorporateActionKind::SymbolChange {
                from: Symbol::try_new(from).expect("이전 심볼"),
                to: Symbol::try_new(to).expect("이후 심볼"),
            },
        )
        .expect("심볼 변경")
    }

    fn delisting_action(id: &InstrumentId) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            date(12),
            CorporateActionKind::Delisting,
        )
        .expect("상장폐지")
    }

    fn change_evidence(
        disclosure: DisclosureKey,
        action: CorporateAction,
        reason: &str,
    ) -> IdentityChangeEvidence {
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure,
            action,
            note: note(reason),
        }
    }

    fn confirmed_change(
        id: &InstrumentId,
        previous: InstrumentSource,
        next: InstrumentSource,
        evidence: IdentityChangeEvidence,
        at: i64,
    ) -> ConfirmedIdentifierChange {
        ConfirmedIdentifierChange::new(
            id.clone(),
            previous,
            next,
            IdentityChangeConfirmation::new(
                IdentityConfirmationOrigin::UserConfirmed,
                evidence,
                timestamp(at),
            ),
        )
    }

    fn correction_confirmation(
        origin: IdentityConfirmationOrigin,
        evidence: MappingCorrectionEvidence,
        at: i64,
    ) -> MappingCorrectionConfirmation {
        MappingCorrectionConfirmation::new(origin, evidence, timestamp(at))
    }

    fn valid_correction(disclosure: &DisclosureKey) -> MappingCorrectionConfirmation {
        correction_confirmation(
            IdentityConfirmationOrigin::UserConfirmed,
            MappingCorrectionEvidence::DisclosureAndNote {
                disclosure: disclosure.clone(),
                note: note("오기록 정정 확인"),
            },
            1_700_002_000,
        )
    }

    fn seed_instrument(
        ledger: &mut InstrumentMappings,
        generator: &mut IndependentInstrumentIdIssuer,
        src: &InstrumentSource,
    ) -> InstrumentId {
        let MappingAssignment::Issued(instrument_id) = ledger
            .ensure(src, generator.issue().expect("발급"))
            .expect("등록")
        else {
            panic!("신규 발급이어야 한다");
        };
        instrument_id
    }

    #[test]
    fn confirmed_isin_change_supersedes_previous_and_keeps_internal_id() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let after_split = source(KnownMarket::Kr, "005930", "KR7005930004");
        let instrument_id = seed_instrument(&mut ledger, &mut generator, &first);

        let split_change = confirmed_change(
            &instrument_id,
            first.clone(),
            after_split.clone(),
            change_evidence(
                dart_disclosure("20261110000001"),
                split_action(&instrument_id),
                "액면분할 ISIN 변경 확인",
            ),
            1_700_000_100,
        );
        assert_eq!(
            ledger.link_confirmed_change(&split_change),
            Ok(instrument_id.clone())
        );
        assert_eq!(
            ledger.resolve(&first),
            Err(InstrumentMappingError::Superseded)
        );
        assert_eq!(ledger.resolve(&after_split), Ok(instrument_id.clone()));
        assert_eq!(
            ledger.ensure(&first, generator.issue().expect("발급")),
            Err(InstrumentMappingError::Superseded)
        );
        assert_eq!(
            ledger.ensure(&after_split, generator.issue().expect("발급")),
            Ok(MappingAssignment::Existing(instrument_id))
        );
        assert_eq!(
            ledger.retire(&first),
            Err(InstrumentMappingError::Superseded)
        );
        assert_eq!(
            ledger.lookup(&first).expect("이전 이력").status(),
            InstrumentMappingStatus::Superseded
        );
        assert_eq!(
            ledger.lookup(&after_split).expect("현재 이력").status(),
            InstrumentMappingStatus::Active
        );
        assert_eq!(
            ledger.resolve(&source(KnownMarket::Kr, "005930", "KR7005930099")),
            Err(InstrumentMappingError::IdentityUncertain)
        );
    }

    fn link_change(
        ledger: &mut InstrumentMappings,
        id: &InstrumentId,
        previous: &InstrumentSource,
        next: &InstrumentSource,
        evidence: IdentityChangeEvidence,
    ) -> Result<InstrumentId, MappingTransitionError> {
        ledger.link_confirmed_change(&confirmed_change(
            id,
            previous.clone(),
            next.clone(),
            evidence,
            200,
        ))
    }

    fn expected_change_event(
        id: &InstrumentId,
        previous: &InstrumentSource,
        next: &InstrumentSource,
        disclosure: DisclosureKey,
        action: CorporateAction,
    ) -> MappingHistoryEvent {
        MappingHistoryEvent::IdentifierChanged {
            instrument_id: id.clone(),
            previous: previous.clone(),
            next: next.clone(),
            disclosure,
            action: Box::new(action),
            note: note("연쇄 변경"),
            confirmed_at: timestamp(200),
        }
    }

    #[test]
    fn chained_changes_preserve_ordered_records_and_audit_history() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let second = source(KnownMarket::Kr, "005930", "KR7005930004");
        let third = source(KnownMarket::Kr, "005931", "KR7005930004");
        let fourth = source(KnownMarket::Kr, "005932", "KR7005930005");
        let id = seed_instrument(&mut ledger, &mut generator, &first);
        let merger = CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            date(12),
            CorporateActionKind::Merger,
        )
        .expect("합병");
        let steps = [
            (
                &first,
                &second,
                dart_disclosure("20261110000001"),
                split_action(&id),
            ),
            (
                &second,
                &third,
                kind_disclosure("20261111000002"),
                symbol_change_action(&id, "005930", "005931"),
            ),
            (&third, &fourth, dart_disclosure("20261112000003"), merger),
        ];
        for (prev, next, disc, act) in &steps {
            let evidence = change_evidence(disc.clone(), act.clone(), "연쇄 변경");
            link_change(&mut ledger, &id, prev, next, evidence).expect("변경");
        }
        let records = ledger.for_id(&id).expect("이력");
        let statuses: Vec<_> = records
            .iter()
            .map(domain::InstrumentMappingRecord::status)
            .collect();
        assert_eq!(
            statuses,
            vec![
                InstrumentMappingStatus::Superseded,
                InstrumentMappingStatus::Superseded,
                InstrumentMappingStatus::Superseded,
                InstrumentMappingStatus::Active,
            ]
        );
        assert_eq!(records[3].source(), &fourth);
        let expected: Vec<_> = steps
            .into_iter()
            .map(|(prev, next, disc, act)| expected_change_event(&id, prev, next, disc, act))
            .collect();
        assert_eq!(ledger.history_for_id(&id), expected);
        assert_eq!(ledger.history().len(), 3);
        assert_eq!(ledger.retire(&fourth), Ok(id));
        assert_eq!(
            ledger.resolve(&fourth),
            Err(InstrumentMappingError::Retired)
        );
    }

    #[test]
    fn unconfirmed_origins_stay_identity_uncertain() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let previous = source(KnownMarket::Us, "FB", "US30303M1027");
        let next = source(KnownMarket::Us, "META", "US30303M1027");
        let id = seed_instrument(&mut ledger, &mut generator, &previous);
        let disclosure = sec_disclosure("0001326801-26-000001");
        let action = symbol_change_action(&id, "FB", "META");
        let before = ledger.clone();

        for origin in [
            IdentityConfirmationOrigin::BrokerObservation,
            IdentityConfirmationOrigin::AiSuggestion,
        ] {
            let attempt = ConfirmedIdentifierChange::new(
                id.clone(),
                previous.clone(),
                next.clone(),
                IdentityChangeConfirmation::new(
                    origin,
                    change_evidence(disclosure.clone(), action.clone(), "자동 추정"),
                    timestamp(1_700_001_000),
                ),
            );
            assert_eq!(
                ledger.link_confirmed_change(&attempt),
                Err(MappingTransitionError::IdentityUncertain)
            );
            assert_eq!(ledger, before);
        }
        assert_eq!(
            ledger.resolve(&next),
            Err(InstrumentMappingError::IdentityUncertain)
        );
    }

    #[test]
    fn insufficient_change_evidence_stays_identity_uncertain() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let previous = source(KnownMarket::Us, "FB", "US30303M1027");
        let next = source(KnownMarket::Us, "META", "US30303M1027");
        let id = seed_instrument(&mut ledger, &mut generator, &previous);
        let disclosure = sec_disclosure("0001326801-26-000001");
        let action = symbol_change_action(&id, "FB", "META");
        let before = ledger.clone();

        for evidence in [
            IdentityChangeEvidence::None,
            IdentityChangeEvidence::NoteOnly(note("사용자 메모만")),
            IdentityChangeEvidence::DisclosureOnly {
                disclosure,
                note: note("공시만"),
            },
            IdentityChangeEvidence::ActionOnly {
                action,
                note: note("기업행위만"),
            },
        ] {
            let attempt = confirmed_change(&id, previous.clone(), next.clone(), evidence, 1_700);
            assert_eq!(
                ledger.link_confirmed_change(&attempt),
                Err(MappingTransitionError::IdentityUncertain)
            );
            assert_eq!(ledger, before);
        }
        assert_eq!(
            ledger.resolve(&next),
            Err(InstrumentMappingError::IdentityUncertain)
        );
    }

    #[test]
    fn identifier_change_rejects_invalid_shape_and_action_evidence() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let alpha = source(KnownMarket::Kr, "005930", "KR7005930003");
        let beta = source(KnownMarket::Kr, "000660", "KR7000660001");
        let alpha_id = seed_instrument(&mut ledger, &mut generator, &alpha);
        let beta_id = seed_instrument(&mut ledger, &mut generator, &beta);
        let disclosure = dart_disclosure("20261115000001");
        let before = ledger.clone();

        let cases = [
            (
                alpha.clone(),
                split_action(&alpha_id),
                MappingTransitionError::UnchangedSource,
            ),
            (
                source(KnownMarket::Us, "005930", "US0000000001"),
                split_action(&alpha_id),
                MappingTransitionError::MarketMismatch,
            ),
            (
                source(KnownMarket::Kr, "005930", "KR7005930004"),
                split_action(&beta_id),
                MappingTransitionError::InvalidEvidence,
            ),
            (
                source(KnownMarket::Kr, "005930", "KR7005930004"),
                delisting_action(&alpha_id),
                MappingTransitionError::InvalidEvidence,
            ),
            (
                source(KnownMarket::Kr, "005931", "KR7005930003"),
                symbol_change_action(&alpha_id, "005939", "005931"),
                MappingTransitionError::InvalidEvidence,
            ),
            (
                source(KnownMarket::Kr, "005931", "KR7005930003"),
                symbol_change_action(&alpha_id, "005930", "005939"),
                MappingTransitionError::InvalidEvidence,
            ),
        ];
        for (next, action, expected) in cases {
            let evidence = change_evidence(disclosure.clone(), action, "검증 실패");
            assert_eq!(
                link_change(&mut ledger, &alpha_id, &alpha, &next, evidence),
                Err(expected)
            );
        }
        assert_eq!(ledger, before);
    }

    #[test]
    fn identifier_change_rejects_missing_and_cross_id_conflicts() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let alpha = source(KnownMarket::Kr, "005930", "KR7005930003");
        let beta = source(KnownMarket::Kr, "000660", "KR7000660001");
        let alpha_id = seed_instrument(&mut ledger, &mut generator, &alpha);
        let _beta_id = seed_instrument(&mut ledger, &mut generator, &beta);
        let disclosure = dart_disclosure("20261115000001");
        let before = ledger.clone();

        let cases = [
            (
                source(KnownMarket::Kr, "035420", "KR7035420009"),
                source(KnownMarket::Kr, "035421", "KR7035420009"),
                symbol_change_action(&alpha_id, "035420", "035421"),
                MappingTransitionError::Missing,
            ),
            (
                source(KnownMarket::Kr, "005930", "KR7005930099"),
                source(KnownMarket::Kr, "005931", "KR7005930099"),
                symbol_change_action(&alpha_id, "005930", "005931"),
                MappingTransitionError::IdentityUncertain,
            ),
            (
                beta.clone(),
                source(KnownMarket::Kr, "000661", "KR7000660001"),
                symbol_change_action(&alpha_id, "000660", "000661"),
                MappingTransitionError::SourceBoundToOtherId,
            ),
            (
                alpha.clone(),
                beta,
                symbol_change_action(&alpha_id, "005930", "000660"),
                MappingTransitionError::SourceBoundToOtherId,
            ),
            (
                alpha.clone(),
                source(KnownMarket::Kr, "000660", "KR7005930099"),
                symbol_change_action(&alpha_id, "005930", "000660"),
                MappingTransitionError::IdentityUncertain,
            ),
            (
                alpha,
                source(KnownMarket::Kr, "005931", "kr7000660001"),
                symbol_change_action(&alpha_id, "005930", "005931"),
                MappingTransitionError::IdentityUncertain,
            ),
        ];
        for (prev, next, action, expected) in cases {
            let attempt = confirmed_change(
                &alpha_id,
                prev,
                next,
                change_evidence(disclosure.clone(), action, "충돌 검사"),
                110,
            );
            assert_eq!(ledger.link_confirmed_change(&attempt), Err(expected));
        }
        assert_eq!(ledger, before);
    }

    #[test]
    fn identifier_change_rejects_branching_cycles_and_retired_sources() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let alpha = source(KnownMarket::Kr, "005930", "KR7005930003");
        let alpha_next = source(KnownMarket::Kr, "005930", "KR7005930004");
        let alpha_id = seed_instrument(&mut ledger, &mut generator, &alpha);
        let disclosure = dart_disclosure("20261115000001");

        ledger
            .link_confirmed_change(&confirmed_change(
                &alpha_id,
                alpha.clone(),
                alpha_next.clone(),
                change_evidence(disclosure.clone(), split_action(&alpha_id), "정상 변경"),
                112,
            ))
            .expect("정상 변경");
        let after_link = ledger.clone();
        assert_eq!(
            ledger.link_confirmed_change(&confirmed_change(
                &alpha_id,
                alpha.clone(),
                source(KnownMarket::Kr, "005930", "KR7005930005"),
                change_evidence(
                    disclosure.clone(),
                    split_action(&alpha_id),
                    "교체 원본 분기"
                ),
                113,
            )),
            Err(MappingTransitionError::SourceNotActive)
        );
        assert_eq!(
            ledger.link_confirmed_change(&confirmed_change(
                &alpha_id,
                alpha_next.clone(),
                alpha,
                change_evidence(disclosure.clone(), split_action(&alpha_id), "순환 연결"),
                114,
            )),
            Err(MappingTransitionError::SourceAlreadyRecorded)
        );
        assert_eq!(ledger, after_link);

        ledger.retire(&alpha_next).expect("폐지");
        let after_retire = ledger.clone();
        assert_eq!(
            ledger.link_confirmed_change(&confirmed_change(
                &alpha_id,
                alpha_next,
                source(KnownMarket::Kr, "005930", "KR7005930006"),
                change_evidence(disclosure, split_action(&alpha_id), "폐지 원본 연결"),
                115,
            )),
            Err(MappingTransitionError::SourceNotActive)
        );
        assert_eq!(ledger, after_retire);
    }

    #[test]
    fn retirement_correction_rejects_unconfirmed_origin_or_missing_evidence() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let first_id = seed_instrument(&mut ledger, &mut generator, &first);
        let disclosure = kind_disclosure("20261116000001");
        ledger.retire(&first).expect("폐지");
        let snapshot = ledger.clone();
        let invalid = [
            correction_confirmation(
                IdentityConfirmationOrigin::BrokerObservation,
                MappingCorrectionEvidence::DisclosureAndNote {
                    disclosure: disclosure.clone(),
                    note: note("자동 복구"),
                },
                10,
            ),
            correction_confirmation(
                IdentityConfirmationOrigin::AiSuggestion,
                MappingCorrectionEvidence::DisclosureAndNote {
                    disclosure,
                    note: note("AI 정정"),
                },
                11,
            ),
            correction_confirmation(
                IdentityConfirmationOrigin::UserConfirmed,
                MappingCorrectionEvidence::None,
                12,
            ),
            correction_confirmation(
                IdentityConfirmationOrigin::UserConfirmed,
                MappingCorrectionEvidence::NoteOnly(note("메모만")),
                13,
            ),
        ];
        for bad in invalid {
            let attempt =
                ConfirmedRetirementCorrection::new(first_id.clone(), first.clone(), bad.clone());
            assert_eq!(
                ledger.correct_retirement(&attempt),
                Err(MappingTransitionError::IdentityUncertain)
            );
            let link_attempt = ConfirmedLinkCorrection::new(
                first_id.clone(),
                first.clone(),
                source(KnownMarket::Kr, "005931", "KR7005930004"),
                bad,
            );
            assert_eq!(
                ledger.correct_link(&link_attempt),
                Err(MappingTransitionError::IdentityUncertain)
            );
            assert_eq!(ledger, snapshot);
        }
    }

    #[test]
    fn retirement_correction_validates_owner_state_and_restores_active() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let other = source(KnownMarket::Us, "AAPL", "US0378331005");
        let first_id = seed_instrument(&mut ledger, &mut generator, &first);
        let other_id = seed_instrument(&mut ledger, &mut generator, &other);
        let disclosure = kind_disclosure("20261116000001");
        let check =
            |target_id: &InstrumentId, src: &InstrumentSource, current: &mut InstrumentMappings| {
                current.correct_retirement(&ConfirmedRetirementCorrection::new(
                    target_id.clone(),
                    src.clone(),
                    valid_correction(&disclosure),
                ))
            };

        assert_eq!(
            check(&first_id, &first, &mut ledger),
            Err(MappingTransitionError::SourceNotRetired)
        );
        ledger.retire(&first).expect("폐지");
        assert_eq!(
            check(&other_id, &first, &mut ledger),
            Err(MappingTransitionError::SourceBoundToOtherId)
        );
        assert_eq!(
            check(
                &first_id,
                &source(KnownMarket::Kr, "005930", "KR7005930099"),
                &mut ledger
            ),
            Err(MappingTransitionError::IdentityUncertain)
        );
        assert_eq!(
            check(
                &first_id,
                &source(KnownMarket::Kr, "099999", "KR7099999001"),
                &mut ledger
            ),
            Err(MappingTransitionError::Missing)
        );
        assert_eq!(check(&first_id, &first, &mut ledger), Ok(first_id.clone()));
        assert_eq!(ledger.resolve(&first), Ok(first_id.clone()));
        assert_eq!(
            ledger.history_for_id(&first_id),
            vec![MappingHistoryEvent::RetirementCorrected {
                instrument_id: first_id,
                source: first,
                disclosure,
                note: note("오기록 정정 확인"),
                corrected_at: timestamp(1_700_002_000),
            }]
        );
    }

    fn seed_two_links(
        ledger: &mut InstrumentMappings,
        id: &InstrumentId,
        sources: [&InstrumentSource; 3],
        disclosure: &DisclosureKey,
    ) {
        let [first, second, third] = sources;
        link_change(
            ledger,
            id,
            first,
            second,
            change_evidence(
                disclosure.clone(),
                symbol_change_action(id, first.symbol().as_str(), second.symbol().as_str()),
                "첫 연결",
            ),
        )
        .expect("첫 연결");
        link_change(
            ledger,
            id,
            second,
            third,
            change_evidence(
                disclosure.clone(),
                symbol_change_action(id, second.symbol().as_str(), third.symbol().as_str()),
                "둘째 연결",
            ),
        )
        .expect("둘째 연결");
    }

    #[test]
    fn link_correction_rejects_non_latest_wrong_owner_and_retired_tip() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let second = source(KnownMarket::Kr, "005931", "KR7005930004");
        let third = source(KnownMarket::Kr, "005932", "KR7005930005");
        let other = source(KnownMarket::Us, "AAPL", "US0378331005");
        let first_id = seed_instrument(&mut ledger, &mut generator, &first);
        let _other_id = seed_instrument(&mut ledger, &mut generator, &other);
        let disclosure = kind_disclosure("20261116000001");
        let try_unlink =
            |prev: &InstrumentSource, next: &InstrumentSource, target: &mut InstrumentMappings| {
                target.correct_link(&ConfirmedLinkCorrection::new(
                    first_id.clone(),
                    prev.clone(),
                    next.clone(),
                    valid_correction(&disclosure),
                ))
            };

        assert_eq!(
            try_unlink(&first, &first, &mut ledger),
            Err(MappingTransitionError::UnchangedSource)
        );
        assert_eq!(
            try_unlink(&first, &second, &mut ledger),
            Err(MappingTransitionError::Missing)
        );
        seed_two_links(
            &mut ledger,
            &first_id,
            [&first, &second, &third],
            &disclosure,
        );
        assert_eq!(
            ledger.correct_retirement(&ConfirmedRetirementCorrection::new(
                first_id.clone(),
                first.clone(),
                valid_correction(&disclosure),
            )),
            Err(MappingTransitionError::SourceNotRetired)
        );
        assert_eq!(
            try_unlink(&first, &second, &mut ledger),
            Err(MappingTransitionError::LinkNotLatest)
        );
        assert_eq!(
            try_unlink(&other, &third, &mut ledger),
            Err(MappingTransitionError::SourceBoundToOtherId)
        );
        assert_eq!(
            try_unlink(&second, &other, &mut ledger),
            Err(MappingTransitionError::SourceBoundToOtherId)
        );
        ledger.retire(&third).expect("폐지");
        assert_eq!(
            try_unlink(&second, &third, &mut ledger),
            Err(MappingTransitionError::LinkNotLatest)
        );
    }

    #[test]
    fn link_correction_unwinds_latest_link_and_releases_mistaken_source() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let first = source(KnownMarket::Kr, "005930", "KR7005930003");
        let second = source(KnownMarket::Kr, "005931", "KR7005930004");
        let third = source(KnownMarket::Kr, "005932", "KR7005930005");
        let other = source(KnownMarket::Us, "AAPL", "US0378331005");
        let first_id = seed_instrument(&mut ledger, &mut generator, &first);
        let other_id = seed_instrument(&mut ledger, &mut generator, &other);
        let disclosure = kind_disclosure("20261116000001");
        seed_two_links(
            &mut ledger,
            &first_id,
            [&first, &second, &third],
            &disclosure,
        );

        let unwind_second = ConfirmedLinkCorrection::new(
            first_id.clone(),
            second.clone(),
            third.clone(),
            valid_correction(&disclosure),
        );
        assert_eq!(ledger.correct_link(&unwind_second), Ok(first_id.clone()));
        assert_eq!(ledger.resolve(&second), Ok(first_id.clone()));
        assert_eq!(ledger.resolve(&third), Err(InstrumentMappingError::Missing));
        let third_own_id = seed_instrument(&mut ledger, &mut generator, &third);
        assert_ne!(third_own_id, first_id);

        let unwind_first = ConfirmedLinkCorrection::new(
            first_id.clone(),
            first.clone(),
            second.clone(),
            valid_correction(&disclosure),
        );
        assert_eq!(ledger.correct_link(&unwind_first), Ok(first_id.clone()));
        assert_eq!(ledger.resolve(&first), Ok(first_id.clone()));
        assert_eq!(
            ledger.resolve(&second),
            Err(InstrumentMappingError::Missing)
        );
        assert_eq!(ledger.for_id(&first_id).expect("이력").len(), 1);
        assert_eq!(ledger.history_for_id(&first_id).len(), 4);
        assert_eq!(ledger.history_for_id(&other_id), Vec::new());
    }

    #[test]
    fn independent_issuer_and_ledger_enforce_existing_id_adoption() {
        let legacy = InstrumentId::try_new("legacy-stock-1").expect("기존 ID");
        let first_seq = InstrumentId::try_new("inst-0000000000000001").expect("첫 순번 ID");
        let mut issuer =
            IndependentInstrumentIdIssuer::from_existing(&[legacy.clone(), first_seq.clone()]);
        assert_eq!(issuer.reservation(&legacy), ReservationStatus::Reserved);
        assert_eq!(issuer.reservation(&first_seq), ReservationStatus::Reserved);
        assert_eq!(issuer.adopt_existing(legacy), IdAdoption::AlreadyKnown);
        let third_reserved = InstrumentId::try_new("inst-0000000000000003").expect("셋째 순번 ID");
        assert_eq!(
            issuer.reservation(&third_reserved),
            ReservationStatus::Available
        );
        assert_eq!(issuer.adopt_existing(third_reserved), IdAdoption::Adopted);

        let issued_a = issuer.issue().expect("발급 A");
        assert_eq!(
            issued_a.as_instrument_id().as_str(),
            "inst-0000000000000002"
        );
        let issued_b = issuer.issue().expect("발급 B");
        assert_eq!(
            issued_b.as_instrument_id().as_str(),
            "inst-0000000000000004"
        );

        let mut ledger = InstrumentMappings::default();
        assert_eq!(
            ledger.adopt_existing_id(first_seq.clone()),
            IdAdoption::Adopted
        );
        assert_eq!(
            ledger.adopt_existing_id(first_seq.clone()),
            IdAdoption::AlreadyKnown
        );
        assert_eq!(
            ledger.id_reservation(&first_seq),
            ReservationStatus::Reserved
        );

        let mut unsynced_issuer = IndependentInstrumentIdIssuer::new();
        let colliding = unsynced_issuer.issue().expect("충돌 후보");
        let stock = source(KnownMarket::Kr, "005930", "KR7005930003");
        assert_eq!(
            ledger.ensure(&stock, colliding),
            Err(InstrumentMappingError::IdAlreadyUsed)
        );
        assert_eq!(ledger.resolve(&stock), Err(InstrumentMappingError::Missing));
        assert_eq!(
            ledger.ensure(&stock, issued_a),
            Ok(MappingAssignment::Issued(
                InstrumentId::try_new("inst-0000000000000002").expect("발급 ID")
            ))
        );
    }

    #[test]
    fn identifier_change_handles_ascii_case_invariants_without_bypassing_cycle_guard() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let lower = source(KnownMarket::Us, "fb", "us30303m1027");
        let upper = source(KnownMarket::Us, "FB", "US30303M1027");
        let meta = source(KnownMarket::Us, "META", "US30303M1027");
        let id = seed_instrument(&mut ledger, &mut generator, &lower);
        let disclosure = sec_disclosure("0001326801-26-000099");

        assert_eq!(
            link_change(
                &mut ledger,
                &id,
                &lower,
                &upper,
                change_evidence(disclosure.clone(), split_action(&id), "대소문자만 변경"),
            ),
            Err(MappingTransitionError::UnchangedSource)
        );
        assert_eq!(
            link_change(
                &mut ledger,
                &id,
                &lower,
                &meta,
                change_evidence(
                    disclosure.clone(),
                    symbol_change_action(&id, "FB", "meta"),
                    "대소문자 무시 심볼 변경",
                ),
            ),
            Ok(id.clone())
        );
        assert_eq!(
            link_change(
                &mut ledger,
                &id,
                &meta,
                &upper,
                change_evidence(
                    disclosure,
                    symbol_change_action(&id, "META", "FB"),
                    "대소문자 우회 순환 시도",
                ),
            ),
            Err(MappingTransitionError::SourceAlreadyRecorded)
        );
    }

    #[test]
    fn instrument_master_accepts_superseded_as_explicit_unavailable_entry() {
        let entry = InstrumentMasterEntry::Unavailable {
            source: source(KnownMarket::Kr, "005930", "KR7005930003"),
            reason: InstrumentMappingError::Superseded,
        };
        let master = InstrumentMaster::try_new(vec![entry.clone()]).expect("마스터");
        assert_eq!(master.entries(), &[entry]);
        assert_eq!(
            master.complete_facts(),
            Err(InstrumentMasterError::UnavailableInstrument)
        );
    }

    #[test]
    fn issuer_never_reemits_adopted_or_previously_issued_ids() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(vec(1_u64..40, 0..20), 1_usize..25),
                |(adopted_slots, count)| {
                    let adopted: Vec<InstrumentId> = adopted_slots
                        .iter()
                        .map(|slot| {
                            InstrumentId::try_new(&format!("inst-{slot:016x}")).expect("순번 ID")
                        })
                        .collect();
                    let adopted_set: BTreeSet<InstrumentId> = adopted.iter().cloned().collect();
                    let mut generator = IndependentInstrumentIdIssuer::from_existing(&adopted);
                    let mut seen = BTreeSet::new();
                    for _ in 0..count {
                        let issued = generator.issue().expect("발급");
                        let value = issued.as_instrument_id().clone();
                        prop_assert!(!adopted_set.contains(&value));
                        prop_assert!(seen.insert(value.clone()));
                        prop_assert_eq!(generator.reservation(&value), ReservationStatus::Reserved);
                    }
                    Ok(())
                },
            )
            .expect("발급 유일성 속성");
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ModelState {
        Active,
        Superseded,
        Retired,
    }

    struct HistoryModel {
        sources: [InstrumentSource; 6],
        records: BTreeMap<usize, (InstrumentId, ModelState)>,
        links: BTreeMap<InstrumentId, Vec<(usize, usize)>>,
        reserved: BTreeSet<InstrumentId>,
        history_len: usize,
        ledger: InstrumentMappings,
    }

    impl HistoryModel {
        fn new(salt: u16) -> Self {
            Self {
                sources: [
                    source(KnownMarket::Kr, &format!("S{salt}0"), &format!("K{salt}0")),
                    source(KnownMarket::Kr, &format!("S{salt}0"), &format!("K{salt}1")),
                    source(KnownMarket::Kr, &format!("S{salt}1"), &format!("K{salt}1")),
                    source(KnownMarket::Kr, &format!("S{salt}2"), &format!("K{salt}2")),
                    source(KnownMarket::Kr, &format!("S{salt}2"), &format!("K{salt}3")),
                    source(KnownMarket::Us, &format!("U{salt}0"), &format!("U{salt}0")),
                ],
                records: BTreeMap::new(),
                links: BTreeMap::new(),
                reserved: BTreeSet::new(),
                history_len: 0,
                ledger: InstrumentMappings::default(),
            }
        }

        fn overlaps(&self, left: usize, right: usize) -> bool {
            let lhs = &self.sources[left];
            let rhs = &self.sources[right];
            (lhs.market() == rhs.market()
                && lhs
                    .symbol()
                    .as_str()
                    .eq_ignore_ascii_case(rhs.symbol().as_str()))
                || lhs
                    .isin()
                    .as_str()
                    .eq_ignore_ascii_case(rhs.isin().as_str())
        }

        fn candidate_at(slot: u8) -> domain::IssuedInstrumentId {
            let mut issuer = IndependentInstrumentIdIssuer::new();
            for _ in 0..slot {
                let _skip = issuer.issue().expect("건너뛰기");
            }
            issuer.issue().expect("후보")
        }

        fn id_at(slot: u8) -> InstrumentId {
            Self::candidate_at(slot).as_instrument_id().clone()
        }

        fn record_active(
            active_by_id: &mut BTreeMap<InstrumentId, usize>,
            id: &InstrumentId,
            src_idx: usize,
            state: ModelState,
        ) -> proptest::test_runner::TestCaseResult {
            if state == ModelState::Active {
                prop_assert_eq!(active_by_id.insert(id.clone(), src_idx), None);
            }
            Ok(())
        }

        fn lookup_error(error: &InstrumentMappingError) -> TestCaseError {
            TestCaseError::fail(format!("등록된 원본 조회 실패: {error:?}"))
        }

        fn check_invariants(&self) -> proptest::test_runner::TestCaseResult {
            let mut active_by_id = BTreeMap::new();
            for (src_idx, (id, state)) in &self.records {
                Self::record_active(&mut active_by_id, id, *src_idx, *state)?;
                let expected_status = match state {
                    ModelState::Active => InstrumentMappingStatus::Active,
                    ModelState::Superseded => InstrumentMappingStatus::Superseded,
                    ModelState::Retired => InstrumentMappingStatus::Retired,
                };
                let lookup = self
                    .ledger
                    .lookup(&self.sources[*src_idx])
                    .map_err(|error| Self::lookup_error(&error))?;
                prop_assert_eq!(lookup.id(), id);
                prop_assert_eq!(lookup.status(), expected_status);
            }
            prop_assert_eq!(self.ledger.history().len(), self.history_len);
            Ok(())
        }

        fn lookup_owned(
            &self,
            src: usize,
            target_id: &InstrumentId,
        ) -> Result<ModelState, MappingTransitionError> {
            match self.records.get(&src) {
                Some((owner, _)) if owner != target_id => {
                    Err(MappingTransitionError::SourceBoundToOtherId)
                }
                Some((_, state)) => Ok(*state),
                None if self.records.keys().any(|other| self.overlaps(*other, src)) => {
                    Err(MappingTransitionError::IdentityUncertain)
                }
                None => Err(MappingTransitionError::Missing),
            }
        }

        fn check_next_model(
            &self,
            next: usize,
            target_id: &InstrumentId,
        ) -> Result<(), MappingTransitionError> {
            match self.records.get(&next) {
                Some((owner, _)) if owner == target_id => {
                    Err(MappingTransitionError::SourceAlreadyRecorded)
                }
                Some(_) => Err(MappingTransitionError::SourceBoundToOtherId),
                None if self.records.iter().any(|(other, (owner, _))| {
                    owner != target_id && self.overlaps(*other, next)
                }) =>
                {
                    Err(MappingTransitionError::IdentityUncertain)
                }
                None => Ok(()),
            }
        }

        fn apply_link_model(
            &mut self,
            target_id: InstrumentId,
            prev: usize,
            next: usize,
            origin: IdentityConfirmationOrigin,
        ) -> Result<InstrumentId, MappingTransitionError> {
            if origin != IdentityConfirmationOrigin::UserConfirmed {
                return Err(MappingTransitionError::IdentityUncertain);
            }
            if prev == next {
                return Err(MappingTransitionError::UnchangedSource);
            }
            if self.sources[prev].market() != self.sources[next].market() {
                return Err(MappingTransitionError::MarketMismatch);
            }
            if self.lookup_owned(prev, &target_id)? != ModelState::Active {
                return Err(MappingTransitionError::SourceNotActive);
            }
            self.check_next_model(next, &target_id)?;
            self.records
                .insert(prev, (target_id.clone(), ModelState::Superseded));
            self.records
                .insert(next, (target_id.clone(), ModelState::Active));
            self.links
                .entry(target_id.clone())
                .or_default()
                .push((prev, next));
            self.history_len = self.history_len.saturating_add(1);
            Ok(target_id)
        }

        fn apply_correct_link_model(
            &mut self,
            target_id: InstrumentId,
            prev: usize,
            next: usize,
        ) -> Result<InstrumentId, MappingTransitionError> {
            if prev == next {
                return Err(MappingTransitionError::UnchangedSource);
            }
            let _prev_state = self.lookup_owned(prev, &target_id)?;
            let next_state = self.lookup_owned(next, &target_id)?;
            let latest = self
                .links
                .get(&target_id)
                .and_then(|stack| stack.last())
                .copied();
            if latest != Some((prev, next)) || next_state != ModelState::Active {
                return Err(MappingTransitionError::LinkNotLatest);
            }
            if let Some(stack) = self.links.get_mut(&target_id) {
                let _popped = stack.pop();
            }
            self.records
                .insert(prev, (target_id.clone(), ModelState::Active));
            self.records.remove(&next);
            self.history_len = self.history_len.saturating_add(1);
            Ok(target_id)
        }

        fn step_adopt(&mut self, slot: u8) -> proptest::test_runner::TestCaseResult {
            let id = Self::id_at(slot);
            let expected = if self.reserved.insert(id.clone()) {
                IdAdoption::Adopted
            } else {
                IdAdoption::AlreadyKnown
            };
            prop_assert_eq!(self.ledger.adopt_existing_id(id), expected);
            Ok(())
        }

        fn step_ensure(&mut self, src: usize, slot: u8) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let candidate = Self::candidate_at(slot);
            let candidate_id = candidate.as_instrument_id().clone();
            let expected = match self.records.get(&src) {
                Some((id, ModelState::Active)) => Ok(MappingAssignment::Existing(id.clone())),
                Some((_, ModelState::Superseded)) => Err(InstrumentMappingError::Superseded),
                Some((_, ModelState::Retired)) => Err(InstrumentMappingError::Retired),
                None if self.records.keys().any(|other| self.overlaps(*other, src)) => {
                    Err(InstrumentMappingError::IdentityUncertain)
                }
                None if self.reserved.contains(&candidate_id) => {
                    Err(InstrumentMappingError::IdAlreadyUsed)
                }
                None => {
                    self.records
                        .insert(src, (candidate_id.clone(), ModelState::Active));
                    self.reserved.insert(candidate_id.clone());
                    Ok(MappingAssignment::Issued(candidate_id))
                }
            };
            let actual = self.ledger.ensure(&self.sources[src], candidate);
            if expected.is_err() {
                prop_assert_eq!(&self.ledger, &before);
            }
            prop_assert_eq!(actual, expected);
            Ok(())
        }

        fn step_retire(&mut self, src: usize) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let expected = match self.records.get(&src).cloned() {
                Some((id, ModelState::Active | ModelState::Retired)) => {
                    self.records.insert(src, (id.clone(), ModelState::Retired));
                    Ok(id)
                }
                Some((_, ModelState::Superseded)) => Err(InstrumentMappingError::Superseded),
                None if self.records.keys().any(|other| self.overlaps(*other, src)) => {
                    Err(InstrumentMappingError::IdentityUncertain)
                }
                None => Err(InstrumentMappingError::Missing),
            };
            let actual = self.ledger.retire(&self.sources[src]);
            if expected.is_err() {
                prop_assert_eq!(&self.ledger, &before);
            }
            prop_assert_eq!(actual, expected);
            Ok(())
        }

        fn step_link(
            &mut self,
            id_slot: u8,
            prev: usize,
            next: usize,
            origin: IdentityConfirmationOrigin,
        ) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let target_id = Self::id_at(id_slot);
            let change = ConfirmedIdentifierChange::new(
                target_id.clone(),
                self.sources[prev].clone(),
                self.sources[next].clone(),
                IdentityChangeConfirmation::new(
                    origin,
                    change_evidence(
                        dart_disclosure("20261120000001"),
                        split_action(&target_id),
                        "속성 테스트 변경",
                    ),
                    timestamp(1_700_005_000),
                ),
            );
            let expected = self.apply_link_model(target_id, prev, next, origin);
            let actual = self.ledger.link_confirmed_change(&change);
            if expected.is_err() {
                prop_assert_eq!(&self.ledger, &before);
            }
            prop_assert_eq!(actual, expected);
            Ok(())
        }

        fn step_correct_retirement(
            &mut self,
            id_slot: u8,
            src: usize,
        ) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let target_id = Self::id_at(id_slot);
            let correction = ConfirmedRetirementCorrection::new(
                target_id.clone(),
                self.sources[src].clone(),
                valid_correction(&kind_disclosure("20261120000002")),
            );
            let expected = match self.lookup_owned(src, &target_id) {
                Ok(ModelState::Retired) => {
                    self.records
                        .insert(src, (target_id.clone(), ModelState::Active));
                    self.history_len = self.history_len.saturating_add(1);
                    Ok(target_id)
                }
                Ok(ModelState::Active | ModelState::Superseded) => {
                    Err(MappingTransitionError::SourceNotRetired)
                }
                Err(error) => Err(error),
            };
            let actual = self.ledger.correct_retirement(&correction);
            if expected.is_err() {
                prop_assert_eq!(&self.ledger, &before);
            }
            prop_assert_eq!(actual, expected);
            Ok(())
        }

        fn step_correct_link(
            &mut self,
            id_slot: u8,
            prev: usize,
            next: usize,
        ) -> proptest::test_runner::TestCaseResult {
            let before = self.ledger.clone();
            let target_id = Self::id_at(id_slot);
            let correction = ConfirmedLinkCorrection::new(
                target_id.clone(),
                self.sources[prev].clone(),
                self.sources[next].clone(),
                valid_correction(&kind_disclosure("20261120000003")),
            );
            let expected = self.apply_correct_link_model(target_id, prev, next);
            let actual = self.ledger.correct_link(&correction);
            if expected.is_err() {
                prop_assert_eq!(&self.ledger, &before);
            }
            prop_assert_eq!(actual, expected);
            Ok(())
        }
    }

    #[test]
    fn history_and_correction_state_machine_matches_independent_model() {
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &(
                    0_u16..5_000,
                    vec((0_u8..7, 0_usize..6, 0_usize..6, 0_u8..4), 0..45),
                ),
                |(salt, steps)| {
                    let mut model = HistoryModel::new(salt);
                    for (op, first, second, slot) in steps {
                        match op {
                            0 => model.step_ensure(first, slot)?,
                            1 => model.step_retire(first)?,
                            2 => model.step_link(
                                slot,
                                first,
                                second,
                                IdentityConfirmationOrigin::UserConfirmed,
                            )?,
                            3 => model.step_link(
                                slot,
                                first,
                                second,
                                IdentityConfirmationOrigin::BrokerObservation,
                            )?,
                            4 => model.step_correct_retirement(slot, first)?,
                            5 => model.step_correct_link(slot, first, second)?,
                            _ => model.step_adopt(slot)?,
                        }
                        model.check_invariants()?;
                    }
                    Ok(())
                },
            )
            .expect("이력·정정 상태 기계 속성");
    }
}
