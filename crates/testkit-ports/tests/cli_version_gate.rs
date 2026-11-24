#[cfg(test)]
mod tests {
    use application::{
        AiCheckObservation, AiEnvironmentProblem, AiGate, AiUsage, CheckState, ClaudeLoginProblem,
        CliVersion, CliVersionGateOutcome, EnsureVerifiedCliVersion, EventBusBuilder,
        EventSubscriber, InformationError, IsolationCheck, IsolationFailure, ResetTime, RunKind,
        SelfCheckInput, SubscriberFailure, SystemFailure, SystemPort, SystemPortError,
        evaluate_self_check,
    };
    use domain::{DomainEvent, DomainEventKind};
    use std::collections::BTreeSet;
    use std::sync::mpsc;
    use testkit_ports::chat_fakes::{AiProviderCall, ScriptedAiProvider};
    use testkit_ports::{MemoryVerifiedCliVersions, ScriptedCliVersionSource};

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
        {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("expected ready"),
        }
    }
    fn version(value: &str) -> CliVersion {
        CliVersion::try_new(value).expect("버전")
    }
    struct Seen(mpsc::Sender<DomainEvent>);
    impl EventSubscriber for Seen {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .send(event.clone())
                .map_err(|_error| SubscriberFailure::Unavailable)
        }
    }
    struct Fixture {
        provider: ScriptedAiProvider,
        source: ScriptedCliVersionSource,
        ledger: MemoryVerifiedCliVersions,
        events: application::EventBus,
        received: mpsc::Receiver<DomainEvent>,
    }
    fn fixture(verified: Vec<CliVersion>) -> Fixture {
        let (sender, received) = mpsc::channel();
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            BTreeSet::from([DomainEventKind::SelfCheckChanged]),
            Seen(sender),
        );
        Fixture {
            provider: ScriptedAiProvider::default(),
            source: ScriptedCliVersionSource::default(),
            ledger: MemoryVerifiedCliVersions::with_versions(verified),
            events: builder.build(),
            received,
        }
    }
    fn gate(
        fixture: &Fixture,
        previous: &CliVersionGateOutcome,
    ) -> application::CliVersionGateReport {
        run(EnsureVerifiedCliVersion {
            provider: &fixture.provider,
            source: &fixture.source,
            ledger: &fixture.ledger,
            events: &fixture.events,
        }
        .execute(previous))
    }
    fn passes(fixture: &Fixture, kinds: &[RunKind]) {
        for _kind in kinds {
            fixture
                .provider
                .script_verify_isolation()
                .push(Ok(IsolationCheck::Passed));
        }
    }
    fn healthy() -> SelfCheckInput {
        SelfCheckInput {
            toss_key: CheckState::Healthy,
            allowed_ip: CheckState::Healthy,
            duplicate_key_use: CheckState::Healthy,
            claude_install: CheckState::Healthy,
            claude_login: CheckState::Healthy,
            ai_environment: CheckState::Healthy,
            ai_usage: AiUsage::Unknown,
            order_prerequisite: CheckState::Healthy,
        }
    }

    #[test]
    fn known_version_needs_only_the_analysis_check() {
        let fixture = fixture(vec![version("2.1.322")]);
        passes(&fixture, &[RunKind::Analysis]);
        fixture.source.push(Some(version("2.1.322")));
        let report = gate(&fixture, &CliVersionGateOutcome::VersionUnknown);
        assert_eq!(
            report.outcome,
            CliVersionGateOutcome::AlreadyVerified(version("2.1.322"))
        );
        assert_eq!(report.outcome.ai_enabled(), AiGate::Enabled);
        assert_eq!(
            fixture.provider.calls(),
            vec![AiProviderCall::VerifyIsolation(RunKind::Analysis)]
        );
        assert_eq!(
            fixture.received.try_recv(),
            Ok(DomainEvent::SelfCheckChanged)
        );
    }

    #[test]
    fn new_version_is_recorded_only_after_both_run_kinds_pass() {
        let fixture = fixture(vec![version("2.1.322")]);
        passes(&fixture, &[RunKind::Analysis, RunKind::Collection]);
        fixture.source.push(Some(version("2.2.0")));
        fixture.source.push(Some(version("2.2.0")));
        let previous = CliVersionGateOutcome::AlreadyVerified(version("2.1.322"));
        let report = gate(&fixture, &previous);
        assert_eq!(
            report.outcome,
            CliVersionGateOutcome::NewlyVerified(version("2.2.0"))
        );
        assert_eq!(
            fixture.provider.calls(),
            vec![
                AiProviderCall::VerifyIsolation(RunKind::Analysis),
                AiProviderCall::VerifyIsolation(RunKind::Collection),
            ]
        );
        assert_eq!(
            fixture.ledger.versions(),
            vec![version("2.1.322"), version("2.2.0")]
        );
    }

    #[test]
    fn failing_new_version_turns_ai_off_without_a_main_screen_banner() {
        let fixture = fixture(vec![version("2.1.322")]);
        passes(&fixture, &[RunKind::Analysis]);
        fixture
            .provider
            .script_verify_isolation()
            .push(Ok(IsolationCheck::Rejected(IsolationFailure::Tools)));
        fixture.source.push(Some(version("2.3.0")));
        let report = gate(&fixture, &CliVersionGateOutcome::VersionUnknown);
        assert_eq!(
            report.outcome,
            CliVersionGateOutcome::Disabled(AiCheckObservation::Environment(
                AiEnvironmentProblem::Tools
            ))
        );
        assert_eq!(report.outcome.ai_enabled(), AiGate::Disabled);
        assert_eq!(
            fixture.ledger.versions(),
            vec![version("2.1.322")],
            "기록하지 않음"
        );
        let mut input = healthy();
        report.outcome.apply_to_self_check(&mut input);
        let result = evaluate_self_check(input, Vec::new());
        assert_eq!(result.banner, None, "주 화면 상태 띠 0개");
        assert_eq!(
            result.records.ai_environment,
            CheckState::Problem(AiEnvironmentProblem::Tools),
            "고급 > 상태에 사유"
        );
    }

    #[test]
    fn login_failure_and_probe_error_disable_ai() {
        let fixture = fixture(Vec::new());
        fixture
            .provider
            .script_verify_isolation()
            .push(Ok(IsolationCheck::Rejected(
                IsolationFailure::LoginRequired,
            )));
        let login = gate(&fixture, &CliVersionGateOutcome::VersionUnknown);
        assert_eq!(
            login.outcome,
            CliVersionGateOutcome::Disabled(AiCheckObservation::Login(
                ClaudeLoginProblem::LoginRequired
            ))
        );
        fixture
            .provider
            .script_verify_isolation()
            .push(Err(InformationError::Unavailable(ResetTime::NotProvided)));
        let errored = gate(&fixture, &login.outcome);
        assert_eq!(errored.outcome, CliVersionGateOutcome::ProbeFailed);
        assert_eq!(errored.outcome.ai_enabled(), AiGate::Disabled);
        assert_eq!(
            errored.outcome.login_recheck(),
            application::LoginRecheck::Needed,
            "점검을 못 돌리면 AI-13 로그인 점검을 다시 돌려 5번을 정한다"
        );
        assert_eq!(
            login.outcome.login_recheck(),
            application::LoginRecheck::NotNeeded
        );
        assert_eq!(fixture.ledger.versions(), Vec::<CliVersion>::new());
    }

    #[test]
    fn unknown_or_changing_version_and_storage_failure_stay_disabled() {
        let fixture = fixture(Vec::new());
        passes(&fixture, &[RunKind::Analysis]);
        fixture.source.push(None);
        assert_eq!(
            gate(&fixture, &CliVersionGateOutcome::VersionUnknown).outcome,
            CliVersionGateOutcome::VersionUnknown
        );
        assert!(
            fixture.received.try_recv().is_err(),
            "같은 결과면 이벤트 없음"
        );
        passes(&fixture, &[RunKind::Analysis, RunKind::Collection]);
        fixture.source.push(Some(version("3.0.0")));
        fixture.source.push(Some(version("3.0.1")));
        assert_eq!(
            gate(&fixture, &CliVersionGateOutcome::VersionUnknown).outcome,
            CliVersionGateOutcome::VersionUnknown,
            "점검 중 버전이 바뀌면 기록하지 않음"
        );
        let failure = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        passes(&fixture, &[RunKind::Analysis]);
        fixture.source.push(Some(version("3.0.1")));
        fixture.ledger.fail_next(failure);
        assert_eq!(
            gate(&fixture, &CliVersionGateOutcome::VersionUnknown).outcome,
            CliVersionGateOutcome::Storage(failure)
        );
        assert_eq!(fixture.ledger.versions(), Vec::<CliVersion>::new());
    }

    #[test]
    fn version_text_is_validated() {
        assert_eq!(version("2.1.322-beta+7").as_str(), "2.1.322-beta+7");
        for bad in ["", "2 1", "2/1", "버전"] {
            assert_eq!(
                CliVersion::try_new(bad),
                Err(application::IdentifierError::InvalidFormat)
            );
        }
    }
}
