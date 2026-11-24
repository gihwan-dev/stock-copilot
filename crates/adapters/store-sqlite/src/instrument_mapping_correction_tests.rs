use crate::{
    Database, DatabasePath, InstrumentMappingStore,
    sql::{
        instrument_mapping as sql, instrument_mapping_history_tests as history_sql,
        instrument_mapping_tests as fixtures,
    },
    tests::{Fixture, clock, key, runtime},
};
use application::{
    ConfirmInstrumentIdentity, IdentifierChangePrompt, IdentityConfirmationDecision,
    IdentityConfirmationOutcome, InstrumentMappingCorrectionRepository,
    InstrumentMappingHistoryRepository, InstrumentMappingRepository, LinkCorrectionPrompt,
    MappingTransitionStoreError, RetirementCorrectionPrompt, RetryDelay, SystemFailure, SystemPort,
    SystemPortError,
};
use domain::{
    CalendarDate, CalendarYear, ConfirmedIdentifierChange, ConfirmedLinkCorrection,
    ConfirmedRetirementCorrection, CorporateAction, CorporateActionId, CorporateActionKind,
    DartReceiptNo, DayNumber, DisclosureKey, IdentityChangeConfirmation, IdentityChangeEvidence,
    IdentityConfirmationOrigin, IndependentInstrumentIdIssuer, InstrumentId,
    InstrumentMappingRecord, InstrumentMappingStatus, InstrumentSource, IsinCode,
    IssuedInstrumentId, KindReceiptNo, KnownMarket, MappingCorrectionConfirmation,
    MappingCorrectionEvidence, MappingHistoryEvent, MappingTransitionError, MonthNumber,
    Nanosecond, OffsetSeconds, ReceiptNo, SecEdgarReceiptNo, ShareCount, ShareRatio, Symbol,
    Timestamp, UnixSeconds, UntrustedText, UserReason,
};
use rusqlite::params;
use std::{future::Future, sync::mpsc, time::Duration};
use testkit_ports::{
    IdentityConfirmationIssuanceKind, ScriptedIdentityConfirmation,
    ScriptedIdentityConfirmationIssuer, SteppedClock,
    contracts::{
        instrument_mapping::ContractRuntime,
        instrument_mapping_correction::{
            concurrent_cross_id_overlapping_next_link_has_one_winner,
            concurrent_cross_id_same_next_link_has_one_winner,
            concurrent_double_link_correction_has_one_winner,
            concurrent_double_retirement_correction_has_one_winner,
            concurrent_link_and_ensure_next_has_one_winner,
            concurrent_link_and_ensure_overlapping_next_has_one_winner,
            concurrent_link_and_retire_serializes_cleanly,
            concurrent_link_correction_vs_chained_link_serializes_cleanly,
            concurrent_same_previous_link_has_one_winner,
            correction_lifecycle_and_conflict_rules_contract,
            unpolled_correction_writes_do_not_mutate_contract,
        },
    },
    memory::{MemoryInstrumentMappingRepository, MemoryInstrumentMappingStore},
};

struct TokioRuntime;

impl ContractRuntime for TokioRuntime {
    fn block_on<F: Future>(&self, future: F) -> F::Output {
        runtime().block_on(future)
    }
}

fn open_fixture(fixture: &Fixture) -> Database {
    Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open database")
}

fn source(market: KnownMarket, symbol: &str, isin: &str) -> InstrumentSource {
    InstrumentSource::new(
        market,
        Symbol::try_new(symbol).expect("symbol"),
        IsinCode::try_new(isin).expect("isin"),
    )
}

fn issued_slot(slot: u16) -> IssuedInstrumentId {
    let mut issuer = IndependentInstrumentIdIssuer::new();
    for _ in 0..slot {
        let _skip = issuer.issue().expect("skip slot");
    }
    issuer.issue().expect("issue slot")
}

fn reason(text: &str) -> UserReason {
    UserReason::try_new(UntrustedText::try_new(text).expect("untrusted")).expect("reason")
}

fn ts(seconds: i64, nanos: u32, offset: i32) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(nanos),
        OffsetSeconds::new(offset),
    )
    .expect("timestamp")
}

fn date(day: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(2026),
        MonthNumber::new(11),
        DayNumber::new(day),
    )
    .expect("date")
}

fn dart_disclosure(raw: &str) -> DisclosureKey {
    DisclosureKey::from_receipt(ReceiptNo::Dart(DartReceiptNo::try_new(raw).expect("dart")))
}

fn kind_disclosure(raw: &str) -> DisclosureKey {
    DisclosureKey::from_receipt(ReceiptNo::Kind(KindReceiptNo::try_new(raw).expect("kind")))
}

fn sec_disclosure(raw: &str) -> DisclosureKey {
    DisclosureKey::from_receipt(ReceiptNo::SecEdgar(
        SecEdgarReceiptNo::try_new(raw).expect("sec"),
    ))
}

fn split_action(id: &InstrumentId, day: u8, old: u64, new: u64) -> CorporateAction {
    CorporateAction::try_new(
        CorporateActionId::new(id.clone()),
        date(day),
        CorporateActionKind::Split(ShareRatio::new(
            ShareCount::try_new(old).expect("old"),
            ShareCount::try_new(new).expect("new"),
        )),
    )
    .expect("split")
}

fn reverse_split_action(id: &InstrumentId, day: u8, old: u64, new: u64) -> CorporateAction {
    CorporateAction::try_new(
        CorporateActionId::new(id.clone()),
        date(day),
        CorporateActionKind::ReverseSplit(ShareRatio::new(
            ShareCount::try_new(old).expect("old"),
            ShareCount::try_new(new).expect("new"),
        )),
    )
    .expect("reverse split")
}

fn symbol_action(id: &InstrumentId, day: u8, from: &str, to: &str) -> CorporateAction {
    CorporateAction::try_new(
        CorporateActionId::new(id.clone()),
        date(day),
        CorporateActionKind::SymbolChange {
            from: Symbol::try_new(from).expect("from"),
            to: Symbol::try_new(to).expect("to"),
        },
    )
    .expect("symbol change")
}

fn confirm_change(
    rt: &tokio::runtime::Runtime,
    id: &InstrumentId,
    pair: (&InstrumentSource, &InstrumentSource),
    evidence: IdentityChangeEvidence,
    at: Timestamp,
) -> ConfirmedIdentifierChange {
    let prompt =
        ScriptedIdentityConfirmation::new([IdentityConfirmationDecision::Confirmed], [], []);
    let clock_stub = SteppedClock::new(at);
    let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock_stub);
    let req = IdentifierChangePrompt::new(id.clone(), pair.0.clone(), pair.1.clone(), evidence);
    match rt.block_on(use_case.confirm_identifier_change(req)) {
        IdentityConfirmationOutcome::Confirmed(value) => value,
        IdentityConfirmationOutcome::Denied | IdentityConfirmationOutcome::Unavailable => {
            panic!("confirmed expected")
        }
    }
}

