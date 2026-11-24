use super::{
    ConfirmedIdentifierChange, ConfirmedLinkCorrection, ConfirmedRetirementCorrection,
    IdentityChangeConfirmation, IdentityChangeEvidence, IdentityConfirmationOrigin,
    MappingCorrectionConfirmation, MappingCorrectionEvidence, MappingTransitionError,
    UserIdentityConfirmation, UserIdentityConfirmationSeal,
};
use crate::{
    CalendarDate, CalendarYear, Cancellation, CorporateAction, CorporateActionId,
    CorporateActionKind, Currency, DartReceiptNo, DayNumber, DisclosureKey,
    IndependentInstrumentIdIssuer, InstrumentId, InstrumentMappingRecord, InstrumentMappingStatus,
    InstrumentMappings, InstrumentSource, IsinCode, KnownMarket, MappingHistoryEvent, Market,
    Money, MonthNumber, Nanosecond, NumericInput, OffsetSeconds, ReceiptNo, Settlement, ShareCount,
    ShareRatio, Symbol, Timestamp, Transaction, TransactionData, TransactionKind, UnixSeconds,
    UntrustedText, UserReason, verify_persisted_mapping_ledger,
};
use proptest::{
    prelude::{Strategy, prop},
    test_runner::{Config, TestRunner},
};

fn source(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
    InstrumentSource::new(
        market,
        Symbol::try_new(symbol).expect("심볼"),
        IsinCode::try_new(isin).expect("ISIN"),
    )
}

fn user_origin() -> IdentityConfirmationOrigin {
    IdentityConfirmationOrigin::UserConfirmed(UserIdentityConfirmation(
        UserIdentityConfirmationSeal,
    ))
}

fn disclosure(serial: u8) -> DisclosureKey {
    let raw = ["202611240000", &format!("{serial:02}")].concat();
    DisclosureKey::from_receipt(ReceiptNo::Dart(DartReceiptNo::try_new(&raw).expect("공시")))
}

fn reason(text: &str) -> UserReason {
    UserReason::try_new(UntrustedText::try_new(text).expect("텍스트")).expect("사유")
}

fn ts(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(32400),
    )
    .expect("시각")
}

fn action_date() -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(2026),
        MonthNumber::new(11),
        DayNumber::new(24),
    )
    .expect("일자")
}

fn receipt_transaction(symbol: &str, isin: &str) -> Transaction {
    let date = action_date();
    let zero_krw = Money::parse(&NumericInput::try_new("0").expect("0"), Currency::Krw).expect("0");
    Transaction::try_new(TransactionData {
        kind: TransactionKind::Buy,
        market: Market::Kr,
        currency: Currency::Krw,
        symbol: Some(Symbol::try_new(symbol).expect("거래 심볼")),
        asset_class: None,
        isin: Some(IsinCode::try_new(isin).expect("거래 ISIN")),
        quantity: None,
        filled_price: None,
        amount: None,
        commission: zero_krw,
        tax: zero_krw,
        net_amount: None,
        exchange_rate: None,
        settlement_date: date,
        settlement: Settlement::Settled,
        transaction_time: None,
        cancellation: Cancellation::Regular,
    })
    .expect("첨부 거래")
}

fn symbol_change_action(id: &InstrumentId, from: &str, to: &str) -> CorporateAction {
    CorporateAction::try_new(
        CorporateActionId::new(id.clone()),
        action_date(),
        CorporateActionKind::SymbolChange {
            from: Symbol::try_new(from).expect("이전 심볼"),
            to: Symbol::try_new(to).expect("다음 심볼"),
        },
    )
    .expect("심볼 변경 기업행위")
}

fn split_action(id: &InstrumentId) -> CorporateAction {
    CorporateAction::try_new(
        CorporateActionId::new(id.clone()),
        action_date(),
        CorporateActionKind::Split(ShareRatio::new(
            ShareCount::try_new(1).expect("구주"),
            ShareCount::try_new(2).expect("신주"),
        )),
    )
    .expect("액면분할 기업행위")
}

fn reverse_split_action(id: &InstrumentId) -> CorporateAction {
    CorporateAction::try_new(
        CorporateActionId::new(id.clone()),
        action_date(),
        CorporateActionKind::ReverseSplit(ShareRatio::new(
            ShareCount::try_new(5).expect("구주"),
            ShareCount::try_new(1).expect("신주"),
        )),
    )
    .expect("액면병합 기업행위")
}

