mod fixtures;

use crate::tests::{Fixture, clock, key, runtime};
use crate::{Database, DatabasePath, EstimateStore, SignalStore, sql};
use application::{
    ConditionalDraftOutcome, CurrentDraftOutcome, DraftCheck, DraftStoreCandidate,
    EstimateRepository, ProposalRepository, SignalCommitOutcome, SignalEvaluation,
    SignalInvalidationOutcome, SignalInvalidationRepository, SignalMutation, SignalRepository,
    SystemFailure,
};
use domain::{
    AccountId, AiProbability, AiProbabilityInput, Calibration, CalibrationEstimateId,
    CalibrationFact, CalibrationInstant, CalibrationInterval, CalibrationOutcome, ClientOrderId,
    InstrumentId, Nanosecond, OrderSide, ProposalId, RaisedSignal, SavedCalibrationFact,
    SignalDecision, SignalTransition, UnixSeconds, signal_transition_from_history,
};
use fixtures::{
    BuyFixture, TaskBundle, build_buy_signal, build_draft_fixture, build_sell_signal,
    build_task_bundle, calibration_key, seed_submission, shifted_us,
};

fn is_applied(outcome: &SignalCommitOutcome) -> bool {
    match outcome {
        SignalCommitOutcome::Applied(_) => true,
        SignalCommitOutcome::Conflict | SignalCommitOutcome::SubmissionDiscarded => false,
    }
}

fn is_invalidation_applied(outcome: &SignalInvalidationOutcome) -> bool {
    match outcome {
        SignalInvalidationOutcome::Applied(events) => !events.is_empty(),
        SignalInvalidationOutcome::Conflict | SignalInvalidationOutcome::SubmissionDiscarded => {
            false
        }
    }
}

fn buy_raise_mutation(
    task_id: domain::IssuedEstimateTaskId,
    bundle: &TaskBundle,
    buy: &BuyFixture,
) -> SignalMutation {
    let decision = match &buy.signal {
        RaisedSignal::Buy(val) => SignalDecision::Buy(val.clone()),
        RaisedSignal::Sell(_) => panic!("buy"),
    };
    let transition = signal_transition_from_history(None, &decision).expect("tr");
    SignalMutation::Evaluate(Box::new(SignalEvaluation {
        submission: task_id,
        decision,
        transition,
        calibration_head: Some(bundle.calibration.through().clone()),
        occurrence: Some(buy.occurrence.clone()),
        evidence: Some(buy.evidence.clone()),
    }))
}

async fn advance_calibration(estimates: &EstimateStore<'_>) {
    let key = calibration_key();
    let fact = CalibrationFact::restore_from_saved_state(SavedCalibrationFact {
        id: CalibrationEstimateId::try_new("cal-f1").expect("id"),
        instrument: InstrumentId::try_new("buy-1").expect("inst"),
        key: key.clone(),
        interval: CalibrationInterval::try_new(
            CalibrationInstant::from_parts(UnixSeconds::new(100), Nanosecond::new(0)).expect("s"),
            CalibrationInstant::from_parts(UnixSeconds::new(200), Nanosecond::new(0)).expect("e"),
        )
        .expect("int"),
        predicted_probability: AiProbability::parse(&AiProbabilityInput::try_new("70").expect("p"))
            .expect("prob"),
        outcome: CalibrationOutcome::Hit,
    });
    let cal = Calibration::try_new(key.clone(), &[fact]).expect("cal");
    estimates
        .append_calibration(&key, &cal)
        .await
        .expect("append cal");
}