fn confirm_retire(
    rt: &tokio::runtime::Runtime,
    id: &InstrumentId,
    target: &InstrumentSource,
    evidence: MappingCorrectionEvidence,
    at: Timestamp,
) -> ConfirmedRetirementCorrection {
    let prompt =
        ScriptedIdentityConfirmation::new([], [IdentityConfirmationDecision::Confirmed], []);
    let clock_stub = SteppedClock::new(at);
    let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock_stub);
    let req = RetirementCorrectionPrompt::new(id.clone(), target.clone(), evidence);
    match rt.block_on(use_case.confirm_retirement_correction(req)) {
        IdentityConfirmationOutcome::Confirmed(value) => value,
        IdentityConfirmationOutcome::Denied | IdentityConfirmationOutcome::Unavailable => {
            panic!("confirmed expected")
        }
    }
}

fn confirm_link_undo(
    rt: &tokio::runtime::Runtime,
    id: &InstrumentId,
    pair: (&InstrumentSource, &InstrumentSource),
    evidence: MappingCorrectionEvidence,
    at: Timestamp,
) -> ConfirmedLinkCorrection {
    let prompt =
        ScriptedIdentityConfirmation::new([], [], [IdentityConfirmationDecision::Confirmed]);
    let clock_stub = SteppedClock::new(at);
    let use_case = ConfirmInstrumentIdentity::new(&prompt, &clock_stub);
    let req = LinkCorrectionPrompt::new(id.clone(), pair.0.clone(), pair.1.clone(), evidence);
    match rt.block_on(use_case.confirm_link_correction(req)) {
        IdentityConfirmationOutcome::Confirmed(value) => value,
        IdentityConfirmationOutcome::Denied | IdentityConfirmationOutcome::Unavailable => {
            panic!("confirmed expected")
        }
    }
}

fn integrity_err() -> MappingTransitionStoreError {
    MappingTransitionStoreError::Storage(SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::RepositoryIntegrity,
    ))
}

fn unavailable_err() -> MappingTransitionStoreError {
    MappingTransitionStoreError::Storage(SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::Unavailable(RetryDelay::Unknown),
    ))
}

fn event_count(rt: &tokio::runtime::Runtime, db: &Database) -> i64 {
    rt.block_on(db.read(|conn| conn.query_row(history_sql::COUNT_EVENTS, [], |row| row.get(0))))
        .expect("count events")
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

fn wait_contender_blocked(started_rx: &mpsc::Receiver<()>, done_rx: &mpsc::Receiver<()>) {
    started_rx.recv().expect("contender started");
    assert_eq!(
        done_rx.recv_timeout(Duration::from_millis(80)),
        Err(mpsc::RecvTimeoutError::Timeout),
        "contender must block on BEGIN IMMEDIATE lock"
    );
}

#[test]
fn sqlite_store_passes_shared_correction_lifecycle_and_reopen_roundtrip() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&db);

    rt.block_on(correction_lifecycle_and_conflict_rules_contract(&store));

    let before_history = rt.block_on(store.history()).expect("history before close");
    let us_id = issued_slot(0).as_instrument_id().clone();
    let kr_id = issued_slot(1).as_instrument_id().clone();
    let before_us_history = rt
        .block_on(store.history_for_id(&us_id))
        .expect("us history before close");
    let before_kr_history = rt
        .block_on(store.history_for_id(&kr_id))
        .expect("kr history before close");
    let before_us_rows = rt.block_on(store.for_id(&us_id)).expect("us rows");
    let before_kr_rows = rt.block_on(store.for_id(&kr_id)).expect("kr rows");
    rt.block_on(db.close()).expect("close");

    let reopened = open_fixture(&fixture);
    let reopened_store = InstrumentMappingStore::new(&reopened);
    assert_eq!(rt.block_on(reopened_store.history()), Ok(before_history));
    assert_eq!(
        rt.block_on(reopened_store.history_for_id(&us_id)),
        Ok(before_us_history)
    );
    assert_eq!(
        rt.block_on(reopened_store.history_for_id(&kr_id)),
        Ok(before_kr_history)
    );
    assert_eq!(
        rt.block_on(reopened_store.for_id(&us_id)),
        Ok(before_us_rows)
    );
    assert_eq!(
        rt.block_on(reopened_store.for_id(&kr_id)),
        Ok(before_kr_rows)
    );
    rt.block_on(reopened.close()).expect("close reopened");
}

#[test]
fn sqlite_store_passes_shared_unpolled_correction_drop_contract() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&db);
    rt.block_on(unpolled_correction_writes_do_not_mutate_contract(&store));
    rt.block_on(db.close()).expect("close");
    let reopened = open_fixture(&fixture);
    rt.block_on(reopened.close()).expect("close reopened");
}

fn run_sqlite_race<F>(race: F)
where
    F: for<'a> FnOnce(&InstrumentMappingStore<'a>, &InstrumentMappingStore<'a>, &TokioRuntime),
{
    let fixture = Fixture::new();
    let first_db = open_fixture(&fixture);
    let second_db = open_fixture(&fixture);
    let left = InstrumentMappingStore::new(&first_db);
    let right = InstrumentMappingStore::new(&second_db);
    race(&left, &right, &TokioRuntime);
    TokioRuntime
        .block_on(first_db.close())
        .expect("close first");
    TokioRuntime
        .block_on(second_db.close())
        .expect("close second");
    let reopened = open_fixture(&fixture);
    TokioRuntime
        .block_on(reopened.close())
        .expect("close reopened");
}

#[test]
fn sqlite_store_passes_all_seven_two_connection_correction_races_and_reopen() {
    run_sqlite_race(|left, right, rt| {
        concurrent_same_previous_link_has_one_winner(left, right, rt);
    });
    run_sqlite_race(|left, right, rt| {
        concurrent_cross_id_same_next_link_has_one_winner(left, right, rt);
    });
    run_sqlite_race(|left, right, rt| {
        concurrent_cross_id_overlapping_next_link_has_one_winner(left, right, rt);
    });
    run_sqlite_race(|left, right, rt| {
        concurrent_link_and_retire_serializes_cleanly(left, right, rt);
    });
    run_sqlite_race(|left, right, rt| {
        concurrent_double_retirement_correction_has_one_winner(left, right, rt);
    });
    run_sqlite_race(|left, right, rt| {
        concurrent_double_link_correction_has_one_winner(left, right, rt);
    });
    run_sqlite_race(|left, right, rt| {
        concurrent_link_correction_vs_chained_link_serializes_cleanly(left, right, rt);
    });
}

#[test]
fn sqlite_store_passes_two_connection_link_and_ensure_races_and_reopen() {
    run_sqlite_race(|left, right, rt| {
        concurrent_link_and_ensure_next_has_one_winner(left, right, rt);
    });
    run_sqlite_race(|left, right, rt| {
        concurrent_link_and_ensure_overlapping_next_has_one_winner(left, right, rt);
    });
}

