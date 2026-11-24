#[cfg(test)]
mod tests {
    use application::{
        ConfirmInstrumentIdentity, IdentifierChangePrompt, IdentityConfirmationDecision,
        IdentityConfirmationOutcome, InstrumentIdIssuer, InstrumentMappingRepository,
        LinkCorrectionPrompt, MappingStoreError, RetirementCorrectionPrompt,
    };
    use domain::{
        CalendarDate, CalendarYear, ConfirmedIdentifierChange, ConfirmedLinkCorrection,
        ConfirmedRetirementCorrection, CorporateAction, CorporateActionId, CorporateActionKind,
        DayNumber, Disclosure, DisclosureKey, IdentityChangeConfirmation, IdentityChangeEvidence,
        IdentityConfirmationOrigin, InstrumentId, InstrumentMappingError, InstrumentMappingRecord,
        InstrumentMappingStatus, InstrumentSource, IsinCode, KnownMarket, MappingAssignment,
        MappingCorrectionConfirmation, MappingCorrectionEvidence, MappingHistoryEvent, MonthNumber,
        Nanosecond, NormalizedUrl, OffsetSeconds, ReceiptNo, SecEdgarReceiptNo, Symbol, Timestamp,
        UnixSeconds, UntrustedText, UserReason,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        IdentityConfirmationCall, IdentityConfirmationIssuanceKind, ScriptedIdentityConfirmation,
        ScriptedIdentityConfirmationIssuer, SteppedClock,
        memory::{MemoryGlobalInstrumentIds, MemoryInstrumentMappingRepository},
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

    fn sec_disclosure(receipt: &str) -> DisclosureKey {
        Disclosure::new(
            ReceiptNo::SecEdgar(SecEdgarReceiptNo::try_new(receipt).expect("SEC")),
            NormalizedUrl::try_new("https://www.sec.gov/Archives/edgar/data/1/index.htm")
                .expect("URL"),
            UntrustedText::try_new("Symbol change filing").expect("제목"),
            None,
        )
        .key()
        .clone()
    }

    fn reason(text: &str) -> UserReason {
        UserReason::try_new(UntrustedText::try_new(text).expect("문구")).expect("사유")
    }

    fn symbol_change_action(id: &InstrumentId, from: &str, to: &str) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            CalendarDate::from_parts(
                CalendarYear::new(2026),
                MonthNumber::new(11),
                DayNumber::new(24),
            )
            .expect("날짜"),
            CorporateActionKind::SymbolChange {
                from: Symbol::try_new(from).expect("이전 심볼"),
                to: Symbol::try_new(to).expect("이후 심볼"),
            },
        )
        .expect("기업행위")
    }

    fn seed_repo(initial: &InstrumentSource) -> (MemoryInstrumentMappingRepository, InstrumentId) {
        let global = MemoryGlobalInstrumentIds::default();
        let generator = global.issuer();
        let repo = global.repository();
        let candidate = run(generator.issue()).expect("ID 발급");
        let id = match run(repo.ensure(initial, candidate)).expect("최초 등록") {
            MappingAssignment::Issued(assigned) => assigned,
            MappingAssignment::Existing(_) => panic!("신규 발급이어야 함"),
        };
        (repo, id)
    }

    fn unwrap_confirmed<T>(outcome: IdentityConfirmationOutcome<T>) -> T {
        match outcome {
            IdentityConfirmationOutcome::Confirmed(value) => value,
            IdentityConfirmationOutcome::Denied | IdentityConfirmationOutcome::Unavailable => {
                panic!("확인 성공이어야 함")
            }
        }
    }

    fn fb_to_meta_prompt(
        id: &InstrumentId,
        previous: &InstrumentSource,
        next: &InstrumentSource,
        receipt: &str,
        note_text: &str,
    ) -> (
        IdentifierChangePrompt,
        DisclosureKey,
        CorporateAction,
        UserReason,
    ) {
        let disclosure = sec_disclosure(receipt);
        let action = symbol_change_action(id, "FB", "META");
        let note = reason(note_text);
        let evidence = IdentityChangeEvidence::DisclosureAndAction {
            disclosure: disclosure.clone(),
            action: action.clone(),
            note: note.clone(),
        };
        let request =
            IdentifierChangePrompt::new(id.clone(), previous.clone(), next.clone(), evidence);
        (request, disclosure, action, note)
    }

    fn assert_linked_change_state(
        repo: &MemoryInstrumentMappingRepository,
        id: &InstrumentId,
        previous: &InstrumentSource,
        next: &InstrumentSource,
        expected_event: MappingHistoryEvent,
    ) {
        assert_eq!(
            run(repo.resolve(previous)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::Superseded
            ))
        );
        assert_eq!(run(repo.resolve(next)), Ok(id.clone()));
        assert_eq!(
            run(repo.lookup(previous)),
            Ok(InstrumentMappingRecord::new(
                previous.clone(),
                id.clone(),
                InstrumentMappingStatus::Superseded,
            ))
        );
        assert_eq!(
            run(repo.lookup(next)),
            Ok(InstrumentMappingRecord::new(
                next.clone(),
                id.clone(),
                InstrumentMappingStatus::Active,
            ))
        );
        assert_eq!(repo.history(), vec![expected_event.clone()]);
        assert_eq!(repo.history_for_id(id), vec![expected_event]);
    }

    #[test]
    fn confirmed_identifier_change_succeeds_through_use_case_and_memory_repository() {
        let previous = source(KnownMarket::Us, "FB", "US30303M1027");
        let next = source(KnownMarket::Us, "META", "US30303M1027");
        let (repo, id) = seed_repo(&previous);
        let (request, disclosure, action, note) = fb_to_meta_prompt(
            &id,
            &previous,
            &next,
            "0001326801-26-000101",
            "티커 변경 확인",
        );
        let prompt =
            ScriptedIdentityConfirmation::new([IdentityConfirmationDecision::Confirmed], [], []);
        let confirmed_at = timestamp(1_764_000_100);
        let clock = SteppedClock::new(confirmed_at);
        let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);

        let change = unwrap_confirmed(run(use_case.confirm_identifier_change(request.clone())));
        assert_eq!(change.instrument_id(), &id);
        assert_eq!(change.previous(), &previous);
        assert_eq!(change.next(), &next);
        assert_eq!(change.confirmation().confirmed_at(), confirmed_at);
        assert_eq!(prompt.identifier_change_calls(), vec![request.clone()]);
        assert_eq!(prompt.retirement_correction_calls(), vec![]);
        assert_eq!(prompt.link_correction_calls(), vec![]);
        assert_eq!(
            prompt.calls(),
            vec![IdentityConfirmationCall::IdentifierChange(Box::new(
                request
            ))]
        );
        assert_eq!(clock.calls(), vec![confirmed_at]);

        assert_eq!(repo.link_confirmed_change(change), Ok(id.clone()));
        let expected_event = MappingHistoryEvent::IdentifierChanged {
            instrument_id: id.clone(),
            previous: previous.clone(),
            next: next.clone(),
            disclosure,
            action: Box::new(action),
            note,
            confirmed_at,
        };
        assert_linked_change_state(&repo, &id, &previous, &next, expected_event);
    }

    #[test]
    fn confirmed_retirement_correction_succeeds_through_use_case_and_memory_repository() {
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let (repo, id) = seed_repo(&original);
        assert_eq!(run(repo.retire(&original)), Ok(id.clone()));
        assert_eq!(
            run(repo.resolve(&original)),
            Err(MappingStoreError::Mapping(InstrumentMappingError::Retired))
        );

        let disclosure = sec_disclosure("0001326801-26-000102");
        let note = reason("상장폐지 오기록 정정 확인");
        let evidence = MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: disclosure.clone(),
            note: note.clone(),
        };
        let request = RetirementCorrectionPrompt::new(id.clone(), original.clone(), evidence);
        let prompt =
            ScriptedIdentityConfirmation::new([], [IdentityConfirmationDecision::Confirmed], []);
        let corrected_at = timestamp(1_764_000_200);
        let clock = SteppedClock::new(corrected_at);
        let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);

        let correction =
            unwrap_confirmed(run(use_case.confirm_retirement_correction(request.clone())));
        assert_eq!(correction.instrument_id(), &id);
        assert_eq!(correction.source(), &original);
        assert_eq!(correction.confirmation().corrected_at(), corrected_at);
        assert_eq!(prompt.retirement_correction_calls(), vec![request.clone()]);
        assert_eq!(prompt.identifier_change_calls(), vec![]);
        assert_eq!(prompt.link_correction_calls(), vec![]);
        assert_eq!(
            prompt.calls(),
            vec![IdentityConfirmationCall::RetirementCorrection(request)]
        );
        assert_eq!(clock.calls(), vec![corrected_at]);

        assert_eq!(repo.correct_retirement(correction), Ok(id.clone()));
        assert_eq!(run(repo.resolve(&original)), Ok(id.clone()));
        assert_eq!(
            run(repo.lookup(&original)),
            Ok(InstrumentMappingRecord::new(
                original.clone(),
                id.clone(),
                InstrumentMappingStatus::Active,
            ))
        );
        let expected_event = MappingHistoryEvent::RetirementCorrected {
            instrument_id: id.clone(),
            source: original,
            disclosure,
            note,
            corrected_at,
        };
        assert_eq!(repo.history(), vec![expected_event.clone()]);
        assert_eq!(repo.history_for_id(&id), vec![expected_event]);
    }

    fn link_correction_prompt(
        id: &InstrumentId,
        previous: &InstrumentSource,
        mistaken_next: &InstrumentSource,
        disclosure_key: &str,
        note_text: &str,
    ) -> (LinkCorrectionPrompt, DisclosureKey, UserReason) {
        let disclosure = sec_disclosure(disclosure_key);
        let note = reason(note_text);
        let request = LinkCorrectionPrompt::new(
            id.clone(),
            previous.clone(),
            mistaken_next.clone(),
            MappingCorrectionEvidence::DisclosureAndNote {
                disclosure: disclosure.clone(),
                note: note.clone(),
            },
        );
        (request, disclosure, note)
    }

    fn assert_link_correction_applied(
        repo: &MemoryInstrumentMappingRepository,
        id: &InstrumentId,
        previous: &InstrumentSource,
        mistaken_next: &InstrumentSource,
        expected: &[MappingHistoryEvent],
    ) {
        assert_eq!(run(repo.resolve(previous)), Ok(id.clone()));
        assert_eq!(
            run(repo.lookup(mistaken_next)),
            Err(MappingStoreError::Mapping(
                InstrumentMappingError::IdentityUncertain
            ))
        );
        assert_eq!(repo.history_for_id(id), expected);
    }

    fn expected_link_correction_history(
        id: &InstrumentId,
        previous: &InstrumentSource,
        mistaken_next: &InstrumentSource,
        change_parts: (DisclosureKey, CorporateAction, UserReason, Timestamp),
        unwind_parts: (DisclosureKey, UserReason, Timestamp),
    ) -> [MappingHistoryEvent; 2] {
        let (link_disc, action, link_note, changed_at) = change_parts;
        let (unwind_disc, unwind_note, corrected_at) = unwind_parts;
        [
            MappingHistoryEvent::IdentifierChanged {
                instrument_id: id.clone(),
                previous: previous.clone(),
                next: mistaken_next.clone(),
                disclosure: link_disc,
                action: Box::new(action),
                note: link_note,
                confirmed_at: changed_at,
            },
            MappingHistoryEvent::LinkCorrected {
                instrument_id: id.clone(),
                restored_previous: previous.clone(),
                unlinked_next: mistaken_next.clone(),
                disclosure: unwind_disc,
                note: unwind_note,
                corrected_at,
            },
        ]
    }

    #[test]
    fn confirmed_link_correction_succeeds_through_use_case_and_memory_repository() {
        let previous = source(KnownMarket::Us, "FB", "US30303M1027");
        let mistaken_next = source(KnownMarket::Us, "META", "US30303M1027");
        let (repo, id) = seed_repo(&previous);
        let (change_req, link_disc, action, link_note) = fb_to_meta_prompt(
            &id,
            &previous,
            &mistaken_next,
            "0001326801-26-000103",
            "초기 연결",
        );
        let (unwind_req, unwind_disc, unwind_note) = link_correction_prompt(
            &id,
            &previous,
            &mistaken_next,
            "0001326801-26-000104",
            "오연결 되돌림 확인",
        );
        let prompt = ScriptedIdentityConfirmation::new(
            [IdentityConfirmationDecision::Confirmed],
            [],
            [IdentityConfirmationDecision::Confirmed],
        );
        let changed_at = timestamp(1_764_000_300);
        let corrected_at = timestamp(1_764_000_400);
        let clock = SteppedClock::new(changed_at);
        let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);

        let change = unwrap_confirmed(run(use_case.confirm_identifier_change(change_req.clone())));
        assert_eq!(repo.link_confirmed_change(change), Ok(id.clone()));
        clock.advance_to(corrected_at).expect("시각 전진");
        let correction =
            unwrap_confirmed(run(use_case.confirm_link_correction(unwind_req.clone())));
        assert_eq!(correction.instrument_id(), &id);
        assert_eq!(correction.previous(), &previous);
        assert_eq!(correction.mistaken_next(), &mistaken_next);
        assert_eq!(correction.confirmation().corrected_at(), corrected_at);
        assert_eq!(prompt.link_correction_calls(), vec![unwind_req.clone()]);
        assert_eq!(
            prompt.calls(),
            vec![
                IdentityConfirmationCall::IdentifierChange(Box::new(change_req)),
                IdentityConfirmationCall::LinkCorrection(unwind_req),
            ]
        );
        assert_eq!(clock.calls(), vec![changed_at, corrected_at]);
        assert_eq!(repo.correct_link(correction), Ok(id.clone()));
        let expected = expected_link_correction_history(
            &id,
            &previous,
            &mistaken_next,
            (link_disc, action, link_note, changed_at),
            (unwind_disc, unwind_note, corrected_at),
        );
        assert_link_correction_applied(&repo, &id, &previous, &mistaken_next, &expected);
    }

    #[test]
    fn denied_and_unavailable_decisions_never_issue_confirmations_or_read_clock() {
        let id = InstrumentId::try_new("inst-0000000000000001").expect("종목 ID");
        let previous = source(KnownMarket::Us, "FB", "US30303M1027");
        let next = source(KnownMarket::Us, "META", "US30303M1027");
        let (change_request, disclosure, _, _) =
            fb_to_meta_prompt(&id, &previous, &next, "0001326801-26-000105", "거부 시험");
        let corr_evidence = MappingCorrectionEvidence::DisclosureAndNote {
            disclosure,
            note: reason("거부 정정 시험"),
        };
        let retirement_request =
            RetirementCorrectionPrompt::new(id.clone(), previous.clone(), corr_evidence.clone());
        let link_request = LinkCorrectionPrompt::new(id, previous, next, corr_evidence);

        let refusals = [
            IdentityConfirmationDecision::Denied,
            IdentityConfirmationDecision::Unavailable,
        ];
        let prompt = ScriptedIdentityConfirmation::new(refusals, refusals, refusals);
        let clock = SteppedClock::new(timestamp(1_764_000_500));
        let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);

        for expected in [
            IdentityConfirmationOutcome::Denied,
            IdentityConfirmationOutcome::Unavailable,
        ] {
            assert_eq!(
                run(use_case.confirm_identifier_change(change_request.clone())),
                expected
            );
        }
        for expected in [
            IdentityConfirmationOutcome::Denied,
            IdentityConfirmationOutcome::Unavailable,
        ] {
            assert_eq!(
                run(use_case.confirm_retirement_correction(retirement_request.clone())),
                expected
            );
        }
        for expected in [
            IdentityConfirmationOutcome::Denied,
            IdentityConfirmationOutcome::Unavailable,
        ] {
            assert_eq!(
                run(use_case.confirm_link_correction(link_request.clone())),
                expected
            );
        }

        assert_eq!(clock.calls(), vec![]);
        assert_eq!(
            prompt.identifier_change_calls(),
            vec![change_request.clone(), change_request]
        );
        assert_eq!(
            prompt.retirement_correction_calls(),
            vec![retirement_request.clone(), retirement_request]
        );
        assert_eq!(
            prompt.link_correction_calls(),
            vec![link_request.clone(), link_request]
        );
    }

    #[test]
    fn unpolled_confirmation_futures_never_record_calls_or_consume_scripts() {
        let id = InstrumentId::try_new("inst-0000000000000001").expect("종목 ID");
        let previous = source(KnownMarket::Us, "FB", "US30303M1027");
        let next = source(KnownMarket::Us, "META", "US30303M1027");
        let (request, _, _, _) =
            fb_to_meta_prompt(&id, &previous, &next, "0001326801-26-000106", "미폴링 시험");
        let prompt =
            ScriptedIdentityConfirmation::new([IdentityConfirmationDecision::Confirmed], [], []);
        let clock = SteppedClock::new(timestamp(1_764_000_600));
        let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);
        {
            let _pending = use_case.confirm_identifier_change(request.clone());
        }
        assert_eq!(prompt.calls(), vec![]);
        assert_eq!(clock.calls(), vec![]);
        let _confirmed = unwrap_confirmed(run(use_case.confirm_identifier_change(request)));
    }

    fn apply_scripted_issuer_writes(
        repo: &MemoryInstrumentMappingRepository,
        confirmation_issuer: &ScriptedIdentityConfirmationIssuer,
        id: &InstrumentId,
        previous: &InstrumentSource,
        next: &InstrumentSource,
    ) {
        let disclosure = sec_disclosure("0001326801-26-000107");
        let change_seal = unwrap_confirmed(run(
            confirmation_issuer.issue(IdentityConfirmationIssuanceKind::IdentifierChange)
        ));
        let change = ConfirmedIdentifierChange::new(
            id.clone(),
            previous.clone(),
            next.clone(),
            IdentityChangeConfirmation::new(
                IdentityConfirmationOrigin::UserConfirmed(change_seal),
                IdentityChangeEvidence::DisclosureAndAction {
                    disclosure: disclosure.clone(),
                    action: symbol_change_action(id, "FB", "META"),
                    note: reason("직접 발급원 연결"),
                },
                timestamp(1_764_000_700),
            ),
        );
        assert_eq!(repo.link_confirmed_change(change), Ok(id.clone()));

        let link_corr_seal = unwrap_confirmed(run(
            confirmation_issuer.issue(IdentityConfirmationIssuanceKind::LinkCorrection)
        ));
        let link_corr = ConfirmedLinkCorrection::new(
            id.clone(),
            previous.clone(),
            next.clone(),
            MappingCorrectionConfirmation::new(
                IdentityConfirmationOrigin::UserConfirmed(link_corr_seal),
                MappingCorrectionEvidence::DisclosureAndNote {
                    disclosure: disclosure.clone(),
                    note: reason("직접 발급원 연결 정정"),
                },
                timestamp(1_764_000_701),
            ),
        );
        assert_eq!(repo.correct_link(link_corr), Ok(id.clone()));

        assert_eq!(run(repo.retire(previous)), Ok(id.clone()));
        let retire_corr_seal = unwrap_confirmed(run(
            confirmation_issuer.issue(IdentityConfirmationIssuanceKind::RetirementCorrection)
        ));
        let retire_corr = ConfirmedRetirementCorrection::new(
            id.clone(),
            previous.clone(),
            MappingCorrectionConfirmation::new(
                IdentityConfirmationOrigin::UserConfirmed(retire_corr_seal),
                MappingCorrectionEvidence::DisclosureAndNote {
                    disclosure,
                    note: reason("직접 발급원 폐지 정정"),
                },
                timestamp(1_764_000_702),
            ),
        );
        assert_eq!(repo.correct_retirement(retire_corr), Ok(id.clone()));
    }

    #[test]
    fn protected_scripted_issuer_enforces_decisions_and_opens_all_three_memory_write_paths() {
        let previous = source(KnownMarket::Us, "FB", "US30303M1027");
        let next = source(KnownMarket::Us, "META", "US30303M1027");
        let (repo, id) = seed_repo(&previous);

        let confirmation_issuer = ScriptedIdentityConfirmationIssuer::new([
            IdentityConfirmationDecision::Denied,
            IdentityConfirmationDecision::Unavailable,
            IdentityConfirmationDecision::Confirmed,
            IdentityConfirmationDecision::Confirmed,
            IdentityConfirmationDecision::Confirmed,
        ]);
        {
            let _unpolled =
                confirmation_issuer.issue(IdentityConfirmationIssuanceKind::IdentifierChange);
        }
        assert_eq!(confirmation_issuer.calls(), vec![]);
        assert_eq!(
            run(confirmation_issuer.issue(IdentityConfirmationIssuanceKind::IdentifierChange)),
            IdentityConfirmationOutcome::Denied
        );
        assert_eq!(
            run(confirmation_issuer.issue(IdentityConfirmationIssuanceKind::RetirementCorrection)),
            IdentityConfirmationOutcome::Unavailable
        );

        apply_scripted_issuer_writes(&repo, &confirmation_issuer, &id, &previous, &next);
        assert_eq!(run(repo.resolve(&previous)), Ok(id));
        assert_eq!(
            confirmation_issuer.calls(),
            vec![
                IdentityConfirmationIssuanceKind::IdentifierChange,
                IdentityConfirmationIssuanceKind::RetirementCorrection,
                IdentityConfirmationIssuanceKind::IdentifierChange,
                IdentityConfirmationIssuanceKind::LinkCorrection,
                IdentityConfirmationIssuanceKind::RetirementCorrection,
            ]
        );
    }

    #[test]
    #[should_panic(expected = "대본 없음")]
    fn identifier_change_script_does_not_satisfy_retirement_correction() {
        let id = InstrumentId::try_new("inst-0000000000000001").expect("종목 ID");
        let original = source(KnownMarket::Kr, "005930", "KR7005930003");
        let request = RetirementCorrectionPrompt::new(
            id,
            original,
            MappingCorrectionEvidence::DisclosureAndNote {
                disclosure: sec_disclosure("0001326801-26-000108"),
                note: reason("큐 격리 시험"),
            },
        );
        let prompt =
            ScriptedIdentityConfirmation::new([IdentityConfirmationDecision::Confirmed], [], []);
        let clock = SteppedClock::new(timestamp(1_764_000_800));
        let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);
        let _outcome = run(use_case.confirm_retirement_correction(request));
    }

    #[test]
    #[should_panic(expected = "대본 없음")]
    fn exhausted_scripted_issuer_panics_instead_of_synthesizing_confirmation() {
        let issuer = ScriptedIdentityConfirmationIssuer::new([]);
        let _outcome = run(issuer.issue(IdentityConfirmationIssuanceKind::IdentifierChange));
    }
}