#[test]
fn all_signal_sql_constants_prepare_on_migrated_database() {
    assert_eq!(
        std::fs::read_to_string("migrations/0011_proposals_and_signals.sql").expect("file"),
        sql::signals::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("migrate");
    for statement in [
        sql::signals::UPSERT_MASTER,
        sql::signals::ALL_MASTERS,
        sql::signals::UPSERT_SOURCES,
        sql::signals::SOURCES_BY_TASK,
        sql::signals::UPSERT_SELECTED_ACCOUNT,
        sql::signals::SELECTED_ACCOUNT,
        sql::signals::STATE_BY_INSTRUMENT,
        sql::signals::UPSERT_STATE,
        sql::signals::LATEST_SUBMISSION_RECORD_BY_INSTRUMENT,
        sql::signals::SUBMISSION_RECORD_BY_TASK_ID,
        sql::signals::INSERT_OCCURRENCE,
        sql::signals::OCCURRENCE_BY_ID,
        sql::signals::OCCURRENCES_FOR_INSTRUMENT,
        sql::signals::ALL_OCCURRENCE_IDS,
        sql::signals::IS_DISCARDED,
        sql::signals::INSERT_DISCARDED,
        sql::signals::DRAFT_EXISTS_BY_ID,
        sql::signals::LATEST_CURRENT_DRAFT_BY_ID,
        sql::signals::CURRENT_GUARDED_DRAFT_BY_INSTRUMENT,
        sql::signals::SUPERSEDE_DRAFT_BY_ROW_ID,
        sql::signals::SUPERSEDE_STANDALONE_BY_PROPOSAL_ID,
        sql::signals::SUPERSEDE_GUARDED_BY_INSTRUMENT,
        sql::signals::INSERT_DRAFT,
    ] {
        connection.prepare(statement).expect("sql prepares");
    }
}

#[test]
fn standalone_proposal_roundtrip_and_expiry_offset_preserved() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let bundle = build_task_bundle("sig-draft-1", "ELIG", OrderSide::Buy);
    let (first, first_ev, _) = build_draft_fixture(&bundle, "prop-1", "100", "100", 7);
    let (second, second_ev, _) = build_draft_fixture(&bundle, "prop-1", "101", "101", 7);
    let (other, other_ev, _) = build_draft_fixture(&bundle, "prop-2", "100", "100", 7);
    runtime().block_on(async {
        let db = Database::open(&path, &key(), &clock()).expect("open");
        let store = SignalStore::new(&db);
        testkit_ports::contracts::scenarios::proposal_roundtrip(
            &store,
            (&first, &second),
            &other,
            (&first_ev, &second_ev, &other_ev),
            &bundle.master,
        )
        .await;
        assert_eq!(
            store
                .store_draft(&first, &other_ev, &bundle.master)
                .await
                .expect_err("mismatch")
                .failure(),
            SystemFailure::InvalidData
        );
        db.close().await.expect("close");
        let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
        let loaded = SignalStore::new(&reopened)
            .draft(second.id(), &bundle.master)
            .await
            .expect("draft")
            .expect("present");
        assert_eq!(loaded.expires_at(), second.expires_at());
        assert_eq!(
            loaded.expires_at().offset_seconds(),
            second.expires_at().offset_seconds()
        );
        reopened.close().await.expect("close");
    });
}

#[test]
fn signal_replacement_stale_and_discarded_contracts() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let first_bundle = build_task_bundle("sig-a", "ELIG", OrderSide::Sell);
    let other_bundle = build_task_bundle("sig-b", "OTHR", OrderSide::Sell);
    let (sub_a, sell_one, ev_one) = build_sell_signal(&first_bundle, "10", "5");
    let (_, sell_two, ev_two) = build_sell_signal(&first_bundle, "10", "3");
    let (sub_b, sell_other, ev_other) = build_sell_signal(&other_bundle, "10", "4");
    let inst_a = InstrumentId::try_new("sig-a").expect("id");
    let inst_b = InstrumentId::try_new("sig-b").expect("id");
    runtime().block_on(async {
        let db = Database::open(&path, &key(), &clock()).expect("open");
        let store = SignalStore::new(&db);
        let task_a = seed_submission(
            &EstimateStore::new(&db),
            &store,
            "t-a",
            &first_bundle,
            sub_a,
        )
        .await;
        let _task_b = seed_submission(
            &EstimateStore::new(&db),
            &store,
            "t-b",
            &other_bundle,
            sub_b,
        )
        .await;
        Box::pin(testkit_ports::contracts::scenarios::signal_replacement(
            &store,
            (&inst_a, &inst_b),
            (&sell_one, &sell_two, &sell_other),
            (&ev_one, &ev_two, &ev_other),
        ))
        .await;
        verify_stale_and_discarded(&store, &inst_a, task_a, &first_bundle, (&sell_one, &ev_one))
            .await;
        db.close().await.expect("close");
    });
}