#[test]
fn sqlite_link_confirmed_change_rolls_back_atomically_on_mid_transaction_faults() {
    let rt = runtime();
    for (fault_sql, next_sym) in [
        (history_sql::FAULT_ON_MARK_SUPERSEDED, "META"),
        (history_sql::FAULT_ON_INSERT_NEXT_ACTIVE, "FAILNEXT"),
    ] {
        let fixture = Fixture::new();
        let db = open_fixture(&fixture);
        let store = InstrumentMappingStore::new(&db);
        let prev = source(KnownMarket::Us, "FB", "US30303M1027");
        let next = source(KnownMarket::Us, next_sym, "US30303M1027");
        let cand = issued_slot(0);
        let id = cand.as_instrument_id().clone();
        rt.block_on(store.ensure(&prev, cand)).expect("seed");
        rt.block_on(db.write(move |tx| tx.execute_batch(fault_sql)))
            .expect("install fault");

        let change = confirm_change(
            &rt,
            &id,
            (&prev, &next),
            IdentityChangeEvidence::DisclosureAndAction {
                disclosure: sec_disclosure("0001326801-26-000501"),
                action: split_action(&id, 12, 1, 2),
                note: reason("중간 실패 롤백 검증"),
            },
            ts(1_764_005_000, 0, 0),
        );
        assert_eq!(
            rt.block_on(store.link_confirmed_change(change)),
            Err(integrity_err())
        );
        assert_eq!(event_count(&rt, &db), 0, "사건이 롤백되어 0건이어야 함");
        assert_eq!(rt.block_on(store.resolve(&prev)), Ok(id.clone()));
        assert_eq!(
            rt.block_on(store.for_id(&id)),
            Ok(vec![InstrumentMappingRecord::new(
                prev,
                id,
                InstrumentMappingStatus::Active
            )])
        );

        rt.block_on(db.write(|tx| tx.execute_batch(history_sql::DROP_FAULT_TRIGGER)))
            .expect("remove fault");
        rt.block_on(db.close()).expect("close");
        let reopened = open_fixture(&fixture);
        rt.block_on(reopened.close()).expect("reopen clean");
    }
}

#[test]
fn sqlite_retirement_correction_rolls_back_atomically_on_mid_transaction_fault() {
    let rt = runtime();
    let fixture = Fixture::new();
    let db = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&db);
    let s1 = source(KnownMarket::Kr, "005930", "KR7005930003");
    let cand = issued_slot(0);
    let id = cand.as_instrument_id().clone();
    rt.block_on(store.ensure(&s1, cand)).expect("seed");
    rt.block_on(store.retire(&s1)).expect("retire");

    rt.block_on(db.write(|tx| tx.execute_batch(history_sql::FAULT_ON_RESTORE_RETIRED)))
        .expect("install retire fault");
    let retire_corr = confirm_retire(
        &rt,
        &id,
        &s1,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: dart_disclosure("20261112000502"),
            note: reason("폐지 정정 롤백"),
        },
        ts(1_764_005_100, 0, 32_400),
    );
    assert_eq!(
        rt.block_on(store.correct_retirement(retire_corr)),
        Err(integrity_err())
    );
    assert_eq!(event_count(&rt, &db), 0);
    assert_eq!(
        rt.block_on(store.lookup(&s1)),
        Ok(InstrumentMappingRecord::new(
            s1,
            id,
            InstrumentMappingStatus::Retired
        ))
    );
    rt.block_on(db.write(|tx| tx.execute_batch(history_sql::DROP_FAULT_TRIGGER)))
        .expect("drop fault");
    rt.block_on(db.close()).expect("close");
    let reopened = open_fixture(&fixture);
    rt.block_on(reopened.close()).expect("reopen clean");
}

#[test]
fn sqlite_link_correction_rolls_back_atomically_on_mid_transaction_faults() {
    let rt = runtime();
    let fixture = Fixture::new();
    let db = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&db);
    let s1 = source(KnownMarket::Kr, "005930", "KR7005930003");
    let s2 = source(KnownMarket::Kr, "005931", "KR7005930999");
    let cand = issued_slot(0);
    let id = cand.as_instrument_id().clone();
    rt.block_on(store.ensure(&s1, cand)).expect("seed");
    let ok_link = confirm_change(
        &rt,
        &id,
        (&s1, &s2),
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: kind_disclosure("20261112000504"),
            action: split_action(&id, 12, 1, 2),
            note: reason("정상 연결"),
        },
        ts(1_764_005_120, 0, 32_400),
    );
    rt.block_on(store.link_confirmed_change(ok_link))
        .expect("link");

    for fault in [
        history_sql::FAULT_ON_DELETE_UNLINKED,
        history_sql::FAULT_ON_RESTORE_SUPERSEDED,
    ] {
        rt.block_on(db.write(move |tx| tx.execute_batch(fault)))
            .expect("install link undo fault");
        let undo = confirm_link_undo(
            &rt,
            &id,
            (&s1, &s2),
            MappingCorrectionEvidence::DisclosureAndNote {
                disclosure: dart_disclosure("20261112000505"),
                note: reason("연결 정정 롤백"),
            },
            ts(1_764_005_130, 0, 32_400),
        );
        assert_eq!(rt.block_on(store.correct_link(undo)), Err(integrity_err()));
        assert_eq!(event_count(&rt, &db), 1, "연결 정정 사건 롤백");
        assert_eq!(rt.block_on(store.resolve(&s2)), Ok(id.clone()));
        assert_eq!(
            rt.block_on(store.lookup(&s1)),
            Ok(InstrumentMappingRecord::new(
                s1.clone(),
                id.clone(),
                InstrumentMappingStatus::Superseded
            ))
        );
        rt.block_on(db.write(|tx| tx.execute_batch(history_sql::DROP_FAULT_TRIGGER)))
            .expect("drop fault");
    }

    rt.block_on(db.close()).expect("close");
    let reopened = open_fixture(&fixture);
    rt.block_on(reopened.close()).expect("reopen clean");
}

fn seed_other_event_seq_two(rt: &tokio::runtime::Runtime, store: &InstrumentMappingStore<'_>) {
    let other_src = source(KnownMarket::Kr, "005930", "KR7005930003");
    let other_cand = issued_slot(1);
    let other_id = other_cand.as_instrument_id().clone();
    rt.block_on(store.ensure(&other_src, other_cand))
        .expect("seed other");
    rt.block_on(store.retire(&other_src)).expect("retire other");
    let other_restore = confirm_retire(
        rt,
        &other_id,
        &other_src,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: dart_disclosure("20261113000525"),
            note: reason("타 종목 사건 seq=2 시드"),
        },
        ts(1_764_005_205, 0, 32_400),
    );
    rt.block_on(store.correct_retirement(other_restore))
        .expect("seed seq 2");
}

