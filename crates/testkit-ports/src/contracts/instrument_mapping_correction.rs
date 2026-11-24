use crate::{
    IdentityConfirmationIssuanceKind, ScriptedIdentityConfirmation,
    ScriptedIdentityConfirmationIssuer, SteppedClock,
    contracts::instrument_mapping::ContractRuntime,
};
use application::{
    ConfirmInstrumentIdentity, IdentifierChangePrompt, IdentityConfirmationDecision,
    IdentityConfirmationOutcome, InstrumentMappingCorrectionRepository,
    InstrumentMappingHistoryRepository, InstrumentMappingRepository, LinkCorrectionPrompt,
    MappingStoreError, MappingTransitionStoreError, RetirementCorrectionPrompt,
};
use domain::{
    CalendarDate, CalendarYear, Cancellation, ConfirmedIdentifierChange, ConfirmedLinkCorrection,
    ConfirmedRetirementCorrection, CorporateAction, CorporateActionId, CorporateActionKind,
    Currency, DartReceiptNo, DayNumber, DisclosureKey, IdentityChangeConfirmation,
    IdentityChangeEvidence, IdentityConfirmationOrigin, IndependentInstrumentIdIssuer,
    InstrumentId, InstrumentMappingError, InstrumentMappingRecord, InstrumentMappingStatus,
    InstrumentSource, IsinCode, IssuedInstrumentId, KindReceiptNo, KnownMarket, MappingAssignment,
    MappingCorrectionConfirmation, MappingCorrectionEvidence, MappingHistoryEvent,
    MappingTransitionError, Market, Money, MonthNumber, Nanosecond, NumericInput, OffsetSeconds,
    ReceiptNo, SecEdgarReceiptNo, Settlement, ShareCount, ShareRatio, Symbol, Timestamp,
    Transaction, TransactionData, TransactionKind, UnixSeconds, UntrustedText, UserReason,
};
use std::sync::{Arc, Barrier};

fn parse_source(market: KnownMarket, symbol: &str, isin: &str) -> Option<InstrumentSource> {
    let parsed_symbol = Symbol::try_new(symbol);
    assert!(parsed_symbol.is_ok(), "계약 심볼이 유효해야 함");
    let parsed_isin = IsinCode::try_new(isin);
    assert!(parsed_isin.is_ok(), "계약 ISIN이 유효해야 함");
    match (parsed_symbol, parsed_isin) {
        (Ok(sym), Ok(code)) => Some(InstrumentSource::new(market, sym, code)),
        (Ok(_) | Err(_), Ok(_) | Err(_)) => None,
    }
}

fn issue_slot(slot: u16) -> Option<IssuedInstrumentId> {
    let mut generator = IndependentInstrumentIdIssuer::new();
    for _ in 0..slot {
        let skipped = generator.issue();
        assert!(skipped.is_ok(), "발급원 건너뛰기 유효");
        if skipped.is_err() {
            return None;
        }
    }
    let candidate = generator.issue();
    assert!(candidate.is_ok(), "발급원 발급 유효");
    candidate.ok()
}

fn parse_reason(text: &str) -> Option<UserReason> {
    let untrusted = UntrustedText::try_new(text);
    assert!(untrusted.is_ok(), "사유 텍스트 유효");
    let reason = untrusted
        .ok()
        .and_then(|value| UserReason::try_new(value).ok());
    assert!(reason.is_some(), "사유 유효");
    reason
}

fn parse_ts(seconds: i64, nanos: u32, offset: i32) -> Option<Timestamp> {
    let stamp = Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(offset),
    );
    assert!(stamp.is_ok(), "시각 유효");
    stamp.ok()
}

fn parse_date(year: i32, month: u8, day: u8) -> Option<CalendarDate> {
    let cal = CalendarDate::from_parts(
        CalendarYear::new(year),
        MonthNumber::new(month),
        DayNumber::new(day),
    );
    assert!(cal.is_ok(), "일자 유효");
    cal.ok()
}

fn dart_disclosure(raw: &str) -> Option<DisclosureKey> {
    let receipt = DartReceiptNo::try_new(raw);
    assert!(receipt.is_ok(), "DART 접수번호 유효");
    receipt
        .ok()
        .map(|value| DisclosureKey::from_receipt(ReceiptNo::Dart(value)))
}

fn kind_disclosure(raw: &str) -> Option<DisclosureKey> {
    let receipt = KindReceiptNo::try_new(raw);
    assert!(receipt.is_ok(), "KIND 접수번호 유효");
    receipt
        .ok()
        .map(|value| DisclosureKey::from_receipt(ReceiptNo::Kind(value)))
}

fn sec_disclosure(raw: &str) -> Option<DisclosureKey> {
    let receipt = SecEdgarReceiptNo::try_new(raw);
    assert!(receipt.is_ok(), "SEC 접수번호 유효");
    receipt
        .ok()
        .map(|value| DisclosureKey::from_receipt(ReceiptNo::SecEdgar(value)))
}

fn assert_ok<T, E>(res: Result<T, E>, label: &str) -> Option<T> {
    assert!(res.is_ok(), "{label}");
    res.ok()
}

fn symbol_action(
    id: &InstrumentId,
    cal: CalendarDate,
    from: &str,
    to: &str,
) -> Option<CorporateAction> {
    let from_sym = assert_ok(Symbol::try_new(from), "이전 심볼 유효")?;
    let to_sym = assert_ok(Symbol::try_new(to), "이후 심볼 유효")?;
    assert_ok(
        CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            cal,
            CorporateActionKind::SymbolChange {
                from: from_sym,
                to: to_sym,
            },
        ),
        "심볼 변경 기업행위 유효",
    )
}

fn split_action(
    id: &InstrumentId,
    cal: CalendarDate,
    old_shares: u64,
    new_shares: u64,
) -> Option<CorporateAction> {
    let ratio = ShareRatio::new(
        assert_ok(ShareCount::try_new(old_shares), "분할 이전 주수 유효")?,
        assert_ok(ShareCount::try_new(new_shares), "분할 이후 주수 유효")?,
    );
    assert_ok(
        CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            cal,
            CorporateActionKind::Split(ratio),
        ),
        "분할 기업행위 유효",
    )
}

fn reverse_split_action(
    id: &InstrumentId,
    cal: CalendarDate,
    old_shares: u64,
    new_shares: u64,
) -> Option<CorporateAction> {
    let ratio = ShareRatio::new(
        assert_ok(ShareCount::try_new(old_shares), "병합 이전 주수 유효")?,
        assert_ok(ShareCount::try_new(new_shares), "병합 이후 주수 유효")?,
    );
    assert_ok(
        CorporateAction::try_new(
            CorporateActionId::new(id.clone()),
            cal,
            CorporateActionKind::ReverseSplit(ratio),
        ),
        "병합 기업행위 유효",
    )
}

fn kind_action(
    id: &InstrumentId,
    cal: CalendarDate,
    kind: CorporateActionKind,
) -> Option<CorporateAction> {
    assert_ok(
        CorporateAction::try_new(CorporateActionId::new(id.clone()), cal, kind),
        "기업행위 유효",
    )
}

fn sample_receipt_transaction(cal: CalendarDate, symbol: &str, isin: &str) -> Option<Transaction> {
    let raw_zero = assert_ok(NumericInput::try_new("0"), "0 금액 입력 유효")?;
    let zero = assert_ok(Money::parse(&raw_zero, Currency::Krw), "0원 유효")?;
    let parsed_symbol = assert_ok(Symbol::try_new(symbol), "첨부 거래 심볼 유효")?;
    let parsed_isin = assert_ok(IsinCode::try_new(isin), "첨부 거래 ISIN 유효")?;
    assert_ok(
        Transaction::try_new(TransactionData {
            kind: TransactionKind::Buy,
            market: Market::Kr,
            currency: Currency::Krw,
            symbol: Some(parsed_symbol),
            asset_class: None,
            isin: Some(parsed_isin),
            quantity: None,
            filled_price: None,
            amount: None,
            commission: zero,
            tax: zero,
            net_amount: None,
            exchange_rate: None,
            settlement_date: cal,
            settlement: Settlement::Settled,
            transaction_time: None,
            cancellation: Cancellation::Regular,
        }),
        "첨부 거래 유효",
    )
}

#[derive(Debug, PartialEq, Eq)]
enum ConfirmationOutcomeKind {
    Confirmed,
    Denied,
    Unavailable,
}

const fn outcome_kind<T>(outcome: &IdentityConfirmationOutcome<T>) -> ConfirmationOutcomeKind {
    match outcome {
        IdentityConfirmationOutcome::Confirmed(_) => ConfirmationOutcomeKind::Confirmed,
        IdentityConfirmationOutcome::Denied => ConfirmationOutcomeKind::Denied,
        IdentityConfirmationOutcome::Unavailable => ConfirmationOutcomeKind::Unavailable,
    }
}

async fn confirm_change(
    id: &InstrumentId,
    previous: &InstrumentSource,
    next: &InstrumentSource,
    evidence: IdentityChangeEvidence,
    stamp: Timestamp,
) -> Option<ConfirmedIdentifierChange> {
    let prompt =
        ScriptedIdentityConfirmation::new([IdentityConfirmationDecision::Confirmed], [], []);
    let clock = SteppedClock::new(stamp);
    let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);
    let req = IdentifierChangePrompt::new(id.clone(), previous.clone(), next.clone(), evidence);
    let outcome = use_case.confirm_identifier_change(req).await;
    assert_eq!(
        outcome_kind(&outcome),
        ConfirmationOutcomeKind::Confirmed,
        "확정 대본은 Confirmed를 반환해야 함"
    );
    match outcome {
        IdentityConfirmationOutcome::Confirmed(change) => Some(change),
        IdentityConfirmationOutcome::Denied | IdentityConfirmationOutcome::Unavailable => None,
    }
}

async fn confirm_retire_corr(
    id: &InstrumentId,
    source: &InstrumentSource,
    evidence: MappingCorrectionEvidence,
    stamp: Timestamp,
) -> Option<ConfirmedRetirementCorrection> {
    let prompt =
        ScriptedIdentityConfirmation::new([], [IdentityConfirmationDecision::Confirmed], []);
    let clock = SteppedClock::new(stamp);
    let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);
    let req = RetirementCorrectionPrompt::new(id.clone(), source.clone(), evidence);
    let outcome = use_case.confirm_retirement_correction(req).await;
    assert_eq!(
        outcome_kind(&outcome),
        ConfirmationOutcomeKind::Confirmed,
        "확정 대본은 Confirmed를 반환해야 함"
    );
    match outcome {
        IdentityConfirmationOutcome::Confirmed(corr) => Some(corr),
        IdentityConfirmationOutcome::Denied | IdentityConfirmationOutcome::Unavailable => None,
    }
}

