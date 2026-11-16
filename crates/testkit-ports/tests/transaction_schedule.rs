#[cfg(test)]
mod tests {
    use application::{
        DeliveryReport, EventBus, FullInspectionRepository, HttpStatus, PortError, PortErrorKind,
        ReadEndpoint, RetryDelay, SystemFailure, SystemPort, SystemPortError,
        TransactionRepository, TransactionSchedule, TransactionScheduleError, TransactionSyncError,
        TransactionSyncRecord, TransactionSyncRepository, UnavailableCause, WriteCompleted,
    };
    use domain::{
        AccountId, CalendarDate, CalendarYear, DailyFullInspection, DayNumber, MonthNumber,
        Nanosecond, OffsetSeconds, SettledRowSet, SettlementWindow, SyncRecord, Timestamp,
        TransactionSyncTimeError, UnixSeconds,
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
    fn schedule<'a>(
        broker: &'a ScriptedBrokerRead,
        repository: &'a Repository,
        clock: &'a SteppedClock,
        events: &'a EventBus,
    ) -> TransactionSchedule<'a, ScriptedBrokerRead, Repository, Repository, SteppedClock> {
        TransactionSchedule {
            broker,
            repository,
            inspections: repository,
            clock,
            events,
        }
    }
    #[test]
    fn daily_status_midnight_retry_and_accounts() {
        let repo = Repository::default();
        let broker = ScriptedBrokerRead::default();
        let clock = SteppedClock::new(at(1_794_841_199));
        let events = EventBus::default();
        let sync = schedule(&broker, &repo, &clock, &events);
        let account = AccountId::new(91);
        for last in [None, None, Some(at(1_794_841_198))] {
            repo.last.push(Ok(last));
            assert_eq!(
                run(sync.full_inspection_status(&account)),
                Ok(if last.is_some() {
                    DailyFullInspection::CompletedToday
                } else {
                    DailyFullInspection::Required
                })
            );
        }
        clock.advance_to(at(1_794_841_200)).expect("자정");
        repo.last.push(Ok(Some(at(1_794_841_198))));
        assert_eq!(
            run(sync.full_inspection_status(&account)),
            Ok(DailyFullInspection::Required)
        );
        repo.last.push(Ok(None));
        assert_eq!(
            run(sync.full_inspection_status(&AccountId::new(92))),
            Ok(DailyFullInspection::Required)
        );
        assert_eq!(
            repo.last.calls(),
            vec![
                account.clone(),
                account.clone(),
                account.clone(),
                account,
                AccountId::new(92)
            ]
        );
        assert_eq!(repo.completed.calls(), []);
        assert_eq!(broker.calls(), []);
    }
    #[test]
    fn completion_uses_current_clock_and_preserves_previous_for_atomic_write() {
        let repo = Repository::default();
        let broker = ScriptedBrokerRead::default();
        let clock = SteppedClock::new(at(1_794_841_201));
        let events = EventBus::default();
        let sync = schedule(&broker, &repo, &clock, &events);
        for previous in [None, Some(at(1_794_841_198)), Some(at(1_794_841_201))] {
            repo.last.push(Ok(previous));
            repo.completed.push(Ok(WriteCompleted));
            assert_eq!(
                run(sync.record_full_inspection_completed(&AccountId::new(93))),
                Ok(WriteCompleted)
            );
            assert_eq!(
                repo.completed.calls().last(),
                Some(&(AccountId::new(93), previous, at(1_794_841_201)))
            );
        }
        repo.last.push(Ok(Some(at(1_794_841_201))));
        assert_eq!(
            run(sync.full_inspection_status(&AccountId::new(93))),
            Ok(DailyFullInspection::CompletedToday)
        );
        assert_eq!(clock.calls(), vec![at(1_794_841_201); 4]);
    }
    #[test]
    fn repository_errors_propagate_and_failed_completion_remains_due() {
        let repo = Repository::default();
        let broker = ScriptedBrokerRead::default();
        let clock = SteppedClock::new(at(1_794_841_201));
        let events = EventBus::default();
        let sync = schedule(&broker, &repo, &clock, &events);
        repo.last.push(Err(failure()));
        assert_eq!(
            run(sync.full_inspection_status(&AccountId::new(94))),
            Err(TransactionScheduleError::Repository(failure()))
        );
        repo.last.push(Err(failure()));
        assert_eq!(
            run(sync.record_full_inspection_completed(&AccountId::new(94))),
            Err(TransactionScheduleError::Repository(failure()))
        );
        assert_eq!(clock.calls(), []);
        assert_eq!(repo.completed.calls(), []);
        repo.last.push(Ok(None));
        repo.completed.push(Err(failure()));
        assert_eq!(
            run(sync.record_full_inspection_completed(&AccountId::new(94))),
            Err(TransactionScheduleError::Repository(failure()))
        );
        repo.last.push(Ok(None));
        assert_eq!(
            run(sync.full_inspection_status(&AccountId::new(94))),
            Ok(DailyFullInspection::Required)
        );
    }
    #[test]
    fn future_completion_and_out_of_range_fail_before_writing() {
        for (now, last, error) in [
            (
                at(1_794_841_201),
                Some(at(1_794_841_202)),
                TransactionSyncTimeError::ClockReversed,
            ),
            (
                at(253_402_300_799),
                None,
                TransactionSyncTimeError::OutOfRange,
            ),
        ] {
            let repo = Repository::default();
            let broker = ScriptedBrokerRead::default();
            let clock = SteppedClock::new(now);
            let events = EventBus::default();
            let sync = schedule(&broker, &repo, &clock, &events);
            repo.last.push(Ok(last));
            assert_eq!(
                run(sync.full_inspection_status(&AccountId::new(95))),
                Err(TransactionScheduleError::Time(error))
            );
            repo.last.push(Ok(last));
            assert_eq!(
                run(sync.record_full_inspection_completed(&AccountId::new(95))),
                Err(TransactionScheduleError::Time(error))
            );
            assert_eq!(repo.completed.calls(), []);
        }
    }
    #[test]
    fn recent_window_midnight_uses_one_clock_sample_and_never_marks_full_completion() {
        for (now, start, end) in [
            (1_794_841_199, date(3), date(16)),
            (1_794_841_200, date(4), date(17)),
        ] {
            let repo = Repository::default();
            let broker = ScriptedBrokerRead::default();
            let clock = SteppedClock::new(at(now));
            let events = EventBus::default();
            let sync = schedule(&broker, &repo, &clock, &events);
            let account = AccountId::new(96);
            repo.state.push(Ok(SettledRowSet::new(account.clone())));
            repo.latest.push(Ok(None));
            repo.records.push(Ok(WriteCompleted));
            broker.script_transactions().push(Err(PortError::new(
                ReadEndpoint::Transactions,
                HttpStatus::NotReceived,
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::Network,
                    retry: RetryDelay::Unknown,
                },
            )));
            assert_eq!(
                run(sync.sync_recent(&account)),
                Ok(DeliveryReport::default())
            );
            let window = SettlementWindow::try_new(start, end).expect("독립 기대 창");
            assert_eq!(
                broker.calls(),
                vec![BrokerReadCall::Transactions(
                    account.clone(),
                    window,
                    domain::TransactionFilter::all()
                )]
            );
            let records = repo.records.calls();
            let [(key, previous, previous_at, record)] = records.as_slice() else {
                panic!("한 기록")
            };
            assert_eq!(key, &account);
            assert_eq!(previous, &SettledRowSet::new(account.clone()));
            assert_eq!(*previous_at, None);
            assert_eq!(record.window(), window);
            assert_eq!(record.recorded_at(), at(now));
            assert_eq!(clock.calls(), vec![at(now)]);
            assert_eq!(repo.completed.calls(), []);
            assert_eq!(repo.last.calls(), []);
        }
    }
    #[test]
    fn recent_range_and_sync_errors_are_preserved() {
        let repo = Repository::default();
        let broker = ScriptedBrokerRead::default();
        let events = EventBus::default();
        let clock = SteppedClock::new(at(253_402_300_799));
        assert_eq!(
            run(schedule(&broker, &repo, &clock, &events).sync_recent(&AccountId::new(97))),
            Err(TransactionScheduleError::Time(
                TransactionSyncTimeError::OutOfRange
            ))
        );
        assert_eq!(repo.state.calls(), []);
        let valid_clock = SteppedClock::new(at(1_794_841_201));
        repo.state.push(Err(failure()));
        assert_eq!(
            run(schedule(&broker, &repo, &valid_clock, &events).sync_recent(&AccountId::new(97))),
            Err(TransactionScheduleError::Sync(
                TransactionSyncError::Repository(failure())
            ))
        );
        assert_eq!(broker.calls(), []);
    }
}