async fn verify_stale_and_discarded(
    store: &SignalStore<'_>,
    inst: &InstrumentId,
    task_id: domain::IssuedEstimateTaskId,
    bundle: &TaskBundle,
    signal_and_evidence: (&RaisedSignal, &application::SignalCommitEvidence),
) {
    let (sell_one, ev_one) = signal_and_evidence;
    let stale = store.snapshot(inst).await.expect("snap0");
    let decision = match sell_one {
        RaisedSignal::Sell(val) => SignalDecision::Sell(val.clone()),
        RaisedSignal::Buy(_) => panic!("sell"),
    };
    let raise_eval = SignalMutation::Evaluate(Box::new(SignalEvaluation {
        submission: task_id,
        decision,
        transition: SignalTransition::Raised(sell_one.clone()),
        calibration_head: Some(bundle.calibration.through().clone()),
        occurrence: None,
        evidence: Some(ev_one.clone()),
    }));
    assert!(is_applied(
        &store
            .commit(inst, &stale, &raise_eval)
            .await
            .expect("raise")
    ));
    let after_raise = store.snapshot(inst).await.expect("snap1");
    testkit_ports::contracts::scenarios::signal_rejects_stale_snapshot(
        store,
        inst,
        &stale,
        &after_raise,
        &raise_eval,
    )
    .await;
    let discard = SignalMutation::Discard(Box::new(SignalTransition::Cleared(inst.clone())));
    assert!(is_applied(
        &store
            .commit(inst, &after_raise, &discard)
            .await
            .expect("disc")
    ));
    let after_discard = store.snapshot(inst).await.expect("snap2");
    testkit_ports::contracts::scenarios::signal_rejects_discarded_submission(
        store,
        inst,
        &after_discard,
        &raise_eval,
    )
    .await;
}

#[test]
fn buy_signal_occurrences_invalidation_and_guarded_draft_lifecycle() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let bundle = build_task_bundle("buy-1", "BUYX", OrderSide::Buy);
    let buy = build_buy_signal(&bundle, "occ-1");
    let (draft_one, draft_ev_one, card) = build_draft_fixture(&bundle, "g-prop-1", "100", "100", 9);
    let inst = InstrumentId::try_new("buy-1").expect("id");
    let account = AccountId::new(9);
    runtime().block_on(async {
        let db = Database::open(&path, &key(), &clock()).expect("open");
        let estimates = EstimateStore::new(&db);
        let store = SignalStore::new(&db);
        let task_id = seed_submission(
            &estimates,
            &store,
            "buy-t1",
            &bundle,
            buy.submission.clone(),
        )
        .await;
        store
            .save_sources(&task_id, &buy.sources)
            .await
            .expect("src");
        assert_eq!(store.sources(&task_id).await.expect("load"), buy.sources);
        store.select_account(&account).await.expect("account");
        let snap0 = store.snapshot(&inst).await.expect("snap0");
        assert!(is_applied(
            &store
                .commit(&inst, &snap0, &buy_raise_mutation(task_id, &bundle, &buy))
                .await
                .expect("commit")
        ));
        let snap1 = store.snapshot(&inst).await.expect("snap1");
        let occs = store
            .occurrences(&inst, buy.occurrence.instrument())
            .await
            .expect("occs");
        assert_eq!(occs.len(), 1);
        assert_eq!(occs[0].id(), buy.occurrence.id());
        let check = DraftCheck {
            account: &account,
            at: shifted_us(61, 21_604),
            observation: &card,
            master: &bundle.master,
        };
        let candidate = DraftStoreCandidate {
            proposal: &draft_one,
            evidence: &draft_ev_one,
        };
        verify_guarded_invalidation((&estimates, &store), &inst, &snap1, candidate, check).await;
        db.close().await.expect("close");
    });
}