async fn confirm_link_corr(
    id: &InstrumentId,
    previous: &InstrumentSource,
    mistaken_next: &InstrumentSource,
    evidence: MappingCorrectionEvidence,
    stamp: Timestamp,
) -> Option<ConfirmedLinkCorrection> {
    let prompt =
        ScriptedIdentityConfirmation::new([], [], [IdentityConfirmationDecision::Confirmed]);
    let clock = SteppedClock::new(stamp);
    let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock);
    let req = LinkCorrectionPrompt::new(
        id.clone(),
        previous.clone(),
        mistaken_next.clone(),
        evidence,
    );
    let outcome = use_case.confirm_link_correction(req).await;
    assert_eq!(
        outcome_kind(&outcome),
        ConfirmationOutcomeKind::Confirmed,
        "확정 대본은 Confirmed를 반환해야 함"
    );
    match outcome {
        IdentityConfirmationOutcome::Confirmed(corr) => Some(corr),
        IdentityConfirmationOutcome::Denied | IdentityConfirmationOutcome::Unavailable => None,
    }
}

async fn issue_user_origin(
    kind: IdentityConfirmationIssuanceKind,
) -> Option<IdentityConfirmationOrigin> {
    let issuer = ScriptedIdentityConfirmationIssuer::new([IdentityConfirmationDecision::Confirmed]);
    let outcome = issuer.issue(kind).await;
    assert_eq!(
        outcome_kind(&outcome),
        ConfirmationOutcomeKind::Confirmed,
        "확정 발급기는 Confirmed를 반환해야 함"
    );
    match outcome {
        IdentityConfirmationOutcome::Confirmed(seal) => {
            Some(IdentityConfirmationOrigin::UserConfirmed(seal))
        }
        IdentityConfirmationOutcome::Denied | IdentityConfirmationOutcome::Unavailable => None,
    }
}

const fn tr_err(error: MappingTransitionError) -> MappingTransitionStoreError {
    MappingTransitionStoreError::Transition(error)
}

const fn map_err(error: InstrumentMappingError) -> MappingStoreError {
    MappingStoreError::Mapping(error)
}

const fn market_rank(market: KnownMarket) -> u8 {
    match market {
        KnownMarket::Kr => 0,
        KnownMarket::Us => 1,
    }
}

const fn status_rank(status: InstrumentMappingStatus) -> u8 {
    match status {
        InstrumentMappingStatus::Active => 0,
        InstrumentMappingStatus::Superseded => 1,
        InstrumentMappingStatus::Retired => 2,
    }
}

fn sort_records(mut rows: Vec<InstrumentMappingRecord>) -> Vec<InstrumentMappingRecord> {
    rows.sort_by(|left, right| {
        (
            left.id().as_str(),
            market_rank(left.source().market()),
            left.source().symbol().as_str(),
            left.source().isin().as_str(),
            status_rank(left.status()),
        )
            .cmp(&(
                right.id().as_str(),
                market_rank(right.source().market()),
                right.source().symbol().as_str(),
                right.source().isin().as_str(),
                status_rank(right.status()),
            ))
    });
    rows
}

fn event_ts(event: &MappingHistoryEvent) -> Timestamp {
    match event {
        MappingHistoryEvent::IdentifierChanged { confirmed_at, .. } => *confirmed_at,
        MappingHistoryEvent::RetirementCorrected { corrected_at, .. }
        | MappingHistoryEvent::LinkCorrected { corrected_at, .. } => *corrected_at,
    }
}

fn assert_history_exact(
    actual: Result<Vec<MappingHistoryEvent>, application::SystemPortError>,
    expected: &[MappingHistoryEvent],
    context: &str,
) {
    assert_eq!(actual, Ok(expected.to_vec()), "{context}");
    if let Ok(got) = actual {
        for (actual_event, expected_event) in got.iter().zip(expected.iter()) {
            let got_ts = event_ts(actual_event);
            let want_ts = event_ts(expected_event);
            assert_eq!(
                got_ts.nanosecond(),
                want_ts.nanosecond(),
                "{context}: 나노초"
            );
            assert_eq!(
                got_ts.offset_seconds(),
                want_ts.offset_seconds(),
                "{context}: 오프셋"
            );
        }
    }
}

struct ContractWitness {
    us_id: InstrumentId,
    kr_id: InstrumentId,
    us_first: InstrumentSource,
    us_second: InstrumentSource,
    us_third: InstrumentSource,
    kr_first: InstrumentSource,
    kr_second: InstrumentSource,
    expected_history: Vec<MappingHistoryEvent>,
}

struct LinkStepSpec<'a> {
    id: &'a InstrumentId,
    previous: &'a InstrumentSource,
    next: &'a InstrumentSource,
    disclosure: DisclosureKey,
    action: CorporateAction,
    note: UserReason,
    stamp: Timestamp,
    label: &'a str,
}

async fn apply_and_record_link<P>(repo: &P, spec: LinkStepSpec<'_>) -> Option<MappingHistoryEvent>
where
    P: InstrumentMappingCorrectionRepository,
{
    let req = confirm_change(
        spec.id,
        spec.previous,
        spec.next,
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: spec.disclosure.clone(),
            action: spec.action.clone(),
            note: spec.note.clone(),
        },
        spec.stamp,
    )
    .await?;
    assert_eq!(
        repo.link_confirmed_change(req).await,
        Ok(spec.id.clone()),
        "{}",
        spec.label
    );
    Some(MappingHistoryEvent::IdentifierChanged {
        instrument_id: spec.id.clone(),
        previous: spec.previous.clone(),
        next: spec.next.clone(),
        disclosure: spec.disclosure,
        action: Box::new(spec.action),
        note: spec.note,
        confirmed_at: spec.stamp,
    })
}

struct RetireCorrSpec<'a> {
    id: &'a InstrumentId,
    target: &'a InstrumentSource,
    disclosure: DisclosureKey,
    note: UserReason,
    stamp: Timestamp,
}

async fn apply_and_record_retire_corr<P>(
    repo: &P,
    spec: RetireCorrSpec<'_>,
) -> Option<MappingHistoryEvent>
where
    P: InstrumentMappingCorrectionRepository,
{
    let req = confirm_retire_corr(
        spec.id,
        spec.target,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: spec.disclosure.clone(),
            note: spec.note.clone(),
        },
        spec.stamp,
    )
    .await?;
    assert_eq!(
        repo.correct_retirement(req).await,
        Ok(spec.id.clone()),
        "폐지 정정 성공"
    );
    Some(MappingHistoryEvent::RetirementCorrected {
        instrument_id: spec.id.clone(),
        source: spec.target.clone(),
        disclosure: spec.disclosure,
        note: spec.note,
        corrected_at: spec.stamp,
    })
}

struct LinkCorrSpec<'a> {
    id: &'a InstrumentId,
    previous: &'a InstrumentSource,
    mistaken_next: &'a InstrumentSource,
    disclosure: DisclosureKey,
    note: UserReason,
    stamp: Timestamp,
}

async fn apply_and_record_link_corr<P>(
    repo: &P,
    spec: LinkCorrSpec<'_>,
) -> Option<MappingHistoryEvent>
where
    P: InstrumentMappingCorrectionRepository,
{
    let req = confirm_link_corr(
        spec.id,
        spec.previous,
        spec.mistaken_next,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: spec.disclosure.clone(),
            note: spec.note.clone(),
        },
        spec.stamp,
    )
    .await?;
    assert_eq!(
        repo.correct_link(req).await,
        Ok(spec.id.clone()),
        "연결 정정 성공"
    );
    Some(MappingHistoryEvent::LinkCorrected {
        instrument_id: spec.id.clone(),
        restored_previous: spec.previous.clone(),
        unlinked_next: spec.mistaken_next.clone(),
        disclosure: spec.disclosure,
        note: spec.note,
        corrected_at: spec.stamp,
    })
}

async fn assert_seeded_chain_state<P>(repo: &P, witness: &ContractWitness) -> Option<()>
where
    P: InstrumentMappingRepository + InstrumentMappingHistoryRepository,
{
    assert_eq!(
        repo.resolve(&witness.us_first).await,
        Err(map_err(InstrumentMappingError::Superseded)),
        "승계된 1차 원본 resolve는 Superseded여야 함"
    );
    assert_eq!(
        repo.resolve(&witness.us_second).await,
        Err(map_err(InstrumentMappingError::Superseded)),
        "승계된 2차 원본 resolve는 Superseded여야 함"
    );
    assert_eq!(
        repo.resolve(&witness.us_third).await,
        Ok(witness.us_id.clone()),
        "활성 종단 resolve 성공"
    );
    assert_eq!(
        repo.ensure(&witness.us_first, issue_slot(10)?).await,
        Err(map_err(InstrumentMappingError::Superseded)),
        "승계 원본 ensure 거부"
    );
    assert_eq!(
        repo.retire(&witness.us_first).await,
        Err(map_err(InstrumentMappingError::Superseded)),
        "승계 원본 retire 거부"
    );
    assert_eq!(
        repo.for_id(&witness.us_id).await.map(sort_records),
        Ok(sort_records(vec![
            InstrumentMappingRecord::new(
                witness.us_first.clone(),
                witness.us_id.clone(),
                InstrumentMappingStatus::Superseded,
            ),
            InstrumentMappingRecord::new(
                witness.us_second.clone(),
                witness.us_id.clone(),
                InstrumentMappingStatus::Superseded,
            ),
            InstrumentMappingRecord::new(
                witness.us_third.clone(),
                witness.us_id.clone(),
                InstrumentMappingStatus::Active,
            ),
        ])),
        "US 2단계 체인 원장 행 보존"
    );
    assert_history_exact(
        repo.history().await,
        &witness.expected_history,
        "체인 연결 이력 순서",
    );
    Some(())
}

async fn seed_initial_pair<P>(repo: &P) -> Option<ContractWitness>
where
    P: InstrumentMappingRepository,
{
    let us_first = parse_source(KnownMarket::Us, "fb.A", "us30303m1027")?;
    let us_second = parse_source(KnownMarket::Us, "META", "us30303m1027")?;
    let us_third = parse_source(KnownMarket::Us, "fb.A", "US30303M9999")?;
    let kr_first = parse_source(KnownMarket::Kr, "005930", "KR7005930003")?;
    let kr_second = parse_source(KnownMarket::Kr, "005930", "KR7005930999")?;

    let us_cand = issue_slot(0)?;
    let us_id = us_cand.as_instrument_id().clone();
    let kr_cand = issue_slot(1)?;
    let kr_id = kr_cand.as_instrument_id().clone();

    assert_eq!(
        repo.ensure(&us_first, us_cand).await,
        Ok(MappingAssignment::Issued(us_id.clone())),
        "US 초기 등록"
    );
    assert_eq!(
        repo.ensure(&kr_first, kr_cand).await,
        Ok(MappingAssignment::Issued(kr_id.clone())),
        "KR 초기 등록"
    );
    Some(ContractWitness {
        us_id,
        kr_id,
        us_first,
        us_second,
        us_third,
        kr_first,
        kr_second,
        expected_history: Vec::new(),
    })
}

