#[cfg(test)]
mod tests {
    use application::{
        DeliveryReport, EventBus, FullInspectionRepository, FullInspectionRun,
        FullInspectionRunError, HttpStatus, PortError, PortErrorKind, ReadEndpoint, RetryDelay,
        RunFullInspection, SystemFailure, SystemPort, SystemPortError, TransactionRepository,
        TransactionSyncRecord, TransactionSyncRepository, UnavailableCause, WriteCompleted,
    };
    use domain::{
        AccountId, CalendarDate, CalendarYear, DayNumber, FullInspectionCompletionError,
        FullInspectionPlan, InspectionRangeDays, MonthNumber, Nanosecond, OffsetSeconds,
        SettledRowSet, SettlementWindow, SyncRecord, Timestamp, UnixSeconds,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        Script, SteppedClock,
        scripted::{BrokerReadCall, ScriptedBrokerRead},
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
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn failure() -> SystemPortError {
        SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied)
    }
    type SyncWrite = (
        AccountId,
        SettledRowSet,
        Option<Timestamp>,
        TransactionSyncRecord,
    );
    #[derive(Default)]
    struct Repository {
        last: Script<AccountId, Result<Option<Timestamp>, SystemPortError>>,
        completed: Script<
            (AccountId, Option<Timestamp>, Timestamp),
            Result<WriteCompleted, SystemPortError>,
        >,
        state: Script<AccountId, Result<SettledRowSet, SystemPortError>>,
        latest: Script<AccountId, Result<Option<TransactionSyncRecord>, SystemPortError>>,
        records: Script<SyncWrite, Result<WriteCompleted, SystemPortError>>,
    }
    impl FullInspectionRepository for Repository {
        async fn last_completed(
            &self,
            account: &AccountId,
        ) -> Result<Option<Timestamp>, SystemPortError> {
            self.last.run(account.clone()).await
        }
        async fn record_completed(
            &self,
            account: &AccountId,
            previous: Option<Timestamp>,
            completed_at: Timestamp,
        ) -> Result<WriteCompleted, SystemPortError> {
            self.completed
                .run((account.clone(), previous, completed_at))
                .await
        }
    }
    impl TransactionRepository for Repository {
        async fn record(
            &self,
            _account: &AccountId,
            _record: &SyncRecord<SettledRowSet>,
        ) -> Result<WriteCompleted, SystemPortError> {
            std::future::ready(Err(failure())).await
        }
        async fn latest(
            &self,
            _account: &AccountId,
        ) -> Result<Option<SyncRecord<SettledRowSet>>, SystemPortError> {
            std::future::ready(Err(failure())).await
        }
    }
    impl TransactionSyncRepository for Repository {
        async fn state(&self, account: &AccountId) -> Result<SettledRowSet, SystemPortError> {
            self.state.run(account.clone()).await
        }
        async fn latest_sync(
            &self,
            account: &AccountId,
        ) -> Result<Option<TransactionSyncRecord>, SystemPortError> {
            self.latest.run(account.clone()).await
        }
        async fn record_sync(
            &self,
            account: &AccountId,
            previous: &SettledRowSet,
            previous_recorded_at: Option<Timestamp>,
            record: &TransactionSyncRecord,
        ) -> Result<WriteCompleted, SystemPortError> {
            self.records
                .run((
                    account.clone(),
                    previous.clone(),
                    previous_recorded_at,
                    record.clone(),
                ))
                .await
        }
    }