async fn verify_guarded_invalidation(
    stores: (&EstimateStore<'_>, &SignalStore<'_>),
    inst: &InstrumentId,
    snap: &application::SignalSnapshot,
    candidate: DraftStoreCandidate<'_>,
    check: DraftCheck<'_>,
) {
    let (estimates, store) = stores;
    assert_eq!(
        store.current_draft(inst, snap, check).await.expect("abs"),
        CurrentDraftOutcome::Absent
    );
    assert_eq!(
        store
            .store_current_draft(inst, snap, candidate, check)
            .await
            .expect("stored"),
        ConditionalDraftOutcome::Stored
    );
    assert_eq!(
        store.current_draft(inst, snap, check).await.expect("exist"),
        CurrentDraftOutcome::Existing(candidate.proposal.id().clone())
    );
    assert!(
        store
            .draft(candidate.proposal.id(), check.master)
            .await
            .expect("read")
            .is_some()
    );
    store
        .select_account(&AccountId::new(10))
        .await
        .expect("switch");
    assert_eq!(
        store.current_draft(inst, snap, check).await.expect("acct"),
        CurrentDraftOutcome::AccountChanged
    );
    store.select_account(check.account).await.expect("restore");
    advance_calibration(estimates).await;
    assert_eq!(
        store.current_draft(inst, snap, check).await.expect("cal"),
        CurrentDraftOutcome::Conflict
    );
    assert!(
        store
            .draft(candidate.proposal.id(), check.master)
            .await
            .expect("stale")
            .is_none()
    );
    assert!(is_invalidation_applied(
        &store.invalidate(inst, snap).await.expect("inv")
    ));
    assert_eq!(
        store.current(inst).await.expect("cleared"),
        Vec::<domain::HistoricalSignal>::new()
    );
}

#[test]
fn two_connection_optimistic_concurrency_conflict_for_signal_and_draft() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let bundle = build_task_bundle("conc-1", "CONC", OrderSide::Buy);
    let buy = build_buy_signal(&bundle, "occ-conc-1");
    let (draft_one, draft_ev_one, card) = build_draft_fixture(&bundle, "c-prop-1", "100", "100", 5);
    let (draft_two, draft_ev_two, _) = build_draft_fixture(&bundle, "c-prop-2", "100", "100", 5);
    let inst = InstrumentId::try_new("conc-1").expect("id");
    let account = AccountId::new(5);
    runtime().block_on(async {
        let db1 = Database::open(&path, &key(), &clock()).expect("open1");
        let db2 = Database::open(&path, &key(), &clock()).expect("open2");
        let store1 = SignalStore::new(&db1);
        let store2 = SignalStore::new(&db2);
        let task_id = seed_submission(
            &EstimateStore::new(&db1),
            &store1,
            "conc-t1",
            &bundle,
            buy.submission.clone(),
        )
        .await;
        store1.select_account(&account).await.expect("acct");
        let snap_a = store1.snapshot(&inst).await.expect("snap1");
        let snap_b = store2.snapshot(&inst).await.expect("snap2");
        let mutation = buy_raise_mutation(task_id, &bundle, &buy);
        assert!(is_applied(
            &store1.commit(&inst, &snap_a, &mutation).await.expect("c1")
        ));
        assert_eq!(
            store2.commit(&inst, &snap_b, &mutation).await.expect("c2"),
            SignalCommitOutcome::Conflict
        );
        verify_two_conn_draft_race(
            &store1,
            &store2,
            &inst,
            (
                DraftStoreCandidate {
                    proposal: &draft_one,
                    evidence: &draft_ev_one,
                },
                DraftStoreCandidate {
                    proposal: &draft_two,
                    evidence: &draft_ev_two,
                },
            ),
            DraftCheck {
                account: &account,
                at: shifted_us(61, 21_604),
                observation: &card,
                master: &bundle.master,
            },
        )
        .await;
        db1.close().await.expect("close1");
        db2.close().await.expect("close2");
    });
}