async fn seed_and_verify_link_chains<P>(repo: &P) -> Option<ContractWitness>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let mut witness = seed_initial_pair(repo).await?;
    let first_event = apply_and_record_link(
        repo,
        LinkStepSpec {
            id: &witness.us_id,
            previous: &witness.us_first,
            next: &witness.us_second,
            disclosure: sec_disclosure("0001326801-26-000101")?,
            action: symbol_action(&witness.us_id, parse_date(2026, 10, 28)?, "FB.A", "META")?,
            note: parse_reason("\0사명 변경 확인")?,
            stamp: parse_ts(1_764_000_200, 123_456_789, -18_000)?,
            label: "1단계 심볼 변경 연결 성공",
        },
    )
    .await?;
    let second_event = apply_and_record_link(
        repo,
        LinkStepSpec {
            id: &witness.kr_id,
            previous: &witness.kr_first,
            next: &witness.kr_second,
            disclosure: dart_disclosure("20261101000001")?,
            action: reverse_split_action(&witness.kr_id, parse_date(2026, 11, 1)?, u64::MAX, 1)?,
            note: parse_reason(&["가".repeat(170), "aa".to_owned()].concat())?,
            stamp: parse_ts(1_764_000_100, 999_999_999, 32_400)?,
            label: "KR 역분할 u64::MAX 및 비단조 시각 연결 성공",
        },
    )
    .await?;
    let third_event = apply_and_record_link(
        repo,
        LinkStepSpec {
            id: &witness.us_id,
            previous: &witness.us_second,
            next: &witness.us_third,
            disclosure: kind_disclosure("20261102000002")?,
            action: split_action(&witness.us_id, parse_date(2026, 11, 2)?, 1, 10)?,
            note: parse_reason("2단계 분할 및 동일 ID 구 심볼 재사용")?,
            stamp: parse_ts(1_764_000_150, 42, 0)?,
            label: "2단계 체인 연결 성공",
        },
    )
    .await?;

    witness.expected_history = vec![first_event, second_event, third_event];
    assert_seeded_chain_state(repo, &witness).await?;
    Some(witness)
}

async fn verify_link_origin_and_incomplete_evidence<P>(
    repo: &P,
    witness: &ContractWitness,
) -> Option<()>
where
    P: InstrumentMappingCorrectionRepository,
{
    let stamp = parse_ts(1_764_000_300, 0, 32_400)?;
    let cal = parse_date(2026, 11, 3)?;
    let disc = dart_disclosure("20261103000010")?;
    let note = parse_reason("거부 검증")?;
    let fresh_us = parse_source(KnownMarket::Us, "NEWU", "US9999999991")?;
    let valid_action = split_action(&witness.us_id, cal, 1, 2)?;
    let valid_ev = IdentityChangeEvidence::DisclosureAndAction {
        disclosure: disc.clone(),
        action: valid_action.clone(),
        note: note.clone(),
    };

    for bad_origin in [
        IdentityConfirmationOrigin::BrokerObservation,
        IdentityConfirmationOrigin::AiSuggestion,
    ] {
        let req = ConfirmedIdentifierChange::new(
            witness.us_id.clone(),
            witness.us_third.clone(),
            fresh_us.clone(),
            IdentityChangeConfirmation::new(bad_origin, valid_ev.clone(), stamp),
        );
        assert_eq!(
            repo.link_confirmed_change(req).await,
            Err(tr_err(MappingTransitionError::IdentityUncertain)),
            "비사용자 확인 출처는 거부되어야 함"
        );
    }

    for incomplete in [
        IdentityChangeEvidence::None,
        IdentityChangeEvidence::NoteOnly(note.clone()),
        IdentityChangeEvidence::DisclosureOnly {
            disclosure: disc,
            note: note.clone(),
        },
        IdentityChangeEvidence::ActionOnly {
            action: valid_action,
            note,
        },
    ] {
        let origin = issue_user_origin(IdentityConfirmationIssuanceKind::IdentifierChange).await?;
        let req = ConfirmedIdentifierChange::new(
            witness.us_id.clone(),
            witness.us_third.clone(),
            fresh_us.clone(),
            IdentityChangeConfirmation::new(origin, incomplete, stamp),
        );
        assert_eq!(
            repo.link_confirmed_change(req).await,
            Err(tr_err(MappingTransitionError::IdentityUncertain)),
            "불완전 증거는 UserConfirmed여도 거부되어야 함"
        );
    }
    Some(())
}

fn build_bad_actions(
    witness: &ContractWitness,
    cal: CalendarDate,
) -> Option<[(CorporateAction, MappingTransitionError); 7]> {
    let with_receipt = assert_ok(
        split_action(&witness.us_id, cal, 1, 2)?.with_receipt(sample_receipt_transaction(
            cal,
            "005930",
            "KR7005930003",
        )?),
        "첨부 거래 결합 유효",
    )?;
    Some([
        (
            split_action(&witness.kr_id, cal, 1, 2)?,
            MappingTransitionError::InvalidEvidence,
        ),
        (
            symbol_action(&witness.us_id, cal, "WRONG", "NEWU")?,
            MappingTransitionError::InvalidEvidence,
        ),
        (
            kind_action(&witness.us_id, cal, CorporateActionKind::Delisting)?,
            MappingTransitionError::InvalidEvidence,
        ),
        (
            kind_action(&witness.us_id, cal, CorporateActionKind::Demerger)?,
            MappingTransitionError::InvalidEvidence,
        ),
        (with_receipt, MappingTransitionError::InvalidEvidence),
        (
            kind_action(&witness.us_id, cal, CorporateActionKind::Merger)?,
            MappingTransitionError::IdentityUncertain,
        ),
        (
            kind_action(&witness.us_id, cal, CorporateActionKind::Exchange)?,
            MappingTransitionError::IdentityUncertain,
        ),
    ])
}

async fn verify_link_bad_actions<P>(repo: &P, witness: &ContractWitness) -> Option<()>
where
    P: InstrumentMappingHistoryRepository + InstrumentMappingCorrectionRepository,
{
    let stamp = parse_ts(1_764_000_305, 0, 32_400)?;
    let cal = parse_date(2026, 11, 3)?;
    let disc = dart_disclosure("20261103000010")?;
    let note = parse_reason("기업행위 거부 검증")?;
    let fresh_us = parse_source(KnownMarket::Us, "NEWU", "US9999999991")?;
    for (bad_act, expected_err) in build_bad_actions(witness, cal)? {
        let req = confirm_change(
            &witness.us_id,
            &witness.us_third,
            &fresh_us,
            IdentityChangeEvidence::DisclosureAndAction {
                disclosure: disc.clone(),
                action: bad_act,
                note: note.clone(),
            },
            stamp,
        )
        .await?;
        assert_eq!(
            repo.link_confirmed_change(req).await,
            Err(tr_err(expected_err)),
            "부적합 기업행위 증거 거부"
        );
    }
    assert_history_exact(
        repo.history().await,
        &witness.expected_history,
        "증거 거부 후 이력 무변경",
    );
    Some(())
}

type LinkConflictCase<'a> = (
    &'a InstrumentId,
    InstrumentSource,
    InstrumentSource,
    MappingTransitionError,
);

fn build_link_conflict_cases_first(witness: &ContractWitness) -> Option<[LinkConflictCase<'_>; 5]> {
    let us = &witness.us_id;
    let active = &witness.us_third;
    let next_tmp = parse_source(KnownMarket::Us, "NEXT", "US0000000098")?;
    Some([
        (
            us,
            active.clone(),
            parse_source(KnownMarket::Us, "FB.a", "us30303m9999")?,
            MappingTransitionError::UnchangedSource,
        ),
        (
            us,
            active.clone(),
            parse_source(KnownMarket::Kr, "005999", "KR7005999001")?,
            MappingTransitionError::MarketMismatch,
        ),
        (
            us,
            parse_source(KnownMarket::Us, "MISS", "US0000000099")?,
            next_tmp.clone(),
            MappingTransitionError::Missing,
        ),
        (
            us,
            parse_source(KnownMarket::Us, "FB.A", "US30303M9999")?,
            next_tmp,
            MappingTransitionError::IdentityUncertain,
        ),
        (
            us,
            witness.kr_second.clone(),
            parse_source(KnownMarket::Kr, "005998", "KR7005998001")?,
            MappingTransitionError::SourceBoundToOtherId,
        ),
    ])
}

fn build_link_conflict_cases_second(
    witness: &ContractWitness,
) -> Option<[LinkConflictCase<'_>; 5]> {
    let us = &witness.us_id;
    let active = &witness.us_third;
    Some([
        (
            us,
            witness.us_first.clone(),
            parse_source(KnownMarket::Us, "NEXT", "US0000000098")?,
            MappingTransitionError::SourceNotActive,
        ),
        (
            us,
            active.clone(),
            witness.us_first.clone(),
            MappingTransitionError::SourceAlreadyRecorded,
        ),
        (
            us,
            active.clone(),
            parse_source(KnownMarket::Us, "meta", "US30303M1027")?,
            MappingTransitionError::SourceAlreadyRecorded,
        ),
        (
            us,
            active.clone(),
            parse_source(KnownMarket::Us, "DIFF", "KR7005930003")?,
            MappingTransitionError::IdentityUncertain,
        ),
        (
            us,
            active.clone(),
            parse_source(KnownMarket::Us, "DIFF", "kr7005930999")?,
            MappingTransitionError::IdentityUncertain,
        ),
    ])
}

async fn verify_link_conflict_cases<P>(
    repo: &P,
    witness: &ContractWitness,
    ev: &IdentityChangeEvidence,
    stamp: Timestamp,
) -> Option<()>
where
    P: InstrumentMappingCorrectionRepository,
{
    let first = build_link_conflict_cases_first(witness)?;
    let second = build_link_conflict_cases_second(witness)?;
    for (target_id, prev, next, expected_err) in first.into_iter().chain(second) {
        let req = confirm_change(target_id, &prev, &next, ev.clone(), stamp).await?;
        assert_eq!(
            repo.link_confirmed_change(req).await,
            Err(tr_err(expected_err)),
            "연결 충돌 규칙 위반 거부"
        );
    }
    Some(())
}