    fn execute(
        repo: &Repository,
        broker: &ScriptedBrokerRead,
        clock: &impl application::Clock,
        cap: u32,
    ) -> Result<FullInspectionRun, FullInspectionRunError> {
        run(RunFullInspection {
            broker,
            repository: repo,
            inspections: repo,
            clock,
            events: &EventBus::default(),
        }
        .run_full_inspection(
            &AccountId::new(701),
            InspectionRangeDays::try_new(cap).map_err(FullInspectionRunError::Plan)?,
        ))
        .outcome
    }
    fn prepare(repo: &Repository, broker: &ScriptedBrokerRead, cap: u32, failed: Option<usize>) {
        let account = AccountId::new(701);
        let plan = FullInspectionPlan::new(
            None,
            at(1_794_841_201),
            InspectionRangeDays::try_new(cap).expect("상한"),
        )
        .expect("계획");
        for (index, window) in plan.ranges().iter().enumerate() {
            repo.state.push(Ok(SettledRowSet::new(account.clone())));
            repo.latest.push(Ok(None));
            repo.records.push(Ok(WriteCompleted));
            broker.script_transactions().push(if failed == Some(index) {
                Err(PortError::new(ReadEndpoint::Transactions, HttpStatus::NotReceived,
                    PortErrorKind::Unavailable { cause: UnavailableCause::Network, retry: RetryDelay::Unknown }))
            } else {
                Ok(application::TransactionRead::Complete(application::CompleteTransactionObservation::after_last_page_without_quarantine(
                    account.clone(), *window, domain::TransactionFilter::all(),
                    application::ObservedTransactionBatch::try_new(vec![]).expect("행"),
                    domain::CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
                ).expect("완전 응답")))
            });
        }
    }
    #[test]
    fn all_ranges_save_exact_completion_and_skip_today() {
        for cap in [u32::MAX, 400_000, 365] {
            let repo = Repository::default();
            let broker = ScriptedBrokerRead::default();
            let clock = SteppedClock::new(at(1_794_841_201));
            repo.last.push(Ok(Some(at(1_794_841_198))));
            prepare(&repo, &broker, cap, None);
            repo.completed.push(Ok(WriteCompleted));
            let result = execute(&repo, &broker, &clock, cap).expect("완료");
            let count = match cap {
                365 => 2028,
                400_000 => 2,
                _ => 1,
            };
            assert_eq!(
                result,
                FullInspectionRun::Completed {
                    completed_at: at(1_794_841_201),
                }
            );
            assert_eq!(repo.last.calls(), [AccountId::new(701)]);
            assert_eq!(repo.records.calls().len(), count);
            assert_eq!(broker.calls().len(), count);
            assert_eq!(
                repo.completed.calls(),
                [(
                    AccountId::new(701),
                    Some(at(1_794_841_198)),
                    at(1_794_841_201)
                )]
            );
            let calls = broker.calls();
            let Some(BrokerReadCall::Transactions(account, first, filter)) = calls.first() else {
                panic!("첫 창")
            };
            assert_eq!(account, &AccountId::new(701));
            assert_eq!(filter, &domain::TransactionFilter::all());
            assert_eq!(
                first.from(),
                CalendarDate::from_parts(
                    CalendarYear::new(1),
                    MonthNumber::new(1),
                    DayNumber::new(1)
                )
                .expect("시작")
            );
            let Some(BrokerReadCall::Transactions(_, last, _)) = calls.last() else {
                panic!("끝 창")
            };
            assert_eq!(last.to(), date(17));
            repo.last.push(Ok(Some(at(1_794_841_201))));
            assert_eq!(
                execute(&repo, &broker, &clock, cap),
                Ok(FullInspectionRun::AlreadyCompletedToday)
            );
            assert_eq!(broker.calls().len(), count);
            assert_eq!(repo.completed.calls().len(), 1);
        }
    }
    #[test]
    fn failed_range_never_completes_and_next_run_retries_every_range() {
        for index in [0, 1] {
            let repo = Repository::default();
            let broker = ScriptedBrokerRead::default();
            let clock = SteppedClock::new(at(1_794_841_201));
            repo.last.push(Ok(None));
            prepare(&repo, &broker, 400_000, Some(index));
            assert_eq!(
                execute(&repo, &broker, &clock, 400_000),
                Err(FullInspectionRunError::Completion(
                    FullInspectionCompletionError::Incomplete
                ))
            );
            assert_eq!(repo.completed.calls(), []);
            repo.last.push(Ok(None));
            prepare(&repo, &broker, 400_000, None);
            repo.completed.push(Ok(WriteCompleted));
            let result = execute(&repo, &broker, &clock, 400_000).expect("재시도 완료");
            assert_eq!(
                result,
                FullInspectionRun::Completed {
                    completed_at: at(1_794_841_201),
                }
            );
            assert_eq!(broker.calls().len(), 4);
            assert_eq!(repo.completed.calls().len(), 1);
        }
    }
    struct SequenceClock(std::sync::RwLock<std::collections::VecDeque<Timestamp>>);
    impl application::Clock for SequenceClock {
        fn now(&self) -> Timestamp {
            self.0.write().expect("시계").pop_front().expect("표본")
        }
    }
    #[test]
    fn midnight_rejects_completion_and_exact_sample_is_not_resampled() {
        for (samples, expected) in [
            (
                vec![at(1_794_927_599), at(1_794_927_599), at(1_794_927_600)],
                Err(FullInspectionRunError::Completion(
                    FullInspectionCompletionError::DayMismatch,
                )),
            ),
            (
                vec![at(1_794_841_201), at(1_794_841_202), at(1_794_841_203)],
                Ok(FullInspectionRun::Completed {
                    completed_at: at(1_794_841_203),
                }),
            ),
        ] {
            let repo = Repository::default();
            let broker = ScriptedBrokerRead::default();
            repo.last.push(Ok(None));
            prepare(&repo, &broker, u32::MAX, None);
            repo.completed.push(Ok(WriteCompleted));
            let clock = SequenceClock(std::sync::RwLock::new(samples.into()));
            assert_eq!(execute(&repo, &broker, &clock, u32::MAX), expected);
            match expected {
                Ok(_) => assert_eq!(
                    repo.completed.calls(),
                    [(AccountId::new(701), None, at(1_794_841_203))]
                ),
                Err(_) => assert_eq!(repo.completed.calls(), []),
            }
            assert!(clock.0.write().expect("시계").is_empty());
        }
    }
    #[test]
    fn duplicate_and_foreign_reports_cannot_produce_a_completion() {
        let plan = FullInspectionPlan::new(
            None,
            at(1_794_841_201),
            InspectionRangeDays::try_new(u32::MAX).expect("상한"),
        )
        .expect("계획");
        for range in [
            plan.ranges()[0],
            SettlementWindow::try_new(date(4), date(17)).expect("외부 창"),
        ] {
            let mut progress = plan.begin();
            progress
                .report(plan.ranges()[0], domain::InspectionRangeResult::Succeeded)
                .expect("성공");
            assert!(
                progress
                    .report(range, domain::InspectionRangeResult::Succeeded)
                    .is_err()
            );
            assert_eq!(
                progress.completion_at(at(1_794_841_201)).expect_err("무효"),
                FullInspectionCompletionError::Invalid(if range == plan.ranges()[0] {
                    domain::InspectionReportError::DuplicateRange
                } else {
                    domain::InspectionReportError::UnexpectedRange
                })
            );
        }
    }