#[test]
fn sqlite_correct_link_rolls_back_when_superseded_event_seq_drifted() {
    let rt = runtime();
    let fixture = Fixture::new();
    let db = open_fixture(&fixture);
    let store = InstrumentMappingStore::new(&db);
    let s1 = source(KnownMarket::Us, "FB", "US30303M1027");
    let s2 = source(KnownMarket::Us, "META", "US30303M9999");
    let cand = issued_slot(0);
    let id = cand.as_instrument_id().clone();
    rt.block_on(store.ensure(&s1, cand)).expect("seed");
    let ch = confirm_change(
        &rt,
        &id,
        (&s1, &s2),
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: sec_disclosure("0001326801-26-000520"),
            action: split_action(&id, 13, 1, 2),
            note: reason("링크 시드"),
        },
        ts(1_764_005_200, 0, 0),
    );
    rt.block_on(store.link_confirmed_change(ch)).expect("link");
    seed_other_event_seq_two(&rt, &store);

    rt.block_on(db.write(|tx| {
        tx.execute_batch(fixtures::DROP_GUARD_TRIGGERS)?;
        tx.execute(
            history_sql::UPDATE_SUPERSEDED_SEQ_DIRECT,
            params!["inst-0000000000000001", "US", "FB", "US30303M1027", 2_i64],
        )?;
        tx.execute_batch(fixtures::RESTORE_GUARD_TRIGGERS)?;
        Ok(())
    }))
    .expect("corrupt superseded_event_seq");

    let undo = confirm_link_undo(
        &rt,
        &id,
        (&s1, &s2),
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: dart_disclosure("20261113000521"),
            note: reason("불일치 seq 롤백"),
        },
        ts(1_764_005_210, 0, 32_400),
    );
    assert_eq!(rt.block_on(store.correct_link(undo)), Err(integrity_err()));
    assert_eq!(event_count(&rt, &db), 2, "드리프트 롤백으로 이벤트 미추가");
    assert_eq!(rt.block_on(store.resolve(&s2)), Ok(id));
    rt.block_on(db.close()).expect("close");
}

fn seed_orphan_superseded_db(rt: &tokio::runtime::Runtime, db: &Database) {
    rt.block_on(db.write(|tx| {
        tx.execute_batch(fixtures::RECREATE_UNCONSTRAINED_STRICT)?;
        tx.execute(
            history_sql::INSERT_WITH_SUPERSEDED_SEQ,
            params![
                "inst-0000000000000001",
                "US",
                "FB",
                "US30303M1027",
                "superseded",
                1_i64
            ],
        )?;
        Ok(())
    }))
    .expect("seed orphan superseded without terminal");
}

#[test]
fn sqlite_correction_writes_reject_orphan_superseded_ledger() {
    let rt = runtime();
    let s1 = source(KnownMarket::Us, "FB", "US30303M1027");
    let s2 = source(KnownMarket::Us, "META", "US30303M9999");
    let id = issued_slot(0).as_instrument_id().clone();
    let orphan_fixture = Fixture::new();
    let orphan_db = open_fixture(&orphan_fixture);
    let orphan_store = InstrumentMappingStore::new(&orphan_db);
    seed_orphan_superseded_db(&rt, &orphan_db);

    let bad_ch = confirm_change(
        &rt,
        &id,
        (&s1, &s2),
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: sec_disclosure("0001326801-26-000522"),
            action: split_action(&id, 13, 1, 2),
            note: reason("고아 검증"),
        },
        ts(1_764_005_220, 0, 0),
    );
    assert_eq!(
        rt.block_on(orphan_store.link_confirmed_change(bad_ch)),
        Err(integrity_err())
    );
    let bad_ret = confirm_retire(
        &rt,
        &id,
        &s1,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: dart_disclosure("20261113000523"),
            note: reason("고아 폐지 정정 검증"),
        },
        ts(1_764_005_230, 0, 32_400),
    );
    assert_eq!(
        rt.block_on(orphan_store.correct_retirement(bad_ret)),
        Err(integrity_err())
    );
    let bad_undo = confirm_link_undo(
        &rt,
        &id,
        (&s1, &s2),
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: dart_disclosure("20261113000524"),
            note: reason("고아 연결 정정 검증"),
        },
        ts(1_764_005_240, 0, 32_400),
    );
    assert_eq!(
        rt.block_on(orphan_store.correct_link(bad_undo)),
        Err(integrity_err())
    );
    rt.block_on(orphan_db.close()).expect("close orphan");
}

#[test]
fn sqlite_correction_writes_reject_overflowing_and_non_positive_event_seq() {
    let rt = runtime();
    for bad_seq in [i64::MAX, 0_i64] {
        let fixture = Fixture::new();
        let db = open_fixture(&fixture);
        let store = InstrumentMappingStore::new(&db);
        let s1 = source(KnownMarket::Kr, "005930", "KR7005930003");
        let cand = issued_slot(0);
        let id = cand.as_instrument_id().clone();
        rt.block_on(store.ensure(&s1, cand)).expect("seed");
        rt.block_on(store.retire(&s1)).expect("retire");
        rt.block_on(db.write(move |tx| {
            tx.execute_batch(history_sql::RECREATE_UNCONSTRAINED_EVENTS)?;
            tx.execute(
                history_sql::INSERT_EVENT,
                params![
                    bad_seq,
                    "retirement_corrected",
                    "inst-0000000000000099",
                    "KR",
                    "005939",
                    "KR7005930991",
                    Option::<&str>::None,
                    Option::<&str>::None,
                    "DART",
                    "20261113000530",
                    Option::<&str>::None,
                    Option::<i64>::None,
                    Option::<i64>::None,
                    Option::<i64>::None,
                    Option::<&str>::None,
                    Option::<&str>::None,
                    Option::<&str>::None,
                    Option::<&str>::None,
                    "오버플로 시드",
                    1_764_005_300_i64,
                    0_i64,
                    32_400_i64,
                ],
            )?;
            Ok(())
        }))
        .expect("seed extreme seq");

        let corr = confirm_retire(
            &rt,
            &id,
            &s1,
            MappingCorrectionEvidence::DisclosureAndNote {
                disclosure: dart_disclosure("20261113000531"),
                note: reason("순번 한계 거부"),
            },
            ts(1_764_005_310, 0, 32_400),
        );
        assert_eq!(
            rt.block_on(store.correct_retirement(corr)),
            Err(integrity_err())
        );
        assert_eq!(event_count(&rt, &db), 1);
        rt.block_on(db.close()).expect("close");
    }
}

