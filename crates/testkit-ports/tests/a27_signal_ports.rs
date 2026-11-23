#[cfg(test)]
mod tests {
    mod a27_fixtures;
    use a27_fixtures as fixtures;
    use application::{
        EstimateSubmissionRepository, EventBusBuilder, SignalCommitOutcome, SignalEvaluation,
        SignalMutation, SignalRepository, SignalRevision, SignalSourceFacts, SubmitEstimate,
        SystemFailure, SystemPort, SystemPortError,
    };
    use domain::signal_transition_from_history;
    use domain::{
        BuySignalOccurrenceId, BuySignalRecord, CurrentBuySignal, DartReceiptNo, Disclosure,
        DomainEvent, HorizonSelection, InstrumentId, Market, NoSignalReason, NormalizedUrl,
        ReceiptNo, SignalDecision, SignalTransition, ToolReference, UntrustedText,
        signal_transition,
    };
    use std::{
        future::Future,
        sync::Barrier,
        task::{Context, Poll, Waker},
    };
    struct HistoricalDecision(Option<domain::HistoricalSignal>);
    impl HistoricalDecision {
        fn into_signal(self) -> Option<domain::HistoricalSignal> {
            self.0
        }
    }
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

    #[test]
    fn empty_instrument_has_an_initial_revision_and_cannot_discard_a_missing_signal() {
        let repository = MemorySignalEvaluationRepository::default();
        let instrument = InstrumentId::try_new("empty-instrument").expect("종목");
        let snapshot = run(repository.snapshot(&instrument)).expect("조회");
        assert_eq!(snapshot.revision, SignalRevision::new(0));
        assert_eq!(snapshot.submission, None);
        assert_eq!(snapshot.current, None);
        assert_eq!(
            run(repository.commit(
                &instrument,
                &snapshot,
                &SignalMutation::Discard(Box::new(SignalTransition::Unchanged))
            )),
            Ok(SignalCommitOutcome::Applied(application::SignalCommit {
                events: vec![],
                no_signal_reason: None,
            }))
        );
        assert_eq!(
            run(repository.commit(
                &instrument,
                &snapshot,
                &SignalMutation::Discard(Box::new(SignalTransition::Cleared(instrument.clone())))
            )),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(snapshot));
    }

