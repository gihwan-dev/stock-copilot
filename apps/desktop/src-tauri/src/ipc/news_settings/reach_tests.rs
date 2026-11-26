use super::{BoxFuture, RouteLookup, UiFeedSource, unreachable_sources};
use application::{InformationError, ResetTime, SourceAttemptOutcome, SourceStatus};
use domain::{DataSource, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

struct Statuses;

impl RouteLookup for Statuses {
    fn source_status(&self, source: DataSource) -> BoxFuture<'_, Option<SourceStatus>> {
        let failed =
            SourceAttemptOutcome::Failed(InformationError::Unavailable(ResetTime::NotProvided));
        let status = if source == DataSource::Maekyung {
            Some(SourceStatus::initial(source).apply_attempt(at(10), failed))
        } else if source == DataSource::Hankyung {
            Some(
                SourceStatus::initial(source)
                    .apply_attempt(at(10), SourceAttemptOutcome::Succeeded)
                    .apply_attempt(at(20), failed),
            )
        } else {
            None
        };
        Box::pin(std::future::ready(status))
    }
}

#[test]
fn only_sources_that_never_connected_on_this_device_are_marked_unreachable() {
    assert_eq!(
        tauri::async_runtime::block_on(unreachable_sources(Some(&Statuses))),
        vec![UiFeedSource::Maekyung],
        "한 번이라도 성공한 소스는 일시 실패로 본다"
    );
    assert_eq!(
        tauri::async_runtime::block_on(unreachable_sources(None)),
        Vec::new()
    );
}