fn run_blocked_contender(
    rt: &tokio::runtime::Runtime,
    second_db: &Database,
    change: ConfirmedIdentifierChange,
    holder_signals: (
        mpsc::Receiver<()>,
        mpsc::Sender<()>,
        tokio::sync::oneshot::Receiver<Result<(), SystemPortError>>,
    ),
) -> Result<InstrumentId, MappingTransitionStoreError> {
    let (locked_rx, release_tx, holder) = holder_signals;
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    std::thread::scope(|scope| {
        locked_rx.recv().expect("holder holds lock");
        let handle = scope.spawn(move || {
            let local_rt = runtime();
            let store = InstrumentMappingStore::new(second_db);
            started_tx.send(()).expect("started");
            let out = local_rt.block_on(store.link_confirmed_change(change));
            done_tx.send(()).expect("done");
            out
        });
        wait_contender_blocked(&started_rx, &done_rx);
        release_tx.send(()).expect("commit holder");
        rt.block_on(holder).expect("recv").expect("commit");
        done_rx.recv().expect("contender finished");
        handle.join().expect("join")
    })
}

#[test]
fn sqlite_correction_writes_serialize_immediate_lock() {
    let rt = runtime();
    let fixture = Fixture::new();
    let first_db = open_fixture(&fixture);
    let second_db = open_fixture(&fixture);
    let s1 = source(KnownMarket::Us, "FB", "US30303M1027");
    let s2 = source(KnownMarket::Us, "META", "US30303M1027");
    let cand = issued_slot(0);
    let id = cand.as_instrument_id().clone();
    let first_store = InstrumentMappingStore::new(&first_db);
    rt.block_on(first_store.ensure(&s1, cand)).expect("seed");

    let (locked_tx, locked_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let holder = first_db
        .writer
        .submit(move |tx| {
            tx.execute(
                sql::RETIRE,
                params!["inst-0000000000000001", "US", "FB", "US30303M1027"],
            )?;
            locked_tx.send(()).expect("lock held");
            release_rx.recv().expect("release");
            Ok(())
        })
        .expect("submit holder");

    let contender_change = confirm_change(
        &rt,
        &id,
        (&s1, &s2),
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: sec_disclosure("0001326801-26-000540"),
            action: symbol_action(&id, 14, "FB", "META"),
            note: reason("잠금 대기 후 거부"),
        },
        ts(1_764_005_400, 0, 0),
    );
    let contender_outcome = run_blocked_contender(
        &rt,
        &second_db,
        contender_change,
        (locked_rx, release_tx, holder),
    );
    assert_eq!(
        contender_outcome,
        Err(MappingTransitionStoreError::Transition(
            MappingTransitionError::SourceNotActive
        ))
    );
    assert_eq!(event_count(&rt, &first_db), 0);
    rt.block_on(first_db.close()).expect("close first");
    rt.block_on(second_db.close()).expect("close second");
    let reopened = open_fixture(&fixture);
    rt.block_on(reopened.close()).expect("reopen clean");
}

fn assert_all_three_writes_unavailable(
    rt: &tokio::runtime::Runtime,
    store: &InstrumentMappingStore<'_>,
    id: &InstrumentId,
    pair: (&InstrumentSource, &InstrumentSource),
) {
    let timeout_ch = confirm_change(
        rt,
        id,
        pair,
        IdentityChangeEvidence::DisclosureAndAction {
            disclosure: sec_disclosure("0001326801-26-000541"),
            action: split_action(id, 14, 1, 2),
            note: reason("잠금 시간 초과 연결"),
        },
        ts(1_764_005_410, 0, 0),
    );
    assert_eq!(
        rt.block_on(store.link_confirmed_change(timeout_ch)),
        Err(unavailable_err())
    );
    let timeout_ret = confirm_retire(
        rt,
        id,
        pair.0,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: dart_disclosure("20261114000542"),
            note: reason("잠금 시간 초과 폐지 정정"),
        },
        ts(1_764_005_420, 0, 32_400),
    );
    assert_eq!(
        rt.block_on(store.correct_retirement(timeout_ret)),
        Err(unavailable_err())
    );
    let timeout_undo = confirm_link_undo(
        rt,
        id,
        pair,
        MappingCorrectionEvidence::DisclosureAndNote {
            disclosure: kind_disclosure("20261114000543"),
            note: reason("잠금 시간 초과 연결 정정"),
        },
        ts(1_764_005_430, 0, 32_400),
    );
    assert_eq!(
        rt.block_on(store.correct_link(timeout_undo)),
        Err(unavailable_err())
    );
}

#[test]
fn sqlite_correction_writes_report_unavailable_on_lock_timeout() {
    let rt = runtime();
    let fixture = Fixture::new();
    let first_db = open_fixture(&fixture);
    let second_db = open_fixture(&fixture);
    let s1 = source(KnownMarket::Us, "FB", "US30303M1027");
    let s2 = source(KnownMarket::Us, "META", "US30303M1027");
    let cand = issued_slot(0);
    let id = cand.as_instrument_id().clone();
    let first_store = InstrumentMappingStore::new(&first_db);
    rt.block_on(first_store.ensure(&s1, cand)).expect("seed");

    rt.block_on(second_db.write(|tx| tx.execute_batch(fixtures::ZERO_BUSY_TIMEOUT)))
        .expect("zero busy timeout");
    let (busy_lock_tx, busy_lock_rx) = mpsc::channel();
    let (busy_rel_tx, busy_rel_rx) = mpsc::channel();
    let busy_holder = first_db
        .writer
        .submit(move |tx| {
            let _count: i64 = tx.query_row(history_sql::COUNT_EVENTS, [], |row| row.get(0))?;
            busy_lock_tx.send(()).expect("busy lock held");
            busy_rel_rx.recv().expect("busy release");
            Ok(())
        })
        .expect("busy holder");
    busy_lock_rx.recv().expect("busy lock acquired");

    let second_store = InstrumentMappingStore::new(&second_db);
    assert_all_three_writes_unavailable(&rt, &second_store, &id, (&s1, &s2));

    busy_rel_tx.send(()).expect("release busy holder");
    rt.block_on(busy_holder).expect("recv").expect("commit");
    assert_eq!(event_count(&rt, &first_db), 0);
    rt.block_on(first_db.close()).expect("close first");
    rt.block_on(second_db.close()).expect("close second");
    let reopened = open_fixture(&fixture);
    rt.block_on(reopened.close()).expect("reopen clean");
}

fn assert_plan_uses_index(
    rt: &tokio::runtime::Runtime,
    db: &Database,
    query: &'static str,
    expected_index: &str,
) {
    let details = rt
        .block_on(db.read(move |conn| {
            let mut stmt = conn.prepare(query)?;
            stmt.query_map([], |row| row.get::<_, String>(3))?
                .collect::<rusqlite::Result<Vec<_>>>()
        }))
        .expect("explain");
    assert!(
        details.iter().any(|line| line.contains(expected_index)),
        "expected index {expected_index} in plan {details:?}"
    );
    assert!(
        details
            .iter()
            .all(|line| !line.starts_with("SCAN instrument_mappings")),
        "unexpected full scan in plan {details:?}"
    );
}

