#[cfg(test)]
mod tests {
    use application::{
        AiEvent, AiProvider, AiRunId, AiUsage, ChatHistory, EstimateAssignmentRepository,
        EstimateCommitOutcome, EstimateRunContext, EstimateRunIdentity, EstimateRunRegistry,
        EstimateTaskMaterialSource, EstimateTaskMaterials, EstimateTrigger, InformationError,
        IntradayReviewSubject, IsolationCheck, RunInput, RunKind, RunPurpose, RunSingleEstimate,
        RunSpec, SingleEstimateOutcome, SystemFailure, SystemPort, SystemPortError, TrackingTarget,
    };
    use domain::{
        CalibrationEstimateId, CalibrationModel, EstimateAssignments, EstimateConsumption,
        EstimatePrompt, EstimateTask, InstrumentId, MarketCalendar, ModelIdentityPart, PromptHash,
        ScorecardPromptId, Timestamp, UntrustedText,
    };
    use std::future::Future;
    use std::sync::{Arc, RwLock};
    use std::task::{Context, Poll, Waker};
    use testkit_ports::SteppedClock;
    use testkit_ports::chat_fakes::ScriptedAiEventStream;
    use testkit_ports::estimate_fixtures::{
        sample_day, sample_estimate_materials, sample_kr_calendars,
    };

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

