use crate::sql::{briefing as briefing_sql, briefing_tests as sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{BriefingStore, Database, DatabasePath};
use application::{
    BriefingFailure, BriefingOutcome, BriefingRecord, BriefingRepository, BriefingSource,
    SystemFailure,
};
use domain::{
    BriefingId, BriefingKind, CalendarDate, CalendarYear, DayNumber, DisclosureSource,
    InformationSource, MonthNumber, Nanosecond, NewsSource, NormalizedUrl, OffsetSeconds,
    Timestamp, UnixSeconds, UntrustedText,
};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(11),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn date(day: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(2026),
        MonthNumber::new(11),
        DayNumber::new(day),
    )
    .expect("날짜")
}

fn text(raw: &str) -> UntrustedText {
    UntrustedText::try_new(raw).expect("문구")
}

fn completed(day: u8, kind: BriefingKind, body: &str) -> BriefingRecord {
    BriefingRecord {
        id: BriefingId::new(kind, date(day)),
        finished_at: at(i64::from(day)),
        outcome: BriefingOutcome::Completed {
            body: text(body),
            sources: vec![
                BriefingSource {
                    title: text("연합 기사"),
                    url: Some(NormalizedUrl::try_new("https://example.com/n").expect("주소")),
                    origin: InformationSource::News(NewsSource::Yonhap),
                    observed_at: at(5),
                },
                BriefingSource {
                    title: text("공시"),
                    url: None,
                    origin: InformationSource::Disclosure(DisclosureSource::Kind),
                    observed_at: at(6),
                },
                BriefingSource {
                    title: text("웹"),
                    url: None,
                    origin: InformationSource::Web,
                    observed_at: at(7),
                },
            ],
        },
    }
}

fn failed(day: u8) -> BriefingRecord {
    BriefingRecord {
        id: BriefingId::new(BriefingKind::PreUsMarket, date(day)),
        finished_at: at(100),
        outcome: BriefingOutcome::Failed(BriefingFailure::RateLimited),
    }
}

#[test]
fn briefings_replace_by_id_and_list_in_save_order() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let db = Database::open(&path, &key(), &clock()).expect("open");
    let store = BriefingStore::new(&db);
    let morning = completed(25, BriefingKind::Morning, "아침 요약");
    let closing = completed(25, BriefingKind::ClosingReview, "마감 요약");
    assert_eq!(rt.block_on(store.recent()).expect("empty"), Vec::new());
    assert_eq!(rt.block_on(store.find(&morning.id)).expect("none"), None);
    for record in [&morning, &closing, &failed(24)] {
        rt.block_on(store.save(record)).expect("save");
    }
    let rewritten = completed(25, BriefingKind::Morning, "아침 요약 다시");
    rt.block_on(store.save(&rewritten)).expect("replace");
    rt.block_on(db.close()).expect("close");
    let reopened = Database::open(&path, &key(), &clock()).expect("reopen");
    let reopened_store = BriefingStore::new(&reopened);
    assert_eq!(
        rt.block_on(reopened_store.recent()).expect("recent"),
        vec![closing, failed(24), rewritten.clone()]
    );
    assert_eq!(
        rt.block_on(reopened_store.find(&rewritten.id))
            .expect("find"),
        Some(rewritten)
    );
    rt.block_on(reopened.close()).expect("close");
}

#[test]
fn briefing_table_rejects_deletes_and_corrupt_rows() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let rt = runtime();
    let store = BriefingStore::new(&db);
    rt.block_on(store.save(&failed(1))).expect("save");
    for statement in [sql::DELETE, sql::ZERO_SEQ] {
        assert!(
            rt.block_on(db.write(move |transaction| transaction.execute_batch(statement)))
                .is_err(),
            "{statement}"
        );
    }
    rt.block_on(db.write(|transaction| transaction.execute_batch(sql::CORRUPT)))
        .expect("corrupt");
    assert_eq!(
        rt.block_on(store.recent()).expect_err("corrupt").failure(),
        SystemFailure::RepositoryIntegrity
    );
    rt.block_on(db.close()).expect("close");
}

#[test]
fn briefing_sql_constants_match_the_migration_and_prepare() {
    assert_eq!(
        std::fs::read_to_string("migrations/0019_briefings.sql").expect("file"),
        briefing_sql::CREATE
    );
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [
        briefing_sql::UPSERT,
        briefing_sql::FIND,
        briefing_sql::RECENT,
    ] {
        connection.prepare(statement).expect(statement);
    }
}
