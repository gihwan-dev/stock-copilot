#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use application::{
        AnalysisFailure, BackgroundBudget, CycleAdmission, DeepResearchCycleError,
        DeepResearchCycleProgress, DeepResearchError, DeepResearchJob, DeepResearchPause,
        DeepResearchRun, DeepResearchTargetOutcome, ResearchOutcome, ResearchStage,
        RunDeepResearchCycle, SystemFailure, SystemPort, SystemPortError, TrackedUniverse,
        TrackingTarget, Watchlist,
    };
    use domain::{
        InstrumentId, Nanosecond, OffsetSeconds, ResearchCount, Thesis, ThesisStatus, Timestamp,
        UnixSeconds,
    };
    use testkit_ports::{Script, SteppedClock};

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
    fn kst(hour: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                1_799_971_200_i64
                    .checked_add(hour.checked_mul(3_600).expect("시"))
                    .and_then(|value| value.checked_sub(32_400))
                    .expect("시각"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn id(value: &str) -> InstrumentId {
        InstrumentId::try_new(value).expect("종목")
    }
    fn universe() -> TrackedUniverse {
        TrackedUniverse::compose(
            &[id("held-a"), id("held-b")],
            &Watchlist::default(),
            &[id("candidate-c")],
        )
    }
    fn outcome() -> ResearchOutcome {
        ResearchOutcome {
            tally: domain::tally_research_claims(&[]),
            source_warnings: Vec::new(),
            principle_violations: Vec::new(),
            thesis_status: ThesisStatus::NoBreakSignal,
            pending_changes: ResearchCount::of_pending(&Thesis::new()),
            deliveries: Vec::new(),
        }
    }

    type JobReply = Result<DeepResearchRun, DeepResearchError>;
    #[derive(Default)]
    struct Job(Script<InstrumentId, JobReply>);
    impl DeepResearchJob for Job {
        async fn research(&self, target: &TrackingTarget) -> JobReply {
            self.0.run(target.instrument().clone()).await
        }
    }
    #[derive(Default)]
    struct Budget {
        admit: Script<Timestamp, Result<CycleAdmission, SystemPortError>>,
        release: Script<Timestamp, Result<(), SystemPortError>>,
    }
    impl BackgroundBudget for Budget {
        async fn admit(&self, now: Timestamp) -> Result<CycleAdmission, SystemPortError> {
            self.admit.run(now).await
        }
        async fn release(&self, now: Timestamp) -> Result<(), SystemPortError> {
            self.release.run(now).await
        }
    }

    fn execute(
        job: &Job,
        budget: &Budget,
        clock: &SteppedClock,
        start_at: Option<&InstrumentId>,
    ) -> Result<application::DeepResearchCycleReport, DeepResearchCycleError> {
        run(RunDeepResearchCycle { job, budget, clock }.execute(&universe(), start_at))
    }

    #[test]
    fn night_runs_every_monitored_target_inside_the_budget() {
        let job = Job::default();
        job.0.push(Ok(DeepResearchRun::Finished(outcome())));
        job.0
            .push(Err(DeepResearchError::Verify(AnalysisFailure::Rejected)));
        let budget = Budget::default();
        for _ in 0_u8..2 {
            budget.admit.push(Ok(CycleAdmission::Admitted));
            budget.release.push(Ok(()));
        }
        let clock = SteppedClock::new(kst(23));
        let report = execute(&job, &budget, &clock, None).expect("순환");
        assert_eq!(report.progress, DeepResearchCycleProgress::Finished);
        assert_eq!(
            report.attempted,
            vec![
                (id("held-a"), DeepResearchTargetOutcome::Finished),
                (
                    id("held-b"),
                    DeepResearchTargetOutcome::Failed(DeepResearchError::Verify(
                        AnalysisFailure::Rejected
                    ))
                ),
            ]
        );
        assert_eq!(job.0.calls(), vec![id("held-a"), id("held-b")]);
        assert_eq!(budget.release.calls().len(), 2);
    }

    #[test]
    fn daytime_waits_for_night_without_touching_the_budget() {
        let job = Job::default();
        let budget = Budget::default();
        let clock = SteppedClock::new(kst(14));
        let report = execute(&job, &budget, &clock, None).expect("순환");
        assert_eq!(
            report.progress,
            DeepResearchCycleProgress::Paused {
                next: id("held-a"),
                resumes_at: Some(kst(22)),
                reason: DeepResearchPause::OutsideNight,
            }
        );
        assert_eq!(budget.admit.calls().len(), 0);
        assert_eq!(job.0.calls().len(), 0);
    }

    #[test]
    fn budget_limit_and_chat_interruption_pause_at_the_target_to_retry() {
        let job = Job::default();
        job.0.push(Ok(DeepResearchRun::Finished(outcome())));
        let budget = Budget::default();
        budget.admit.push(Ok(CycleAdmission::Admitted));
        budget.release.push(Ok(()));
        budget.admit.push(Ok(CycleAdmission::Paused {
            resumes_at: Some(kst(25)),
        }));
        let clock = SteppedClock::new(kst(1));
        let report = execute(&job, &budget, &clock, None).expect("순환");
        assert_eq!(
            report.progress,
            DeepResearchCycleProgress::Paused {
                next: id("held-b"),
                resumes_at: Some(kst(25)),
                reason: DeepResearchPause::Budget,
            }
        );
        assert_eq!(report.attempted.len(), 1);

        let chat = Job::default();
        chat.0
            .push(Ok(DeepResearchRun::Paused(ResearchStage::Draft)));
        let admitted = Budget::default();
        admitted.admit.push(Ok(CycleAdmission::Admitted));
        admitted.release.push(Ok(()));
        let start = id("held-b");
        let interrupted = execute(&chat, &admitted, &clock, Some(&start)).expect("순환");
        assert_eq!(
            interrupted.progress,
            DeepResearchCycleProgress::Paused {
                next: id("held-b"),
                resumes_at: None,
                reason: DeepResearchPause::Chat,
            }
        );
        assert_eq!(chat.0.calls(), vec![id("held-b")]);
        assert_eq!(admitted.release.calls().len(), 1);
    }

    #[test]
    fn unknown_start_restarts_from_the_first_target_and_budget_errors_stop() {
        let job = Job::default();
        let storage = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let budget = Budget::default();
        budget.admit.push(Err(storage));
        let clock = SteppedClock::new(kst(23));
        let gone = id("sold");
        assert_eq!(
            execute(&job, &budget, &clock, Some(&gone)),
            Err(DeepResearchCycleError::Budget(storage))
        );
        assert_eq!(budget.admit.calls(), vec![kst(23)]);
    }
}