#[test]
fn sqlite_phase2_sql_prepares_and_uses_indexes_without_table_scan() {
    let fixture = Fixture::new();
    let rt = runtime();
    let db = open_fixture(&fixture);

    rt.block_on(db.read(|conn| {
        for statement in [
            sql::BY_TWO_SOURCES_OR_ID,
            sql::MAX_EVENT_SEQ,
            sql::INSERT_EVENT,
            sql::MARK_SUPERSEDED,
            sql::RESTORE_RETIRED,
            sql::DELETE_UNLINKED_ACTIVE,
            sql::RESTORE_SUPERSEDED,
        ] {
            conn.prepare(statement)?;
        }
        Ok(())
    }))
    .expect("all phase2 SQL prepares");

    for (query, expected_index) in [
        (
            history_sql::EXPLAIN_BY_TWO_SOURCES_OR_ID,
            "instrument_mappings_symbol_key",
        ),
        (
            history_sql::EXPLAIN_BY_TWO_SOURCES_OR_ID,
            "instrument_mappings_isin_key",
        ),
        (
            history_sql::EXPLAIN_BY_TWO_SOURCES_OR_ID,
            "sqlite_autoindex_instrument_mappings_1",
        ),
        (
            history_sql::EXPLAIN_MARK_SUPERSEDED,
            "sqlite_autoindex_instrument_mappings_",
        ),
        (
            history_sql::EXPLAIN_RESTORE_RETIRED,
            "sqlite_autoindex_instrument_mappings_",
        ),
        (
            history_sql::EXPLAIN_DELETE_UNLINKED_ACTIVE,
            "sqlite_autoindex_instrument_mappings_",
        ),
        (
            history_sql::EXPLAIN_RESTORE_SUPERSEDED,
            "sqlite_autoindex_instrument_mappings_",
        ),
    ] {
        assert_plan_uses_index(&rt, &db, query, expected_index);
    }
    rt.block_on(db.close()).expect("close");
}

struct SplitMix64(u64);

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    fn next_usize(&mut self, upper: usize) -> usize {
        let bound = u64::try_from(upper).expect("upper fits u64");
        usize::try_from(self.next_u64().rem_euclid(bound)).expect("rem fits usize")
    }
}

#[derive(Default)]
struct DiffStats {
    ok_ops: usize,
    err_ops: usize,
    id_changed: usize,
    retire_corr: usize,
    link_corr: usize,
    re_retire_re_corrected: usize,
    max_chain_len: usize,
}

impl DiffStats {
    fn observe_outcome<T, E>(&mut self, result: &Result<T, E>) {
        match result {
            Ok(_) => self.ok_ops = self.ok_ops.saturating_add(1),
            Err(_) => self.err_ops = self.err_ops.saturating_add(1),
        }
    }

    fn bump_retire_slot(
        retire_counts: &mut [usize; 3],
        ids: &[InstrumentId; 3],
        target: &InstrumentId,
    ) {
        for (slot, tracked) in ids.iter().enumerate() {
            if tracked == target
                && let Some(cell) = retire_counts.get_mut(slot)
            {
                *cell = cell.saturating_add(1);
            }
        }
    }

    fn observe_final(&mut self, repo: &MemoryInstrumentMappingRepository, ids: &[InstrumentId; 3]) {
        let mut retire_counts = [0_usize; 3];
        for event in &repo.history() {
            match event {
                MappingHistoryEvent::IdentifierChanged { .. } => {
                    self.id_changed = self.id_changed.saturating_add(1);
                }
                MappingHistoryEvent::RetirementCorrected { instrument_id, .. } => {
                    self.retire_corr = self.retire_corr.saturating_add(1);
                    Self::bump_retire_slot(&mut retire_counts, ids, instrument_id);
                }
                MappingHistoryEvent::LinkCorrected { .. } => {
                    self.link_corr = self.link_corr.saturating_add(1);
                }
            }
        }
        for count in retire_counts {
            if count >= 2 {
                self.re_retire_re_corrected = self.re_retire_re_corrected.saturating_add(1);
            }
        }
        for tracked in ids {
            if let Ok(rows) = TokioRuntime.block_on(repo.for_id(tracked)) {
                let superseded = rows
                    .iter()
                    .filter(|entry| match entry.status() {
                        InstrumentMappingStatus::Superseded => true,
                        InstrumentMappingStatus::Active | InstrumentMappingStatus::Retired => false,
                    })
                    .count();
                self.max_chain_len = self.max_chain_len.max(superseded);
            }
        }
    }
}

fn active_for(
    repo: &MemoryInstrumentMappingRepository,
    id: &InstrumentId,
) -> Option<InstrumentSource> {
    TokioRuntime
        .block_on(repo.for_id(id))
        .ok()?
        .into_iter()
        .find_map(|entry| match entry.status() {
            InstrumentMappingStatus::Active => Some(entry.source().clone()),
            InstrumentMappingStatus::Superseded | InstrumentMappingStatus::Retired => None,
        })
}

fn retired_for(
    repo: &MemoryInstrumentMappingRepository,
    id: &InstrumentId,
) -> Option<InstrumentSource> {
    TokioRuntime
        .block_on(repo.for_id(id))
        .ok()?
        .into_iter()
        .find_map(|entry| match entry.status() {
            InstrumentMappingStatus::Retired => Some(entry.source().clone()),
            InstrumentMappingStatus::Active | InstrumentMappingStatus::Superseded => None,
        })
}

fn latest_link_for(
    repo: &MemoryInstrumentMappingRepository,
    id: &InstrumentId,
) -> Option<(InstrumentSource, InstrumentSource)> {
    let mut stack = Vec::new();
    for event in repo.history_for_id(id) {
        match event {
            MappingHistoryEvent::IdentifierChanged { previous, next, .. } => {
                stack.push((previous, next));
            }
            MappingHistoryEvent::LinkCorrected { .. } => {
                let _popped = stack.pop();
            }
            MappingHistoryEvent::RetirementCorrected { .. } => {}
        }
    }
    stack.pop()
}

fn diff_sources() -> [InstrumentSource; 10] {
    [
        source(KnownMarket::Kr, "005930", "KR7005930003"),
        source(KnownMarket::Kr, "005930", "kr7005930003"),
        source(KnownMarket::Kr, "005931", "KR7005930003"),
        source(KnownMarket::Kr, "005930", "KR7005930999"),
        source(KnownMarket::Kr, "005932", "KR7005930888"),
        source(KnownMarket::Us, "fb.A", "us30303m1027"),
        source(KnownMarket::Us, "FB.a", "US30303M1027"),
        source(KnownMarket::Us, "meta", "US30303M1027"),
        source(KnownMarket::Us, "META", "us30303m9999"),
        source(KnownMarket::Us, "meta.B", "us30303m8888"),
    ]
}

fn diff_disclosure(step: usize) -> DisclosureKey {
    const DART: [&str; 3] = ["20261115000001", "20261115000002", "20261115000003"];
    const KIND: [&str; 3] = ["20261115000101", "20261115000102", "20261115000103"];
    const SEC: [&str; 3] = [
        "0001326801-26-000601",
        "0001326801-26-000602",
        "0001326801-26-000603",
    ];
    let slot = step.rem_euclid(3);
    match step.div_euclid(3).rem_euclid(3) {
        0 => dart_disclosure(DART[slot]),
        1 => kind_disclosure(KIND[slot]),
        _ => sec_disclosure(SEC[slot]),
    }
}

