#[cfg(test)]
mod tests {
    use domain::{
        CalendarDate, CalendarYear, Cancellation, CorporateAction, CorporateActionId,
        CorporateActionKind, Currency, DartReceiptNo, DayNumber, DisclosureKey, DisclosureSource,
        InstrumentId, InstrumentMappingRecord, InstrumentMappingStatus, InstrumentSource, IsinCode,
        KindReceiptNo, KnownMarket, MappingHistoryEvent, MappingLedgerIntegrityError, Market,
        Money, MonthNumber, Nanosecond, NumericInput, OffsetSeconds, ReceiptNo, SecEdgarReceiptNo,
        Settlement, ShareCount, ShareRatio, Symbol, Timestamp, Transaction, TransactionData,
        TransactionKind, UnixSeconds, UntrustedText, UserReason, verify_persisted_mapping_ledger,
    };

    fn id(raw: &str) -> InstrumentId {
        InstrumentId::try_new(raw).expect("id")
    }

    fn src(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
        InstrumentSource::new(
            market,
            Symbol::try_new(symbol).expect("symbol"),
            IsinCode::try_new(isin).expect("isin"),
        )
    }

    fn rec(
        source: InstrumentSource,
        instrument_id: InstrumentId,
        status: InstrumentMappingStatus,
    ) -> InstrumentMappingRecord {
        InstrumentMappingRecord::new(source, instrument_id, status)
    }

    fn dart(raw: &str) -> DisclosureKey {
        DisclosureKey::from_receipt(ReceiptNo::Dart(DartReceiptNo::try_new(raw).expect("dart")))
    }

    fn note(text: &str) -> UserReason {
        UserReason::try_new(UntrustedText::try_new(text).expect("text")).expect("note")
    }

