use crate::{
    Database, DatabasePath, ThesisStore,
    tests::{Fixture, clock, key, runtime},
};
use application::ThesisRepository;
use domain::{
    ChangeDisposition, ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, ClaimVerdict,
    Evidence, EvidenceRef, EvidenceUrl, InstrumentId, NormalizedUrl, ProposedChange, Thesis,
    ThesisOperation, ThesisPatch, ThesisStatus, UntrustedText, UserReason,
};

fn text(value: &str) -> UntrustedText {
    UntrustedText::try_new(value).expect("text")
}

fn instrument(value: &str) -> InstrumentId {
    InstrumentId::try_new(value).expect("instrument")
}

fn changed_thesis() -> Thesis {
    let mut thesis = Thesis::new();
    let claim = Claim::new(
        ClaimId::try_new("claim-1").expect("id"),
        ClaimRole::Core,
        ClaimContent::try_new(
            text("statement"),
            text("metric"),
            text("expected"),
            text("observe"),
            text("refutation"),
        )
        .expect("claim"),
    );
    let evidence = Evidence::try_new(vec![EvidenceRef::Web(
        EvidenceUrl::try_new(NormalizedUrl::try_new("https://example.com/first").expect("url"))
            .expect("evidence"),
    )])
    .expect("evidence");
    let reason =
        ChangeReason::try_new(text("reason"), text("model"), text("version")).expect("reason");
    thesis
        .apply_batch(
            thesis.version(),
            ThesisPatch::try_new(vec![ProposedChange::new(
                ThesisOperation::AddClaim(claim),
                evidence,
                reason,
            )])
            .expect("patch"),
        )
        .expect("change");
    thesis
}

#[test]
fn thesis_history_roundtrip_replays_pending_confirmation_and_isolates_instruments() {
    let fixture = Fixture::new();
    let rt = runtime();
    let path = DatabasePath::new(fixture.path());
    let id = instrument("instrument-a");
    let other = instrument("instrument-b");
    let mut thesis = changed_thesis();
    let pending = thesis.history()[0].sequence();
    assert_eq!(
        thesis.history()[0].disposition(),
        ChangeDisposition::NeedsReview
    );
    {
        let database = Database::open(&path, &key(), &clock()).expect("open");
        let store = ThesisStore::new(&database);
        assert_eq!(rt.block_on(store.load(&other)).expect("missing"), None);
        rt.block_on(store.store(&id, &thesis)).expect("store");
        let read = rt
            .block_on(store.load(&id))
            .expect("load")
            .expect("present");
        assert_eq!(read.history(), thesis.history());
        assert_eq!(read.pending().len(), 1);
        thesis
            .confirm_by_user(
                thesis.version(),
                pending,
                UserReason::try_new(text("confirm")).expect("reason"),
            )
            .expect("confirm");
        rt.block_on(store.store(&id, &thesis)).expect("append")
    };
    let database = Database::open(&path, &key(), &clock()).expect("reopen");
    let store = ThesisStore::new(&database);
    let read = rt
        .block_on(store.load(&id))
        .expect("load")
        .expect("present");
    assert_eq!(read.history(), thesis.history());
    assert_eq!(read.claims(), thesis.claims());
    assert_eq!(read.status(), ThesisStatus::NoBreakSignal);
    assert_eq!(read.pending().len(), 0);
    assert_eq!(rt.block_on(store.load(&other)).expect("missing"), None);
}

#[test]
fn thesis_store_rejects_shortened_or_divergent_history() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ThesisStore::new(&database);
    let id = instrument("instrument-a");
    let mut original = changed_thesis();
    let pending = original.history()[0].sequence();
    original
        .confirm_by_user(
            original.version(),
            pending,
            UserReason::try_new(text("confirm")).expect("reason"),
        )
        .expect("confirm");
    rt.block_on(store.store(&id, &original)).expect("store");
    rt.block_on(store.store(&id, &changed_thesis()))
        .expect_err("rejected");
    let mut divergent = changed_thesis();
    divergent
        .reject_by_user(
            divergent.version(),
            pending,
            UserReason::try_new(text("reject")).expect("reason"),
        )
        .expect("reject");
    rt.block_on(store.store(&id, &divergent))
        .expect_err("rejected");
    assert_eq!(
        rt.block_on(store.load(&id))
            .expect("load")
            .expect("present")
            .history(),
        original.history()
    );
}