async fn verify_link_conflict_rejections<P>(repo: &P, witness: &ContractWitness) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let stamp = parse_ts(1_764_000_310, 0, 32_400)?;
    let cal = parse_date(2026, 11, 3)?;
    let ev = IdentityChangeEvidence::DisclosureAndAction {
        disclosure: dart_disclosure("20261103000011")?,
        action: split_action(&witness.us_id, cal, 1, 2)?,
        note: parse_reason("충돌 검증")?,
    };
    verify_link_conflict_cases(repo, witness, &ev, stamp).await?;

    let other_us = parse_source(KnownMarket::Us, "AAPL", "US0378331005")?;
    let other_cand = issue_slot(2)?;
    let other_id = other_cand.as_instrument_id().clone();
    assert_eq!(
        repo.ensure(&other_us, other_cand).await,
        Ok(MappingAssignment::Issued(other_id)),
        "대조용 타 ID 등록"
    );
    let to_other_active = confirm_change(
        &witness.us_id,
        &witness.us_third,
        &other_us,
        ev.clone(),
        stamp,
    )
    .await?;
    assert_eq!(
        repo.link_confirmed_change(to_other_active).await,
        Err(tr_err(MappingTransitionError::SourceBoundToOtherId)),
        "타 ID 활성 원본으로 연결 거부"
    );
    let other_overlap = parse_source(KnownMarket::Us, "aapl", "US0000000077")?;
    let to_other_overlap =
        confirm_change(&witness.us_id, &witness.us_third, &other_overlap, ev, stamp).await?;
    assert_eq!(
        repo.link_confirmed_change(to_other_overlap).await,
        Err(tr_err(MappingTransitionError::IdentityUncertain)),
        "타 ID 심볼 부분 겹침 연결 거부"
    );
    assert_history_exact(
        repo.history().await,
        &witness.expected_history,
        "충돌 거부 후 이력 무변경",
    );
    Some(())
}

async fn verify_retirement_precondition_rejections<P>(
    repo: &P,
    witness: &ContractWitness,
    ev: &MappingCorrectionEvidence,
    stamp: Timestamp,
) -> Option<()>
where
    P: InstrumentMappingRepository + InstrumentMappingCorrectionRepository,
{
    for (target_src, expected_err) in [
        (
            witness.kr_second.clone(),
            MappingTransitionError::SourceNotRetired,
        ),
        (
            witness.kr_first.clone(),
            MappingTransitionError::SourceNotRetired,
        ),
        (
            witness.us_third.clone(),
            MappingTransitionError::SourceBoundToOtherId,
        ),
        (
            parse_source(KnownMarket::Kr, "005930", "kr7005930999")?,
            MappingTransitionError::IdentityUncertain,
        ),
        (
            parse_source(KnownMarket::Kr, "999999", "KR7999999999")?,
            MappingTransitionError::Missing,
        ),
    ] {
        let req = confirm_retire_corr(&witness.kr_id, &target_src, ev.clone(), stamp).await?;
        assert_eq!(
            repo.correct_retirement(req).await,
            Err(tr_err(expected_err)),
            "폐지 정정 사전 조건 위반 거부"
        );
    }

    assert_eq!(
        repo.retire(&witness.kr_second).await,
        Ok(witness.kr_id.clone()),
        "KR 종단 폐지"
    );
    let retired_link = confirm_change(
        &witness.kr_id,
        &witness.kr_second,
        &parse_source(KnownMarket::Kr, "005931", "KR7005930888")?,
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: dart_disclosure("20261104000020")?,
            action: split_action(&witness.kr_id, parse_date(2026, 11, 4)?, 1, 2)?,
            note: parse_reason("폐지 후 연결 시도")?,
        },
        stamp,
    )
    .await?;
    assert_eq!(
        repo.link_confirmed_change(retired_link).await,
        Err(tr_err(MappingTransitionError::SourceNotActive)),
        "폐지 원본의 연결 변경 거부"
    );
    Some(())
}

async fn verify_retirement_origin_and_evidence_rejections<P>(
    repo: &P,
    witness: &ContractWitness,
    ev: &MappingCorrectionEvidence,
    note: &UserReason,
    stamp: Timestamp,
) -> Option<()>
where
    P: InstrumentMappingCorrectionRepository,
{
    for bad_origin in [
        IdentityConfirmationOrigin::BrokerObservation,
        IdentityConfirmationOrigin::AiSuggestion,
    ] {
        let req = ConfirmedRetirementCorrection::new(
            witness.kr_id.clone(),
            witness.kr_second.clone(),
            MappingCorrectionConfirmation::new(bad_origin, ev.clone(), stamp),
        );
        assert_eq!(
            repo.correct_retirement(req).await,
            Err(tr_err(MappingTransitionError::IdentityUncertain)),
            "비사용자 확인 출처 폐지 정정 거부"
        );
    }
    for bad_ev in [
        MappingCorrectionEvidence::None,
        MappingCorrectionEvidence::NoteOnly(note.clone()),
    ] {
        let origin =
            issue_user_origin(IdentityConfirmationIssuanceKind::RetirementCorrection).await?;
        let req = ConfirmedRetirementCorrection::new(
            witness.kr_id.clone(),
            witness.kr_second.clone(),
            MappingCorrectionConfirmation::new(origin, bad_ev, stamp),
        );
        assert_eq!(
            repo.correct_retirement(req).await,
            Err(tr_err(MappingTransitionError::IdentityUncertain)),
            "불완전 증거 폐지 정정 거부"
        );
    }
    Some(())
}

async fn verify_re_retire_and_second_correction<P>(
    repo: &P,
    witness: &mut ContractWitness,
) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    assert_eq!(
        repo.retire(&witness.kr_second).await,
        Ok(witness.kr_id.clone()),
        "폐지 정정 후 재폐지"
    );
    assert_eq!(
        repo.ensure(&witness.kr_second, issue_slot(11)?).await,
        Err(map_err(InstrumentMappingError::Retired)),
        "폐지 정정 후 재폐지된 원본은 ensure로 재활성화 불가"
    );
    let unconfirmed_reactivate = ConfirmedRetirementCorrection::new(
        witness.kr_id.clone(),
        witness.kr_second.clone(),
        MappingCorrectionConfirmation::new(
            IdentityConfirmationOrigin::BrokerObservation,
            MappingCorrectionEvidence::DisclosureAndNote {
                disclosure: kind_disclosure("20261105000021")?,
                note: parse_reason("미확인 재활성화 차단")?,
            },
            parse_ts(1_764_000_450, 0, 32_400)?,
        ),
    );
    assert_eq!(
        repo.correct_retirement(unconfirmed_reactivate).await,
        Err(tr_err(MappingTransitionError::IdentityUncertain)),
        "재폐지 후 미확인 재활성화 거부"
    );
    assert_history_exact(
        repo.history().await,
        &witness.expected_history,
        "재폐지 후 새 사건 없음",
    );

    let restored_second = apply_and_record_retire_corr(
        repo,
        RetireCorrSpec {
            id: &witness.kr_id,
            target: &witness.kr_second,
            disclosure: kind_disclosure("20261105000022")?,
            note: parse_reason("재폐지 뒤 새 확인으로 2차 정정")?,
            stamp: parse_ts(1_764_000_500, 20, 32_400)?,
        },
    )
    .await?;
    assert_eq!(
        repo.resolve(&witness.kr_second).await,
        Ok(witness.kr_id.clone()),
        "2차 폐지 정정 후 활성 복구"
    );
    witness.expected_history.push(restored_second);
    assert_history_exact(
        repo.history().await,
        &witness.expected_history,
        "재폐지 뒤 2차 폐지 정정이 새 순번으로 기록되어야 함",
    );
    Some(())
}

async fn verify_retirement_correction_and_re_retire<P>(
    repo: &P,
    witness: &mut ContractWitness,
) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let first_disc = dart_disclosure("20261104000020")?;
    let first_note = parse_reason("오폐지 1차 복구")?;
    let first_stamp = parse_ts(1_764_000_400, 10, 32_400)?;
    let first_ev = MappingCorrectionEvidence::DisclosureAndNote {
        disclosure: first_disc.clone(),
        note: first_note.clone(),
    };
    verify_retirement_precondition_rejections(repo, witness, &first_ev, first_stamp).await?;
    verify_retirement_origin_and_evidence_rejections(
        repo,
        witness,
        &first_ev,
        &first_note,
        first_stamp,
    )
    .await?;

    let restored_first = apply_and_record_retire_corr(
        repo,
        RetireCorrSpec {
            id: &witness.kr_id,
            target: &witness.kr_second,
            disclosure: first_disc,
            note: first_note,
            stamp: first_stamp,
        },
    )
    .await?;
    assert_eq!(
        repo.resolve(&witness.kr_second).await,
        Ok(witness.kr_id.clone()),
        "1차 폐지 정정 후 활성 복구"
    );
    witness.expected_history.push(restored_first);
    verify_re_retire_and_second_correction(repo, witness).await
}

async fn verify_link_correction_origin_rejections<P>(
    repo: &P,
    witness: &ContractWitness,
    ev: &MappingCorrectionEvidence,
    note: &UserReason,
    stamp: Timestamp,
) -> Option<()>
where
    P: InstrumentMappingCorrectionRepository,
{
    for bad_origin in [
        IdentityConfirmationOrigin::BrokerObservation,
        IdentityConfirmationOrigin::AiSuggestion,
    ] {
        let req = ConfirmedLinkCorrection::new(
            witness.us_id.clone(),
            witness.us_second.clone(),
            witness.us_third.clone(),
            MappingCorrectionConfirmation::new(bad_origin, ev.clone(), stamp),
        );
        assert_eq!(
            repo.correct_link(req).await,
            Err(tr_err(MappingTransitionError::IdentityUncertain)),
            "비사용자 출처 연결 정정 거부"
        );
    }
    for bad_ev in [
        MappingCorrectionEvidence::None,
        MappingCorrectionEvidence::NoteOnly(note.clone()),
    ] {
        let origin = issue_user_origin(IdentityConfirmationIssuanceKind::LinkCorrection).await?;
        let req = ConfirmedLinkCorrection::new(
            witness.us_id.clone(),
            witness.us_second.clone(),
            witness.us_third.clone(),
            MappingCorrectionConfirmation::new(origin, bad_ev, stamp),
        );
        assert_eq!(
            repo.correct_link(req).await,
            Err(tr_err(MappingTransitionError::IdentityUncertain)),
            "불완전 증거 연결 정정 거부"
        );
    }
    Some(())
}

fn build_link_corr_rejection_cases(
    witness: &ContractWitness,
) -> Option<[(InstrumentSource, InstrumentSource, MappingTransitionError); 8]> {
    Some([
        (
            witness.us_third.clone(),
            witness.us_third.clone(),
            MappingTransitionError::UnchangedSource,
        ),
        (
            witness.us_first.clone(),
            witness.us_second.clone(),
            MappingTransitionError::LinkNotLatest,
        ),
        (
            witness.us_third.clone(),
            witness.us_second.clone(),
            MappingTransitionError::LinkNotLatest,
        ),
        (
            witness.kr_second.clone(),
            witness.us_third.clone(),
            MappingTransitionError::SourceBoundToOtherId,
        ),
        (
            witness.us_second.clone(),
            witness.kr_second.clone(),
            MappingTransitionError::SourceBoundToOtherId,
        ),
        (
            parse_source(KnownMarket::Us, "MISS", "US0000000044")?,
            witness.us_third.clone(),
            MappingTransitionError::Missing,
        ),
        (
            witness.us_second.clone(),
            parse_source(KnownMarket::Us, "MISS", "US0000000045")?,
            MappingTransitionError::Missing,
        ),
        (
            parse_source(KnownMarket::Us, "meta", "US30303M1027")?,
            witness.us_third.clone(),
            MappingTransitionError::IdentityUncertain,
        ),
    ])
}