fn diff_note(step: usize) -> UserReason {
    const NOTES: [&str; 4] = [
        "차등 속성 검증",
        "\0앞자리 NUL 차등 사유",
        " \0공백 NUL 차등",
        "중간\0NUL 차등",
    ];
    reason(NOTES[step.rem_euclid(NOTES.len())])
}

fn diff_ts(rng: &mut SplitMix64) -> Timestamp {
    let offset_choices = [-18_000_i32, 0_i32, 32_400_i32];
    let secs = 1_760_000_000_i64
        .checked_add(i64::try_from(rng.next_usize(10_000_000)).expect("secs"))
        .expect("secs sum");
    let nanos = u32::try_from(rng.next_usize(1_000_000_000)).expect("nanos");
    let offset = offset_choices[rng.next_usize(offset_choices.len())];
    ts(secs, nanos, offset)
}

fn build_diff_link_evidence(
    id: &InstrumentId,
    prev: &InstrumentSource,
    next: &InstrumentSource,
    step: usize,
    mode: usize,
) -> IdentityChangeEvidence {
    let disc = diff_disclosure(step);
    let note = diff_note(step);
    if mode == 3 {
        return IdentityChangeEvidence::NoteOnly(note);
    }
    let action = match mode {
        0 => split_action(id, 15, 1, 5),
        1 => reverse_split_action(id, 15, 10, 1),
        _ if prev
            .symbol()
            .as_str()
            .eq_ignore_ascii_case(next.symbol().as_str()) =>
        {
            reverse_split_action(id, 15, 2, 1)
        }
        _ => symbol_action(id, 15, prev.symbol().as_str(), next.symbol().as_str()),
    };
    IdentityChangeEvidence::DisclosureAndAction {
        disclosure: disc,
        action,
        note,
    }
}

fn assert_stores_equal(
    rt: &tokio::runtime::Runtime,
    mem: MemoryInstrumentMappingStore<'_>,
    sql_store: &InstrumentMappingStore<'_>,
    ids: &[InstrumentId; 3],
    sources: &[InstrumentSource; 10],
) {
    assert_eq!(rt.block_on(mem.history()), rt.block_on(sql_store.history()));
    for tracked in ids {
        assert_eq!(
            rt.block_on(mem.history_for_id(tracked)),
            rt.block_on(sql_store.history_for_id(tracked))
        );
        assert_eq!(
            rt.block_on(mem.for_id(tracked)).map(sort_records),
            rt.block_on(sql_store.for_id(tracked)).map(sort_records)
        );
    }
    for src in sources {
        assert_eq!(
            rt.block_on(mem.resolve(src)),
            rt.block_on(sql_store.resolve(src))
        );
        assert_eq!(
            rt.block_on(mem.lookup(src)),
            rt.block_on(sql_store.lookup(src))
        );
    }
}

fn pick_diff_next(
    sources: &[InstrumentSource; 10],
    market: KnownMarket,
    raw: usize,
) -> &InstrumentSource {
    let idx = if raw == 9 {
        match market {
            KnownMarket::Kr => 5,
            KnownMarket::Us => 0,
        }
    } else {
        let base: usize = match market {
            KnownMarket::Kr => 0,
            KnownMarket::Us => 5,
        };
        base.saturating_add(raw.rem_euclid(5))
    };
    &sources[idx]
}

struct PairChangeSpec<'a> {
    id: &'a InstrumentId,
    prev: &'a InstrumentSource,
    next: &'a InstrumentSource,
    ev: &'a IdentityChangeEvidence,
    at: Timestamp,
    use_unconfirmed: usize,
}

fn make_note_only_pair(
    rt: &tokio::runtime::Runtime,
    spec: &PairChangeSpec<'_>,
) -> (ConfirmedIdentifierChange, ConfirmedIdentifierChange) {
    let issuer = ScriptedIdentityConfirmationIssuer::new([
        IdentityConfirmationDecision::Confirmed,
        IdentityConfirmationDecision::Confirmed,
    ]);
    let make = || {
        let seal =
            match rt.block_on(issuer.issue(IdentityConfirmationIssuanceKind::IdentifierChange)) {
                IdentityConfirmationOutcome::Confirmed(token) => token,
                IdentityConfirmationOutcome::Denied | IdentityConfirmationOutcome::Unavailable => {
                    panic!("confirmed")
                }
            };
        ConfirmedIdentifierChange::new(
            spec.id.clone(),
            spec.prev.clone(),
            spec.next.clone(),
            IdentityChangeConfirmation::new(
                IdentityConfirmationOrigin::UserConfirmed(seal),
                spec.ev.clone(),
                spec.at,
            ),
        )
    };
    (make(), make())
}

fn build_pair_change(
    rt: &tokio::runtime::Runtime,
    spec: &PairChangeSpec<'_>,
) -> (ConfirmedIdentifierChange, ConfirmedIdentifierChange) {
    if spec.use_unconfirmed == 0 {
        let make = || {
            ConfirmedIdentifierChange::new(
                spec.id.clone(),
                spec.prev.clone(),
                spec.next.clone(),
                IdentityChangeConfirmation::new(
                    IdentityConfirmationOrigin::BrokerObservation,
                    spec.ev.clone(),
                    spec.at,
                ),
            )
        };
        return (make(), make());
    }
    let is_note_only = match spec.ev {
        IdentityChangeEvidence::NoteOnly(_) => true,
        IdentityChangeEvidence::None
        | IdentityChangeEvidence::DisclosureOnly { .. }
        | IdentityChangeEvidence::ActionOnly { .. }
        | IdentityChangeEvidence::DisclosureAndAction { .. } => false,
    };
    if is_note_only {
        return make_note_only_pair(rt, spec);
    }
    (
        confirm_change(
            rt,
            spec.id,
            (spec.prev, spec.next),
            spec.ev.clone(),
            spec.at,
        ),
        confirm_change(
            rt,
            spec.id,
            (spec.prev, spec.next),
            spec.ev.clone(),
            spec.at,
        ),
    )
}

struct DiffEnv<'a> {
    rt: &'a tokio::runtime::Runtime,
    repo: &'a MemoryInstrumentMappingRepository,
    sql_store: &'a InstrumentMappingStore<'a>,
    ids: &'a [InstrumentId; 3],
    sources: &'a [InstrumentSource; 10],
}

