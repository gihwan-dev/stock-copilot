#[cfg(test)]
mod tests {
    use application::{
        BackgroundBudget, CycleAdmission, CycleProgress, RunEstimateCycle, SingleEstimateOutcome,
        SingleEstimateRunner, SystemFailure, SystemPort, SystemPortError, TrackedUniverse,
        TrackingTarget, Watchlist, WatchlistEntry,
    };
    use domain::{InstrumentId, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use std::{
        collections::VecDeque,
        future::Future,
        sync::RwLock,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::SteppedClock;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료"),
        }
    }
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(0),
            OffsetSeconds::new(32400),
        )
        .expect("시각")
    }
    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("종목")
    }
    fn universe() -> TrackedUniverse {
        TrackedUniverse::compose(
            &[id("held-a"), id("both")],
            &Watchlist::try_from_entries(vec![
                WatchlistEntry::new(id("both"), at(1)),
                WatchlistEntry::new(id("watch-b"), at(1)),
            ])
            .expect("관심"),
            &[id("candidate-c")],
        )
    }

    #[derive(Debug, Default)]
    struct Runner {
        calls: RwLock<Vec<InstrumentId>>,
        failing: Option<InstrumentId>,
    }
    impl SingleEstimateRunner for Runner {
        fn estimate(
            &self,
            target: &TrackingTarget,
        ) -> impl Future<Output = Result<SingleEstimateOutcome, SystemPortError>> + Send {
            self.calls
                .write()
                .expect("잠금")
                .push(target.instrument().clone());
            let result = if self.failing.as_ref() == Some(target.instrument()) {
                Err(SystemPortError::new(
                    SystemPort::Repository,
                    SystemFailure::InvalidData,
                ))
            } else {
                Ok(SingleEstimateOutcome::Submitted)
            };
            std::future::ready(result)
        }
    }

    #[derive(Debug)]
    struct Budget {
        replies: RwLock<VecDeque<CycleAdmission>>,
        log: RwLock<Vec<&'static str>>,
    }
    impl Budget {
        fn new(replies: &[CycleAdmission]) -> Self {
            Self {
                replies: RwLock::new(replies.iter().copied().collect()),
                log: RwLock::new(Vec::new()),
            }
        }
    }
    impl BackgroundBudget for Budget {
        fn admit(
            &self,
            _now: Timestamp,
        ) -> impl Future<Output = Result<CycleAdmission, SystemPortError>> + Send {
            self.log.write().expect("잠금").push("admit");
            let reply = self
                .replies
                .write()
                .expect("잠금")
                .pop_front()
                .unwrap_or(CycleAdmission::Admitted);
            std::future::ready(Ok(reply))
        }
        fn release(
            &self,
            _now: Timestamp,
        ) -> impl Future<Output = Result<(), SystemPortError>> + Send {
            self.log.write().expect("잠금").push("release");
            std::future::ready(Ok(()))
        }
    }

    #[test]
    fn holdings_and_watchlist_are_estimated_one_by_one_without_candidates() {
        let runner = Runner::default();
        let budget = Budget::new(&[]);
        let clock = SteppedClock::new(at(100));
        let report =
            run(RunEstimateCycle::new(&runner, &budget, &clock).execute(&universe(), None))
                .expect("순환");
        assert_eq!(
            *runner.calls.read().expect("잠금"),
            vec![id("held-a"), id("both"), id("watch-b")]
        );
        assert_eq!(report.progress, CycleProgress::Finished);
        assert_eq!(report.attempted.len(), 3);
        assert_eq!(
            *budget.log.read().expect("잠금"),
            vec!["admit", "release", "admit", "release", "admit", "release"]
        );
    }

    #[test]
    fn budget_pause_stops_before_the_next_call_and_resumes_after_the_last_one() {
        let runner = Runner::default();
        let budget = Budget::new(&[
            CycleAdmission::Admitted,
            CycleAdmission::Paused {
                resumes_at: Some(at(500)),
            },
        ]);
        let clock = SteppedClock::new(at(100));
        let report =
            run(RunEstimateCycle::new(&runner, &budget, &clock).execute(&universe(), None))
                .expect("순환");
        assert_eq!(*runner.calls.read().expect("잠금"), vec![id("held-a")]);
        assert_eq!(
            report.progress,
            CycleProgress::Paused {
                resume_after: Some(id("held-a")),
                resumes_at: Some(at(500)),
            }
        );
        let resumed = run(RunEstimateCycle::new(&runner, &budget, &clock)
            .execute(&universe(), Some(&id("held-a"))))
        .expect("재개");
        assert_eq!(resumed.progress, CycleProgress::Finished);
        assert_eq!(
            *runner.calls.read().expect("잠금"),
            vec![id("held-a"), id("both"), id("watch-b")]
        );
    }

    #[test]
    fn a_failed_estimate_is_recorded_and_the_cycle_continues() {
        let runner = Runner {
            calls: RwLock::new(Vec::new()),
            failing: Some(id("both")),
        };
        let budget = Budget::new(&[]);
        let clock = SteppedClock::new(at(100));
        let report =
            run(RunEstimateCycle::new(&runner, &budget, &clock).execute(&universe(), None))
                .expect("순환");
        assert_eq!(
            report.attempted,
            vec![
                (id("held-a"), SingleEstimateOutcome::Submitted),
                (id("both"), SingleEstimateOutcome::Failed),
                (id("watch-b"), SingleEstimateOutcome::Submitted),
            ]
        );
    }

    #[test]
    fn paused_before_any_call_keeps_the_previous_position() {
        let runner = Runner::default();
        let budget = Budget::new(&[CycleAdmission::Paused { resumes_at: None }]);
        let clock = SteppedClock::new(at(100));
        let report =
            run(RunEstimateCycle::new(&runner, &budget, &clock)
                .execute(&universe(), Some(&id("both"))))
            .expect("순환");
        assert_eq!(
            *runner.calls.read().expect("잠금"),
            Vec::<InstrumentId>::new()
        );
        assert_eq!(
            report.progress,
            CycleProgress::Paused {
                resume_after: Some(id("both")),
                resumes_at: None,
            }
        );
    }
}