async fn verify_link_correction_rejections<P>(repo: &P, witness: &ContractWitness) -> Option<()>
where
    P: InstrumentMappingRepository + InstrumentMappingCorrectionRepository,
{
    let note = parse_reason("연결 정정 거부 검증")?;
    let stamp = parse_ts(1_764_000_600, 0, 0)?;
    let ev = MappingCorrectionEvidence::DisclosureAndNote {
        disclosure: sec_disclosure("0001326801-26-000130")?,
        note: note.clone(),
    };
    verify_link_correction_origin_rejections(repo, witness, &ev, &note, stamp).await?;

    for (prev, next, expected_err) in build_link_corr_rejection_cases(witness)? {
        let req = confirm_link_corr(&witness.us_id, &prev, &next, ev.clone(), stamp).await?;
        assert_eq!(
            repo.correct_link(req).await,
            Err(tr_err(expected_err)),
            "비최신·충돌 연결 정정은 거부되어야 함"
        );
    }

    assert_eq!(
        repo.retire(&witness.us_third).await,
        Ok(witness.us_id.clone()),
        "종단 폐지"
    );
    let retired_undo = confirm_link_corr(
        &witness.us_id,
        &witness.us_second,
        &witness.us_third,
        ev,
        stamp,
    )
    .await?;
    assert_eq!(
        repo.correct_link(retired_undo).await,
        Err(tr_err(MappingTransitionError::LinkNotLatest)),
        "종단이 Retired일 때는 연결 정정 거부"
    );
    Some(())
}

async fn verify_link_correction_unwind_two_hops<P>(
    repo: &P,
    witness: &mut ContractWitness,
) -> Option<()>
where
    P: InstrumentMappingRepository + InstrumentMappingCorrectionRepository,
{
    let restore_ev = apply_and_record_retire_corr(
        repo,
        RetireCorrSpec {
            id: &witness.us_id,
            target: &witness.us_third,
            disclosure: dart_disclosure("20261106000030")?,
            note: parse_reason("연결 정정 전 폐지 복구")?,
            stamp: parse_ts(1_764_000_610, 0, 32_400)?,
        },
    )
    .await?;
    witness.expected_history.push(restore_ev);

    let unwind_second = apply_and_record_link_corr(
        repo,
        LinkCorrSpec {
            id: &witness.us_id,
            previous: &witness.us_second,
            mistaken_next: &witness.us_third,
            disclosure: kind_disclosure("20261106000031")?,
            note: parse_reason("2단계 연결 해제")?,
            stamp: parse_ts(1_764_000_620, 77, -14_400)?,
        },
    )
    .await?;
    witness.expected_history.push(unwind_second);
    assert_eq!(
        repo.resolve(&witness.us_second).await,
        Ok(witness.us_id.clone()),
        "2단계 해제 후 us_second 활성 복구"
    );
    assert_eq!(
        repo.resolve(&witness.us_third).await,
        Err(map_err(InstrumentMappingError::IdentityUncertain)),
        "해제된 us_third는 남아있는 us_first와 심볼 겹침으로 IdentityUncertain"
    );

    let unwind_first = apply_and_record_link_corr(
        repo,
        LinkCorrSpec {
            id: &witness.us_id,
            previous: &witness.us_first,
            mistaken_next: &witness.us_second,
            disclosure: dart_disclosure("20261106000032")?,
            note: parse_reason("1단계 연결 해제")?,
            stamp: parse_ts(1_764_000_630, 88, 32_400)?,
        },
    )
    .await?;
    witness.expected_history.push(unwind_first);
    assert_eq!(
        repo.resolve(&witness.us_first).await,
        Ok(witness.us_id.clone()),
        "1단계 해제 후 us_first 활성 복구"
    );
    Some(())
}

async fn assert_final_histories<P>(repo: &P, witness: &ContractWitness)
where
    P: InstrumentMappingHistoryRepository,
{
    assert_history_exact(
        repo.history().await,
        &witness.expected_history,
        "최종 전체 이력 일치",
    );
    let expected_us: Vec<_> = witness
        .expected_history
        .iter()
        .filter(|event| event.instrument_id() == &witness.us_id)
        .cloned()
        .collect();
    let expected_kr: Vec<_> = witness
        .expected_history
        .iter()
        .filter(|event| event.instrument_id() == &witness.kr_id)
        .cloned()
        .collect();
    assert_history_exact(
        repo.history_for_id(&witness.us_id).await,
        &expected_us,
        "최종 US 이력 일치",
    );
    assert_history_exact(
        repo.history_for_id(&witness.kr_id).await,
        &expected_kr,
        "최종 KR 이력 일치",
    );
}

async fn verify_disjoint_unlink_reuse_and_relink<P>(
    repo: &P,
    witness: &mut ContractWitness,
) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let disjoint_next = parse_source(KnownMarket::Us, "DISJ", "US8888888881")?;
    let link_disjoint = apply_and_record_link(
        repo,
        LinkStepSpec {
            id: &witness.us_id,
            previous: &witness.us_first,
            next: &disjoint_next,
            disclosure: sec_disclosure("0001326801-26-000140")?,
            action: split_action(&witness.us_id, parse_date(2026, 11, 7)?, 1, 3)?,
            note: parse_reason("분리 가능 원본 연결")?,
            stamp: parse_ts(1_764_000_640, 0, 0)?,
            label: "분리 가능 원본 연결 성공",
        },
    )
    .await?;
    witness.expected_history.push(link_disjoint);

    let undo_disjoint = apply_and_record_link_corr(
        repo,
        LinkCorrSpec {
            id: &witness.us_id,
            previous: &witness.us_first,
            mistaken_next: &disjoint_next,
            disclosure: dart_disclosure("20261107000041")?,
            note: parse_reason("분리 가능 원본 연결 해제")?,
            stamp: parse_ts(1_764_000_650, 0, 32_400)?,
        },
    )
    .await?;
    witness.expected_history.push(undo_disjoint);

    assert_eq!(
        repo.resolve(&disjoint_next).await,
        Err(map_err(InstrumentMappingError::Missing)),
        "완전 분리된 오연결 원본은 Missing으로 돌아와야 함"
    );
    let fresh_cand = issue_slot(12)?;
    let fresh_id = fresh_cand.as_instrument_id().clone();
    assert_eq!(
        repo.ensure(&disjoint_next, fresh_cand).await,
        Ok(MappingAssignment::Issued(fresh_id)),
        "분리된 원본은 새 ID로 재발급 가능해야 함"
    );

    let relink_event = apply_and_record_link(
        repo,
        LinkStepSpec {
            id: &witness.us_id,
            previous: &witness.us_first,
            next: &witness.us_second,
            disclosure: sec_disclosure("0001326801-26-000142")?,
            action: symbol_action(&witness.us_id, parse_date(2026, 11, 8)?, "FB.A", "META")?,
            note: parse_reason("정정 후 동일 쌍 재연결")?,
            stamp: parse_ts(1_764_000_660, 1, -18_000)?,
            label: "정정 후 동일 쌍 재연결 성공",
        },
    )
    .await?;
    witness.expected_history.push(relink_event);
    assert_final_histories(repo, witness).await;
    Some(())
}

async fn run_correction_lifecycle_inner<P>(repo: &P) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let mut witness = seed_and_verify_link_chains(repo).await?;
    verify_link_origin_and_incomplete_evidence(repo, &witness).await?;
    verify_link_bad_actions(repo, &witness).await?;
    verify_link_conflict_rejections(repo, &witness).await?;
    verify_retirement_correction_and_re_retire(repo, &mut witness).await?;
    verify_link_correction_rejections(repo, &witness).await?;
    verify_link_correction_unwind_two_hops(repo, &mut witness).await?;
    verify_disjoint_unlink_reuse_and_relink(repo, &mut witness).await
}

pub async fn correction_lifecycle_and_conflict_rules_contract<P>(repo: &P)
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let completed = run_correction_lifecycle_inner(repo).await;
    assert!(
        completed.is_some(),
        "정정 생명주기·충돌 계약의 모든 단계가 완료되어야 함"
    );
}

struct UnpolledSeed {
    id: InstrumentId,
    first: InstrumentSource,
    second: InstrumentSource,
    disc: DisclosureKey,
    note: UserReason,
    stamp: Timestamp,
}

async fn check_unpolled_link_and_retire<P>(repo: &P, seed: &UnpolledSeed) -> Option<CorporateAction>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let act = symbol_action(&seed.id, parse_date(2026, 11, 10)?, "FB", "META")?;
    let unpolled_change = confirm_change(
        &seed.id,
        &seed.first,
        &seed.second,
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: seed.disc.clone(),
            action: act.clone(),
            note: seed.note.clone(),
        },
        seed.stamp,
    )
    .await?;
    {
        let _future = repo.link_confirmed_change(unpolled_change);
    }
    assert_eq!(
        repo.resolve(&seed.first).await,
        Ok(seed.id.clone()),
        "미폴링 연결 무변경"
    );
    assert_eq!(repo.history().await, Ok(Vec::new()), "미폴링 연결 이력 0건");

    assert_eq!(
        repo.retire(&seed.first).await,
        Ok(seed.id.clone()),
        "폐지 시드"
    );
    let unpolled_retire = confirm_retire_corr(
        &seed.id,
        &seed.first,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: seed.disc.clone(),
            note: seed.note.clone(),
        },
        seed.stamp,
    )
    .await?;
    {
        let _future = repo.correct_retirement(unpolled_retire);
    }
    assert_eq!(
        repo.resolve(&seed.first).await,
        Err(map_err(InstrumentMappingError::Retired)),
        "미폴링 폐지 정정 무변경"
    );
    assert_eq!(
        repo.history().await,
        Ok(Vec::new()),
        "미폴링 폐지 정정 이력 0건"
    );
    Some(act)
}

async fn check_unpolled_link_correction<P>(
    repo: &P,
    seed: UnpolledSeed,
    act: CorporateAction,
) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let polled_retire = confirm_retire_corr(
        &seed.id,
        &seed.first,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: seed.disc.clone(),
            note: seed.note.clone(),
        },
        seed.stamp,
    )
    .await?;
    assert_eq!(
        repo.correct_retirement(polled_retire).await,
        Ok(seed.id.clone()),
        "폴링된 폐지 정정 성공"
    );
    let polled_change = confirm_change(
        &seed.id,
        &seed.first,
        &seed.second,
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: seed.disc.clone(),
            action: act,
            note: seed.note.clone(),
        },
        seed.stamp,
    )
    .await?;
    assert_eq!(
        repo.link_confirmed_change(polled_change).await,
        Ok(seed.id.clone()),
        "폴링된 연결 성공"
    );
    let unpolled_undo = confirm_link_corr(
        &seed.id,
        &seed.first,
        &seed.second,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: seed.disc,
            note: seed.note,
        },
        seed.stamp,
    )
    .await?;
    {
        let _future = repo.correct_link(unpolled_undo);
    }
    assert_eq!(
        repo.resolve(&seed.second).await,
        Ok(seed.id),
        "미폴링 연결 정정 무변경"
    );
    assert_eq!(
        repo.history().await.map(|events| events.len()),
        Ok(2),
        "미폴링 연결 정정 이력 수 유지"
    );
    Some(())
}

