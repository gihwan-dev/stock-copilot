#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use application::{
        AiRunId, AnalysisFailure, BackgroundBudget, ComparisonArm, ComparisonArmResult,
        ComparisonArmRunner, CycleAdmission, ModelName, PairedComparison, PairedComparisonLog,
        PairingError, PairingRun, RecordOutcome, RunPairedComparison, SystemFailure, SystemPort,
        SystemPortError, TrackingTarget,
    };
    use domain::{InstrumentId, Nanosecond, OffsetSeconds, PairingRate, Timestamp, UnixSeconds};
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
    fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(1_800_000_000),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn target(value: &str) -> TrackingTarget {
        TrackingTarget::for_holding(InstrumentId::try_new(value).expect("종목"))
    }
    fn run_id(value: &str) -> AiRunId {
        AiRunId::try_new(value).expect("실행")
    }
    fn storage() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
    }
    fn challenger() -> ComparisonArm {
        ComparisonArm::Model(ModelName::try_new("other-model").expect("모델"))
    }

    type ArmReply = Result<AiRunId, AnalysisFailure>;
    #[derive(Default)]
    struct Runner(Script<(ComparisonArm, InstrumentId), ArmReply>);
    impl ComparisonArmRunner for Runner {
        async fn record_only(&self, arm: &ComparisonArm, target: &TrackingTarget) -> ArmReply {
            self.0.run((arm.clone(), target.instrument().clone())).await
        }
    }
    #[derive(Default)]
    struct Log(Script<PairedComparison, Result<RecordOutcome, SystemPortError>>);
    impl PairedComparisonLog for Log {
        async fn record(&self, pair: &PairedComparison) -> Result<RecordOutcome, SystemPortError> {
            self.0.run(pair.clone()).await
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
    #[derive(Default)]
    struct Fixture {
        runner: Runner,
        log: Log,
        budget: Budget,
    }
    impl Fixture {
        fn execute(
            &self,
            rate: PairingRate,
            item: &TrackingTarget,
        ) -> Result<PairingRun, PairingError> {
            let clock = SteppedClock::new(at());
            run(RunPairedComparison {
                runner: &self.runner,
                log: &self.log,
                budget: &self.budget,
                clock: &clock,
                rate,
                baseline: ComparisonArm::SingleCall,
                challenger: challenger(),
            }
            .execute(item))
        }
    }
    fn always() -> PairingRate {
        PairingRate::try_new(1).expect("비율")
    }

    #[test]
    fn both_arms_run_on_the_same_target_inside_the_budget_and_are_only_recorded() {
        let fixture = Fixture::default();
        fixture.budget.admit.push(Ok(CycleAdmission::Admitted));
        fixture.budget.release.push(Ok(()));
        fixture.runner.0.push(Ok(run_id("single-1")));
        fixture.runner.0.push(Err(AnalysisFailure::Rejected));
        fixture.log.0.push(Ok(RecordOutcome::Recorded));
        let item = target("A005930");
        let expected = PairedComparison {
            instrument: item.instrument().clone(),
            at: at(),
            baseline: ComparisonArmResult {
                arm: ComparisonArm::SingleCall,
                run: Ok(run_id("single-1")),
            },
            challenger: ComparisonArmResult {
                arm: challenger(),
                run: Err(AnalysisFailure::Rejected),
            },
        };
        assert_eq!(
            fixture.execute(always(), &item),
            Ok(PairingRun::Recorded(expected.clone()))
        );
        assert_eq!(
            fixture.runner.0.calls(),
            vec![
                (ComparisonArm::SingleCall, item.instrument().clone()),
                (challenger(), item.instrument().clone()),
            ]
        );
        assert_eq!(fixture.log.0.calls(), vec![expected]);
    }

    #[test]
    fn unsampled_over_budget_and_interrupted_pairs_are_not_recorded() {
        let rare = PairingRate::try_new(255).expect("비율");
        let skipped = (0_u16..50)
            .map(|index| target(&format!("I{index}")))
            .find(|item| Fixture::default().execute(rare, item) == Ok(PairingRun::NotSampled))
            .expect("표본 밖 종목");
        let fixture = Fixture::default();
        assert_eq!(fixture.execute(rare, &skipped), Ok(PairingRun::NotSampled));
        assert_eq!(fixture.budget.admit.calls().len(), 0);

        let limited = Fixture::default();
        limited.budget.admit.push(Ok(CycleAdmission::Paused {
            resumes_at: Some(at()),
        }));
        assert_eq!(
            limited.execute(always(), &target("A1")),
            Ok(PairingRun::OverBudget(Some(at())))
        );
        assert_eq!(limited.runner.0.calls().len(), 0);

        let chat = Fixture::default();
        chat.budget.admit.push(Ok(CycleAdmission::Admitted));
        chat.budget.release.push(Ok(()));
        chat.runner.0.push(Ok(run_id("single-2")));
        chat.runner.0.push(Err(AnalysisFailure::Interrupted));
        assert_eq!(
            chat.execute(always(), &target("A2")),
            Ok(PairingRun::Interrupted)
        );
        assert_eq!(chat.log.0.calls().len(), 0);
        assert_eq!(chat.budget.release.calls().len(), 1);
    }

    #[test]
    fn budget_and_log_failures_are_errors() {
        let budget = Fixture::default();
        budget.budget.admit.push(Err(storage()));
        assert_eq!(
            budget.execute(always(), &target("A3")),
            Err(PairingError::Budget(storage()))
        );
        let log = Fixture::default();
        log.budget.admit.push(Ok(CycleAdmission::Admitted));
        log.budget.release.push(Ok(()));
        log.runner.0.push(Ok(run_id("single-3")));
        log.runner.0.push(Ok(run_id("model-3")));
        log.log.0.push(Err(storage()));
        assert_eq!(
            log.execute(always(), &target("A4")),
            Err(PairingError::Log(storage()))
        );
    }
}