#[test]
fn thesis_status_is_derived_from_replayed_history() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ThesisStore::new(&database);
    let id = instrument("instrument-a");
    let mut thesis = changed_thesis();
    let pending = thesis.history()[0].sequence();
    thesis
        .confirm_by_user(
            thesis.version(),
            pending,
            UserReason::try_new(text("confirm")).expect("reason"),
        )
        .expect("confirm");
    let evidence = Evidence::try_new(vec![EvidenceRef::News(
        domain::NewsSource::Yonhap,
        EvidenceUrl::try_new(NormalizedUrl::try_new("https://example.com/news").expect("url"))
            .expect("url"),
    )])
    .expect("evidence");
    let reason =
        ChangeReason::try_new(text("judgment"), text("model"), text("version")).expect("reason");
    thesis
        .apply_batch(
            thesis.version(),
            ThesisPatch::try_new(vec![ProposedChange::new(
                ThesisOperation::Judge(
                    ClaimId::try_new("claim-1").expect("id"),
                    ClaimVerdict::Broken,
                ),
                evidence,
                reason,
            )])
            .expect("patch"),
        )
        .expect("judge");
    rt.block_on(store.store(&id, &thesis)).expect("store");
    let read = rt
        .block_on(store.load(&id))
        .expect("load")
        .expect("present");
    assert_eq!(read.history(), thesis.history());
    assert_eq!(read.status(), ThesisStatus::SuspectedBroken);
}

#[test]
fn sqlite_rejects_update_and_delete_of_past_history() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ThesisStore::new(&database);
    let id = instrument("instrument-a");
    let thesis = changed_thesis();
    rt.block_on(store.store(&id, &thesis)).expect("store");
    for statement in [
        crate::sql::thesis_tests::UPDATE_HISTORY,
        crate::sql::thesis_tests::DELETE_HISTORY,
    ] {
        rt.block_on(database.write(move |tx| {
            tx.execute(statement, ["instrument-a"])?;
            Ok(())
        }))
        .expect_err("append only");
    }
    assert_eq!(
        rt.block_on(store.load(&id))
            .expect("load")
            .expect("present")
            .history(),
        thesis.history()
    );
}

#[test]
fn corrupt_history_fails_closed_without_leaking_other_instruments() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ThesisStore::new(&database);
    let bad = instrument("instrument-a");
    let good = instrument("instrument-b");
    let thesis = changed_thesis();
    rt.block_on(store.store(&bad, &thesis)).expect("store");
    rt.block_on(store.store(&good, &thesis))
        .expect("store other");
    rt.block_on(database.write(|tx| {
        tx.execute(crate::sql::thesis_tests::INSERT_BROKEN, ["instrument-a"])?;
        Ok(())
    }))
    .expect("corrupt fixture");
    rt.block_on(store.load(&bad)).expect_err("rejected");
    rt.block_on(store.store(&bad, &thesis))
        .expect_err("rejected");
    assert_eq!(
        rt.block_on(store.load(&good))
            .expect("load")
            .expect("present")
            .history(),
        thesis.history()
    );
}

#[test]
fn failed_second_event_rolls_back_the_entire_store() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ThesisStore::new(&database);
    let id = instrument("instrument-a");
    let mut thesis = changed_thesis();
    let pending = thesis.history()[0].sequence();
    thesis
        .confirm_by_user(
            thesis.version(),
            pending,
            UserReason::try_new(text("confirm")).expect("reason"),
        )
        .expect("confirm");
    rt.block_on(database.write(|tx| tx.execute_batch(crate::sql::thesis_tests::FAIL_SECOND)))
        .expect("fixture");
    rt.block_on(store.store(&id, &thesis))
        .expect_err("rejected");
    assert_eq!(rt.block_on(store.load(&id)).expect("load"), None);
}

use domain::{
    CriterionAssessment, CriterionComparison, CriterionKind, CriterionMetric, CriterionObservation,
    CriterionValue, CriterionVerdict, DartReceiptNo, Disclosure, KillCriterion, KindReceiptNo,
    MachineCondition, NumericInput, Ratio, ReceiptNo, SecEdgarReceiptNo, ToolReference,
};

fn criterion_evidence() -> Evidence {
    let url = |value: &str| {
        EvidenceUrl::try_new(NormalizedUrl::try_new(value).expect("url")).expect("evidence url")
    };
    let disclosure = |receipt, raw_address: &str| {
        let address = url(raw_address);
        let item = Disclosure::new(receipt, address.as_url().clone(), text("title"), None);
        EvidenceRef::Disclosure(item.key().clone(), address)
    };
    Evidence::try_new(vec![
        disclosure(
            ReceiptNo::Dart(DartReceiptNo::try_new("20261123000001").expect("dart")),
            "https://example.com/dart",
        ),
        disclosure(
            ReceiptNo::Kind(KindReceiptNo::try_new("20261123000002").expect("kind")),
            "https://example.com/kind",
        ),
        disclosure(
            ReceiptNo::SecEdgar(SecEdgarReceiptNo::try_new("0123456789-12-123456").expect("sec")),
            "https://example.com/sec",
        ),
        EvidenceRef::Tool(ToolReference::try_new("tool:source/item").expect("tool")),
    ])
    .expect("evidence")
}