async fn run_unpolled_correction_inner<P>(repo: &P) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let first = parse_source(KnownMarket::Us, "FB", "US30303M1027")?;
    let second = parse_source(KnownMarket::Us, "META", "US30303M1027")?;
    let cand = issue_slot(0)?;
    let disc = sec_disclosure("0001326801-26-000201")?;
    let note = parse_reason("미폴링 확인")?;
    let stamp = parse_ts(1_764_001_000, 0, 0)?;
    let id = cand.as_instrument_id().clone();
    assert_eq!(
        repo.ensure(&first, cand).await,
        Ok(MappingAssignment::Issued(id.clone())),
        "미폴링 검증용 초기 등록"
    );
    let seed = UnpolledSeed {
        id,
        first,
        second,
        disc,
        note,
        stamp,
    };
    let act = check_unpolled_link_and_retire(repo, &seed).await?;
    check_unpolled_link_correction(repo, seed, act).await
}

pub async fn unpolled_correction_writes_do_not_mutate_contract<P>(repo: &P)
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
{
    let completed = run_unpolled_correction_inner(repo).await;
    assert!(
        completed.is_some(),
        "미폴링 정정 쓰기 계약의 모든 단계가 완료되어야 함"
    );
}

fn race_pair<L, R, A, B>(left_job: L, right_job: R) -> Option<(A, B)>
where
    L: FnOnce(&Barrier) -> A + Send,
    R: FnOnce(&Barrier) -> B + Send,
    A: Send,
    B: Send,
{
    let barrier = Arc::new(Barrier::new(2));
    std::thread::scope(|scope| {
        let left_barrier = Arc::clone(&barrier);
        let right_barrier = Arc::clone(&barrier);
        let left = scope.spawn(move || left_job(&left_barrier));
        let right = scope.spawn(move || right_job(&right_barrier));
        let left_out = left.join();
        let right_out = right.join();
        assert!(left_out.is_ok(), "왼쪽 경쟁 스레드 완료");
        assert!(right_out.is_ok(), "오른쪽 경쟁 스레드 완료");
        match (left_out, right_out) {
            (Ok(first), Ok(second)) => Some((first, second)),
            (Ok(_) | Err(_), Ok(_) | Err(_)) => None,
        }
    })
}

struct SplitChangeSpec<'a> {
    id: &'a InstrumentId,
    previous: &'a InstrumentSource,
    next: &'a InstrumentSource,
    disc: DisclosureKey,
    note: UserReason,
    stamp: Timestamp,
}

fn build_split_change<R: ContractRuntime>(
    runtime: &R,
    spec: SplitChangeSpec<'_>,
) -> Option<ConfirmedIdentifierChange> {
    let act = split_action(spec.id, parse_date(2026, 11, 11)?, 1, 2)?;
    runtime.block_on(confirm_change(
        spec.id,
        spec.previous,
        spec.next,
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: spec.disc,
            action: act,
            note: spec.note,
        },
        spec.stamp,
    ))
}

fn seed_two_instruments<P, R>(
    repo: &P,
    runtime: &R,
    first_src: &InstrumentSource,
    second_src: &InstrumentSource,
) -> Option<(InstrumentId, InstrumentId)>
where
    P: InstrumentMappingRepository,
    R: ContractRuntime,
{
    let cand_a = issue_slot(0)?;
    let cand_b = issue_slot(1)?;
    let id_a = cand_a.as_instrument_id().clone();
    let id_b = cand_b.as_instrument_id().clone();
    assert_eq!(
        runtime.block_on(repo.ensure(first_src, cand_a)),
        Ok(MappingAssignment::Issued(id_a.clone())),
        "첫 종목 등록"
    );
    assert_eq!(
        runtime.block_on(repo.ensure(second_src, cand_b)),
        Ok(MappingAssignment::Issued(id_b.clone())),
        "둘째 종목 등록"
    );
    Some((id_a, id_b))
}

fn seed_single_link<P, R>(
    repo: &P,
    runtime: &R,
    first_src: &InstrumentSource,
    second_src: &InstrumentSource,
    meta: (&DisclosureKey, &UserReason, Timestamp),
) -> Option<InstrumentId>
where
    P: InstrumentMappingRepository + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let cand = issue_slot(0)?;
    let id = cand.as_instrument_id().clone();
    assert_eq!(
        runtime.block_on(repo.ensure(first_src, cand)),
        Ok(MappingAssignment::Issued(id.clone())),
        "초기 등록"
    );
    let seed_link = build_split_change(
        runtime,
        SplitChangeSpec {
            id: &id,
            previous: first_src,
            next: second_src,
            disc: meta.0.clone(),
            note: meta.1.clone(),
            stamp: meta.2,
        },
    )?;
    assert_eq!(
        runtime.block_on(repo.link_confirmed_change(seed_link)),
        Ok(id.clone()),
        "사전 연결"
    );
    Some(id)
}

fn assert_single_link_winner<P, R>(
    left: &P,
    right: &P,
    runtime: &R,
    winner: &InstrumentSource,
    id: &InstrumentId,
) where
    P: InstrumentMappingRepository + InstrumentMappingHistoryRepository,
    R: ContractRuntime,
{
    assert_eq!(
        runtime.block_on(left.resolve(winner)),
        Ok(id.clone()),
        "왼쪽 연결에서 승자 확인"
    );
    assert_eq!(
        runtime.block_on(right.resolve(winner)),
        Ok(id.clone()),
        "오른쪽 연결에서 승자 확인"
    );
    assert_eq!(
        runtime.block_on(left.history()).map(|events| events.len()),
        Ok(1),
        "승자 사건 1건만 기록되어야 함"
    );
}

type TransitionOutcome = Result<InstrumentId, MappingTransitionStoreError>;

fn race_two_links<P, R>(
    left: &P,
    right: &P,
    runtime: &R,
    pair: (SplitChangeSpec<'_>, SplitChangeSpec<'_>),
) -> Option<(TransitionOutcome, TransitionOutcome)>
where
    P: InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let left_req = build_split_change(runtime, pair.0)?;
    let right_req = build_split_change(runtime, pair.1)?;
    race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.link_confirmed_change(left_req))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.link_confirmed_change(right_req))
        },
    )
}

fn seed_one_instrument<P, R>(
    repo: &P,
    runtime: &R,
    source: &InstrumentSource,
) -> Option<InstrumentId>
where
    P: InstrumentMappingRepository,
    R: ContractRuntime,
{
    let cand = issue_slot(0)?;
    let id = cand.as_instrument_id().clone();
    assert_eq!(
        runtime.block_on(repo.ensure(source, cand)),
        Ok(MappingAssignment::Issued(id.clone())),
        "초기 종목 등록"
    );
    Some(id)
}

fn run_same_previous_link_race<P, R>(left: &P, right: &P, runtime: &R) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let origin_src = parse_source(KnownMarket::Us, "FB", "US30303M1027")?;
    let target_a = parse_source(KnownMarket::Us, "META", "US30303M1027")?;
    let target_b = parse_source(KnownMarket::Us, "METB", "US30303M1027")?;
    let disc = sec_disclosure("0001326801-26-000301")?;
    let note = parse_reason("동시 연결 경쟁")?;
    let stamp = parse_ts(1_764_002_000, 0, 0)?;
    let id = seed_one_instrument(left, runtime, &origin_src)?;
    let left_spec = SplitChangeSpec {
        id: &id,
        previous: &origin_src,
        next: &target_a,
        disc: disc.clone(),
        note: note.clone(),
        stamp,
    };
    let right_spec = SplitChangeSpec {
        id: &id,
        previous: &origin_src,
        next: &target_b,
        disc,
        note,
        stamp,
    };
    let (first_outcome, second_outcome) =
        race_two_links(left, right, runtime, (left_spec, right_spec))?;
    let loser = Err(tr_err(MappingTransitionError::SourceNotActive));
    let winner = if first_outcome == Ok(id.clone()) && second_outcome == loser {
        Some(target_a)
    } else if second_outcome == Ok(id.clone()) && first_outcome == loser {
        Some(target_b)
    } else {
        assert!(
            first_outcome == Ok(id.clone()) && second_outcome == loser,
            "동일 이전 원본 동시 연결은 승자 1건과 SourceNotActive 1건이어야 함: {first_outcome:?}, {second_outcome:?}"
        );
        None
    };
    let winner_src = winner?;
    assert_single_link_winner(left, right, runtime, &winner_src, &id);
    Some(())
}

pub fn concurrent_same_previous_link_has_one_winner<P, R>(left: &P, right: &P, runtime: &R)
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    assert!(
        run_same_previous_link_race(left, right, runtime).is_some(),
        "동일 이전 원본 동시 연결 경쟁 계약 완료"
    );
}

fn run_cross_id_same_next_race<P, R>(left: &P, right: &P, runtime: &R) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let first_src = parse_source(KnownMarket::Us, "AAA", "US0000000101")?;
    let second_src = parse_source(KnownMarket::Us, "BBB", "US0000000102")?;
    let shared_next = parse_source(KnownMarket::Us, "TGT", "US0000000199")?;
    let disc = sec_disclosure("0001326801-26-000302")?;
    let note = parse_reason("타 ID 동일 대상 경쟁")?;
    let stamp = parse_ts(1_764_002_010, 0, 0)?;
    let (id_a, id_b) = seed_two_instruments(left, runtime, &first_src, &second_src)?;
    let left_spec = SplitChangeSpec {
        id: &id_a,
        previous: &first_src,
        next: &shared_next,
        disc: disc.clone(),
        note: note.clone(),
        stamp,
    };
    let right_spec = SplitChangeSpec {
        id: &id_b,
        previous: &second_src,
        next: &shared_next,
        disc,
        note,
        stamp,
    };
    let (first_outcome, second_outcome) =
        race_two_links(left, right, runtime, (left_spec, right_spec))?;
    let loser = Err(tr_err(MappingTransitionError::SourceBoundToOtherId));
    let winner_id = if first_outcome == Ok(id_a.clone()) && second_outcome == loser {
        Some(id_a)
    } else if second_outcome == Ok(id_b.clone()) && first_outcome == loser {
        Some(id_b)
    } else {
        assert!(
            first_outcome == Ok(id_a) && second_outcome == loser,
            "타 ID 동일 대상 경쟁은 승자 1건과 SourceBoundToOtherId 1건이어야 함: {first_outcome:?}, {second_outcome:?}"
        );
        None
    };
    let winner = winner_id?;
    assert_single_link_winner(left, right, runtime, &shared_next, &winner);
    Some(())
}