#[test]
fn link_confirmed_change_rejects_actions_with_attached_receipt() {
    let mut issuer = IndependentInstrumentIdIssuer::new();
    let mut mappings = InstrumentMappings::default();
    let original = source(KnownMarket::Kr, "005930", "KR7005930003");
    let next = source(KnownMarket::Kr, "005931", "KR7005930999");
    let cand = issuer.issue().expect("발급");
    let id = cand.as_instrument_id().clone();
    mappings.ensure(&original, cand).expect("초기 등록");

    let actions = [
        symbol_change_action(&id, "005930", "005931"),
        split_action(&id),
        reverse_split_action(&id),
    ];

    for base_action in actions {
        let with_receipt = base_action
            .with_receipt(receipt_transaction("005930", "KR7005930003"))
            .expect("영수 첨부");
        let change = ConfirmedIdentifierChange::new(
            id.clone(),
            original.clone(),
            next.clone(),
            IdentityChangeConfirmation::new(
                user_origin(),
                IdentityChangeEvidence::DisclosureAndAction {
                    disclosure: disclosure(1),
                    action: with_receipt,
                    note: reason("첨부 거래 포함 행위는 영속 유실 방지를 위해 거부"),
                },
                ts(1_763_942_400),
            ),
        );
        assert_eq!(
            mappings.link_confirmed_change(change),
            Err(MappingTransitionError::InvalidEvidence)
        );
    }
    assert_eq!(mappings.resolve(&original), Ok(id.clone()));
    assert_eq!(mappings.history(), []);
    let current = mappings.for_id(&id).expect("현재 레코드");
    assert_eq!(
        verify_persisted_mapping_ledger(&current, mappings.history()),
        Ok(())
    );
}

#[derive(Debug, Clone, Copy)]
enum LinkActionKind {
    Split,
    Other,
}

#[derive(Debug, Clone, Copy)]
enum Op {
    Ensure {
        source: usize,
        id_slot: usize,
    },
    Retire {
        source: usize,
    },
    Link {
        id_slot: usize,
        prev: usize,
        next: usize,
        action_kind: LinkActionKind,
        timestamp: i64,
    },
    CorrectRetirement {
        id_slot: usize,
        source: usize,
        timestamp: i64,
    },
    CorrectLink {
        id_slot: usize,
        prev: usize,
        next: usize,
        timestamp: i64,
    },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop::strategy::Union::new(vec![
        (0_usize..8, 0_usize..3)
            .prop_map(|(source, id_slot)| Op::Ensure { source, id_slot })
            .boxed(),
        (0_usize..8)
            .prop_map(|source| Op::Retire { source })
            .boxed(),
        (
            0_usize..3,
            0_usize..8,
            0_usize..8,
            prop::bool::ANY,
            1_700_000_000_i64..1_800_000_000_i64,
        )
            .prop_map(|(id_slot, prev, next, use_split, timestamp)| Op::Link {
                id_slot,
                prev,
                next,
                action_kind: if use_split {
                    LinkActionKind::Split
                } else {
                    LinkActionKind::Other
                },
                timestamp,
            })
            .boxed(),
        (0_usize..3, 0_usize..8, 1_700_000_000_i64..1_800_000_000_i64)
            .prop_map(|(id_slot, source, timestamp)| Op::CorrectRetirement {
                id_slot,
                source,
                timestamp,
            })
            .boxed(),
        (
            0_usize..3,
            0_usize..8,
            0_usize..8,
            1_700_000_000_i64..1_800_000_000_i64,
        )
            .prop_map(|(id_slot, prev, next, timestamp)| Op::CorrectLink {
                id_slot,
                prev,
                next,
                timestamp,
            })
            .boxed(),
    ])
}

#[derive(Clone, Copy)]
struct LinkOp<'a> {
    id: &'a InstrumentId,
    prev: &'a InstrumentSource,
    next: &'a InstrumentSource,
    action_kind: LinkActionKind,
    serial: u8,
    timestamp: i64,
}

fn apply_link_op(mappings: &mut InstrumentMappings, input: LinkOp<'_>) {
    let action = match input.action_kind {
        LinkActionKind::Split => split_action(input.id),
        LinkActionKind::Other if input.prev.symbol() == input.next.symbol() => {
            reverse_split_action(input.id)
        }
        LinkActionKind::Other => symbol_change_action(
            input.id,
            input.prev.symbol().as_str(),
            input.next.symbol().as_str(),
        ),
    };
    let change = ConfirmedIdentifierChange::new(
        input.id.clone(),
        input.prev.clone(),
        input.next.clone(),
        IdentityChangeConfirmation::new(
            user_origin(),
            IdentityChangeEvidence::DisclosureAndAction {
                disclosure: disclosure(input.serial),
                action,
                note: reason("속성 테스트 확인 연결"),
            },
            ts(input.timestamp),
        ),
    );
    let _res = mappings.link_confirmed_change(change);
}

fn correction_confirmation(
    serial: u8,
    note_text: &str,
    timestamp: i64,
) -> MappingCorrectionConfirmation {
    MappingCorrectionConfirmation::new(
        user_origin(),
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: disclosure(serial),
            note: reason(note_text),
        },
        ts(timestamp),
    )
}

