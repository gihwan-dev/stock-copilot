use crate::{
    Database, DatabasePath, SharedDatabase, SqliteRepositories,
    tests::{Fixture, clock, key, runtime},
};
use application::{
    JobClockTime, JobCode, JobFailureKind, JobHour, JobItemCount, JobMinute, JobRunEntry,
    JobRunResult, JobScheduleStore, JobSummary, JobSwitch, SavedJobSchedule,
};
use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(32_400),
    )
    .expect("시각")
}

fn code(value: &str) -> JobCode {
    JobCode::try_new(value).expect("작업")
}

fn entry(job: &str, seconds: i64, result: JobRunResult) -> JobRunEntry {
    JobRunEntry {
        job: code(job),
        scheduled_for: at(seconds),
        evaluated_at: at(seconds),
        result,
    }
}

fn open_shared(fixture: &Fixture, shared: &SharedDatabase) {
    runtime().block_on(async {
        let database =
            Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("열기");
        assert!(shared.install(database).await.is_none());
    });
}

#[test]
fn job_schedules_are_replaced_per_job_and_keep_the_optional_time() {
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    let _unopened = runtime()
        .block_on(repositories.schedules())
        .expect_err("미설치");
    open_shared(&fixture, &shared);
    runtime().block_on(async {
        let morning = SavedJobSchedule {
            job: code("morning_briefing"),
            switch: JobSwitch::Enabled,
            time: Some(JobClockTime {
                hour: JobHour::try_new(7).expect("시"),
                minute: JobMinute::try_new(45).expect("분"),
            }),
        };
        let closing = SavedJobSchedule {
            job: code("closing_review"),
            switch: JobSwitch::Disabled,
            time: None,
        };
        let _first = repositories.save_schedule(&morning).await.expect("저장");
        let _second = repositories.save_schedule(&closing).await.expect("저장");
        let turned_off = SavedJobSchedule {
            switch: JobSwitch::Disabled,
            ..morning.clone()
        };
        let _replaced = repositories.save_schedule(&turned_off).await.expect("교체");
        assert_eq!(
            repositories.schedules().await,
            Ok(vec![closing, turned_off])
        );
        let _closed = shared.close().await;
    });
}

async fn append_all(repositories: &SqliteRepositories, entries: &[JobRunEntry]) {
    for item in entries {
        let _saved = repositories.append_run(item).await.expect("기록");
    }
}

fn every_result() -> Vec<JobRunResult> {
    vec![
        JobRunResult::Executed {
            summary: JobSummary::new("completed"),
            processed: JobItemCount::new(3),
        },
        JobRunResult::SkippedMissedWindow,
        JobRunResult::Deferred {
            resumes_at: Some(at(500)),
        },
        JobRunResult::Deferred { resumes_at: None },
        JobRunResult::Failed(JobFailureKind::CalendarUnavailable),
    ]
}

#[test]
fn job_runs_round_trip_every_result_newest_first_and_keep_the_last_fifty() {
    let fixture = Fixture::new();
    let shared = SharedDatabase::new();
    let repositories = SqliteRepositories::new(shared.clone());
    open_shared(&fixture, &shared);
    let results = every_result();
    runtime().block_on(async {
        let first: Vec<_> = (100_i64..)
            .zip(&results)
            .map(|(seconds, result)| entry("morning_briefing", seconds, result.clone()))
            .collect();
        append_all(&repositories, &first).await;
        append_all(
            &repositories,
            &[entry(
                "closing_review",
                1,
                JobRunResult::SkippedMissedWindow,
            )],
        )
        .await;
        let read = repositories
            .recent_runs(&code("morning_briefing"))
            .await
            .expect("조회");
        let expected: Vec<_> = first.iter().rev().cloned().collect();
        assert_eq!(read, expected);
        let more: Vec<_> = (200_i64..260_i64)
            .map(|seconds| {
                entry(
                    "morning_briefing",
                    seconds,
                    JobRunResult::SkippedMissedWindow,
                )
            })
            .collect();
        append_all(&repositories, &more).await;
        let kept = repositories
            .recent_runs(&code("morning_briefing"))
            .await
            .expect("조회");
        assert_eq!(kept.len(), 50);
        assert_eq!(kept.first().map(|item| item.scheduled_for), Some(at(259)));
        assert_eq!(
            repositories
                .recent_runs(&code("closing_review"))
                .await
                .expect("조회")
                .len(),
            1
        );
        let _closed = shared.close().await;
    });
}