    #[test]
    fn incomplete_wrong_scope_and_store_failure_never_complete() {
        let account = AccountId::new(701);
        let window = SettlementWindow::try_new(date(4), date(17)).expect("잘못된 창");
        let partial =
            application::TransactionRead::Incomplete(application::IncompleteTransactions {
                account: account.clone(),
                window,
                filter: domain::TransactionFilter::all(),
                received: vec![],
                quarantined: vec![],
                reason: application::IncompleteReason::PageInterrupted,
            });
        let wrong = application::TransactionRead::Complete(
            application::CompleteTransactionObservation::after_last_page_without_quarantine(
                account.clone(),
                window,
                domain::TransactionFilter::all(),
                application::ObservedTransactionBatch::try_new(vec![]).expect("행"),
                domain::CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
            )
            .expect("응답"),
        );
        for observation in [partial, wrong] {
            let repo = Repository::default();
            let broker = ScriptedBrokerRead::default();
            let clock = SteppedClock::new(at(1_794_841_201));
            repo.last.push(Ok(None));
            repo.state.push(Ok(SettledRowSet::new(account.clone())));
            repo.latest.push(Ok(None));
            repo.records.push(Ok(WriteCompleted));
            broker.script_transactions().push(Ok(observation));
            assert_eq!(
                execute(&repo, &broker, &clock, u32::MAX),
                Err(FullInspectionRunError::Completion(
                    FullInspectionCompletionError::Incomplete
                ))
            );
            assert_eq!(repo.completed.calls(), []);
        }
    }
    #[test]
    fn repository_errors_preserve_failure_and_do_not_mark_completion() {
        let repo = Repository::default();
        let broker = ScriptedBrokerRead::default();
        let clock = SteppedClock::new(at(1_794_841_201));
        repo.last.push(Err(failure()));
        assert_eq!(
            execute(&repo, &broker, &clock, u32::MAX),
            Err(FullInspectionRunError::Repository(failure()))
        );
        assert_eq!(clock.calls(), []);
        repo.last.push(Ok(None));
        repo.state.push(Err(failure()));
        assert_eq!(
            execute(&repo, &broker, &clock, u32::MAX),
            Err(FullInspectionRunError::Sync(
                application::TransactionSyncError::Repository(failure())
            ))
        );
        assert_eq!(repo.completed.calls(), []);
        repo.last.push(Ok(None));
        prepare(&repo, &broker, u32::MAX, None);
        repo.completed.push(Err(failure()));
        assert_eq!(
            execute(&repo, &broker, &clock, u32::MAX),
            Err(FullInspectionRunError::Repository(failure()))
        );
    }
    #[test]
    fn reversed_clock_and_future_completion_fail_closed() {
        let repo = Repository::default();
        let broker = ScriptedBrokerRead::default();
        repo.last.push(Ok(Some(at(1_794_841_202))));
        assert_eq!(
            execute(
                &repo,
                &broker,
                &SteppedClock::new(at(1_794_841_201)),
                u32::MAX
            ),
            Err(FullInspectionRunError::Time(
                domain::TransactionSyncTimeError::ClockReversed
            ))
        );
        repo.last.push(Ok(None));
        prepare(&repo, &broker, u32::MAX, None);
        let clock = SequenceClock(std::sync::RwLock::new(
            vec![at(1_794_841_201), at(1_794_841_201), at(1_794_841_200)].into(),
        ));
        assert_eq!(
            execute(&repo, &broker, &clock, u32::MAX),
            Err(FullInspectionRunError::Time(
                domain::TransactionSyncTimeError::ClockReversed
            ))
        );
        assert_eq!(repo.completed.calls(), []);
    }