fn apply_op(
    mappings: &mut InstrumentMappings,
    sources: &[InstrumentSource; 8],
    ids: &[InstrumentId; 3],
    step: usize,
    op: Op,
) {
    let serial = u8::try_from(step.rem_euclid(90).saturating_add(1)).expect("순번");
    match op {
        Op::Ensure {
            source: source_idx,
            id_slot,
        } => {
            let mut local = IndependentInstrumentIdIssuer::new();
            for _ in 0..id_slot {
                let _skip = local.issue().expect("건너뛰기");
            }
            let cand = local.issue().expect("후보");
            let _res = mappings.ensure(sources.get(source_idx).expect("원본"), cand);
        }
        Op::Retire { source: source_idx } => {
            let _res = mappings.retire(sources.get(source_idx).expect("원본"));
        }
        Op::Link {
            id_slot,
            prev,
            next,
            action_kind,
            timestamp,
        } => apply_link_op(
            mappings,
            LinkOp {
                id: ids.get(id_slot).expect("종목 ID"),
                prev: sources.get(prev).expect("이전 원본"),
                next: sources.get(next).expect("다음 원본"),
                action_kind,
                serial,
                timestamp,
            },
        ),
        Op::CorrectRetirement {
            id_slot,
            source: source_idx,
            timestamp,
        } => {
            let correction = ConfirmedRetirementCorrection::new(
                ids.get(id_slot).expect("종목 ID").clone(),
                sources.get(source_idx).expect("원본").clone(),
                correction_confirmation(serial, "속성 테스트 폐지 정정", timestamp),
            );
            let _res = mappings.correct_retirement(correction);
        }
        Op::CorrectLink {
            id_slot,
            prev,
            next,
            timestamp,
        } => {
            let correction = ConfirmedLinkCorrection::new(
                ids.get(id_slot).expect("종목 ID").clone(),
                sources.get(prev).expect("이전 원본").clone(),
                sources.get(next).expect("다음 원본").clone(),
                correction_confirmation(serial, "속성 테스트 연결 정정", timestamp),
            );
            let _res = mappings.correct_link(correction);
        }
    }
}

fn assert_corrupted_state_rejected(
    records: &[InstrumentMappingRecord],
    history: &[MappingHistoryEvent],
) {
    if let Some(first) = records.first() {
        let mut duplicated = records.to_vec();
        duplicated.push(first.clone());
        assert!(verify_persisted_mapping_ledger(&duplicated, history).is_err());

        let mut flipped = records.to_vec();
        if let Some(terminal) = flipped.iter_mut().find(|entry| match entry.status() {
            InstrumentMappingStatus::Active | InstrumentMappingStatus::Retired => true,
            InstrumentMappingStatus::Superseded => false,
        }) {
            *terminal = InstrumentMappingRecord::new(
                terminal.source().clone(),
                terminal.id().clone(),
                InstrumentMappingStatus::Superseded,
            );
            assert!(verify_persisted_mapping_ledger(&flipped, history).is_err());
        }
    }
    if !history.is_empty()
        && let Some(pos) = records.iter().position(|entry| {
            history
                .iter()
                .any(|event| event.instrument_id() == entry.id())
        })
    {
        let dropped: Vec<_> = records
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx != pos)
            .map(|(_, entry)| entry.clone())
            .collect();
        assert!(verify_persisted_mapping_ledger(&dropped, history).is_err());
    }
}

fn verify_ops_sequence(ops: Vec<Op>) {
    let sources = [
        source(KnownMarket::Kr, "005930", "KR7005930003"),
        source(KnownMarket::Kr, "005930", "kr7005930003"),
        source(KnownMarket::Kr, "005931", "KR7005930003"),
        source(KnownMarket::Kr, "005930", "KR7005930999"),
        source(KnownMarket::Us, "fb.A", "us30303m1027"),
        source(KnownMarket::Us, "FB.a", "US30303M1027"),
        source(KnownMarket::Us, "meta", "US30303M1027"),
        source(KnownMarket::Us, "META", "us30303m9999"),
    ];
    let mut id_lookup = IndependentInstrumentIdIssuer::new();
    let ids = [
        id_lookup.issue().expect("id 0").as_instrument_id().clone(),
        id_lookup.issue().expect("id 1").as_instrument_id().clone(),
        id_lookup.issue().expect("id 2").as_instrument_id().clone(),
    ];
    let mut mappings = InstrumentMappings::default();

    for (step, op) in ops.into_iter().enumerate() {
        apply_op(&mut mappings, &sources, &ids, step, op);
        let mut all_records = Vec::new();
        for tracked_id in &ids {
            if let Ok(rows) = mappings.for_id(tracked_id) {
                all_records.extend(rows);
            }
        }
        assert_eq!(
            verify_persisted_mapping_ledger(&all_records, mappings.history()),
            Ok(())
        );
        assert_corrupted_state_rejected(&all_records, mappings.history());
    }
}

#[test]
fn every_valid_domain_state_passes_persisted_ledger_verification() {
    TestRunner::new(Config::with_cases(testkit::property_cases()))
        .run(&prop::collection::vec(op_strategy(), 1..40), |ops| {
            verify_ops_sequence(ops);
            Ok(())
        })
        .expect("도메인 원장 검증 속성");
}