    fn broken() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity)
    }

    #[derive(Debug, Default)]
    struct Tasks(RwLock<EstimateAssignments<AiRunId>>);

    impl EstimateAssignmentRepository for Tasks {
        fn load_assignments(
            &self,
        ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send
        {
            std::future::ready(
                self.0
                    .read()
                    .map(|guard| guard.clone())
                    .map_err(|_e| broken()),
            )
        }

        fn commit_assignments(
            &self,
            previous: &EstimateAssignments<AiRunId>,
            next: &EstimateAssignments<AiRunId>,
        ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send {
            let result = match self.0.write() {
                Ok(mut guard) if &*guard == previous => {
                    *guard = next.clone();
                    Ok(EstimateCommitOutcome::Committed)
                }
                Ok(_) => Ok(EstimateCommitOutcome::Conflict),
                Err(_) => Err(broken()),
            };
            std::future::ready(result)
        }
    }

    struct Materials {
        materials: Option<EstimateTaskMaterials>,
        calendars: Vec<MarketCalendar>,
    }

    impl EstimateTaskMaterialSource for Materials {
        fn materials(
            &self,
            _target: &TrackingTarget,
            _now: Timestamp,
        ) -> impl Future<Output = Result<Option<EstimateTaskMaterials>, SystemPortError>> + Send
        {
            std::future::ready(Ok(self.materials.clone()))
        }

        fn calendars(
            &self,
            _task: &EstimateTask,
        ) -> impl Future<Output = Result<Vec<MarketCalendar>, SystemPortError>> + Send {
            std::future::ready(Ok(self.calendars.clone()))
        }
    }

    #[derive(Debug, Default)]
    struct Runs {
        registered: RwLock<Vec<EstimateRunContext>>,
        released: RwLock<Vec<AiRunId>>,
    }

    impl EstimateRunRegistry for Runs {
        fn register(&self, context: EstimateRunContext) {
            self.registered.write().expect("등록").push(context);
        }

        fn release(&self, run: &AiRunId) {
            self.released.write().expect("해제").push(run.clone());
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Submits {
        Yes,
        No,
    }

    struct Ai {
        tasks: Arc<Tasks>,
        submits: Submits,
        runs: Arc<Runs>,
        specs: RwLock<Vec<RunSpec>>,
        registered_during_run: RwLock<Vec<usize>>,
    }

    impl AiProvider for Ai {
        type Stream = ScriptedAiEventStream;

        fn run(
            &self,
            spec: RunSpec,
        ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
            self.registered_during_run
                .write()
                .expect("기록")
                .push(self.runs.registered.read().expect("읽기").len());
            if self.submits == Submits::Yes {
                let current = self.tasks.0.read().expect("읽기").clone();
                let assigned: Vec<CalibrationEstimateId> = current
                    .assigned(spec.id())
                    .iter()
                    .map(|entry| entry.id().as_estimate_id().clone())
                    .collect();
                let next = current.consume(spec.id(), &assigned).expect("소비");
                *self.tasks.0.write().expect("쓰기") = next;
            }
            self.specs.write().expect("기록").push(spec);
            std::future::ready(Ok(ScriptedAiEventStream::from_events(vec![
                AiEvent::IsolationChecked(IsolationCheck::Passed),
                AiEvent::TextDelta(UntrustedText::try_new("끝").expect("글")),
            ])))
        }

        fn resume(
            &self,
            _spec: RunSpec,
            _history: ChatHistory,
            _message: UntrustedText,
        ) -> impl Future<Output = Result<Self::Stream, InformationError>> + Send {
            std::future::ready(Err(InformationError::Unavailable(
                application::ResetTime::NotProvided,
            )))
        }

        fn usage(&self) -> impl Future<Output = Result<AiUsage, InformationError>> + Send {
            std::future::ready(Err(InformationError::Unavailable(
                application::ResetTime::NotProvided,
            )))
        }

        fn interrupt(
            &self,
            _run: &AiRunId,
        ) -> impl Future<Output = Result<(), InformationError>> + Send {
            std::future::ready(Ok(()))
        }

        fn verify_isolation(
            &self,
            _kind: RunKind,
        ) -> impl Future<Output = Result<IsolationCheck, InformationError>> + Send {
            std::future::ready(Ok(IsolationCheck::Passed))
        }
    }

    fn part(value: &str) -> ModelIdentityPart {
        ModelIdentityPart::try_new(value).expect("식별")
    }

    fn identity() -> EstimateRunIdentity {
        EstimateRunIdentity {
            model: CalibrationModel::new(part("claude-code"), part("default"), part("cli")),
            cli_version: part("cli"),
            prompt: EstimatePrompt {
                id: ScorecardPromptId::try_new("estimate").expect("ID"),
                version: part("v1"),
                hash: PromptHash::try_new(&"a".repeat(64)).expect("해시"),
            },
        }
    }

    struct Fixture {
        tasks: Arc<Tasks>,
        runs: Arc<Runs>,
        ai: Ai,
        materials: Materials,
        clock: SteppedClock,
        identity: EstimateRunIdentity,
    }

    fn fixture(submits: Submits, calendars: Vec<MarketCalendar>) -> Fixture {
        let tasks = Arc::new(Tasks::default());
        let runs = Arc::new(Runs::default());
        Fixture {
            ai: Ai {
                tasks: Arc::clone(&tasks),
                submits,
                runs: Arc::clone(&runs),
                specs: RwLock::new(Vec::new()),
                registered_during_run: RwLock::new(Vec::new()),
            },
            tasks,
            runs,
            materials: Materials {
                materials: sample_estimate_materials(),
                calendars,
            },
            clock: SteppedClock::new(sample_day(60).expect("시각")),
            identity: identity(),
        }
    }

    impl Fixture {
        fn estimate(&self, trigger: &EstimateTrigger) -> SingleEstimateOutcome {
            let target =
                TrackingTarget::for_holding(InstrumentId::try_new("sample-a").expect("ID"));
            run(RunSingleEstimate {
                materials: &self.materials,
                tasks: self.tasks.as_ref(),
                ai: &self.ai,
                runs: self.runs.as_ref(),
                clock: &self.clock,
                identity: &self.identity,
            }
            .run(&target, trigger))
            .expect("실행")
        }
    }

    fn calendars() -> Vec<MarketCalendar> {
        sample_kr_calendars(60, 66).expect("달력")
    }

    #[test]
    fn issues_a_code_task_runs_ai_with_only_its_task_id_and_reports_submission() {
        let fixture = fixture(Submits::Yes, calendars());
        assert_eq!(
            fixture.estimate(&EstimateTrigger::Cycle),
            SingleEstimateOutcome::Submitted
        );
        let specs = fixture.ai.specs.read().expect("읽기").clone();
        assert_eq!(specs.len(), 1);
        let spec = specs.first().expect("실행");
        assert_eq!(spec.purpose(), RunPurpose::Estimate);
        let stored = fixture.tasks.0.read().expect("읽기").clone();
        let assigned = stored.assigned(spec.id());
        assert_eq!(assigned.len(), 1);
        let issued = assigned.first().expect("과제");
        assert_eq!(issued.consumption(), EstimateConsumption::Consumed);
        let RunInput::Analysis(prompt) = spec.input() else {
            panic!("분석 실행");
        };
        assert!(prompt.as_str().contains(&format!(
            "task_id=\"{}\"",
            issued.id().as_estimate_id().as_str()
        )));
        assert!(!prompt.as_str().contains("100"));
        assert_eq!(
            *fixture.ai.registered_during_run.read().expect("읽기"),
            vec![1]
        );
        let registered = fixture.runs.registered.read().expect("읽기").clone();
        assert_eq!(registered.len(), 1);
        let context = registered.first().expect("맥락");
        assert_eq!(&context.run, spec.id());
        assert_eq!(
            context.horizon,
            domain::Horizon::new(domain::HorizonSelection::FewDays)
        );
        assert_eq!(
            *fixture.runs.released.read().expect("읽기"),
            vec![spec.id().clone()]
        );
    }

    #[test]
    fn run_without_submission_is_not_submitted_and_intraday_uses_its_purpose() {
        let fixture = fixture(Submits::No, calendars());
        let subject =
            IntradayReviewSubject::PriceMove(InstrumentId::try_new("sample-a").expect("ID"));
        assert_eq!(
            fixture.estimate(&EstimateTrigger::Intraday(subject)),
            SingleEstimateOutcome::NotSubmitted
        );
        let specs = fixture.ai.specs.read().expect("읽기").clone();
        assert_eq!(
            specs.first().expect("실행").purpose(),
            RunPurpose::IntradayMonitoring
        );
        assert_eq!(fixture.runs.released.read().expect("읽기").len(), 1);
    }

    #[test]
    fn missing_materials_or_short_calendar_issue_no_task_and_start_no_ai() {
        let mut missing = fixture(Submits::Yes, calendars());
        missing.materials.materials = None;
        assert_eq!(
            missing.estimate(&EstimateTrigger::Cycle),
            SingleEstimateOutcome::NoTask
        );
        let short = fixture(Submits::Yes, sample_kr_calendars(60, 62).expect("달력"));
        assert_eq!(
            short.estimate(&EstimateTrigger::Cycle),
            SingleEstimateOutcome::NoTask
        );
        for case in [&missing, &short] {
            assert!(case.ai.specs.read().expect("읽기").is_empty());
            assert_eq!(case.tasks.0.read().expect("읽기").entries().len(), 0);
            assert!(case.runs.registered.read().expect("읽기").is_empty());
        }
    }

    #[test]
    fn excessive_cost_issues_no_task() {
        let mut costly = fixture(Submits::Yes, calendars());
        let mut materials = sample_estimate_materials().expect("재료");
        materials.commission = Some(domain::CommissionRateFact::from_commissions_result(
            domain::Market::Kr,
            domain::CostRate::try_new(
                domain::Ratio::parse(&domain::NumericInput::try_new("0.10").expect("입력"))
                    .expect("비율"),
            )
            .expect("요율"),
            domain::CommissionValidity::try_new(None, None).expect("유효"),
        ));
        costly.materials.materials = Some(materials);
        assert_eq!(
            costly.estimate(&EstimateTrigger::Cycle),
            SingleEstimateOutcome::NoTask
        );
        assert!(costly.ai.specs.read().expect("읽기").is_empty());
    }
}
