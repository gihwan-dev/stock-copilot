#[cfg(test)]
mod tests {
    mod a27_fixtures;
    mod d56_evidence;
    use a27_fixtures as fixtures;
    use application::{
        AiRunId, EstimateSubmissionInput, EstimateSubmissionRepository, EventBusBuilder,
        ReasoningEffort, SignalCommit, SignalCommitOutcome, SignalEvaluation, SignalMutation,
        SignalRepository, SignalSnapshot, SubmitEstimate, SystemFailure, SystemPort,
        SystemPortError,
    };
    use domain::signal_transition_from_history;
    use domain::{
        BuySignalOccurrenceId, BuySignalRecord, DomainEvent, EstimateAssignments,
        EstimateSubmissionRecord, InstrumentId, IssuedEstimateTask, Market, NoSignalReason,
        SignalDecision, SignalTransition, SignalUnavailable, signal_transition,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::memory::MemorySignalEvaluationRepository;

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
    fn invalid() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
    }
    fn submit(repository: &MemorySignalEvaluationRepository, task: &str, probability: &str) {
        let events = EventBusBuilder::default().build();
        run(SubmitEstimate::new(repository, &events).execute(
            &fixtures::run_id(),
            vec![fixtures::input(task, probability, &fixtures::master())],
        ))
        .expect("제출");
    }
    fn occurrence(decision: &SignalDecision, id: &str) -> BuySignalRecord {
        let SignalDecision::Buy(signal) = decision else {
            panic!("매수")
        };
        let calendars: Vec<_> = (60..=66)
            .map(|day| fixtures::calendar(day, &Market::Kr))
            .collect();
        BuySignalRecord::from_signal(
            BuySignalOccurrenceId::try_new(id).expect("ID"),
            signal,
            fixtures::day(60),
            &calendars,
        )
        .expect("발생")
    }
    fn evaluate(
        snapshot: &SignalSnapshot,
        decision: SignalDecision,
        occurrence: Option<BuySignalRecord>,
    ) -> SignalMutation {
        let transition = signal_transition_from_history(
            snapshot.current.as_ref().map(|entry| &entry.signal),
            &decision,
        )
        .expect("전이");
        let evidence = d56_evidence::evidence_for_decision(&decision, occurrence.as_ref());
        SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snapshot.submission.clone().expect("과제"),
            decision,
            transition,
            calibration_head: Some(fixtures::head(0)),
            occurrence,
            evidence,
        }))
    }
    fn record(
        issued: &IssuedEstimateTask<AiRunId>,
        input: EstimateSubmissionInput,
    ) -> EstimateSubmissionRecord<AiRunId, ReasoningEffort> {
        EstimateSubmissionRecord::create(issued, input.submission, input.narrative, input.metadata)
            .expect("제출 기록")
    }

    #[test]
    fn current_snapshot_without_signal_rejects_forged_clear() {
        let repository = MemorySignalEvaluationRepository::default();
        let instrument = InstrumentId::try_new("empty").expect("종목");
        let state = run(repository.snapshot(&instrument)).expect("상태");
        assert_eq!(
            run(repository.commit(
                &instrument,
                &state,
                &SignalMutation::Discard(Box::new(SignalTransition::Cleared(instrument.clone()))),
            )),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(state));
    }
    #[test]
    fn current_snapshot_without_submission_rejects_evaluation() {
        let assignments = fixtures::assignments();
        let task = assignments
            .lookup(&fixtures::task_id("task-old"))
            .expect("과제")
            .id()
            .clone();
        let repository = MemorySignalEvaluationRepository::new(assignments);
        let instrument = fixtures::id("sample-a");
        let state = run(repository.snapshot(&instrument)).expect("상태");
        let mutation = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: task,
            decision: SignalDecision::None(NoSignalReason::MissingSource),
            transition: SignalTransition::Unchanged,
            calibration_head: None,
            occurrence: None,
            evidence: None,
        }));
        assert_eq!(
            run(repository.commit(&instrument, &state, &mutation)),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(state));
    }
    fn repository_with_discarded_old_task() -> (
        MemorySignalEvaluationRepository,
        domain::IssuedEstimateTaskId,
        SignalDecision,
    ) {
        let repository = MemorySignalEvaluationRepository::new(fixtures::assignments());
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("위치");
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "99");
        let first = run(repository.snapshot(&instrument)).expect("첫 제출");
        let old_task = first.submission.clone().expect("과제");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let raised_result = run(repository.commit(
            &instrument,
            &first,
            &evaluate(
                &first,
                decision.clone(),
                Some(occurrence(&decision, "original")),
            ),
        ))
        .expect("발생");
        let SignalCommitOutcome::Applied(_) = raised_result else {
            panic!("발생 성공")
        };
        let raised = run(repository.snapshot(&instrument)).expect("현재");
        let discarded_result = run(repository.commit(
            &instrument,
            &raised,
            &SignalMutation::Discard(Box::new(SignalTransition::Cleared(instrument.clone()))),
        ))
        .expect("폐기");
        let SignalCommitOutcome::Applied(_) = discarded_result else {
            panic!("폐기 성공")
        };
        (repository, old_task, decision)
    }

    #[test]
    fn discarded_old_task_cannot_mask_newer_submission_as_discarded() {
        let (repository, old_task, decision) = repository_with_discarded_old_task();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-new", "99");
        let latest = run(repository.snapshot(&instrument)).expect("새 제출");
        assert_ne!(latest.submission, Some(old_task.clone()));
        let mut stale = evaluate(
            &latest,
            decision.clone(),
            Some(occurrence(&decision, "stale")),
        );
        let SignalMutation::Evaluate(value) = &mut stale else {
            panic!("평가")
        };
        value.submission = old_task;
        assert_eq!(
            run(repository.commit(&instrument, &latest, &stale)),
            Ok(SignalCommitOutcome::Conflict)
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(latest.clone()));
        let latest_decision = fixtures::decision(&fixtures::master(), "99", None);
        let new_evaluation = evaluate(
            &latest,
            latest_decision.clone(),
            Some(occurrence(&latest_decision, "latest")),
        );
        let SignalCommitOutcome::Applied(commit) =
            run(repository.commit(&instrument, &latest, &new_evaluation)).expect("새 평가")
        else {
            panic!("새 제출의 평가가 저장돼야 함");
        };
        assert_eq!(
            commit.events,
            vec![DomainEvent::SignalRaised(instrument.clone())]
        );
        assert_eq!(
            run(repository.snapshot(&instrument))
                .expect("현재")
                .current
                .expect("새 신호")
                .submission,
            latest.submission.expect("새 제출")
        );
    }
    #[test]
    fn assignment_change_without_records_is_rejected_atomically() {
        let previous = fixtures::assignments();
        let next = previous
            .consume(&fixtures::run_id(), &[fixtures::task_id("task-old")])
            .expect("소비");
        let repository = MemorySignalEvaluationRepository::new(previous.clone());
        assert_eq!(
            run(repository.commit_submission(&previous, &previous, &[])),
            Ok(application::EstimateCommitOutcome::Committed)
        );
        assert_eq!(
            run(repository.commit_submission(&previous, &next, &[])),
            Err(invalid())
        );
        assert_eq!(run(repository.assignments()), Ok(previous));
        assert_eq!(
            run(repository.submission_records(&fixtures::id("sample-a"))),
            Ok(vec![])
        );
    }
    #[test]
    fn forged_record_from_other_run_is_rejected_atomically() {
        let previous = fixtures::assignments();
        let ids = [fixtures::task_id("task-old"), fixtures::task_id("task-new")];
        let next = previous.consume(&fixtures::run_id(), &ids).expect("소비");
        let other_run = AiRunId::try_new("different-run").expect("실행");
        let other = EstimateAssignments::default()
            .issue(
                ids[1].clone(),
                other_run,
                fixtures::eligible_for_master("102", &fixtures::master()),
            )
            .expect("다른 실행");
        let records = vec![
            record(
                previous.lookup(&ids[0]).expect("기존 과제"),
                fixtures::input("task-old", "99", &fixtures::master()),
            ),
            record(
                other.lookup(&ids[1]).expect("다른 실행 과제"),
                fixtures::input("task-new", "98", &fixtures::master()),
            ),
        ];
        let repository = MemorySignalEvaluationRepository::new(previous.clone());
        assert_eq!(
            run(repository.commit_submission(&previous, &next, &records)),
            Err(invalid())
        );
        assert_eq!(run(repository.assignments()), Ok(previous));
        assert_eq!(
            run(repository.submission_records(&fixtures::id("sample-a"))),
            Ok(vec![])
        );
    }
    #[test]
    fn forged_issued_task_content_is_rejected_atomically() {
        let previous = fixtures::assignments();
        let id = fixtures::task_id("task-old");
        let next = previous
            .consume(&fixtures::run_id(), std::slice::from_ref(&id))
            .expect("소비");
        let other = EstimateAssignments::default()
            .issue(
                id.clone(),
                fixtures::run_id(),
                fixtures::eligible_for_master("104", &fixtures::other_master()),
            )
            .expect("다른 과제");
        let forged = record(
            other.lookup(&id).expect("발급"),
            fixtures::input("task-old", "99", &fixtures::other_master()),
        );
        let repository = MemorySignalEvaluationRepository::new(previous.clone());
        assert_eq!(
            run(repository.commit_submission(&previous, &next, &[forged])),
            Err(invalid())
        );
        assert_eq!(run(repository.assignments()), Ok(previous));
        assert_eq!(
            run(repository.submission_records(&fixtures::id("sample-a"))),
            Ok(vec![])
        );
        assert_eq!(
            run(repository.submission_records(&fixtures::id("sample-b"))),
            Ok(vec![])
        );
    }
    #[test]
    fn missing_calibration_cannot_store_a_signal_but_preserves_missing_reason() {
        let repository = MemorySignalEvaluationRepository::new(fixtures::assignments());
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "99");
        let state = run(repository.snapshot(&instrument)).expect("상태");
        let buy = fixtures::decision(&fixtures::master(), "99", None);
        let mut mutation = evaluate(&state, buy.clone(), Some(occurrence(&buy, "missing-head")));
        assert_eq!(
            run(repository.commit(&instrument, &state, &mutation)),
            Ok(SignalCommitOutcome::Conflict)
        );
        let SignalMutation::Evaluate(value) = &mut mutation else {
            panic!("평가")
        };
        value.calibration_head = None;
        assert_eq!(
            run(repository.commit(&instrument, &state, &mutation)),
            Err(invalid())
        );
        let reason = NoSignalReason::Unavailable(SignalUnavailable::MissingCalibration);
        let no_signal = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: state.submission.clone().expect("과제"),
            decision: SignalDecision::None(reason),
            transition: SignalTransition::Unchanged,
            calibration_head: None,
            occurrence: None,
            evidence: None,
        }));
        assert_eq!(
            run(repository.commit(&instrument, &state, &no_signal)),
            Ok(SignalCommitOutcome::Applied(SignalCommit {
                events: vec![],
                no_signal_reason: Some(reason),
            }))
        );
        assert_eq!(
            run(repository.snapshot(&instrument)).expect("상태").current,
            None
        );
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            )),
            Ok(vec![])
        );
    }

    #[test]
    fn missing_calibration_cannot_store_a_sell_signal() {
        let repository = MemorySignalEvaluationRepository::new(fixtures::assignments());
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "10");
        let state = run(repository.snapshot(&instrument)).expect("상태");
        let sell = fixtures::decision(&fixtures::master(), "10", Some("7"));
        let SignalDecision::Sell(_) = &sell else {
            panic!("매도 신호 필요");
        };
        let mut mutation = evaluate(&state, sell, None);
        let SignalMutation::Evaluate(value) = &mut mutation else {
            panic!("평가 필요");
        };
        value.calibration_head = None;
        assert_eq!(
            run(repository.commit(&instrument, &state, &mutation)),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(state));
    }
    #[test]
    fn empty_history_transition_matches_live_decision() {
        let decision = fixtures::decision(&fixtures::master(), "10", Some("7"));
        assert_eq!(
            signal_transition(None, &decision),
            signal_transition_from_history(None, &decision)
        );
    }
}
