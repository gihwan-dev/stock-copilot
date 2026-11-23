#[cfg(test)]
mod tests {
    use domain::{
        CalendarDate, CalendarYear, ConfirmedIdentifierChange, ConfirmedLinkCorrection,
        ConfirmedRetirementCorrection, CorporateAction, CorporateActionId, CorporateActionKind,
        DayNumber, Disclosure, DisclosureKey, IdAdoption, IdentityChangeConfirmation,
        IdentityChangeEvidence, IdentityConfirmationOrigin, IndependentInstrumentIdIssuer,
        InstrumentId, InstrumentMappingError, InstrumentMappings, InstrumentMaster,
        InstrumentMasterEntry, InstrumentMasterError, InstrumentSource, IsinCode, KnownMarket,
        MappingAssignment, MappingCorrectionConfirmation, MappingCorrectionEvidence,
        MappingTransitionError, MonthNumber, Nanosecond, NormalizedUrl, OffsetSeconds, ReceiptNo,
        ReservationStatus, SecEdgarReceiptNo, Symbol, Timestamp, UnixSeconds, UntrustedText,
        UserReason,
    };
    use proptest::{
        collection::vec,
        prop_assert, prop_assert_eq,
        test_runner::{Config, TestRunner},
    };
    use std::collections::BTreeSet;

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

    fn note(text: &str) -> UserReason {
        UserReason::try_new(UntrustedText::try_new(text).expect("문구")).expect("사유")
    }

    fn symbol_change_action(id: &InstrumentId, from: &str, to: &str) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            CalendarDate::from_parts(
                CalendarYear::new(2026),
                MonthNumber::new(11),
                DayNumber::new(11),
            )
            .expect("날짜"),
            CorporateActionKind::SymbolChange {
                from: Symbol::try_new(from).expect("이전 심볼"),
                to: Symbol::try_new(to).expect("이후 심볼"),
            },
        )
        .expect("심볼 변경")
    }

    fn seed_instrument(
        ledger: &mut InstrumentMappings,
        generator: &mut IndependentInstrumentIdIssuer,
        src: &InstrumentSource,
    ) -> InstrumentId {
        let candidate = generator.issue().expect("발급");
        let id = candidate.as_instrument_id().clone();
        assert_eq!(
            ledger.ensure(src, candidate),
            Ok(MappingAssignment::Issued(id.clone()))
        );
        id
    }

    fn assert_unconfirmed_origin_rejected(
        ledger: &mut InstrumentMappings,
        id: &InstrumentId,
        previous: &InstrumentSource,
        next: &InstrumentSource,
        make_origin: fn() -> IdentityConfirmationOrigin,
    ) {
        let disclosure = sec_disclosure("0001326801-26-000001");
        let action = symbol_change_action(id, "FB", "META");
        let change_ev = IdentityChangeEvidence::DisclosureAndAction {
            disclosure: disclosure.clone(),
            action,
            note: note("비확인 변경 시도"),
        };
        let change = ConfirmedIdentifierChange::new(
            id.clone(),
            previous.clone(),
            next.clone(),
            IdentityChangeConfirmation::new(make_origin(), change_ev, timestamp(1_700_001_000)),
        );
        assert_eq!(
            ledger.link_confirmed_change(change),
            Err(MappingTransitionError::IdentityUncertain)
        );
        let corr_ev = MappingCorrectionEvidence::DisclosureAndNote {
            disclosure,
            note: note("비확인 정정 시도"),
        };
        assert_eq!(
            ledger.correct_retirement(ConfirmedRetirementCorrection::new(
                id.clone(),
                previous.clone(),
                MappingCorrectionConfirmation::new(
                    make_origin(),
                    corr_ev.clone(),
                    timestamp(1_700_001_001),
                ),
            )),
            Err(MappingTransitionError::IdentityUncertain)
        );
        assert_eq!(
            ledger.correct_link(ConfirmedLinkCorrection::new(
                id.clone(),
                previous.clone(),
                next.clone(),
                MappingCorrectionConfirmation::new(
                    make_origin(),
                    corr_ev,
                    timestamp(1_700_001_002),
                ),
            )),
            Err(MappingTransitionError::IdentityUncertain)
        );
    }

    #[test]
    fn external_unconfirmed_origins_stay_identity_uncertain_across_transitions() {
        let mut generator = IndependentInstrumentIdIssuer::new();
        let mut ledger = InstrumentMappings::default();
        let previous = source(KnownMarket::Us, "FB", "US30303M1027");
        let next = source(KnownMarket::Us, "META", "US30303M1027");
        let id = seed_instrument(&mut ledger, &mut generator, &previous);
        let before = ledger.clone();
        let origins: [fn() -> IdentityConfirmationOrigin; 2] = [
            || IdentityConfirmationOrigin::BrokerObservation,
            || IdentityConfirmationOrigin::AiSuggestion,
        ];
        for make_origin in origins {
            assert_unconfirmed_origin_rejected(&mut ledger, &id, &previous, &next, make_origin);
            assert_eq!(ledger, before);
        }
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
}