    fn expect_buy(decision: &SignalDecision) {
        match decision {
            SignalDecision::Buy(_) => (),
            SignalDecision::Sell(_) | SignalDecision::None(_) => panic!("매수 신호 필요"),
        }
    }
    fn expect_sell(decision: &SignalDecision) {
        match decision {
            SignalDecision::Sell(_) => (),
            SignalDecision::Buy(_) | SignalDecision::None(_) => panic!("매도 신호 필요"),
        }
    }
    fn committed(
        result: Result<SignalCommitOutcome, SystemPortError>,
    ) -> application::SignalCommit {
        match result.expect("저장 성공") {
            SignalCommitOutcome::Applied(commit) => commit,
            SignalCommitOutcome::Conflict => panic!("예상치 못한 충돌"),
            SignalCommitOutcome::SubmissionDiscarded => panic!("예상치 못한 폐기"),
        }
    }
    fn invalid() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
    }
    #[test]
    fn submitted_decisions_have_valid_fixtures() {
        expect_buy(&fixtures::decision(&fixtures::master(), "99", None));
        expect_sell(&fixtures::decision(&fixtures::master(), "10", Some("7")));
    }

    fn repository() -> MemorySignalEvaluationRepository {
        let repository = MemorySignalEvaluationRepository::new(fixtures::assignments());
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("보정 위치");
        repository
    }

    #[derive(Clone, Copy)]
    enum InstrumentChoice {
        First,
        Other,
    }
    fn submit(
        repository: &MemorySignalEvaluationRepository,
        name: &str,
        probability: &str,
        choice: InstrumentChoice,
    ) {
        let source = match choice {
            InstrumentChoice::First => fixtures::master(),
            InstrumentChoice::Other => fixtures::other_master(),
        };
        let events = EventBusBuilder::default().build();
        run(SubmitEstimate::new(repository, &events).execute(
            &fixtures::run_id(),
            vec![fixtures::input(name, probability, &source)],
        ))
        .expect("제출");
    }

    fn evaluation(
        snapshot: &application::SignalSnapshot,
        decision: SignalDecision,
        occurrence: Option<BuySignalRecord>,
    ) -> SignalMutation {
        let transition = signal_transition_from_history(
            snapshot.current.as_ref().map(|entry| &entry.signal),
            &decision,
        )
        .expect("전이");
        let evidence = fixtures::evidence_for_decision(&decision, occurrence.as_ref());
        SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: snapshot.submission.clone().expect("판정 근거 제출"),
            decision,
            transition,
            calibration_head: Some(fixtures::head(0)),
            occurrence,
            evidence,
        }))
    }

    fn buy_record(decision: &SignalDecision, name: &str) -> BuySignalRecord {
        let SignalDecision::Buy(signal) = decision else {
            panic!("매수 필요")
        };
        let calendars: Vec<_> = (60..=66)
            .map(|value| fixtures::calendar(value, &Market::Kr))
            .collect();
        BuySignalRecord::from_signal(
            BuySignalOccurrenceId::try_new(name).expect("발생 ID"),
            signal,
            fixtures::day(60),
            &calendars,
        )
        .expect("발생")
    }

    #[test]
    fn simultaneous_evaluations_commit_and_emit_once() {
        let repository = repository();
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let snapshot = run(repository.snapshot(&instrument)).expect("조회");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let mutation = evaluation(
            &snapshot,
            decision.clone(),
            Some(buy_record(&decision, "occ-1")),
        );
        let barrier = Barrier::new(3);
        let results = std::thread::scope(|scope| {
            let run_attempt = || {
                barrier.wait();
                run(repository.commit(&instrument, &snapshot, &mutation)).expect("저장")
            };
            let first = scope.spawn(run_attempt);
            let second = scope.spawn(run_attempt);
            barrier.wait();
            vec![
                first.join().expect("첫 스레드"),
                second.join().expect("두 번째 스레드"),
            ]
        });
        let events = results
            .iter()
            .flat_map(|value| match value {
                SignalCommitOutcome::Applied(commit) => commit.events.clone(),
                SignalCommitOutcome::Conflict | SignalCommitOutcome::SubmissionDiscarded => vec![],
            })
            .collect::<Vec<_>>();
        assert_eq!(events, vec![DomainEvent::SignalRaised(instrument.clone())]);
        assert_eq!(
            results
                .iter()
                .filter(|value| **value == SignalCommitOutcome::Conflict)
                .count(),
            1
        );
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            ))
            .expect("이력")
            .len(),
            1
        );
    }

    #[test]
    fn latest_submission_wins_even_if_older_evaluation_finishes_last() {
        let repository = repository();
        submit(&repository, "task-old", "10", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let old = run(repository.snapshot(&instrument)).expect("이전");
        let old_decision = fixtures::decision(&fixtures::master(), "10", Some("7"));
        let old_mutation = evaluation(&old, old_decision, None);
        submit(&repository, "task-new", "99", InstrumentChoice::First);
        let newest = run(repository.snapshot(&instrument)).expect("최신");
        assert_ne!(old.submission, newest.submission);
        let live_decision = fixtures::decision(&fixtures::master(), "99", None);
        let new_mutation = evaluation(
            &newest,
            live_decision.clone(),
            Some(buy_record(&live_decision, "new-occ")),
        );
        committed(run(repository.commit(&instrument, &newest, &new_mutation)));
        assert_eq!(
            run(repository.commit(&instrument, &old, &old_mutation)),
            Ok(SignalCommitOutcome::Conflict)
        );
        let current = run(repository.snapshot(&instrument))
            .expect("현재")
            .current
            .expect("신호");
        let new_decision = HistoricalDecision(
            live_decision
                .into_signal()
                .as_ref()
                .map(domain::HistoricalSignal::from_signal),
        );
        assert_eq!(current.submission, newest.submission.expect("최신 제출"));
        assert_eq!(Some(current.signal), new_decision.into_signal());
        assert_eq!(
            run(repository.submission_records(&instrument))
                .expect("제출 이력")
                .len(),
            2
        );
    }

    #[test]
    fn discarded_task_cannot_return_even_after_the_evaluator_rereads() {
        let repository = repository();
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let initial = run(repository.snapshot(&instrument)).expect("초기");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let raised = evaluation(
            &initial,
            decision.clone(),
            Some(buy_record(&decision, "discarded-occ")),
        );
        committed(run(repository.commit(&instrument, &initial, &raised)));
        let previous = run(repository.snapshot(&instrument)).expect("발생 후");
        let stale = evaluation(&previous, decision.clone(), None);
        let discard =
            SignalMutation::Discard(Box::new(SignalTransition::Cleared(instrument.clone())));
        let result = run(repository.commit(&instrument, &previous, &discard)).expect("폐기");
        assert_eq!(
            committed(Ok(result)).events,
            vec![DomainEvent::SignalCleared(instrument.clone())]
        );
        assert_eq!(
            run(repository.commit(&instrument, &previous, &stale)),
            Ok(SignalCommitOutcome::Conflict)
        );
        let after = run(repository.snapshot(&instrument)).expect("폐기 후");
        let retried = evaluation(
            &after,
            decision.clone(),
            Some(buy_record(&decision, "retry-occ")),
        );
        assert_eq!(
            run(repository.commit(&instrument, &after, &retried)),
            Ok(SignalCommitOutcome::SubmissionDiscarded)
        );
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            ))
            .expect("이력")
            .len(),
            1
        );
        submit(&repository, "task-new", "98", InstrumentChoice::First);
        let latest = run(repository.snapshot(&instrument)).expect("새 제출");
        let next = fixtures::decision(&fixtures::master(), "98", None);
        let fresh = evaluation(&latest, next.clone(), Some(buy_record(&next, "fresh-occ")));
        committed(run(repository.commit(&instrument, &latest, &fresh)));
    }

    #[test]
    fn another_instrument_does_not_invalidate_the_first_snapshot() {
        let repository = repository();
        submit(&repository, "task-old", "10", InstrumentChoice::First);
        let first = fixtures::id("sample-a");
        let before = run(repository.snapshot(&first)).expect("첫 종목");
        submit(&repository, "task-other", "10", InstrumentChoice::Other);
        let second = fixtures::id("sample-b");
        let other = run(repository.snapshot(&second)).expect("다른 종목");
        let second_decision = fixtures::decision(&fixtures::other_master(), "10", Some("11"));
        committed(run(repository.commit(
            &second,
            &other,
            &evaluation(&other, second_decision, None),
        )));
        let decision = fixtures::decision(&fixtures::master(), "10", Some("7"));
        committed(run(repository.commit(
            &first,
            &before,
            &evaluation(&before, decision, None),
        )));
        assert_ne!(
            run(repository.snapshot(&first)).expect("첫 종목").current,
            run(repository.snapshot(&second))
                .expect("다른 종목")
                .current
        );
    }

    #[test]
    fn failed_transition_has_no_occurrence_and_update_keeps_one() {
        let repository = repository();
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let initial = run(repository.snapshot(&instrument)).expect("초기");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let live_occurrence = buy_record(&decision, "stable-occ");
        let mutation = evaluation(&initial, decision, Some(live_occurrence.clone()));
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        repository.fail_next_commit(error);
        assert_eq!(
            run(repository.commit(&instrument, &initial, &mutation)),
            Err(error)
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(initial.clone()));
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            )),
            Ok(vec![])
        );
        committed(run(repository.commit(&instrument, &initial, &mutation)));
        submit(&repository, "task-new", "98", InstrumentChoice::First);
        let updated = run(repository.snapshot(&instrument)).expect("갱신 전");
        let next = fixtures::decision(&fixtures::master(), "98", None);
        let result =
            run(repository.commit(&instrument, &updated, &evaluation(&updated, next, None)))
                .expect("갱신");
        assert_eq!(committed(Ok(result)).events, vec![]);
        let occurrence = domain::HistoricalSignalOccurrence::from_record(&live_occurrence);
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            )),
            Ok(vec![occurrence.clone()])
        );
        assert_eq!(
            run(repository.snapshot(&instrument))
                .expect("현재")
                .current
                .expect("매수")
                .occurrence,
            Some(occurrence)
        );
    }

    #[test]
    fn source_facts_are_independent_and_reason_survives_the_commit() {
        let repository = repository();
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let state = run(repository.snapshot(&instrument)).expect("상태");
        let task = state.submission.clone().expect("과제");
        assert!(
            run(repository.submission_records(&instrument)).expect("제출")[0]
                .submission()
                .evidence()
                .is_some()
        );
        assert_eq!(run(repository.sources(&task)), Err(invalid()));
        repository.save_sources(task.clone(), SignalSourceFacts::default());
        let empty = run(repository.sources(&task)).expect("빈 원장");
        assert_eq!(empty, SignalSourceFacts::default());
        assert_eq!(
            fixtures::decision_with_sources(&fixtures::master(), "99", None, &empty.items),
            SignalDecision::None(NoSignalReason::MissingSource)
        );
        let expected = SignalSourceFacts {
            items: vec![fixtures::item()],
            disclosures: vec![Disclosure::new(
                ReceiptNo::Dart(DartReceiptNo::try_new("20261117000123").expect("공시 번호")),
                NormalizedUrl::try_new("https://example.com/disclosure/123").expect("공시 주소"),
                UntrustedText::try_new("별도 공시").expect("제목"),
                None,
            )],
            tools: vec![ToolReference::try_new("tool:quote/731").expect("도구 참조")],
        };
        repository.save_sources(task.clone(), expected.clone());
        assert_eq!(run(repository.sources(&task)), Ok(expected.clone()));
        expect_buy(&fixtures::decision_with_sources(
            &fixtures::master(),
            "99",
            None,
            &expected.items,
        ));
        let decision = SignalDecision::None(NoSignalReason::MissingSource);
        let result =
            run(repository.commit(&instrument, &state, &evaluation(&state, decision, None)))
                .expect("저장");
        let commit = committed(Ok(result));
        assert_eq!(commit.no_signal_reason, Some(NoSignalReason::MissingSource));
        assert_eq!(commit.events, vec![]);
    }

    #[test]
    fn newer_calibration_head_and_foreign_signal_are_rejected() {
        let repository = repository();
        submit(&repository, "task-old", "10", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let state = run(repository.snapshot(&instrument)).expect("상태");
        let wrong = fixtures::decision(&fixtures::other_master(), "10", Some("7"));
        assert_eq!(
            run(repository.commit(&instrument, &state, &evaluation(&state, wrong, None))),
            Err(invalid())
        );
        let decision = fixtures::decision(&fixtures::master(), "10", Some("7"));
        let old_evaluation = evaluation(&state, decision, None);
        repository
            .advance_calibration_head(fixtures::head(1))
            .expect("새 위치");
        assert_eq!(
            run(repository.commit(&instrument, &state, &old_evaluation)),
            Ok(SignalCommitOutcome::Conflict)
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(state));
    }

    #[test]
    fn discarded_old_signal_does_not_discard_a_newer_submission() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "10", InstrumentChoice::First);
        let initial = run(repository.snapshot(&instrument)).expect("이전 제출");
        let old = fixtures::decision(&fixtures::master(), "10", Some("7"));
        committed(run(repository.commit(
            &instrument,
            &initial,
            &evaluation(&initial, old, None),
        )));
        submit(&repository, "task-new", "99", InstrumentChoice::First);
        let snapshot = run(repository.snapshot(&instrument)).expect("새 제출과 이전 신호");
        assert_ne!(
            snapshot.submission.as_ref(),
            snapshot.current.as_ref().map(|value| &value.submission)
        );
        let discard =
            SignalMutation::Discard(Box::new(SignalTransition::Cleared(instrument.clone())));
        committed(run(repository.commit(&instrument, &snapshot, &discard)));
        let after = run(repository.snapshot(&instrument)).expect("폐기 후");
        let next = fixtures::decision(&fixtures::master(), "99", None);
        committed(run(repository.commit(
            &instrument,
            &after,
            &evaluation(
                &after,
                next.clone(),
                Some(buy_record(&next, "new-after-old-discard")),
            ),
        )));
    }

    #[test]
    fn occurrence_id_collision_is_atomic_and_history_matches_id_or_symbol() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        let identity = fixtures::master().order_instrument().expect("종목");
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let initial = run(repository.snapshot(&instrument)).expect("초기");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let live_record = buy_record(&decision, "same-id");
        committed(run(repository.commit(
            &instrument,
            &initial,
            &evaluation(&initial, decision, Some(live_record.clone())),
        )));
        let raised = run(repository.snapshot(&instrument)).expect("발생");
        committed(run(repository.commit(
            &instrument,
            &raised,
            &SignalMutation::Discard(Box::new(SignalTransition::Cleared(instrument.clone()))),
        )));
        submit(&repository, "task-new", "98", InstrumentChoice::First);
        let latest = run(repository.snapshot(&instrument)).expect("최신");
        let next = fixtures::decision(&fixtures::master(), "98", None);
        let duplicate = buy_record(&next, "same-id");
        assert_eq!(
            run(repository.commit(
                &instrument,
                &latest,
                &evaluation(&latest, next, Some(duplicate))
            )),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(latest));
        let record = domain::HistoricalSignalOccurrence::from_record(&live_record);
        assert_eq!(
            run(repository.occurrences(&instrument, &identity)),
            Ok(vec![record.clone()])
        );
        assert_eq!(
            run(repository.occurrences(&fixtures::id("different-id"), &identity)),
            Ok(vec![record.clone()])
        );
        let other_identity = fixtures::other_master()
            .order_instrument()
            .expect("다른 종목");
        assert_eq!(
            run(repository.occurrences(&instrument, &other_identity)),
            Ok(vec![record])
        );
        assert_eq!(
            run(repository.occurrences(&fixtures::id("different-id"), &other_identity)),
            Ok(vec![])
        );
    }

    #[test]
    fn clearing_and_recreating_does_not_reuse_an_old_empty_snapshot() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "10", InstrumentChoice::First);
        let initial = run(repository.snapshot(&instrument)).expect("초기");
        let sell = fixtures::decision(&fixtures::master(), "10", Some("7"));
        let raise = evaluation(&initial, sell.clone(), None);
        committed(run(repository.commit(&instrument, &initial, &raise)));
        let raised = run(repository.snapshot(&instrument)).expect("발생");
        let clear = evaluation(
            &raised,
            SignalDecision::None(NoSignalReason::ConditionsUnmet),
            None,
        );
        committed(run(repository.commit(&instrument, &raised, &clear)));
        let cleared = run(repository.snapshot(&instrument)).expect("해제");
        assert_eq!(cleared.current, initial.current);
        assert_eq!(cleared.submission, initial.submission);
        assert_ne!(cleared.revision, initial.revision);
        assert_eq!(
            run(repository.commit(&instrument, &initial, &raise)),
            Ok(SignalCommitOutcome::Conflict)
        );
        committed(run(repository.commit(
            &instrument,
            &cleared,
            &evaluation(&cleared, sell, None),
        )));
    }

    #[test]
    fn direction_reversal_returns_clear_then_raise() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "10", InstrumentChoice::First);
        let before = run(repository.snapshot(&instrument)).expect("이전");
        let sell = fixtures::decision(&fixtures::master(), "10", Some("7"));
        committed(run(repository.commit(
            &instrument,
            &before,
            &evaluation(&before, sell, None),
        )));
        submit(&repository, "task-new", "99", InstrumentChoice::First);
        let state = run(repository.snapshot(&instrument)).expect("새 제출");
        let buy = fixtures::decision(&fixtures::master(), "99", None);
        let result = run(repository.commit(
            &instrument,
            &state,
            &evaluation(&state, buy.clone(), Some(buy_record(&buy, "reversal"))),
        ))
        .expect("방향 전환");
        assert_eq!(
            committed(Ok(result)).events,
            vec![
                DomainEvent::SignalCleared(instrument.clone()),
                DomainEvent::SignalRaised(instrument)
            ]
        );
    }

    #[test]
    fn evaluation_cannot_claim_a_head_newer_than_the_signal_used() {
        let repository = repository();
        submit(&repository, "task-old", "10", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let state = run(repository.snapshot(&instrument)).expect("상태");
        let decision = fixtures::decision(&fixtures::master(), "10", Some("7"));
        let mut old_evaluation = evaluation(&state, decision, None);
        repository
            .advance_calibration_head(fixtures::head(1))
            .expect("원장 증가");
        if let SignalMutation::Evaluate(ref mut value) = old_evaluation {
            value.calibration_head = Some(fixtures::head(1));
        }
        assert_eq!(
            run(repository.commit(&instrument, &state, &old_evaluation)),
            Err(invalid())
        );
        assert_eq!(
            repository.advance_calibration_head(fixtures::head(0)),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(state));
    }
    #[test]
    fn identical_old_submission_cannot_be_rebound_to_a_newer_snapshot() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let old = run(repository.snapshot(&instrument)).expect("이전 제출");
        let old_id = old.submission.expect("이전 과제");
        let old_decision = fixtures::decision(&fixtures::master(), "99", None);
        submit(&repository, "task-new", "99", InstrumentChoice::First);
        let latest = run(repository.snapshot(&instrument)).expect("새 제출");
        let records = run(repository.submission_records(&instrument)).expect("제출 이력");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].submission(), records[1].submission());
        assert_ne!(old_id, latest.submission.clone().expect("새 과제"));
        let mut old_result = evaluation(
            &latest,
            old_decision.clone(),
            Some(buy_record(&old_decision, "old-basis")),
        );
        let SignalMutation::Evaluate(value) = &mut old_result else {
            panic!("평가 전이 필요");
        };
        value.submission = old_id;
        assert_eq!(
            run(repository.commit(&instrument, &latest, &old_result)),
            Ok(SignalCommitOutcome::Conflict)
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(latest));
    }

    #[test]
    fn fresh_snapshot_for_a_discarded_submission_is_not_a_retryable_conflict() {
        let repository = repository();
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let initial = run(repository.snapshot(&instrument)).expect("초기");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        committed(run(repository.commit(
            &instrument,
            &initial,
            &evaluation(
                &initial,
                decision.clone(),
                Some(buy_record(&decision, "terminal-original")),
            ),
        )));
        let raised = run(repository.snapshot(&instrument)).expect("발생");
        committed(run(repository.commit(
            &instrument,
            &raised,
            &SignalMutation::Discard(Box::new(SignalTransition::Cleared(instrument.clone()))),
        )));
        let fresh = run(repository.snapshot(&instrument)).expect("폐기 후 재조회");
        let mutation = evaluation(
            &fresh,
            decision.clone(),
            Some(buy_record(&decision, "terminal-retry")),
        );
        let result = run(repository.commit(&instrument, &fresh, &mutation));
        assert_eq!(result, Ok(SignalCommitOutcome::SubmissionDiscarded));
        assert_eq!(run(repository.snapshot(&instrument)), Ok(fresh.clone()));
        run(
            testkit_ports::contracts::scenarios::signal_rejects_discarded_submission(
                &repository,
                &instrument,
                &fresh,
                &mutation,
            ),
        );
        assert_eq!(
            run(repository.commit(
                &instrument,
                &fresh,
                &evaluation(
                    &fresh,
                    SignalDecision::None(NoSignalReason::MissingSource),
                    None,
                ),
            )),
            Ok(SignalCommitOutcome::SubmissionDiscarded)
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(fresh));
    }

    #[test]
    fn submission_alone_invalidates_the_older_evaluation_snapshot() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let old = run(repository.snapshot(&instrument)).expect("이전 제출");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let mutation = evaluation(
            &old,
            decision.clone(),
            Some(buy_record(&decision, "stale-after-submit")),
        );
        submit(&repository, "task-new", "98", InstrumentChoice::First);
        let latest = run(repository.snapshot(&instrument)).expect("새 제출");
        assert_eq!(old.revision, latest.revision);
        assert_ne!(old.submission, latest.submission);
        assert_eq!(latest.current, None);
        assert_eq!(
            run(repository.commit(&instrument, &old, &mutation)),
            Ok(SignalCommitOutcome::Conflict)
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(latest.clone()));
        run(
            testkit_ports::contracts::scenarios::signal_rejects_stale_snapshot(
                &repository,
                &instrument,
                &old,
                &latest,
                &mutation,
            ),
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
    fn forged_clear_or_discard_transition_leaves_current_and_history_intact() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let initial = run(repository.snapshot(&instrument)).expect("초기");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let live_record = buy_record(&decision, "forgery-original");
        committed(run(repository.commit(
            &instrument,
            &initial,
            &evaluation(&initial, decision, Some(live_record.clone())),
        )));
        let raised = run(repository.snapshot(&instrument)).expect("발생");
        let forged = SignalMutation::Evaluate(Box::new(SignalEvaluation {
            submission: raised.submission.clone().expect("제출"),
            decision: SignalDecision::None(NoSignalReason::ConditionsUnmet),
            transition: SignalTransition::Unchanged,
            calibration_head: Some(fixtures::head(0)),
            occurrence: None,
            evidence: None,
        }));
        let record = domain::HistoricalSignalOccurrence::from_record(&live_record);
        for mutation in [
            forged,
            SignalMutation::Discard(Box::new(SignalTransition::Unchanged)),
            SignalMutation::Discard(Box::new(SignalTransition::Cleared(fixtures::id(
                "sample-b",
            )))),
        ] {
            assert_eq!(
                run(repository.commit(&instrument, &raised, &mutation)),
                Err(invalid())
            );
            assert_eq!(run(repository.snapshot(&instrument)), Ok(raised.clone()));
            assert_eq!(
                run(repository.occurrences(
                    &instrument,
                    &fixtures::master().order_instrument().expect("종목")
                )),
                Ok(vec![record.clone()])
            );
        }
    }

    #[test]
    fn signal_and_buy_occurrence_must_match_their_submission_and_instrument() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "98", InstrumentChoice::First);
        let snapshot = run(repository.snapshot(&instrument)).expect("제출");
        let wrong_submission = fixtures::decision(&fixtures::master(), "99", None);
        let wrong_occurrence = fixtures::decision(&fixtures::other_master(), "99", None);
        let correct = fixtures::decision(&fixtures::master(), "98", None);
        let mutations = [
            evaluation(
                &snapshot,
                wrong_submission.clone(),
                Some(buy_record(&wrong_submission, "wrong-submission")),
            ),
            evaluation(
                &snapshot,
                correct,
                Some(buy_record(&wrong_occurrence, "wrong-occurrence")),
            ),
        ];
        for mutation in mutations {
            assert_eq!(
                run(repository.commit(&instrument, &snapshot, &mutation)),
                Err(invalid())
            );
            assert_eq!(run(repository.snapshot(&instrument)), Ok(snapshot.clone()));
            assert_eq!(
                run(repository.occurrences(
                    &instrument,
                    &fixtures::master().order_instrument().expect("종목")
                )),
                Ok(vec![])
            );
        }
    }

    #[test]
    fn supplied_occurrence_is_rejected_without_a_new_buy() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "10", InstrumentChoice::First);
        let initial = run(repository.snapshot(&instrument)).expect("이전 제출");
        let unrelated_buy = fixtures::decision(&fixtures::master(), "99", None);
        let extra = buy_record(&unrelated_buy, "unexpected-occurrence");
        for decision in [
            fixtures::decision(&fixtures::master(), "10", Some("7")),
            SignalDecision::None(NoSignalReason::ConditionsUnmet),
        ] {
            assert_eq!(
                run(repository.commit(
                    &instrument,
                    &initial,
                    &evaluation(&initial, decision, Some(extra.clone())),
                )),
                Err(invalid())
            );
            assert_eq!(run(repository.snapshot(&instrument)), Ok(initial.clone()));
        }
        submit(&repository, "task-new", "99", InstrumentChoice::First);
        let latest = run(repository.snapshot(&instrument)).expect("새 제출");
        let buy = fixtures::decision(&fixtures::master(), "99", None);
        let live_original = buy_record(&buy, "actual-occurrence");
        committed(run(repository.commit(
            &instrument,
            &latest,
            &evaluation(&latest, buy.clone(), Some(live_original.clone())),
        )));
        let raised = run(repository.snapshot(&instrument)).expect("발생");
        assert_eq!(
            run(repository.commit(&instrument, &raised, &evaluation(&raised, buy, Some(extra)),)),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(raised));
        let original = domain::HistoricalSignalOccurrence::from_record(&live_original);
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            )),
            Ok(vec![original])
        );
    }

    #[test]
    fn updated_buy_rejects_a_second_occurrence_and_preserves_the_first() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let initial = run(repository.snapshot(&instrument)).expect("초기");
        let first = fixtures::decision(&fixtures::master(), "99", None);
        let live_original = buy_record(&first, "update-original");
        committed(run(repository.commit(
            &instrument,
            &initial,
            &evaluation(&initial, first, Some(live_original.clone())),
        )));
        submit(&repository, "task-new", "98", InstrumentChoice::First);
        let latest = run(repository.snapshot(&instrument)).expect("새 제출");
        let next = fixtures::decision(&fixtures::master(), "98", None);
        let extra = buy_record(&next, "update-extra");
        let mutation = evaluation(&latest, next.clone(), Some(extra));
        let SignalMutation::Evaluate(value) = &mutation else {
            panic!("평가 필요");
        };
        assert_eq!(
            value.transition,
            SignalTransition::Updated(next.into_signal().expect("갱신 신호"))
        );
        assert_eq!(
            run(repository.commit(&instrument, &latest, &mutation)),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(latest));
        let original = domain::HistoricalSignalOccurrence::from_record(&live_original);
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            )),
            Ok(vec![original])
        );
    }

    fn race_evaluation_and_discard(
        repository: &MemorySignalEvaluationRepository,
        instrument: &InstrumentId,
        raised: &application::SignalSnapshot,
        mutations: (&SignalMutation, &SignalMutation),
    ) -> (SignalCommitOutcome, SignalCommitOutcome) {
        let (evaluation_mutation, discard) = mutations;
        let barrier = Barrier::new(3);
        std::thread::scope(|scope| {
            let evaluate = scope.spawn(|| {
                barrier.wait();
                run(repository.commit(instrument, raised, evaluation_mutation)).expect("평가")
            });
            let remove = scope.spawn(|| {
                barrier.wait();
                run(repository.commit(instrument, raised, discard)).expect("폐기")
            });
            barrier.wait();
            (
                evaluate.join().expect("평가 스레드"),
                remove.join().expect("폐기 스레드"),
            )
        })
    }

    #[test]
    fn concurrent_discard_and_evaluation_cannot_resurrect_the_discarded_task() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let initial = run(repository.snapshot(&instrument)).expect("초기");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let live_record = buy_record(&decision, "race-original");
        committed(run(repository.commit(
            &instrument,
            &initial,
            &evaluation(&initial, decision.clone(), Some(live_record.clone())),
        )));
        let raised = run(repository.snapshot(&instrument)).expect("발생");
        let evaluation_mutation = evaluation(&raised, decision.clone(), None);
        let discard =
            SignalMutation::Discard(Box::new(SignalTransition::Cleared(instrument.clone())));
        let (evaluated, discarded) = race_evaluation_and_discard(
            &repository,
            &instrument,
            &raised,
            (&evaluation_mutation, &discard),
        );
        let cleared = match (evaluated, discarded) {
            (SignalCommitOutcome::Applied(evaluation), SignalCommitOutcome::Conflict) => {
                assert_eq!(evaluation.events, vec![]);
                let latest = run(repository.snapshot(&instrument)).expect("평가 후");
                committed(run(repository.commit(&instrument, &latest, &discard)))
            }
            (SignalCommitOutcome::Conflict, SignalCommitOutcome::Applied(commit)) => commit,
            other => panic!("경쟁 결과: {other:?}"),
        };
        assert_eq!(
            cleared.events,
            vec![DomainEvent::SignalCleared(instrument.clone())]
        );
        let after = run(repository.snapshot(&instrument)).expect("폐기 후");
        assert_eq!(after.current, None);
        assert_eq!(
            run(repository.commit(
                &instrument,
                &after,
                &evaluation(
                    &after,
                    decision.clone(),
                    Some(buy_record(&decision, "race-retry")),
                ),
            )),
            Ok(SignalCommitOutcome::SubmissionDiscarded)
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(after));
        let record = domain::HistoricalSignalOccurrence::from_record(&live_record);
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            )),
            Ok(vec![record])
        );
    }

    #[test]
    fn new_buy_rejects_an_occurrence_from_a_different_horizon() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let state = run(repository.snapshot(&instrument)).expect("제출");
        let short = fixtures::decision(&fixtures::master(), "99", None);
        let long = fixtures::decision_with_horizon(
            &fixtures::master(),
            "99",
            None,
            &[fixtures::item()],
            (HorizonSelection::WeeksToMonths, None),
        );
        let SignalDecision::Buy(long_signal) = &long else {
            panic!("긴 기간 매수 신호 필요: {long:?}");
        };
        let calendars: Vec<_> = (60..=101)
            .map(|value| fixtures::calendar(value, &Market::Kr))
            .collect();
        let foreign = BuySignalRecord::from_signal(
            BuySignalOccurrenceId::try_new("different-horizon").expect("발생 ID"),
            long_signal,
            fixtures::day(60),
            &calendars,
        )
        .expect("긴 기간 발생");
        let original = buy_record(&short, "matching-horizon");
        assert_ne!(foreign.interval(), original.interval());
        assert_eq!(
            run(repository.commit(
                &instrument,
                &state,
                &evaluation(&state, short, Some(foreign)),
            )),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(state));
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            )),
            Ok(vec![])
        );
    }

    #[test]
    fn new_buy_rejects_a_different_task_even_when_the_horizon_matches() {
        let repository = repository();
        let instrument = fixtures::id("sample-a");
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let state = run(repository.snapshot(&instrument)).expect("제출");
        let original = fixtures::decision(&fixtures::master(), "99", None);
        let other = fixtures::decision_with_horizon(
            &fixtures::master(),
            "99",
            None,
            &[fixtures::item()],
            (HorizonSelection::FewDays, Some("104")),
        );
        let own_record = buy_record(&original, "own-task");
        let other_record = buy_record(&other, "other-task");
        assert_eq!(own_record.interval(), other_record.interval());
        assert_ne!(
            own_record.originating_task(),
            other_record.originating_task()
        );
        assert_eq!(
            run(repository.commit(
                &instrument,
                &state,
                &evaluation(&state, original, Some(other_record)),
            )),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(state));
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            )),
            Ok(vec![])
        );
    }

    #[test]
    fn continuing_buy_keeps_its_original_occurrence_when_horizon_changes() {
        let short = fixtures::decision(&fixtures::master(), "99", None);
        let long = fixtures::decision_with_horizon(
            &fixtures::master(),
            "99",
            None,
            &[fixtures::item()],
            (HorizonSelection::WeeksToMonths, None),
        );
        let original = buy_record(&short, "existing-horizon");
        let SignalDecision::Buy(short_signal) = short else {
            panic!("짧은 기간 매수 신호 필요");
        };
        let SignalDecision::Buy(long_signal) = long else {
            panic!("긴 기간 매수 신호 필요");
        };
        assert_ne!(
            short_signal.facts().task().horizon(),
            long_signal.facts().task().horizon()
        );
        let current = CurrentBuySignal::try_new(short_signal, original.clone()).expect("현재 매수");
        let updated = current.update(long_signal).expect("발생을 이어가는 갱신");
        assert_eq!(updated.occurrence(), &original);
    }

    fn repository_with_horizon_change() -> (
        MemorySignalEvaluationRepository,
        domain::CostEligibleTask,
        domain::CalibrationLedgerHead,
    ) {
        let source = fixtures::master();
        let short_task = fixtures::eligible_for_master("102", &source);
        let long_task =
            fixtures::eligible_for_horizon("102", &source, HorizonSelection::WeeksToMonths);
        let assignments = domain::EstimateAssignments::default()
            .issue(
                fixtures::task_id("task-old"),
                fixtures::run_id(),
                short_task,
            )
            .expect("짧은 과제")
            .issue(
                fixtures::task_id("task-new"),
                fixtures::run_id(),
                long_task.clone(),
            )
            .expect("긴 과제");
        let repository = MemorySignalEvaluationRepository::new(assignments);
        repository
            .advance_calibration_head(fixtures::head(0))
            .expect("짧은 기간 보정");
        let long_key = fixtures::calibration_key_for(HorizonSelection::WeeksToMonths);
        let long_head =
            domain::CalibrationLedgerHead::new(long_key, domain::CalibrationPosition::new(0));
        repository
            .advance_calibration_head(long_head.clone())
            .expect("긴 기간 보정");
        (repository, long_task, long_head)
    }

    fn submit_long_horizon(
        repository: &MemorySignalEvaluationRepository,
        task: &domain::CostEligibleTask,
        head: &domain::CalibrationLedgerHead,
    ) {
        let mut input = fixtures::input("task-new", "99", &fixtures::master());
        input.submission = fixtures::submission_with_key(task, "99", head.key().clone());
        let events = EventBusBuilder::default().build();
        run(SubmitEstimate::new(repository, &events).execute(&fixtures::run_id(), vec![input]))
            .expect("긴 기간 제출");
    }

    #[test]
    fn new_long_buy_rejects_a_shorter_horizon_occurrence() {
        let (repository, long_task, long_head) = repository_with_horizon_change();
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        submit_long_horizon(&repository, &long_task, &long_head);
        let instrument = fixtures::id("sample-a");
        let state = run(repository.snapshot(&instrument)).expect("긴 제출");
        let short = fixtures::decision(&fixtures::master(), "99", None);
        let long = fixtures::decision_with_horizon(
            &fixtures::master(),
            "99",
            None,
            &[fixtures::item()],
            (HorizonSelection::WeeksToMonths, None),
        );
        let shorter = buy_record(&short, "shorter-horizon");
        let mut mutation = evaluation(&state, long, Some(shorter));
        let SignalMutation::Evaluate(value) = &mut mutation else {
            panic!("평가 필요");
        };
        value.calibration_head = Some(long_head);
        assert_eq!(
            run(repository.commit(&instrument, &state, &mutation)),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(state));
        assert_eq!(
            run(repository.occurrences(
                &instrument,
                &fixtures::master().order_instrument().expect("종목")
            )),
            Ok(vec![])
        );
    }

    #[test]
    fn repository_update_preserves_original_horizon_across_new_submission() {
        let source = fixtures::master();
        let (repository, long_task, long_head) = repository_with_horizon_change();
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let initial = run(repository.snapshot(&instrument)).expect("짧은 제출");
        let first = fixtures::decision(&source, "99", None);
        let live_original = buy_record(&first, "keep-original-horizon");
        committed(run(repository.commit(
            &instrument,
            &initial,
            &evaluation(&initial, first, Some(live_original.clone())),
        )));
        submit_long_horizon(&repository, &long_task, &long_head);
        let snapshot = run(repository.snapshot(&instrument)).expect("새 제출");
        let long = fixtures::decision_with_horizon(
            &source,
            "99",
            None,
            &[fixtures::item()],
            (HorizonSelection::WeeksToMonths, None),
        );
        let mut mutation = evaluation(&snapshot, long.clone(), None);
        let SignalMutation::Evaluate(value) = &mut mutation else {
            panic!("평가 필요");
        };
        assert_eq!(
            value.transition,
            SignalTransition::Updated(long.into_signal().expect("갱신 신호"))
        );
        value.calibration_head = Some(long_head);
        let result = committed(run(repository.commit(&instrument, &snapshot, &mutation)));
        assert_eq!(result.events, vec![]);
        let current = run(repository.snapshot(&instrument))
            .expect("갱신")
            .current
            .expect("매수");
        let original = domain::HistoricalSignalOccurrence::from_record(&live_original);
        assert_eq!(current.occurrence, Some(original.clone()));
        assert_eq!(
            run(repository.occurrences(&instrument, &source.order_instrument().expect("종목"))),
            Ok(vec![original])
        );
    }
    #[test]
    fn signal_commit_rejects_missing_or_corrupted_restore_evidence_atomically() {
        let repository = repository();
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let snapshot = run(repository.snapshot(&instrument)).expect("상태");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let occurrence = buy_record(&decision, "evidence-check");
        let mutation = evaluation(&snapshot, decision, Some(occurrence));
        let mut missing = mutation.clone();
        let SignalMutation::Evaluate(missing_value) = &mut missing else {
            panic!("평가")
        };
        missing_value.evidence = None;
        assert_eq!(
            run(repository.commit(&instrument, &snapshot, &missing)),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(snapshot.clone()));
        let mut damaged = mutation.clone();
        let SignalMutation::Evaluate(value) = &mut damaged else {
            panic!("평가")
        };
        let proof = value.evidence.as_mut().expect("복원 근거");
        proof.signal.evidence.probability =
            domain::AiProbability::parse(&domain::AiProbabilityInput::try_new("10").expect("입력"))
                .expect("확률");
        assert_eq!(
            run(repository.commit(&instrument, &snapshot, &damaged)),
            Err(invalid())
        );
        assert_eq!(run(repository.snapshot(&instrument)), Ok(snapshot.clone()));
        assert_eq!(
            committed(run(repository.commit(&instrument, &snapshot, &mutation))).events,
            vec![DomainEvent::SignalRaised(instrument)]
        );
    }
    #[test]
    fn persisted_history_uses_the_current_master_lookup_each_time() {
        let repository = repository();
        submit(&repository, "task-old", "99", InstrumentChoice::First);
        let instrument = fixtures::id("sample-a");
        let snapshot = run(repository.snapshot(&instrument)).expect("상태");
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        let occurrence = buy_record(&decision, "master-refresh");
        let mutation = evaluation(&snapshot, decision, Some(occurrence));
        committed(run(repository.commit(&instrument, &snapshot, &mutation)));
        let before = run(repository.snapshot(&instrument)).expect("정상 조회");
        let source = fixtures::master();
        let identity = source.order_instrument().expect("종목");
        let mut changed = source.row().clone();
        changed.symbol = domain::Symbol::try_new("000123").expect("심볼");
        repository.save_master(domain::MasterInstrument::try_new(changed).expect("마스터"));
        let integrity =
            SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity);
        assert_eq!(run(repository.snapshot(&instrument)), Err(integrity));
        assert_eq!(
            run(repository.occurrences(&instrument, &identity)),
            Err(integrity)
        );
        repository.save_master(source);
        assert_eq!(run(repository.snapshot(&instrument)), Ok(before));
    }
    #[test]
    fn new_signal_transition_agrees_with_empty_history() {
        let decision = fixtures::decision(&fixtures::master(), "99", None);
        assert_eq!(
            signal_transition(None, &decision),
            signal_transition_from_history(None, &decision)
        );
    }
}
