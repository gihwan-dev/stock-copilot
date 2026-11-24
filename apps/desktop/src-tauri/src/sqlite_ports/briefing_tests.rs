use super::{SharedDatabase, SqliteRepositories};
use crate::ipc::briefing::{BriefingIpcState, UiBriefingStatus, load_briefings_view};
use application::{BriefingOutcome, BriefingRecord, BriefingRepository, SecretValue};
use domain::{
    BriefingId, BriefingKind, CalendarDate, CalendarYear, DayNumber, MonthNumber, UntrustedText,
};
use std::sync::Arc;

fn finished_at() -> domain::Timestamp {
    broker_toss::TossTimestamp::try_from("2026-11-24T23:20:00Z")
        .expect("clock")
        .into_domain()
}

fn clock() -> testkit_ports::SteppedClock {
    testkit_ports::SteppedClock::new(finished_at())
}

#[test]
fn installed_database_serves_saved_briefings_to_the_briefing_ipc() {
    let directory = std::path::PathBuf::from("../../../target/chat2")
        .join(format!("sqlite-briefings-{}", std::process::id()));
    let shared = SharedDatabase::new();
    let repositories = Arc::new(SqliteRepositories::new(shared.clone()));
    let state = BriefingIpcState::from_repository(Arc::clone(&repositories));
    tauri::async_runtime::block_on(async {
        let missing = load_briefings_view(&state).await;
        assert!(missing.is_err(), "설치 전에는 읽지 못한다");
        let path = store_sqlite::DatabasePath::new(directory.join("store.db"));
        crate::database::prepare_directory(&path).expect("폴더");
        let database =
            store_sqlite::Database::open(&path, &SecretValue::new(vec![83; 32]), &clock())
                .expect("열기");
        assert!(shared.install(database).await.is_none());
        let date = CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(25),
        )
        .expect("date");
        let record = BriefingRecord {
            id: BriefingId::new(BriefingKind::Morning, date),
            finished_at: finished_at(),
            outcome: BriefingOutcome::Completed {
                body: UntrustedText::try_new("오늘 브리핑").expect("text"),
                sources: Vec::new(),
            },
        };
        let _saved = repositories.save(&record).await.expect("저장");
        let view = load_briefings_view(&state).await.expect("조회");
        let latest = view.latest.expect("최근 브리핑");
        assert_eq!(latest.body, "오늘 브리핑");
        assert_eq!(latest.status, UiBriefingStatus::Completed);
        assert_eq!(latest.finished_at, "2026-11-25T08:20:00+09:00");
        let _closed = shared.close().await;
    });
    std::fs::remove_dir_all(directory).expect("정리");
}