fn criterion_operations() -> Vec<ThesisOperation> {
    let metric = CriterionMetric::try_new("revenue.metric").expect("metric");
    let value = |raw: &str| {
        CriterionValue::new(
            Ratio::parse(&NumericInput::try_new(raw).expect("numeric"))
                .expect("decimal")
                .as_decimal(),
        )
    };
    vec![
        ThesisOperation::AddKillCriterion(
            KillCriterion::new(
                ClaimId::try_new("machine").expect("id"),
                CriterionKind::Machine(MachineCondition::new(
                    metric.clone(),
                    CriterionComparison::AtLeast,
                    value("12.5"),
                )),
            )
            .expect("criterion"),
        ),
        ThesisOperation::JudgeCriterion(
            ClaimId::try_new("machine").expect("id"),
            CriterionAssessment::Observed(CriterionObservation::new(
                metric,
                value("13.25"),
                ToolReference::try_new("tool:observed").expect("source"),
            )),
        ),
        ThesisOperation::AddKillCriterion(
            KillCriterion::new(
                ClaimId::try_new("narrative").expect("id"),
                CriterionKind::Narrative(text("narrative condition")),
            )
            .expect("criterion"),
        ),
        ThesisOperation::JudgeCriterion(
            ClaimId::try_new("narrative").expect("id"),
            CriterionAssessment::Ai(CriterionVerdict::NotMet),
        ),
    ]
}

#[test]
fn criterion_and_all_source_variants_roundtrip() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ThesisStore::new(&database);
    let id = instrument("instrument-a");
    let mut thesis = Thesis::new();
    let evidence = criterion_evidence();
    let reason = ChangeReason::try_new(text("criterion reason"), text("model A"), text("v2"))
        .expect("reason");
    for operation in criterion_operations() {
        thesis
            .apply_batch(
                thesis.version(),
                ThesisPatch::try_new(vec![ProposedChange::new(
                    operation,
                    evidence.clone(),
                    reason.clone(),
                )])
                .expect("patch"),
            )
            .expect("apply");
    }
    rt.block_on(store.store(&id, &thesis)).expect("store");
    let loaded = rt
        .block_on(store.load(&id))
        .expect("load")
        .expect("present");
    assert_eq!(loaded.history(), thesis.history());
    assert_eq!(loaded.kill_criteria(), thesis.kill_criteria());
    assert_eq!(loaded.status(), ThesisStatus::SuspectedBroken);
}

#[test]
fn every_thesis_sql_statement_prepares_on_migrated_database() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("migration");
    for sql in [
        crate::sql::thesis::NOTE,
        crate::sql::thesis::INSERT_NOTE,
        crate::sql::thesis::HISTORY,
        crate::sql::thesis::INSERT_HISTORY,
    ] {
        connection.prepare(sql).expect("prepare");
    }
}

#[test]
fn empty_thesis_is_distinct_from_absent_thesis() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = ThesisStore::new(&database);
    let id = instrument("instrument-a");
    assert_eq!(rt.block_on(store.load(&id)).expect("load"), None);
    rt.block_on(store.store(&id, &Thesis::new()))
        .expect("store empty");
    assert_eq!(
        rt.block_on(store.load(&id)).expect("load"),
        Some(Thesis::new())
    );
}
#[test]
fn orphan_history_is_corruption_instead_of_an_absent_note() {
    let fixture = Fixture::new();
    let rt = runtime();
    let path = DatabasePath::new(fixture.path());
    let database = Database::open(&path, &key(), &clock()).expect("open");
    let store = ThesisStore::new(&database);
    let id = instrument("instrument-orphan");
    let connection = rusqlite::Connection::open(fixture.path()).expect("independent connection");
    connection
        .execute_batch(crate::sql::tests::RAW_KEY)
        .expect("key");
    connection
        .execute_batch(crate::sql::thesis_tests::DISABLE_FK_FIXTURE)
        .expect("corrupt fixture connection");
    assert_eq!(
        connection
            .query_row(crate::sql::FOREIGN_KEYS, [], |row| row.get::<_, i64>(0))
            .expect("foreign keys"),
        0
    );
    connection
        .execute(crate::sql::thesis_tests::INSERT_ORPHAN, [id.as_str()])
        .expect("orphan fixture");
    rt.block_on(store.load(&id))
        .expect_err("orphan history is corrupt");
    rt.block_on(store.store(&id, &Thesis::new()))
        .expect_err("orphan history is corrupt");
}