pub fn concurrent_cross_id_same_next_link_has_one_winner<P, R>(left: &P, right: &P, runtime: &R)
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    assert!(
        run_cross_id_same_next_race(left, right, runtime).is_some(),
        "타 ID 동일 대상 경쟁 계약 완료"
    );
}

fn run_cross_id_overlap_race<P, R>(left: &P, right: &P, runtime: &R) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let first_src = parse_source(KnownMarket::Us, "AAA", "US0000000201")?;
    let second_src = parse_source(KnownMarket::Us, "BBB", "US0000000202")?;
    let next_a = parse_source(KnownMarket::Us, "OVLP", "US0000000291")?;
    let next_b = parse_source(KnownMarket::Us, "ovlp", "US0000000292")?;
    let disc = sec_disclosure("0001326801-26-000303")?;
    let note = parse_reason("타 ID 부분 겹침 경쟁")?;
    let stamp = parse_ts(1_764_002_020, 0, 0)?;
    let (id_a, id_b) = seed_two_instruments(left, runtime, &first_src, &second_src)?;
    let left_spec = SplitChangeSpec {
        id: &id_a,
        previous: &first_src,
        next: &next_a,
        disc: disc.clone(),
        note: note.clone(),
        stamp,
    };
    let right_spec = SplitChangeSpec {
        id: &id_b,
        previous: &second_src,
        next: &next_b,
        disc,
        note,
        stamp,
    };
    let (first_outcome, second_outcome) =
        race_two_links(left, right, runtime, (left_spec, right_spec))?;
    let loser = Err(tr_err(MappingTransitionError::IdentityUncertain));
    let winner = if first_outcome == Ok(id_a.clone()) && second_outcome == loser {
        Some((next_a, id_a))
    } else if second_outcome == Ok(id_b.clone()) && first_outcome == loser {
        Some((next_b, id_b))
    } else {
        assert!(
            first_outcome == Ok(id_a) && second_outcome == loser,
            "타 ID 부분 겹침 경쟁은 승자 1건과 IdentityUncertain 1건이어야 함: {first_outcome:?}, {second_outcome:?}"
        );
        None
    };
    let (winner_src, winner_id) = winner?;
    assert_single_link_winner(left, right, runtime, &winner_src, &winner_id);
    Some(())
}

pub fn concurrent_cross_id_overlapping_next_link_has_one_winner<P, R>(
    left: &P,
    right: &P,
    runtime: &R,
) where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    assert!(
        run_cross_id_overlap_race(left, right, runtime).is_some(),
        "타 ID 부분 겹침 경쟁 계약 완료"
    );
}

#[derive(Clone, Copy)]
enum LinkOrRetireWinner {
    Link,
    Retire,
}

fn assert_link_vs_retire_state<P, R>(
    repo: &P,
    runtime: &R,
    id: &InstrumentId,
    sources: (&InstrumentSource, &InstrumentSource),
    winner: LinkOrRetireWinner,
) where
    P: InstrumentMappingRepository + InstrumentMappingHistoryRepository,
    R: ContractRuntime,
{
    match winner {
        LinkOrRetireWinner::Link => {
            assert_eq!(
                runtime.block_on(repo.resolve(sources.1)),
                Ok(id.clone()),
                "연결 승자 시 다음 원본 활성"
            );
            assert_eq!(
                runtime.block_on(repo.history()).map(|events| events.len()),
                Ok(1),
                "연결 승자 사건 1건"
            );
        }
        LinkOrRetireWinner::Retire => {
            assert_eq!(
                runtime.block_on(repo.resolve(sources.0)),
                Err(map_err(InstrumentMappingError::Retired)),
                "폐지 승자 시 원본 폐지 상태"
            );
            assert_eq!(
                runtime.block_on(repo.history()).map(|events| events.len()),
                Ok(0),
                "폐지 승자 사건 0건"
            );
        }
    }
}

fn run_link_and_retire_race<P, R>(left: &P, right: &P, runtime: &R) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let first_src = parse_source(KnownMarket::Kr, "005930", "KR7005930003")?;
    let next_src = parse_source(KnownMarket::Kr, "005931", "KR7005930003")?;
    let disc = dart_disclosure("20261111000304")?;
    let note = parse_reason("연결 대 폐지 경쟁")?;
    let stamp = parse_ts(1_764_002_030, 0, 32_400)?;
    let id = seed_one_instrument(left, runtime, &first_src)?;
    let req = build_split_change(
        runtime,
        SplitChangeSpec {
            id: &id,
            previous: &first_src,
            next: &next_src,
            disc,
            note,
            stamp,
        },
    )?;
    let (link_outcome, retire_outcome) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.link_confirmed_change(req))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.retire(&first_src))
        },
    )?;
    let link_won = link_outcome == Ok(id.clone())
        && retire_outcome == Err(map_err(InstrumentMappingError::Superseded));
    let retire_won = link_outcome == Err(tr_err(MappingTransitionError::SourceNotActive))
        && retire_outcome == Ok(id.clone());
    assert!(
        link_won || retire_won,
        "연결 대 폐지 경쟁은 한쪽만 성공해야 함: {link_outcome:?}, {retire_outcome:?}"
    );
    let winner = if link_won {
        LinkOrRetireWinner::Link
    } else {
        LinkOrRetireWinner::Retire
    };
    assert_link_vs_retire_state(left, runtime, &id, (&first_src, &next_src), winner);
    Some(())
}

pub fn concurrent_link_and_retire_serializes_cleanly<P, R>(left: &P, right: &P, runtime: &R)
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    assert!(
        run_link_and_retire_race(left, right, runtime).is_some(),
        "연결 대 폐지 경쟁 계약 완료"
    );
}

fn run_double_retirement_corr_race<P, R>(left: &P, right: &P, runtime: &R) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let target_src = parse_source(KnownMarket::Kr, "005930", "KR7005930003")?;
    let disc = dart_disclosure("20261111000305")?;
    let note = parse_reason("이중 폐지 정정 경쟁")?;
    let stamp = parse_ts(1_764_002_040, 0, 32_400)?;
    let id = seed_one_instrument(left, runtime, &target_src)?;
    assert_eq!(
        runtime.block_on(left.retire(&target_src)),
        Ok(id.clone()),
        "사전 폐지"
    );
    let ev = MappingCorrectionEvidence::DisclosureAndNote {
        disclosure: disc,
        note,
    };
    let first_req = runtime.block_on(confirm_retire_corr(&id, &target_src, ev.clone(), stamp))?;
    let second_req = runtime.block_on(confirm_retire_corr(&id, &target_src, ev, stamp))?;
    let (first_outcome, second_outcome) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.correct_retirement(first_req))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.correct_retirement(second_req))
        },
    )?;
    let loser = Err(tr_err(MappingTransitionError::SourceNotRetired));
    let valid = (first_outcome == Ok(id.clone()) && second_outcome == loser)
        || (second_outcome == Ok(id.clone()) && first_outcome == loser);
    assert!(
        valid,
        "이중 폐지 정정 경쟁은 승자 1건과 SourceNotRetired 1건이어야 함: {first_outcome:?}, {second_outcome:?}"
    );
    assert_eq!(
        runtime.block_on(left.resolve(&target_src)),
        Ok(id),
        "폐지 정정 후 활성"
    );
    assert_eq!(
        runtime.block_on(left.history()).map(|events| events.len()),
        Ok(1),
        "사건 1건 기록"
    );
    Some(())
}

pub fn concurrent_double_retirement_correction_has_one_winner<P, R>(
    left: &P,
    right: &P,
    runtime: &R,
) where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    assert!(
        run_double_retirement_corr_race(left, right, runtime).is_some(),
        "이중 폐지 정정 경쟁 계약 완료"
    );
}

fn assert_double_link_corr_state<P, R>(
    repo: &P,
    runtime: &R,
    id: &InstrumentId,
    first_src: &InstrumentSource,
    second_src: &InstrumentSource,
) where
    P: InstrumentMappingRepository + InstrumentMappingHistoryRepository,
    R: ContractRuntime,
{
    assert_eq!(
        runtime.block_on(repo.resolve(first_src)),
        Ok(id.clone()),
        "이전 원본 활성 복구"
    );
    assert_eq!(
        runtime.block_on(repo.resolve(second_src)),
        Err(map_err(InstrumentMappingError::Missing)),
        "분리된 원본 미등록"
    );
    assert_eq!(
        runtime.block_on(repo.history()).map(|events| events.len()),
        Ok(2),
        "사건 2건 기록"
    );
}

fn run_double_link_corr_race<P, R>(left: &P, right: &P, runtime: &R) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let first_src = parse_source(KnownMarket::Us, "AAA", "US0000000301")?;
    let second_src = parse_source(KnownMarket::Us, "BBB", "US0000000302")?;
    let disc = sec_disclosure("0001326801-26-000306")?;
    let note = parse_reason("이중 연결 정정 경쟁")?;
    let stamp = parse_ts(1_764_002_050, 0, 0)?;
    let id = seed_single_link(
        left,
        runtime,
        &first_src,
        &second_src,
        (&disc, &note, stamp),
    )?;
    let ev = MappingCorrectionEvidence::DisclosureAndNote {
        disclosure: disc,
        note,
    };
    let first_undo = runtime.block_on(confirm_link_corr(
        &id,
        &first_src,
        &second_src,
        ev.clone(),
        stamp,
    ))?;
    let second_undo =
        runtime.block_on(confirm_link_corr(&id, &first_src, &second_src, ev, stamp))?;
    let (first_outcome, second_outcome) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.correct_link(first_undo))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.correct_link(second_undo))
        },
    )?;
    let loser = Err(tr_err(MappingTransitionError::Missing));
    let valid = (first_outcome == Ok(id.clone()) && second_outcome == loser)
        || (second_outcome == Ok(id.clone()) && first_outcome == loser);
    assert!(
        valid,
        "이중 연결 정정 경쟁은 승자 1건과 Missing 1건이어야 함: {first_outcome:?}, {second_outcome:?}"
    );
    assert_double_link_corr_state(left, runtime, &id, &first_src, &second_src);
    Some(())
}

pub fn concurrent_double_link_correction_has_one_winner<P, R>(left: &P, right: &P, runtime: &R)
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    assert!(
        run_double_link_corr_race(left, right, runtime).is_some(),
        "이중 연결 정정 경쟁 계약 완료"
    );
}

#[derive(Clone, Copy)]
enum UndoOrChainWinner {
    Undo,
    Chain,
}