async fn verify_two_conn_draft_race(
    store1: &SignalStore<'_>,
    store2: &SignalStore<'_>,
    inst: &InstrumentId,
    candidates: (DraftStoreCandidate<'_>, DraftStoreCandidate<'_>),
    check: DraftCheck<'_>,
) {
    let snap1 = store1.snapshot(inst).await.expect("live1");
    let snap2 = store2.snapshot(inst).await.expect("live2");
    assert_eq!(
        store1
            .store_current_draft(inst, &snap1, candidates.0, check)
            .await
            .expect("d1"),
        ConditionalDraftOutcome::Stored
    );
    assert_eq!(
        store2
            .store_current_draft(inst, &snap2, candidates.1, check)
            .await
            .expect("d2"),
        ConditionalDraftOutcome::Existing(candidates.0.proposal.id().clone())
    );
    assert!(is_invalidation_applied(
        &store1.invalidate(inst, &snap1).await.expect("inv")
    ));
    assert_eq!(
        store2
            .store_current_draft(inst, &snap2, candidates.1, check)
            .await
            .expect("stale"),
        ConditionalDraftOutcome::Conflict
    );
}

#[test]
fn append_only_triggers_and_corrupt_rows_fail_closed() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let bundle = build_task_bundle("trig-1", "TRIG", OrderSide::Buy);
    let buy = build_buy_signal(&bundle, "occ-trig-1");
    let (draft_one, draft_ev_one, _) = build_draft_fixture(&bundle, "t-prop-1", "100", "100", 3);
    let (draft_two, draft_ev_two, _) = build_draft_fixture(&bundle, "t-prop-1", "101", "101", 3);
    let inst = InstrumentId::try_new("trig-1").expect("id");
    runtime().block_on(async {
        let db = Database::open(&path, &key(), &clock()).expect("open");
        let store = SignalStore::new(&db);
        let task_id = seed_submission(
            &EstimateStore::new(&db),
            &store,
            "trig-t1",
            &bundle,
            buy.submission.clone(),
        )
        .await;
        let snap0 = store.snapshot(&inst).await.expect("snap0");
        let eval = buy_raise_mutation(task_id, &bundle, &buy);
        store.commit(&inst, &snap0, &eval).await.expect("commit");
        let snap1 = store.snapshot(&inst).await.expect("snap1");
        store
            .commit(
                &inst,
                &snap1,
                &SignalMutation::Discard(Box::new(SignalTransition::Cleared(inst.clone()))),
            )
            .await
            .expect("discard");
        store
            .store_draft(&draft_one, &draft_ev_one, &bundle.master)
            .await
            .expect("draft1");
        store
            .store_draft(&draft_two, &draft_ev_two, &bundle.master)
            .await
            .expect("draft2");
        verify_triggers_and_corruption(&db, &store, &bundle.master).await;
        db.close().await.expect("close");
    });
}

async fn verify_triggers_and_corruption(
    db: &Database,
    store: &SignalStore<'_>,
    master: &domain::MasterInstrument,
) {
    for stmt in [
        sql::signals_tests::UPDATE_OCCURRENCE,
        sql::signals_tests::DELETE_OCCURRENCE,
        sql::signals_tests::UPDATE_DISCARDED,
        sql::signals_tests::DELETE_DISCARDED,
        sql::signals_tests::DELETE_DRAFT,
        sql::signals_tests::MUTATE_DRAFT_EXPIRY,
        sql::signals_tests::REVERT_DRAFT_STATE,
    ] {
        assert_eq!(
            db.write(move |tx| tx.execute_batch(stmt))
                .await
                .expect_err("trigger rejects")
                .failure(),
            SystemFailure::RepositoryIntegrity
        );
    }
    db.write(|tx| tx.execute_batch(sql::signals_tests::INSERT_CORRUPT_STATE))
        .await
        .expect("seed corrupt state");
    db.write(|tx| tx.execute_batch(sql::signals_tests::INSERT_CORRUPT_DRAFT))
        .await
        .expect("seed corrupt draft");
    assert_eq!(
        store
            .snapshot(&InstrumentId::try_new("corrupt-sig").expect("id"))
            .await
            .expect_err("corrupt state")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
    assert_eq!(
        store
            .draft(
                &ProposalId::new(ClientOrderId::try_new("corrupt-draft").expect("id")),
                master
            )
            .await
            .expect_err("corrupt draft")
            .failure(),
        SystemFailure::RepositoryIntegrity
    );
}
