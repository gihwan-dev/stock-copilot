#[cfg(test)]
mod tests {
    use application::{
        AiEvent, AiJobFinishRecord, AiJobOutcome, AiJobRecordStore, AiJobRun, AiJobTiming,
        AiJobTimingError, AiJobUsageMeter, IsolationCheck, ListAiJobUsage, ListAiJobUsageError,
        RunKind, RunPurpose, SystemFailure, SystemPort, SystemPortError, TokenCount, TokenUsage,
    };
    use domain::{
        AiJobId, Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds,
        UntrustedText,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::MemoryAiJobRecords;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("expected ready"),
        }
    }
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("time")
    }
    fn usage(input: u64, output: u64, read: u64, creation: u64) -> TokenUsage {
        TokenUsage::new(
            TokenCount::new(input),
            TokenCount::new(output),
            TokenCount::new(read),
            TokenCount::new(creation),
        )
    }
    fn record(id: &str, started: i64, finished: i64, tokens: TokenUsage) -> AiJobFinishRecord {
        AiJobFinishRecord::new(
            AiJobRun::new(
                AiJobId::try_new(id).expect("ID"),
                RunKind::Analysis,
                RunPurpose::MorningBriefing,
            ),
            AiJobOutcome::Completed,
            AiJobTiming::try_new(at(started), at(finished)).expect("시간"),
            tokens,
        )
    }

    #[test]
    fn meter_sums_token_usage_events_and_measures_elapsed_time() {
        let mut meter = AiJobUsageMeter::start(at(1_000));
        for event in [
            AiEvent::IsolationChecked(IsolationCheck::Passed),
            AiEvent::TokenUsage(usage(100, 20, 5, 1)),
            AiEvent::TextDelta(UntrustedText::try_new("답").expect("글")),
            AiEvent::TokenUsage(usage(50, 10, 0, 2)),
        ] {
            meter.observe(&event);
        }
        assert_eq!(meter.tokens(), &usage(150, 30, 5, 3));
        let (timing, tokens) = meter.finish(at(1_042)).expect("끝");
        assert_eq!(tokens, usage(150, 30, 5, 3));
        assert_eq!(timing.started_at(), at(1_000));
        assert_eq!(timing.finished_at(), at(1_042));
        assert_eq!(
            timing.span(),
            TimeSpan::try_new(SpanSeconds::new(42), Nanosecond::new(0)).expect("간격")
        );
    }

    #[test]
    fn meter_without_usage_is_zero_and_rejects_end_before_start() {
        let meter = AiJobUsageMeter::start(at(500));
        assert_eq!(meter.tokens(), &TokenUsage::zero());
        let (timing, tokens) = meter.clone().finish(at(500)).expect("같은 시각");
        assert_eq!(tokens, TokenUsage::zero());
        assert_eq!(
            timing.span(),
            TimeSpan::try_new(SpanSeconds::new(0), Nanosecond::new(0)).expect("0")
        );
        assert_eq!(
            meter.finish(at(499)),
            Err(AiJobTimingError::EndsBeforeStart)
        );
        assert_eq!(
            AiJobTiming::try_new(at(10), at(9)),
            Err(AiJobTimingError::EndsBeforeStart)
        );
    }

    #[test]
    fn token_sums_saturate_instead_of_wrapping() {
        let huge = usage(u64::MAX, 1, 0, 0);
        assert_eq!(
            huge.saturating_add(&usage(5, 2, 0, 0)),
            usage(u64::MAX, 3, 0, 0)
        );
    }

    #[test]
    fn listing_returns_window_newest_first_with_tokens_and_time() {
        let store = MemoryAiJobRecords::default();
        for (id, start, end) in [("a", 10, 20), ("b", 30, 50), ("c", 60, 90), ("d", 5, 8)] {
            run(store.store_finished(&record(id, start, end, usage(1, 2, 0, 0)))).expect("저장");
        }
        let listed = run(ListAiJobUsage::new(&store).execute(at(20), at(90))).expect("조회");
        let ids: Vec<&str> = listed.iter().map(|row| row.job().as_str()).collect();
        assert_eq!(ids, vec!["c", "b", "a"], "창 안의 기록만, 최근 것부터");
        let newest = listed.first().expect("첫 행");
        assert_eq!(newest.tokens(), &usage(1, 2, 0, 0));
        assert_eq!(
            newest.timing().span(),
            TimeSpan::try_new(SpanSeconds::new(30), Nanosecond::new(0)).expect("간격")
        );
    }

    #[test]
    fn listing_rejects_reversed_window_and_reports_storage_failure() {
        let store = MemoryAiJobRecords::default();
        assert_eq!(
            run(ListAiJobUsage::new(&store).execute(at(10), at(9))),
            Err(ListAiJobUsageError::InvalidWindow)
        );
        let failure = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        store.fail_next_lookup(failure);
        assert_eq!(
            run(ListAiJobUsage::new(&store).execute(at(0), at(9))),
            Err(ListAiJobUsageError::Storage(failure))
        );
    }
}