fn assert_undo_vs_chain_state<P, R>(
    repo: &P,
    runtime: &R,
    id: &InstrumentId,
    sources: (&InstrumentSource, &InstrumentSource),
    winner: UndoOrChainWinner,
) where
    P: InstrumentMappingRepository + InstrumentMappingHistoryRepository,
    R: ContractRuntime,
{
    match winner {
        UndoOrChainWinner::Undo => {
            assert_eq!(
                runtime.block_on(repo.resolve(sources.0)),
                Ok(id.clone()),
                "정정 승자 시 1차 원본 활성"
            );
        }
        UndoOrChainWinner::Chain => {
            assert_eq!(
                runtime.block_on(repo.resolve(sources.1)),
                Ok(id.clone()),
                "연결 승자 시 3차 원본 활성"
            );
            assert_eq!(
                runtime.block_on(repo.for_id(id)).map(|rows| rows.len()),
                Ok(3),
                "연결 승자 시 원장 3행"
            );
        }
    }
    assert_eq!(
        runtime.block_on(repo.history()).map(|events| events.len()),
        Ok(2),
        "사건 총 2건 기록"
    );
}

fn verify_undo_vs_chain_outcomes<P, R>(
    repo: &P,
    runtime: &R,
    id: &InstrumentId,
    sources: (&InstrumentSource, &InstrumentSource),
    outcomes: (TransitionOutcome, TransitionOutcome),
) where
    P: InstrumentMappingRepository + InstrumentMappingHistoryRepository,
    R: ContractRuntime,
{
    let (undo_outcome, chain_outcome) = outcomes;
    let undo_won = undo_outcome == Ok(id.clone())
        && chain_outcome == Err(tr_err(MappingTransitionError::Missing));
    let chain_won = undo_outcome == Err(tr_err(MappingTransitionError::LinkNotLatest))
        && chain_outcome == Ok(id.clone());
    assert!(
        undo_won || chain_won,
        "연결 정정 대 후속 연결 경쟁은 한쪽만 성공해야 함: {undo_outcome:?}, {chain_outcome:?}"
    );
    let winner = if undo_won {
        UndoOrChainWinner::Undo
    } else {
        UndoOrChainWinner::Chain
    };
    assert_undo_vs_chain_state(repo, runtime, id, sources, winner);
}

fn run_undo_vs_chain_race<P, R>(left: &P, right: &P, runtime: &R) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let first_src = parse_source(KnownMarket::Us, "AAA", "US0000000401")?;
    let second_src = parse_source(KnownMarket::Us, "BBB", "US0000000402")?;
    let third_src = parse_source(KnownMarket::Us, "CCC", "US0000000403")?;
    let disc = sec_disclosure("0001326801-26-000307")?;
    let note = parse_reason("연결 정정 대 후속 연결 경쟁")?;
    let stamp = parse_ts(1_764_002_060, 0, 0)?;
    let id = seed_single_link(
        left,
        runtime,
        &first_src,
        &second_src,
        (&disc, &note, stamp),
    )?;
    let undo_ev = MappingCorrectionEvidence::DisclosureAndNote {
        disclosure: disc.clone(),
        note: note.clone(),
    };
    let chain_spec = SplitChangeSpec {
        id: &id,
        previous: &second_src,
        next: &third_src,
        disc,
        note,
        stamp,
    };
    let undo_req = runtime.block_on(confirm_link_corr(
        &id,
        &first_src,
        &second_src,
        undo_ev,
        stamp,
    ))?;
    let chain_req = build_split_change(runtime, chain_spec)?;
    let outcomes = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.correct_link(undo_req))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.link_confirmed_change(chain_req))
        },
    )?;
    verify_undo_vs_chain_outcomes(left, runtime, &id, (&first_src, &third_src), outcomes);
    Some(())
}

pub fn concurrent_link_correction_vs_chained_link_serializes_cleanly<P, R>(
    left: &P,
    right: &P,
    runtime: &R,
) where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    assert!(
        run_undo_vs_chain_race(left, right, runtime).is_some(),
        "연결 정정 대 후속 연결 경쟁 계약 완료"
    );
}

#[derive(Clone, Copy)]
enum LinkOrEnsureWinner {
    Link,
    Ensure,
}

fn assert_link_vs_ensure_state<P, R>(
    repo: &P,
    runtime: &R,
    ids: (&InstrumentId, &InstrumentId),
    sources: (&InstrumentSource, &InstrumentSource),
    winner: LinkOrEnsureWinner,
) where
    P: InstrumentMappingRepository + InstrumentMappingHistoryRepository,
    R: ContractRuntime,
{
    match winner {
        LinkOrEnsureWinner::Link => {
            assert_eq!(
                runtime.block_on(repo.resolve(sources.1)),
                Ok(ids.0.clone()),
                "연결 승자 시 다음 원본은 기존 ID에 귀속"
            );
            assert_eq!(
                runtime.block_on(repo.resolve(sources.0)),
                Err(map_err(InstrumentMappingError::Superseded)),
                "연결 승자 시 이전 원본은 Superseded"
            );
            assert_eq!(
                runtime.block_on(repo.for_id(ids.1)),
                Err(map_err(InstrumentMappingError::Missing)),
                "연결 승자 시 ensure 후보 ID는 미등록"
            );
            assert_eq!(
                runtime.block_on(repo.history()).map(|events| events.len()),
                Ok(1),
                "연결 승자 사건 1건"
            );
        }
        LinkOrEnsureWinner::Ensure => {
            assert_eq!(
                runtime.block_on(repo.resolve(sources.1)),
                Ok(ids.1.clone()),
                "ensure 승자 시 다음 원본은 신규 ID에 귀속"
            );
            assert_eq!(
                runtime.block_on(repo.resolve(sources.0)),
                Ok(ids.0.clone()),
                "ensure 승자 시 기존 원본 활성 유지"
            );
            assert_eq!(
                runtime.block_on(repo.history()).map(|events| events.len()),
                Ok(0),
                "ensure 승자 사건 0건"
            );
        }
    }
}

fn run_link_and_ensure_next_race<P, R>(left: &P, right: &P, runtime: &R) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let prev_src = parse_source(KnownMarket::Us, "FB", "US30303M1027")?;
    let next_src = parse_source(KnownMarket::Us, "META", "US30303M9999")?;
    let disc = sec_disclosure("0001326801-26-000308")?;
    let note = parse_reason("연결 대 ensure 동일 대상 경쟁")?;
    let stamp = parse_ts(1_764_002_070, 0, 0)?;
    let id_a = seed_one_instrument(left, runtime, &prev_src)?;
    let cand_b = issue_slot(1)?;
    let id_b = cand_b.as_instrument_id().clone();
    let req = build_split_change(
        runtime,
        SplitChangeSpec {
            id: &id_a,
            previous: &prev_src,
            next: &next_src,
            disc,
            note,
            stamp,
        },
    )?;
    let (link_outcome, ensure_outcome) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.link_confirmed_change(req))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.ensure(&next_src, cand_b))
        },
    )?;
    let link_won = link_outcome == Ok(id_a.clone())
        && ensure_outcome == Ok(MappingAssignment::Existing(id_a.clone()));
    let ensure_won = link_outcome == Err(tr_err(MappingTransitionError::SourceBoundToOtherId))
        && ensure_outcome == Ok(MappingAssignment::Issued(id_b.clone()));
    assert!(
        link_won || ensure_won,
        "연결 대 ensure 동일 대상 경쟁은 한쪽만 선점해야 함: {link_outcome:?}, {ensure_outcome:?}"
    );
    let winner = if link_won {
        LinkOrEnsureWinner::Link
    } else {
        LinkOrEnsureWinner::Ensure
    };
    assert_link_vs_ensure_state(
        left,
        runtime,
        (&id_a, &id_b),
        (&prev_src, &next_src),
        winner,
    );
    Some(())
}

pub fn concurrent_link_and_ensure_next_has_one_winner<P, R>(left: &P, right: &P, runtime: &R)
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    assert!(
        run_link_and_ensure_next_race(left, right, runtime).is_some(),
        "연결 대 ensure 동일 대상 경쟁 계약 완료"
    );
}

fn run_link_and_ensure_overlap_race<P, R>(left: &P, right: &P, runtime: &R) -> Option<()>
where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    let prev_src = parse_source(KnownMarket::Us, "FB", "US30303M1027")?;
    let next_a = parse_source(KnownMarket::Us, "OVLP", "US0000000591")?;
    let next_b = parse_source(KnownMarket::Us, "ovlp", "US0000000592")?;
    let disc = sec_disclosure("0001326801-26-000309")?;
    let note = parse_reason("연결 대 ensure 부분 겹침 경쟁")?;
    let stamp = parse_ts(1_764_002_080, 0, 0)?;
    let id_a = seed_one_instrument(left, runtime, &prev_src)?;
    let cand_b = issue_slot(1)?;
    let id_b = cand_b.as_instrument_id().clone();
    let req = build_split_change(
        runtime,
        SplitChangeSpec {
            id: &id_a,
            previous: &prev_src,
            next: &next_a,
            disc,
            note,
            stamp,
        },
    )?;
    let (link_outcome, ensure_outcome) = race_pair(
        |barrier| {
            barrier.wait();
            runtime.block_on(left.link_confirmed_change(req))
        },
        |barrier| {
            barrier.wait();
            runtime.block_on(right.ensure(&next_b, cand_b))
        },
    )?;
    let link_won = link_outcome == Ok(id_a.clone())
        && ensure_outcome == Err(map_err(InstrumentMappingError::IdentityUncertain));
    let ensure_won = link_outcome == Err(tr_err(MappingTransitionError::IdentityUncertain))
        && ensure_outcome == Ok(MappingAssignment::Issued(id_b.clone()));
    assert!(
        link_won || ensure_won,
        "연결 대 ensure 부분 겹침 경쟁은 승자 1건과 IdentityUncertain 1건이어야 함: {link_outcome:?}, {ensure_outcome:?}"
    );
    let (winner_src, winner_id, expected_events) = if link_won {
        (&next_a, &id_a, 1_usize)
    } else {
        (&next_b, &id_b, 0_usize)
    };
    assert_eq!(
        runtime.block_on(left.resolve(winner_src)),
        Ok(winner_id.clone()),
        "부분 겹침 경쟁 승자 원본 활성"
    );
    assert_eq!(
        runtime.block_on(left.history()).map(|events| events.len()),
        Ok(expected_events),
        "부분 겹침 경쟁 사건 수 일치"
    );
    Some(())
}

pub fn concurrent_link_and_ensure_overlapping_next_has_one_winner<P, R>(
    left: &P,
    right: &P,
    runtime: &R,
) where
    P: InstrumentMappingRepository
        + InstrumentMappingHistoryRepository
        + InstrumentMappingCorrectionRepository,
    R: ContractRuntime,
{
    assert!(
        run_link_and_ensure_overlap_race(left, right, runtime).is_some(),
        "연결 대 ensure 부분 겹침 경쟁 계약 완료"
    );
}