    fn ts(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(32400),
        )
        .expect("ts")
    }

    fn action_date() -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(24),
        )
        .expect("date")
    }

    fn symbol_change(inst: &InstrumentId, from: &str, to: &str) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(inst.clone()),
            action_date(),
            CorporateActionKind::SymbolChange {
                from: Symbol::try_new(from).expect("from"),
                to: Symbol::try_new(to).expect("to"),
            },
        )
        .expect("symbol change")
    }

    fn split(inst: &InstrumentId, old: u64, new: u64) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(inst.clone()),
            action_date(),
            CorporateActionKind::Split(ShareRatio::new(
                ShareCount::try_new(old).expect("old"),
                ShareCount::try_new(new).expect("new"),
            )),
        )
        .expect("split")
    }

    fn reverse_split(inst: &InstrumentId, old: u64, new: u64) -> CorporateAction {
        CorporateAction::try_new(
            CorporateActionId::new(inst.clone()),
            action_date(),
            CorporateActionKind::ReverseSplit(ShareRatio::new(
                ShareCount::try_new(old).expect("old"),
                ShareCount::try_new(new).expect("new"),
            )),
        )
        .expect("reverse split")
    }

    fn id_changed(
        inst: &InstrumentId,
        previous: InstrumentSource,
        next: InstrumentSource,
        action: CorporateAction,
        seconds: i64,
    ) -> MappingHistoryEvent {
        MappingHistoryEvent::IdentifierChanged {
            instrument_id: inst.clone(),
            previous,
            next,
            disclosure: dart("20261124000001"),
            action: Box::new(action),
            note: note("식별자 변경"),
            confirmed_at: ts(seconds),
        }
    }

    fn retire_corrected(
        inst: &InstrumentId,
        source: InstrumentSource,
        seconds: i64,
    ) -> MappingHistoryEvent {
        MappingHistoryEvent::RetirementCorrected {
            instrument_id: inst.clone(),
            source,
            disclosure: dart("20261124000005"),
            note: note("폐지 정정"),
            corrected_at: ts(seconds),
        }
    }

    fn link_corrected(
        inst: &InstrumentId,
        restored_previous: InstrumentSource,
        unlinked_next: InstrumentSource,
        seconds: i64,
    ) -> MappingHistoryEvent {
        MappingHistoryEvent::LinkCorrected {
            instrument_id: inst.clone(),
            restored_previous,
            unlinked_next,
            disclosure: dart("20261124000003"),
            note: note("연결 정정"),
            corrected_at: ts(seconds),
        }
    }

    fn receipt_tx() -> Transaction {
        let zero_krw =
            Money::parse(&NumericInput::try_new("0").expect("0"), Currency::Krw).expect("0");
        Transaction::try_new(TransactionData {
            kind: TransactionKind::Buy,
            market: Market::Kr,
            currency: Currency::Krw,
            symbol: Some(Symbol::try_new("005930").expect("sym")),
            asset_class: None,
            isin: Some(IsinCode::try_new("KR7005930003").expect("isin")),
            quantity: None,
            filled_price: None,
            amount: None,
            commission: zero_krw,
            tax: zero_krw,
            net_amount: None,
            exchange_rate: None,
            settlement_date: action_date(),
            settlement: Settlement::Settled,
            transaction_time: None,
            cancellation: Cancellation::Regular,
        })
        .expect("tx")
    }

    #[test]
    fn disclosure_key_from_receipt_preserves_source_and_receipt_number() {
        let dart_receipt = ReceiptNo::Dart(DartReceiptNo::try_new("20261124000001").expect("dart"));
        let kind_receipt = ReceiptNo::Kind(KindReceiptNo::try_new("20261124000002").expect("kind"));
        let sec_receipt =
            ReceiptNo::SecEdgar(SecEdgarReceiptNo::try_new("0000320193-26-000001").expect("sec"));

        let dart_key = DisclosureKey::from_receipt(dart_receipt.clone());
        let kind_key = DisclosureKey::from_receipt(kind_receipt.clone());
        let sec_key = DisclosureKey::from_receipt(sec_receipt.clone());

        assert_eq!(dart_key.source(), DisclosureSource::Dart);
        assert_eq!(dart_key.receipt_no(), &dart_receipt);
        assert_eq!(kind_key.source(), DisclosureSource::Kind);
        assert_eq!(kind_key.receipt_no(), &kind_receipt);
        assert_eq!(sec_key.source(), DisclosureSource::SecEdgar);
        assert_eq!(sec_key.receipt_no(), &sec_receipt);
    }

    #[test]
    fn verify_accepts_empty_and_complex_multi_event_histories_including_non_monotonic_timestamps() {
        assert_eq!(verify_persisted_mapping_ledger(&[], &[]), Ok(()));

        let id_a = id("inst-0000000000000001");
        let id_b = id("inst-0000000000000002");
        let id_c = id("inst-0000000000000003");
        let a1 = src(KnownMarket::Us, "FB", "US30303M1027");
        let a2 = src(KnownMarket::Us, "META", "US30303M1027");
        let a_mistake = src(KnownMarket::Us, "MIST", "US30303M9999");
        let a3 = src(KnownMarket::Us, "META2", "US30303M2222");
        let c1 = src(KnownMarket::Kr, "005930", "KR7005930003");

        let events = vec![
            id_changed(
                &id_a,
                a1.clone(),
                a2.clone(),
                symbol_change(&id_a, "FB", "META"),
                2_000,
            ),
            id_changed(
                &id_a,
                a2.clone(),
                a_mistake.clone(),
                split(&id_a, 1, 2),
                1_500,
            ),
            link_corrected(&id_a, a2.clone(), a_mistake.clone(), 1_000),
            id_changed(
                &id_a,
                a2.clone(),
                a3.clone(),
                reverse_split(&id_a, 5, 1),
                900,
            ),
            retire_corrected(&id_a, a3.clone(), 800),
            retire_corrected(&id_b, a_mistake.clone(), 700),
        ];

        let records = vec![
            rec(a3, id_a.clone(), InstrumentMappingStatus::Retired),
            rec(a1, id_a.clone(), InstrumentMappingStatus::Superseded),
            rec(a2, id_a, InstrumentMappingStatus::Superseded),
            rec(a_mistake, id_b, InstrumentMappingStatus::Active),
            rec(c1, id_c, InstrumentMappingStatus::Retired),
        ];

        assert_eq!(verify_persisted_mapping_ledger(&records, &events), Ok(()));
    }

    #[test]
    fn verify_rejects_duplicate_and_overlapping_stored_records() {
        let id_a = id("inst-0000000000000001");
        let id_b = id("inst-0000000000000002");
        let kr1 = src(KnownMarket::Kr, "005930", "KR7005930003");
        let kr1_case = src(KnownMarket::Kr, "005930", "kr7005930003");
        let kr2 = src(KnownMarket::Kr, "005930", "KR7005930999");
        let kr3 = src(KnownMarket::Kr, "005931", "KR7005930003");

        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(kr1.clone(), id_a.clone(), InstrumentMappingStatus::Active),
                    rec(kr1.clone(), id_a.clone(), InstrumentMappingStatus::Active),
                ],
                &[]
            ),
            Err(MappingLedgerIntegrityError::DuplicateRecord)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(kr1.clone(), id_a.clone(), InstrumentMappingStatus::Active),
                    rec(kr1.clone(), id_b.clone(), InstrumentMappingStatus::Active),
                ],
                &[]
            ),
            Err(MappingLedgerIntegrityError::DuplicateRecord)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(kr1.clone(), id_a.clone(), InstrumentMappingStatus::Active),
                    rec(kr1_case, id_a.clone(), InstrumentMappingStatus::Superseded),
                ],
                &[]
            ),
            Err(MappingLedgerIntegrityError::DuplicateRecord)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(kr1.clone(), id_a.clone(), InstrumentMappingStatus::Active),
                    rec(kr2, id_b.clone(), InstrumentMappingStatus::Active),
                ],
                &[]
            ),
            Err(MappingLedgerIntegrityError::OverlappingSources)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(kr1, id_a, InstrumentMappingStatus::Active),
                    rec(kr3, id_b, InstrumentMappingStatus::Active),
                ],
                &[]
            ),
            Err(MappingLedgerIntegrityError::OverlappingSources)
        );
    }

    #[test]
    fn verify_rejects_terminal_and_market_violations_in_stored_records() {
        let id_a = id("inst-0000000000000001");
        let kr1 = src(KnownMarket::Kr, "005930", "KR7005930003");
        let kr2 = src(KnownMarket::Kr, "005930", "KR7005930999");
        let us1 = src(KnownMarket::Us, "AAPL", "US0378331005");

        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(
                        kr1.clone(),
                        id_a.clone(),
                        InstrumentMappingStatus::Superseded
                    ),
                    rec(us1, id_a.clone(), InstrumentMappingStatus::Active),
                ],
                &[]
            ),
            Err(MappingLedgerIntegrityError::LedgerStateMismatch)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[rec(
                    kr1.clone(),
                    id_a.clone(),
                    InstrumentMappingStatus::Superseded
                )],
                &[]
            ),
            Err(MappingLedgerIntegrityError::OrphanSupersededRecord)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(kr1.clone(), id_a.clone(), InstrumentMappingStatus::Active),
                    rec(kr2.clone(), id_a.clone(), InstrumentMappingStatus::Retired),
                ],
                &[]
            ),
            Err(MappingLedgerIntegrityError::MultipleTerminalRecordsForId)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(kr1, id_a.clone(), InstrumentMappingStatus::Superseded),
                    rec(kr2, id_a, InstrumentMappingStatus::Active),
                ],
                &[]
            ),
            Err(MappingLedgerIntegrityError::OrphanSupersededRecord)
        );
    }

    #[test]
    fn verify_rejects_invalid_event_evidence_and_attached_receipt() {
        let id_a = id("inst-0000000000000001");
        let id_b = id("inst-0000000000000002");
        let a1 = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a1_case = src(KnownMarket::Kr, "005930", "kr7005930003");
        let a2 = src(KnownMarket::Kr, "005931", "KR7005930003");
        let us1 = src(KnownMarket::Us, "AAPL", "US0378331005");
        let records = [
            rec(
                a1.clone(),
                id_a.clone(),
                InstrumentMappingStatus::Superseded,
            ),
            rec(a2.clone(), id_a.clone(), InstrumentMappingStatus::Active),
        ];
        let delisting = CorporateAction::try_new(
            CorporateActionId::new(id_a.clone()),
            action_date(),
            CorporateActionKind::Delisting,
        )
        .expect("상장폐지");
        let with_receipt = split(&id_a, 1, 2)
            .with_receipt(receipt_tx())
            .expect("첨부 거래");

        let bad_events = [
            id_changed(&id_a, a1.clone(), a1_case, split(&id_a, 1, 2), 1_000),
            id_changed(&id_a, a1.clone(), us1, split(&id_a, 1, 2), 1_000),
            id_changed(
                &id_a,
                a1.clone(),
                a2.clone(),
                symbol_change(&id_b, "005930", "005931"),
                1_000,
            ),
            id_changed(&id_a, a1.clone(), a2.clone(), delisting, 1_000),
            id_changed(&id_a, a1.clone(), a2, with_receipt, 1_000),
            link_corrected(&id_a, a1.clone(), a1.clone(), 1_000),
            link_corrected(
                &id_a,
                a1,
                src(KnownMarket::Kr, "005930", "kr7005930003"),
                1_000,
            ),
        ];

        for bad_event in bad_events {
            assert_eq!(
                verify_persisted_mapping_ledger(&records, &[bad_event]),
                Err(MappingLedgerIntegrityError::InvalidEventEvidence)
            );
        }
    }

    #[test]
    fn verify_rejects_unlinked_and_branching_transitions_within_same_id() {
        let id_a = id("inst-0000000000000001");
        let a1 = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a2 = src(KnownMarket::Kr, "005931", "KR7005930003");
        let a3 = src(KnownMarket::Kr, "005932", "KR7005930003");
        let a1_case = src(KnownMarket::Kr, "005930", "kr7005930003");
        let link_1_2 = id_changed(
            &id_a,
            a1.clone(),
            a2.clone(),
            symbol_change(&id_a, "005930", "005931"),
            1_000,
        );
        let valid_pair = [
            rec(
                a1.clone(),
                id_a.clone(),
                InstrumentMappingStatus::Superseded,
            ),
            rec(a2.clone(), id_a.clone(), InstrumentMappingStatus::Active),
        ];

        assert_eq!(
            verify_persisted_mapping_ledger(
                &[rec(
                    a1.clone(),
                    id_a.clone(),
                    InstrumentMappingStatus::Active
                )],
                &[link_corrected(&id_a, a1.clone(), a2.clone(), 1_000)],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
        let branch = id_changed(
            &id_a,
            a1,
            a3,
            symbol_change(&id_a, "005930", "005932"),
            2_000,
        );
        assert_eq!(
            verify_persisted_mapping_ledger(&valid_pair, &[link_1_2.clone(), branch]),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
        let cycle = id_changed(
            &id_a,
            a2,
            a1_case,
            symbol_change(&id_a, "005931", "005930"),
            2_000,
        );
        assert_eq!(
            verify_persisted_mapping_ledger(&valid_pair, &[link_1_2, cycle]),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
    }

    #[test]
    fn verify_rejects_stale_retirement_and_empty_superseded_link_correction() {
        let id_a = id("inst-0000000000000001");
        let a1 = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a2 = src(KnownMarket::Kr, "005931", "KR7005930003");
        let link_1_2 = id_changed(
            &id_a,
            a1.clone(),
            a2.clone(),
            symbol_change(&id_a, "005930", "005931"),
            1_000,
        );
        let valid_pair = [
            rec(
                a1.clone(),
                id_a.clone(),
                InstrumentMappingStatus::Superseded,
            ),
            rec(a2.clone(), id_a.clone(), InstrumentMappingStatus::Active),
        ];

        assert_eq!(
            verify_persisted_mapping_ledger(
                &valid_pair,
                &[link_1_2, retire_corrected(&id_a, a1.clone(), 2_000)],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[rec(
                    a1.clone(),
                    id_a.clone(),
                    InstrumentMappingStatus::Active
                )],
                &[
                    retire_corrected(&id_a, a1.clone(), 1_000),
                    link_corrected(&id_a, a2, a1, 2_000),
                ],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
    }

    #[test]
    fn verify_rejects_non_latest_and_mismatched_previous_link_correction() {
        let id_a = id("inst-0000000000000001");
        let a1 = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a2 = src(KnownMarket::Kr, "005931", "KR7005930003");
        let a3 = src(KnownMarket::Kr, "005932", "KR7005930003");
        let link_1_2 = id_changed(
            &id_a,
            a1.clone(),
            a2.clone(),
            symbol_change(&id_a, "005930", "005931"),
            1_000,
        );
        let link_2_3 = id_changed(
            &id_a,
            a2.clone(),
            a3.clone(),
            symbol_change(&id_a, "005931", "005932"),
            2_000,
        );
        let valid_pair = [
            rec(
                a1.clone(),
                id_a.clone(),
                InstrumentMappingStatus::Superseded,
            ),
            rec(a2.clone(), id_a.clone(), InstrumentMappingStatus::Active),
        ];

        assert_eq!(
            verify_persisted_mapping_ledger(
                &valid_pair,
                &[
                    link_1_2.clone(),
                    link_2_3.clone(),
                    link_corrected(&id_a, a1.clone(), a3, 3_000),
                ],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &valid_pair,
                &[link_1_2, link_2_3, link_corrected(&id_a, a1, a2, 3_000)],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
    }

    #[test]
    fn verify_rejects_cross_id_replay_conflicts() {
        let id_a = id("inst-0000000000000001");
        let id_b = id("inst-0000000000000002");
        let a1 = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a2 = src(KnownMarket::Kr, "005931", "KR7005930003");
        let b1 = src(KnownMarket::Kr, "000660", "KR7000660001");
        let b_sym = src(KnownMarket::Kr, "005930", "KR7000660888");
        let b_isin = src(KnownMarket::Kr, "000661", "KR7005930003");
        let link_1_2 = id_changed(
            &id_a,
            a1.clone(),
            a2.clone(),
            symbol_change(&id_a, "005930", "005931"),
            1_000,
        );
        let three_records = [
            rec(a1, id_a.clone(), InstrumentMappingStatus::Superseded),
            rec(a2.clone(), id_a, InstrumentMappingStatus::Active),
            rec(b1.clone(), id_b.clone(), InstrumentMappingStatus::Active),
        ];

        assert_eq!(
            verify_persisted_mapping_ledger(
                &three_records,
                &[link_1_2.clone(), retire_corrected(&id_b, a2, 2_000)],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &three_records,
                &[
                    link_1_2.clone(),
                    id_changed(&id_b, b1.clone(), b_sym.clone(), split(&id_b, 1, 2), 2_000),
                    link_corrected(&id_b, b1.clone(), b_sym, 3_000),
                ],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &three_records,
                &[
                    link_1_2,
                    id_changed(&id_b, b1.clone(), b_isin.clone(), split(&id_b, 1, 2), 2_000),
                    link_corrected(&id_b, b1, b_isin, 3_000),
                ],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
    }

    #[test]
    fn verify_rejects_cross_id_previous_and_retirement_index_conflicts() {
        let id_a = id("inst-0000000000000001");
        let id_b = id("inst-0000000000000002");
        let a1 = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a2 = src(KnownMarket::Kr, "005931", "KR7005930003");
        let b1 = src(KnownMarket::Kr, "000660", "KR7000660001");
        let b_sym = src(KnownMarket::Kr, "005930", "KR7000660888");
        let link_1_2 = id_changed(
            &id_a,
            a1.clone(),
            a2.clone(),
            symbol_change(&id_a, "005930", "005931"),
            1_000,
        );

        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(
                        a1.clone(),
                        id_a.clone(),
                        InstrumentMappingStatus::Superseded
                    ),
                    rec(a2, id_a.clone(), InstrumentMappingStatus::Active),
                    rec(b1.clone(), id_b.clone(), InstrumentMappingStatus::Active),
                ],
                &[
                    link_1_2,
                    id_changed(&id_b, b_sym.clone(), b1.clone(), split(&id_b, 1, 2), 2_000),
                ],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(a1.clone(), id_a.clone(), InstrumentMappingStatus::Active),
                    rec(b1.clone(), id_b.clone(), InstrumentMappingStatus::Active),
                ],
                &[
                    retire_corrected(&id_a, a1, 1_000),
                    id_changed(&id_b, b1.clone(), b_sym.clone(), split(&id_b, 1, 2), 2_000),
                    link_corrected(&id_b, b1, b_sym, 3_000),
                ],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
    }

    #[test]
    fn verify_rejects_replayed_ledger_state_mismatches() {
        let id_a = id("inst-0000000000000001");
        let a1 = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a2 = src(KnownMarket::Kr, "005931", "KR7005930003");
        let a3 = src(KnownMarket::Kr, "005932", "KR7005930003");
        let link_1_2 = id_changed(
            &id_a,
            a1.clone(),
            a2.clone(),
            symbol_change(&id_a, "005930", "005931"),
            1_000,
        );

        assert_eq!(
            verify_persisted_mapping_ledger(&[], std::slice::from_ref(&link_1_2)),
            Err(MappingLedgerIntegrityError::LedgerStateMismatch)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[rec(
                    a2.clone(),
                    id_a.clone(),
                    InstrumentMappingStatus::Active
                )],
                std::slice::from_ref(&link_1_2),
            ),
            Err(MappingLedgerIntegrityError::LedgerStateMismatch)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(
                        a2.clone(),
                        id_a.clone(),
                        InstrumentMappingStatus::Superseded
                    ),
                    rec(a1, id_a.clone(), InstrumentMappingStatus::Active),
                ],
                std::slice::from_ref(&link_1_2),
            ),
            Err(MappingLedgerIntegrityError::LedgerStateMismatch)
        );
        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(a3, id_a.clone(), InstrumentMappingStatus::Superseded),
                    rec(a2, id_a, InstrumentMappingStatus::Active),
                ],
                &[link_1_2],
            ),
            Err(MappingLedgerIntegrityError::LedgerStateMismatch)
        );
    }

    #[test]
    fn verify_frees_unlinked_isin_for_other_id() {
        let id_a = id("inst-0000000000000001");
        let id_b = id("inst-0000000000000002");
        let a_base = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a_new_isin = src(KnownMarket::Kr, "005930", "KR7005930999");
        let b_free_isin = src(KnownMarket::Kr, "000660", "KR7005930999");

        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(
                        a_base.clone(),
                        id_a.clone(),
                        InstrumentMappingStatus::Active
                    ),
                    rec(
                        b_free_isin.clone(),
                        id_b.clone(),
                        InstrumentMappingStatus::Active
                    ),
                ],
                &[
                    id_changed(
                        &id_a,
                        a_base.clone(),
                        a_new_isin.clone(),
                        split(&id_a, 1, 2),
                        1_000
                    ),
                    link_corrected(&id_a, a_base, a_new_isin, 2_000),
                    retire_corrected(&id_b, b_free_isin, 3_000),
                ],
            ),
            Ok(())
        );
    }

    #[test]
    fn verify_keeps_shared_symbol_owner_across_link_correction() {
        let id_a = id("inst-0000000000000001");
        let id_b = id("inst-0000000000000002");
        let a_base = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a_new_isin = src(KnownMarket::Kr, "005930", "KR7005930999");
        let b_clean = src(KnownMarket::Kr, "000660", "KR7000660001");
        let b_steal_sym = src(KnownMarket::Kr, "005930", "KR7005930888");

        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(
                        a_base.clone(),
                        id_a.clone(),
                        InstrumentMappingStatus::Active
                    ),
                    rec(
                        b_clean.clone(),
                        id_b.clone(),
                        InstrumentMappingStatus::Active
                    ),
                ],
                &[
                    id_changed(
                        &id_a,
                        a_base.clone(),
                        a_new_isin.clone(),
                        split(&id_a, 1, 2),
                        1_000
                    ),
                    link_corrected(&id_a, a_base, a_new_isin, 2_000),
                    id_changed(
                        &id_b,
                        b_clean.clone(),
                        b_steal_sym.clone(),
                        split(&id_b, 1, 2),
                        3_000
                    ),
                    link_corrected(&id_b, b_clean, b_steal_sym, 4_000),
                ],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
    }

    #[test]
    fn verify_keeps_shared_isin_owner_across_link_correction() {
        let id_a = id("inst-0000000000000001");
        let id_b = id("inst-0000000000000002");
        let a_base = src(KnownMarket::Kr, "005930", "KR7005930003");
        let a_new_sym = src(KnownMarket::Kr, "005931", "KR7005930003");
        let b_clean = src(KnownMarket::Kr, "000660", "KR7000660001");
        let b_steal_isin = src(KnownMarket::Kr, "000661", "KR7005930003");

        assert_eq!(
            verify_persisted_mapping_ledger(
                &[
                    rec(
                        a_base.clone(),
                        id_a.clone(),
                        InstrumentMappingStatus::Active
                    ),
                    rec(
                        b_clean.clone(),
                        id_b.clone(),
                        InstrumentMappingStatus::Active
                    ),
                ],
                &[
                    id_changed(
                        &id_a,
                        a_base.clone(),
                        a_new_sym.clone(),
                        symbol_change(&id_a, "005930", "005931"),
                        1_000
                    ),
                    link_corrected(&id_a, a_base, a_new_sym, 2_000),
                    id_changed(
                        &id_b,
                        b_clean.clone(),
                        b_steal_isin.clone(),
                        split(&id_b, 1, 2),
                        3_000
                    ),
                    link_corrected(&id_b, b_clean, b_steal_isin, 4_000),
                ],
            ),
            Err(MappingLedgerIntegrityError::EventReplayMismatch)
        );
    }
}