    #[test]
    fn matching_partial_and_access_denied_never_mark_completion() {
        let account = AccountId::new(701);
        let state = SettledRowSet::new(account.clone());
        let window = SettlementWindow::try_new(
            CalendarDate::from_parts(CalendarYear::new(1), MonthNumber::new(1), DayNumber::new(1))
                .expect("시작"),
            date(17),
        )
        .expect("요청 창");
        let partial =
            application::TransactionRead::Incomplete(application::IncompleteTransactions {
                account,
                window,
                filter: domain::TransactionFilter::all(),
                received: vec![],
                quarantined: vec![],
                reason: application::IncompleteReason::PageInterrupted,
            });
        let denied = PortError::new(
            ReadEndpoint::Transactions,
            HttpStatus::Forbidden,
            PortErrorKind::AccessDenied(application::AccessDenial::Forbidden),
        );
        for (observation, expected) in [
            (Ok(partial), domain::SyncOutcome::Partial(state.clone())),
            (Err(denied), domain::SyncOutcome::AccessBlocked),
        ] {
            let repo = Repository::default();
            let broker = ScriptedBrokerRead::default();
            repo.last.push(Ok(None));
            repo.state.push(Ok(state.clone()));
            repo.latest.push(Ok(None));
            repo.records.push(Ok(WriteCompleted));
            broker.script_transactions().push(observation);
            assert_eq!(
                execute(
                    &repo,
                    &broker,
                    &SteppedClock::new(at(1_794_841_201)),
                    u32::MAX
                ),
                Err(FullInspectionRunError::Completion(
                    FullInspectionCompletionError::Incomplete
                ))
            );
            let records = repo.records.calls();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].3.summary().outcome(), &expected);
            assert_eq!(repo.completed.calls(), []);
        }
    }
    #[test]
    fn day_mismatch_retry_covers_the_new_today() {
        let repo = Repository::default();
        let broker = ScriptedBrokerRead::default();
        repo.last.push(Ok(None));
        prepare(&repo, &broker, u32::MAX, None);
        let clock = SequenceClock(std::sync::RwLock::new(
            vec![at(1_794_927_599), at(1_794_927_599), at(1_794_927_600)].into(),
        ));
        assert_eq!(
            execute(&repo, &broker, &clock, u32::MAX),
            Err(FullInspectionRunError::Completion(
                FullInspectionCompletionError::DayMismatch
            ))
        );
        assert_eq!(repo.completed.calls(), []);
        let account = AccountId::new(701);
        let window = SettlementWindow::try_new(
            CalendarDate::from_parts(CalendarYear::new(1), MonthNumber::new(1), DayNumber::new(1))
                .expect("시작"),
            date(18),
        )
        .expect("새 오늘");
        repo.last.push(Ok(None));
        repo.state.push(Ok(SettledRowSet::new(account.clone())));
        repo.latest.push(Ok(None));
        repo.records.push(Ok(WriteCompleted));
        repo.completed.push(Ok(WriteCompleted));
        broker
            .script_transactions()
            .push(Ok(application::TransactionRead::Complete(
                application::CompleteTransactionObservation::after_last_page_without_quarantine(
                    account.clone(),
                    window,
                    domain::TransactionFilter::all(),
                    application::ObservedTransactionBatch::try_new(vec![]).expect("행"),
                    domain::CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
                )
                .expect("응답"),
            )));
        assert_eq!(
            execute(
                &repo,
                &broker,
                &SteppedClock::new(at(1_794_927_601)),
                u32::MAX
            ),
            Ok(FullInspectionRun::Completed {
                completed_at: at(1_794_927_601),
            })
        );
        assert_eq!(
            broker.calls().last(),
            Some(&BrokerReadCall::Transactions(
                account.clone(),
                window,
                domain::TransactionFilter::all()
            ))
        );
        assert_eq!(broker.calls().len(), 2);
        assert_eq!(repo.completed.calls(), [(account, None, at(1_794_927_601))]);
    }

    fn failing_events() -> (EventBus, DeliveryReport) {
        struct Failing;
        impl application::EventSubscriber for Failing {
            fn on_event(
                &self,
                _event: &domain::DomainEvent,
            ) -> Result<(), application::SubscriberFailure> {
                Err(application::SubscriberFailure::Unavailable)
            }
        }
        let mut builder = application::EventBusBuilder::default();
        let subscription = builder.subscribe(
            [domain::DomainEventKind::TransactionsSyncRecorded].into(),
            Failing,
        );
        (
            builder.build(),
            DeliveryReport {
                failures: vec![application::DeliveryFailure {
                    subscription,
                    cause: application::SubscriberFailure::Unavailable,
                }],
            },
        )
    }
    fn report(
        repo: &Repository,
        broker: &ScriptedBrokerRead,
        clock: &impl application::Clock,
        events: &EventBus,
        cap: u32,
    ) -> application::FullInspectionReport {
        run(RunFullInspection {
            broker,
            repository: repo,
            inspections: repo,
            clock,
            events,
        }
        .run_full_inspection(
            &AccountId::new(701),
            InspectionRangeDays::try_new(cap).expect("상한"),
        ))
    }
    #[test]
    fn blocked_account_stops_after_one_range_and_retries_next_run() {
        for cap in [365, 400_000] {
            let repo = Repository::default();
            let broker = ScriptedBrokerRead::default();
            let clock = SteppedClock::new(at(1_794_841_201));
            let account = AccountId::new(701);
            repo.last.push(Ok(None));
            repo.state.push(Ok(SettledRowSet::new(account)));
            repo.latest.push(Ok(None));
            repo.records.push(Ok(WriteCompleted));
            broker.script_transactions().push(Err(PortError::new(
                ReadEndpoint::Transactions,
                HttpStatus::Forbidden,
                PortErrorKind::AccessDenied(application::AccessDenial::Forbidden),
            )));
            let (events, delivery) = failing_events();
            let result = report(&repo, &broker, &clock, &events, cap);
            assert_eq!(
                result.outcome,
                Err(FullInspectionRunError::Completion(
                    FullInspectionCompletionError::Incomplete
                ))
            );
            assert_eq!(result.deliveries, [delivery]);
            assert_eq!(broker.calls().len(), 1);
            assert_eq!(repo.records.calls().len(), 1);
            assert_eq!(repo.completed.calls(), []);
            repo.last.push(Ok(None));
            prepare(&repo, &broker, cap, None);
            repo.completed.push(Ok(WriteCompleted));
            assert_eq!(
                execute(&repo, &broker, &clock, cap),
                Ok(FullInspectionRun::Completed {
                    completed_at: at(1_794_841_201)
                })
            );
            assert_eq!(repo.completed.calls().len(), 1);
        }
    }
    #[derive(Clone, Copy)]
    enum Ending {
        Incomplete,
        Midnight,
        Save,
    }
    #[test]
    fn reports_survive_incomplete_midnight_and_completion_save_failure() {
        for ending in [Ending::Incomplete, Ending::Midnight, Ending::Save] {
            let repo = Repository::default();
            let broker = ScriptedBrokerRead::default();
            repo.last.push(Ok(None));
            let (failed, samples, expected) = match ending {
                Ending::Incomplete => (
                    Some(0),
                    vec![at(1_794_841_201); 3],
                    FullInspectionRunError::Completion(FullInspectionCompletionError::Incomplete),
                ),
                Ending::Midnight => (
                    None,
                    vec![at(1_794_927_599), at(1_794_927_599), at(1_794_927_600)],
                    FullInspectionRunError::Completion(FullInspectionCompletionError::DayMismatch),
                ),
                Ending::Save => {
                    repo.completed.push(Err(failure()));
                    (
                        None,
                        vec![at(1_794_841_201); 3],
                        FullInspectionRunError::Repository(failure()),
                    )
                }
            };
            prepare(&repo, &broker, u32::MAX, failed);
            let (events, delivery) = failing_events();
            let clock = SequenceClock(std::sync::RwLock::new(samples.into()));
            let result = report(&repo, &broker, &clock, &events, u32::MAX);
            assert_eq!(result.outcome, Err(expected));
            assert_eq!(result.deliveries, [delivery]);
        }
    }
    #[test]
    fn reports_survive_later_sync_failure_and_success() {
        let repo = Repository::default();
        let broker = ScriptedBrokerRead::default();
        let clock = SteppedClock::new(at(1_794_841_201));
        let (events, delivery) = failing_events();
        repo.last.push(Ok(None));
        let plan = FullInspectionPlan::new(
            None,
            at(1_794_841_201),
            InspectionRangeDays::try_new(400_000).expect("상한"),
        )
        .expect("계획");
        let account = AccountId::new(701);
        repo.state.push(Ok(SettledRowSet::new(account.clone())));
        repo.state.push(Err(failure()));
        repo.latest.push(Ok(None));
        repo.records.push(Ok(WriteCompleted));
        broker
            .script_transactions()
            .push(Ok(application::TransactionRead::Complete(
                application::CompleteTransactionObservation::after_last_page_without_quarantine(
                    account,
                    plan.ranges()[0],
                    domain::TransactionFilter::all(),
                    application::ObservedTransactionBatch::try_new(vec![]).expect("행"),
                    domain::CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
                )
                .expect("응답"),
            )));
        let result = report(&repo, &broker, &clock, &events, 400_000);
        assert_eq!(
            result.outcome,
            Err(FullInspectionRunError::Sync(
                application::TransactionSyncError::Repository(failure())
            ))
        );
        assert_eq!(
            result.deliveries.as_slice(),
            std::slice::from_ref(&delivery)
        );
        assert_eq!(repo.completed.calls(), []);
        repo.last.push(Ok(None));
        prepare(&repo, &broker, 400_000, None);
        repo.completed.push(Ok(WriteCompleted));
        let completed = report(&repo, &broker, &clock, &events, 400_000);
        assert_eq!(
            completed.outcome,
            Ok(FullInspectionRun::Completed {
                completed_at: at(1_794_841_201)
            })
        );
        assert_eq!(completed.deliveries, [delivery.clone(), delivery]);
    }
}