fn step_diff_link(
    env: &DiffEnv<'_>,
    rng: &mut SplitMix64,
    target_id: &InstrumentId,
    step: usize,
    stats: &mut DiffStats,
) {
    let mem = MemoryInstrumentMappingStore::new(env.repo);
    let fallback = &env.sources[rng.next_usize(10)];
    let prev = active_for(env.repo, target_id).unwrap_or_else(|| fallback.clone());
    let next = pick_diff_next(env.sources, prev.market(), rng.next_usize(10));
    let mode = rng.next_usize(4);
    let ev = build_diff_link_evidence(target_id, &prev, next, step, mode);
    let at = diff_ts(rng);
    let unconfirmed = rng.next_usize(8);
    let (m_req, s_req) = build_pair_change(
        env.rt,
        &PairChangeSpec {
            id: target_id,
            prev: &prev,
            next,
            ev: &ev,
            at,
            use_unconfirmed: unconfirmed,
        },
    );
    let mem_out = env.rt.block_on(mem.link_confirmed_change(m_req));
    let sql_out = env.rt.block_on(env.sql_store.link_confirmed_change(s_req));
    assert_eq!(mem_out, sql_out);
    stats.observe_outcome(&mem_out);
}

fn step_diff_retire_corr(
    env: &DiffEnv<'_>,
    rng: &mut SplitMix64,
    target_id: &InstrumentId,
    step: usize,
    stats: &mut DiffStats,
) {
    let mem = MemoryInstrumentMappingStore::new(env.repo);
    let fallback = &env.sources[rng.next_usize(10)];
    let target = retired_for(env.repo, target_id).unwrap_or_else(|| fallback.clone());
    let at = diff_ts(rng);
    let ev = MappingCorrectionEvidence::DisclosureAndNote {
        disclosure: diff_disclosure(step),
        note: diff_note(step),
    };
    let (m_req, s_req) = if rng.next_usize(8) == 0 {
        let make = || {
            ConfirmedRetirementCorrection::new(
                target_id.clone(),
                target.clone(),
                MappingCorrectionConfirmation::new(
                    IdentityConfirmationOrigin::AiSuggestion,
                    ev.clone(),
                    at,
                ),
            )
        };
        (make(), make())
    } else {
        (
            confirm_retire(env.rt, target_id, &target, ev.clone(), at),
            confirm_retire(env.rt, target_id, &target, ev, at),
        )
    };
    let mem_out = env.rt.block_on(mem.correct_retirement(m_req));
    let sql_out = env.rt.block_on(env.sql_store.correct_retirement(s_req));
    assert_eq!(mem_out, sql_out);
    stats.observe_outcome(&mem_out);
}

fn step_diff_link_corr(
    env: &DiffEnv<'_>,
    rng: &mut SplitMix64,
    target_id: &InstrumentId,
    step: usize,
    stats: &mut DiffStats,
) {
    let mem = MemoryInstrumentMappingStore::new(env.repo);
    let fallback_prev = &env.sources[rng.next_usize(10)];
    let fallback_next = &env.sources[rng.next_usize(10)];
    let (prev, next) = latest_link_for(env.repo, target_id)
        .unwrap_or_else(|| (fallback_prev.clone(), fallback_next.clone()));
    let at = diff_ts(rng);
    let ev = MappingCorrectionEvidence::DisclosureAndNote {
        disclosure: diff_disclosure(step),
        note: diff_note(step),
    };
    let m_req = confirm_link_undo(env.rt, target_id, (&prev, &next), ev.clone(), at);
    let s_req = confirm_link_undo(env.rt, target_id, (&prev, &next), ev, at);
    let mem_out = env.rt.block_on(mem.correct_link(m_req));
    let sql_out = env.rt.block_on(env.sql_store.correct_link(s_req));
    assert_eq!(mem_out, sql_out);
    stats.observe_outcome(&mem_out);
}

fn step_differential_op(
    env: &DiffEnv<'_>,
    rng: &mut SplitMix64,
    step: usize,
    stats: &mut DiffStats,
) {
    let mem = MemoryInstrumentMappingStore::new(env.repo);
    let op_roll = rng.next_usize(12);
    let id_slot = rng.next_usize(3);
    let target_id = &env.ids[id_slot];
    match op_roll {
        0 | 1 => {
            let src = &env.sources[rng.next_usize(10)];
            let slot = u16::try_from(id_slot).expect("slot");
            let mem_out = env.rt.block_on(mem.ensure(src, issued_slot(slot)));
            let sql_out = env
                .rt
                .block_on(env.sql_store.ensure(src, issued_slot(slot)));
            assert_eq!(mem_out, sql_out);
            stats.observe_outcome(&mem_out);
        }
        2 | 3 => {
            let fallback = &env.sources[rng.next_usize(10)];
            let target = active_for(env.repo, target_id).unwrap_or_else(|| fallback.clone());
            let mem_out = env.rt.block_on(mem.retire(&target));
            let sql_out = env.rt.block_on(env.sql_store.retire(&target));
            assert_eq!(mem_out, sql_out);
            stats.observe_outcome(&mem_out);
        }
        4..=7 => step_diff_link(env, rng, target_id, step, stats),
        8 | 9 => step_diff_retire_corr(env, rng, target_id, step, stats),
        _ => step_diff_link_corr(env, rng, target_id, step, stats),
    }
}

#[test]
fn domain_memory_and_sqlite_match_across_state_machine_differential_sequences() {
    let rt = runtime();
    let sources = diff_sources();
    let ids = [
        issued_slot(0).as_instrument_id().clone(),
        issued_slot(1).as_instrument_id().clone(),
        issued_slot(2).as_instrument_id().clone(),
    ];
    let cases = testkit::property_cases().min(48);
    let mut rng = SplitMix64::new(0x5240_2026_1124_0002);
    let mut stats = DiffStats::default();

    for _case in 0..cases {
        let fixture = Fixture::new();
        let db = open_fixture(&fixture);
        let sql_store = InstrumentMappingStore::new(&db);
        let mem_repo = MemoryInstrumentMappingRepository::default();
        let mem_store = MemoryInstrumentMappingStore::new(&mem_repo);
        let steps = rng.next_usize(20).saturating_add(10);
        let env = DiffEnv {
            rt: &rt,
            repo: &mem_repo,
            sql_store: &sql_store,
            ids: &ids,
            sources: &sources,
        };

        for step in 0..steps {
            step_differential_op(&env, &mut rng, step, &mut stats);
            assert_stores_equal(&rt, mem_store, &sql_store, &ids, &sources);
        }
        stats.observe_final(&mem_repo, &ids);
        rt.block_on(db.close()).expect("close");

        let reopened = open_fixture(&fixture);
        let reopened_store = InstrumentMappingStore::new(&reopened);
        assert_stores_equal(&rt, mem_store, &reopened_store, &ids, &sources);
        rt.block_on(reopened.close()).expect("close reopened");
    }

    assert!(stats.ok_ops > 0);
    assert!(stats.err_ops > 0);
    assert!(stats.id_changed > 0);
    assert!(stats.retire_corr > 0);
    assert!(stats.link_corr > 0);
    assert!(stats.re_retire_re_corrected > 0);
    assert!(stats.max_chain_len >= 2);
}
