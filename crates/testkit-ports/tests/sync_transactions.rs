#[cfg(test)]
mod tests {
    use application::{
        DeliveryReport, EventBusBuilder, EventSubscriber, HttpStatus, IncompleteReason,
        IncompleteTransactions, PortError, PortErrorKind, ReadEndpoint, RetryDelay,
        SubscriberFailure, SyncTransactions, SystemFailure, SystemPort, SystemPortError,
        TransactionRead, TransactionRepository, TransactionSyncError, TransactionSyncRecord,
        TransactionSyncRejection, TransactionSyncRepository, UnavailableCause, WriteCompleted,
    };
    use domain::{
        AccountId, CalendarDate, CalendarYear, Cancellation, CompleteTransactionResponse,
        CompletionAttestation, Currency, DayNumber, DomainEvent, DomainEventKind, Market, Money,
        MonthNumber, Nanosecond, OffsetSeconds, ReconciliationError, SettledRowSet, Settlement,
        SettlementWindow, Timestamp, Transaction, TransactionBatch, TransactionData,
        TransactionFilter, TransactionKind, UnixSeconds,
    };
    use std::{
        future::Future,
        sync::{Arc, RwLock},
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{
        SteppedClock,
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
    fn at(value: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(value),
            Nanosecond::new(7),
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
    fn window() -> SettlementWindow {
        SettlementWindow::try_new(date(3), date(16)).expect("명시적 창")
    }
    fn money(value: &str) -> Money {
        Money::parse(
            &domain::NumericInput::try_new(value).expect("숫자 입력"),
            Currency::Krw,
        )
        .expect("금액")
    }
    fn row(day: u8) -> Transaction {
        Transaction::try_new(TransactionData {
            kind: TransactionKind::Deposit,
            market: Market::Kr,
            currency: Currency::Krw,
            symbol: None,
            asset_class: None,
            isin: None,
            quantity: None,
            filled_price: None,
            amount: Some(money("101")),
            commission: money("2"),
            tax: money("3"),
            net_amount: Some(money("96")),
            exchange_rate: None,
            settlement_date: date(day),
            settlement: Settlement::Settled,
            transaction_time: Some(at(5)),
            cancellation: Cancellation::Regular,
        })
        .expect("행")
    }
    fn complete(
        account: AccountId,
        scope: SettlementWindow,
        filter: TransactionFilter,
        rows: Vec<Transaction>,
    ) -> TransactionRead {
        TransactionRead::Complete(
            CompleteTransactionResponse::after_last_page_without_quarantine(
                account,
                scope,
                filter,
                TransactionBatch::try_new(rows).expect("행들"),
                CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
            )
            .expect("완전 응답"),
        )
    }
    fn response(rows: Vec<Transaction>) -> TransactionRead {
        complete(AccountId::new(91), window(), TransactionFilter::all(), rows)
    }
    fn initial() -> SettledRowSet {
        let scope = SettlementWindow::try_new(date(1), date(30)).expect("전체 fixture");
        let TransactionRead::Complete(received) = complete(
            AccountId::new(91),
            scope,
            TransactionFilter::all(),
            vec![row(2), row(3), row(3), row(16), row(17)],
        ) else {
            panic!("완전 응답")
        };
        SettledRowSet::new(AccountId::new(91))
            .reconcile(&received, at(10))
            .expect("기존 상태")
            .state()
            .clone()
    }
    #[derive(Debug, Default)]
    struct Trace {
        records: Vec<(AccountId, TransactionSyncRecord)>,
        summaries: Vec<(AccountId, domain::SyncRecord<SettledRowSet>)>,
        events: Vec<DomainEvent>,
    }
    #[derive(Debug, Clone, Copy)]
    enum Gate {
        Open,
        Held,
    }
    #[derive(Debug)]
    struct Repository {
        state: SettledRowSet,
        failure: Option<SystemPortError>,
        trace: Arc<RwLock<Trace>>,
        gate: Arc<RwLock<Gate>>,
    }
    impl Repository {
        fn current_state(&self, stored: &Trace) -> SettledRowSet {
            stored
                .summaries
                .iter()
                .rev()
                .find_map(|(_account, record)| match record.outcome() {
                    domain::SyncOutcome::Complete(state) | domain::SyncOutcome::Partial(state) => {
                        Some(state.clone())
                    }
                    domain::SyncOutcome::Failed | domain::SyncOutcome::AccessBlocked => None,
                })
                .unwrap_or_else(|| self.state.clone())
        }
    }
    impl TransactionRepository for Repository {
        fn record(
            &self,
            account: &AccountId,
            record: &domain::SyncRecord<SettledRowSet>,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            self.trace
                .write()
                .expect("잠금")
                .summaries
                .push((account.clone(), record.clone()));
            std::future::ready(Ok(WriteCompleted))
        }
        fn latest(
            &self,
            account: &AccountId,
        ) -> impl Future<
            Output = Result<Option<domain::SyncRecord<SettledRowSet>>, SystemPortError>,
        > + Send {
            let stored = self.trace.read().expect("잠금");
            let found = stored
                .summaries
                .iter()
                .rev()
                .find(|(key, _record)| key == account)
                .map(|(_key, record)| record.clone());
            std::future::ready(Ok(found))
        }
    }
    impl TransactionSyncRepository for Repository {
        fn state(
            &self,
            _account: &AccountId,
        ) -> impl Future<Output = Result<SettledRowSet, SystemPortError>> + Send {
            std::future::ready(Ok(self.current_state(&self.trace.read().expect("잠금"))))
        }
        fn latest_sync(
            &self,
            account: &AccountId,
        ) -> impl Future<Output = Result<Option<TransactionSyncRecord>, SystemPortError>> + Send
        {
            let stored = self.trace.read().expect("잠금");
            let found = stored
                .records
                .iter()
                .rev()
                .find(|(key, _record)| key == account)
                .map(|(_key, record)| record.clone());
            std::future::ready(Ok(found))
        }
        async fn record_sync(
            &self,
            account: &AccountId,
            previous: &SettledRowSet,
            record: &TransactionSyncRecord,
        ) -> Result<WriteCompleted, SystemPortError> {
            std::future::poll_fn(|_context| {
                let gate = *self.gate.read().expect("gate");
                match gate {
                    Gate::Open => Poll::Ready(()),
                    Gate::Held => Poll::Pending,
                }
            })
            .await;
            if let Some(error) = self.failure {
                return Err(error);
            }
            let mut stored = self.trace.write().map_err(|_error| {
                SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData)
            })?;
            if &self.current_state(&stored) != previous {
                return Err(SystemPortError::new(
                    SystemPort::Repository,
                    SystemFailure::InvalidData,
                ));
            }
            stored.summaries.push((account.clone(), record.summary()));
            stored.records.push((account.clone(), record.clone()));
            Ok(WriteCompleted)
        }
    }
    #[derive(Debug)]
    struct Subscriber(Arc<RwLock<Trace>>);
    impl EventSubscriber for Subscriber {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            let mut stored = self
                .0
                .write()
                .map_err(|_error| SubscriberFailure::Unavailable)?;
            if stored.records.len() != stored.events.len().saturating_add(1) {
                return Err(SubscriberFailure::Unavailable);
            }
            stored.events.push(event.clone());
            Ok(())
        }
    }
    fn sync_events(trace: &Arc<RwLock<Trace>>) -> application::EventBus {
        let mut builder = EventBusBuilder::default();
        let _subscription = builder.subscribe(
            [DomainEventKind::TransactionsSyncRecorded].into(),
            Subscriber(Arc::clone(trace)),
        );
        builder.build()
    }
    fn execute_scope(
        state: SettledRowSet,
        observation: Result<TransactionRead, PortError>,
        failure: Option<SystemPortError>,
        now: Timestamp,
        scope: SettlementWindow,
    ) -> (
        Result<DeliveryReport, TransactionSyncError>,
        Arc<RwLock<Trace>>,
    ) {
        let trace = Arc::new(RwLock::new(Trace::default()));
        let repository = Repository {
            state,
            failure,
            trace: Arc::clone(&trace),
            gate: Arc::new(RwLock::new(Gate::Open)),
        };
        let broker = ScriptedBrokerRead::default();
        broker.script_transactions().push(observation);
        let clock = SteppedClock::new(now);
        let events = sync_events(&trace);
        let sync = SyncTransactions {
            broker: &broker,
            repository: &repository,
            clock: &clock,
            events: &events,
        };
        let result = run(sync.sync_window(&AccountId::new(91), scope));
        if result.is_ok() {
            let latest = run(repository.latest_sync(&AccountId::new(91)))
                .expect("상세 재조회")
                .expect("기록");
            assert_eq!(latest.window, scope);
            assert_eq!(
                run(repository.latest(&AccountId::new(91))).expect("기존 재조회"),
                Some(latest.summary())
            );
            assert_eq!(
                latest,
                trace.read().expect("잠금").records.last().expect("저장").1
            );
            assert_eq!(
                broker.calls(),
                vec![BrokerReadCall::Transactions(
                    AccountId::new(91),
                    scope,
                    TransactionFilter::all()
                )]
            );
            assert_eq!(clock.calls(), vec![now]);
            assert_eq!(
                trace.write().expect("잠금").events,
                vec![DomainEvent::TransactionsSyncRecorded(AccountId::new(91))]
            );
        }
        (result, trace)
    }
    fn execute(
        state: SettledRowSet,
        observation: Result<TransactionRead, PortError>,
        failure: Option<SystemPortError>,
        now: Timestamp,
    ) -> (
        Result<DeliveryReport, TransactionSyncError>,
        Arc<RwLock<Trace>>,
    ) {
        execute_scope(state, observation, failure, now, window())
    }
    #[test]
    fn duplicate_disappearance_preserves_both_boundaries_and_outside_rows() {
        let (result, trace) = execute(initial(), Ok(response(vec![row(3)])), None, at(20));
        assert_eq!(result, Ok(DeliveryReport::default()));
        let stored = trace.write().expect("잠금");
        let (account, record) = stored.records.first().expect("저장");
        assert_eq!(account, &AccountId::new(91));
        assert_eq!(record.recorded_at, at(20));
        assert_eq!(record.window, window());
        assert_eq!(record.summary().recorded_at(), at(20));
        assert_eq!(
            record.summary().outcome(),
            &domain::SyncOutcome::Complete(record.state.clone())
        );
        assert_eq!(record.observation, Ok(response(vec![row(3)])));
        assert_eq!(record.state.history_rows().len(), 5);
        let active = record
            .state
            .active_settled_rows()
            .map(|value| value.transaction().clone())
            .collect::<Vec<_>>();
        assert_eq!(active, vec![row(2), row(3), row(17)]);
        let gone = record
            .state
            .history_rows()
            .iter()
            .filter(|value| value.disappeared_at().is_some())
            .map(|value| (value.transaction().clone(), value.disappeared_at()))
            .collect::<Vec<_>>();
        assert_eq!(gone, vec![(row(3), Some(at(20))), (row(16), Some(at(20)))]);
        assert_eq!(
            record
                .reconciliation
                .as_ref()
                .expect("대조 성공")
                .as_ref()
                .expect("대조")
                .state(),
            &record.state
        );
    }
    #[test]
    fn incomplete_cursor_preserves_received_rows_without_disappearance() {
        let incomplete = TransactionRead::Incomplete(IncompleteTransactions {
            account: AccountId::new(91),
            window: window(),
            filter: TransactionFilter::all(),
            received: vec![TransactionBatch::try_new(vec![row(3)]).expect("받은 페이지")],
            quarantined: vec![],
            reason: IncompleteReason::PageInterrupted,
        });
        let (result, trace) = execute(initial(), Ok(incomplete.clone()), None, at(21));
        assert_eq!(result, Ok(DeliveryReport::default()));
        let stored = trace.write().expect("잠금");
        let record = &stored.records.first().expect("저장").1;
        assert_eq!(record.state, initial());
        assert_eq!(record.observation, Ok(incomplete));
        assert_eq!(
            record.summary().outcome(),
            &domain::SyncOutcome::Partial(initial())
        );
        assert_eq!(record.reconciliation, Ok(None));
    }
    #[test]
    fn broker_failure_and_access_denial_are_recorded_without_erasing_state() {
        let causes = [
            (
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::Network,
                    retry: RetryDelay::Unknown,
                },
                domain::SyncOutcome::Failed,
            ),
            (
                PortErrorKind::AccessDenied(application::AccessDenial::Forbidden),
                domain::SyncOutcome::AccessBlocked,
            ),
        ];
        for (cause, expected) in causes {
            let error = PortError::new(ReadEndpoint::Transactions, HttpStatus::NotReceived, cause);
            let (result, trace) = execute(initial(), Err(error), None, at(22));
            assert_eq!(result, Ok(DeliveryReport::default()));
            let stored = trace.write().expect("잠금");
            let record = &stored.records.first().expect("저장").1;
            assert_eq!(record.state, initial());
            assert_eq!(record.observation, Err(error));
            assert_eq!(record.summary().outcome(), &expected);
            assert_eq!(record.reconciliation, Ok(None));
        }
    }
    #[test]
    fn failed_save_emits_nothing() {
        let error = SystemPortError::new(SystemPort::Repository, SystemFailure::PermissionDenied);
        let (result, trace) = execute(initial(), Ok(response(vec![])), Some(error), at(23));
        assert_eq!(result, Err(TransactionSyncError::Repository(error)));
        let stored = trace.write().expect("잠금");
        assert_eq!(stored.records, []);
        assert_eq!(stored.events, []);
    }
    #[test]
    fn records_wrong_response_account_window_or_filter_without_changing_state() {
        let observations = [
            complete(
                AccountId::new(92),
                window(),
                TransactionFilter::all(),
                vec![],
            ),
            complete(
                AccountId::new(91),
                SettlementWindow::try_new(date(4), date(16)).expect("다른 창"),
                TransactionFilter::all(),
                vec![],
            ),
            complete(
                AccountId::new(91),
                window(),
                TransactionFilter::only(vec![TransactionKind::Buy]).expect("필터"),
                vec![],
            ),
        ];
        for observation in observations {
            let (result, trace) = execute(initial(), Ok(observation), None, at(24));
            assert_eq!(result, Ok(DeliveryReport::default()));
            let stored = trace.write().expect("잠금");
            let record = &stored.records.first().expect("실패 기록").1;
            assert_eq!(record.state, initial());
            assert_eq!(
                record.reconciliation,
                Err(TransactionSyncRejection::ResponseScopeMismatch)
            );
            assert_eq!(record.summary().outcome(), &domain::SyncOutcome::Failed);
        }
    }
    #[test]
    fn rejects_wrong_stored_account_and_clock_reversal() {
        for (state, now, expected) in [
            (
                SettledRowSet::new(AccountId::new(92)),
                at(25),
                TransactionSyncError::ResponseScopeMismatch,
            ),
            (
                initial(),
                at(9),
                TransactionSyncError::Reconciliation(ReconciliationError::InvalidHistory),
            ),
        ] {
            let (result, trace) = execute(state, Ok(response(vec![])), None, now);
            assert_eq!(result, Err(expected));
            let stored = trace.write().expect("잠금");
            assert_eq!(stored.records, []);
            assert_eq!(stored.events, []);
        }
    }
    #[test]
    fn incomplete_responses_must_match_all_requested_scope_fields() {
        let original = IncompleteTransactions {
            account: AccountId::new(91),
            window: window(),
            filter: TransactionFilter::all(),
            received: vec![],
            quarantined: vec![],
            reason: IncompleteReason::PageInterrupted,
        };
        let mut other_account = original.clone();
        other_account.account = AccountId::new(92);
        let mut other_window = original.clone();
        other_window.window = SettlementWindow::try_new(date(4), date(16)).expect("다른 창");
        let mut other_filter = original;
        other_filter.filter =
            TransactionFilter::only(vec![TransactionKind::Buy]).expect("다른 종류");
        for observation in [other_account, other_window, other_filter] {
            let (result, trace) = execute(
                initial(),
                Ok(TransactionRead::Incomplete(observation)),
                None,
                at(26),
            );
            assert_eq!(result, Ok(DeliveryReport::default()));
            let stored = trace.write().expect("잠금");
            let record = &stored.records.first().expect("실패 기록").1;
            assert_eq!(record.state, initial());
            assert_eq!(
                record.reconciliation,
                Err(TransactionSyncRejection::ResponseScopeMismatch)
            );
            assert_eq!(record.summary().outcome(), &domain::SyncOutcome::Failed);
        }
    }
    #[test]
    fn new_rows_and_equal_clock_are_accepted_without_changing_transaction_fields() {
        let (result, trace) = execute(
            initial(),
            Ok(response(vec![row(3), row(3), row(16), row(8)])),
            None,
            at(10),
        );
        assert_eq!(result, Ok(DeliveryReport::default()));
        let stored = trace.write().expect("잠금");
        let record = &stored.records.first().expect("저장").1;
        assert_eq!(
            record
                .state
                .active_settled_rows()
                .map(|entry| entry.transaction().clone())
                .collect::<Vec<_>>(),
            vec![row(2), row(3), row(3), row(16), row(17), row(8)]
        );
        assert_eq!(record.state.last_synchronized_at(), Some(at(10)));
        assert_eq!(
            record
                .reconciliation
                .as_ref()
                .expect("대조 성공")
                .as_ref()
                .expect("대조")
                .state(),
            &record.state
        );
    }
    fn interrupted() -> TransactionRead {
        TransactionRead::Incomplete(IncompleteTransactions {
            account: AccountId::new(91),
            window: window(),
            filter: TransactionFilter::all(),
            received: vec![],
            quarantined: vec![],
            reason: IncompleteReason::PageInterrupted,
        })
    }
    fn network_error() -> PortError {
        PortError::new(
            ReadEndpoint::Transactions,
            HttpStatus::NotReceived,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
        )
    }
    #[test]
    fn clock_reversal_rejects_incomplete_and_failed_reads() {
        for observation in [Ok(interrupted()), Err(network_error())] {
            let (result, trace) = execute(initial(), observation, None, at(9));
            assert_eq!(
                result,
                Err(TransactionSyncError::Reconciliation(
                    ReconciliationError::InvalidHistory
                ))
            );
            let stored = trace.read().expect("잠금");
            assert_eq!(stored.records, []);
            assert_eq!(stored.events, []);
        }
    }
    #[test]
    fn disappearance_warning_survives_storage_and_lookup() {
        let TransactionRead::Complete(received) = response(vec![row(3); 5]) else {
            panic!("완전 응답")
        };
        let state = SettledRowSet::new(AccountId::new(91))
            .reconcile(&received, at(10))
            .expect("시작")
            .state()
            .clone();
        let (result, trace) = execute(state, Ok(response(vec![])), None, at(30));
        assert_eq!(result, Ok(DeliveryReport::default()));
        let stored = trace.read().expect("잠금");
        let reconciliation = stored
            .records
            .first()
            .expect("기록")
            .1
            .reconciliation
            .as_ref()
            .expect("처리")
            .as_ref()
            .expect("대조");
        let domain::DisappearanceWarning::MassDisappearance {
            missing,
            scoped_active,
        } = reconciliation.disappearance_warning()
        else {
            panic!("경고")
        };
        assert_eq!(*missing.as_usize(), 5);
        assert_eq!(*scoped_active.as_usize(), 5);
    }
    #[test]
    fn cancellation_notice_survives_storage_and_lookup() {
        let mut changed = row(2).data().clone();
        changed.settlement_date = date(8);
        changed.cancellation = Cancellation::Cancelled;
        let cancellation = Transaction::try_new(changed).expect("취소");
        let (result, trace) = execute(
            initial(),
            Ok(response(vec![cancellation.clone()])),
            None,
            at(31),
        );
        assert_eq!(result, Ok(DeliveryReport::default()));
        let stored_notice = trace.read().expect("잠금");
        let notices = stored_notice
            .records
            .first()
            .expect("기록")
            .1
            .reconciliation
            .as_ref()
            .expect("처리")
            .as_ref()
            .expect("대조")
            .notices();
        let [
            domain::ReconciliationNotice::UnconfirmedCancellation {
                cancellation: actual,
                possible_originals,
            },
        ] = notices
        else {
            panic!("취소 알림")
        };
        assert_eq!(actual, &cancellation);
        assert_eq!(*possible_originals.as_usize(), 2);
    }
    #[test]
    fn failed_observation_retains_the_requested_window() {
        let wider = SettlementWindow::try_new(date(1), date(30)).expect("전체 fixture 창");
        let (_, first_trace) =
            execute_scope(initial(), Err(network_error()), None, at(32), window());
        let (_, second_trace) = execute_scope(initial(), Err(network_error()), None, at(32), wider);
        let first = first_trace
            .read()
            .expect("잠금")
            .records
            .first()
            .expect("기록")
            .1
            .clone();
        let second = second_trace
            .read()
            .expect("잠금")
            .records
            .first()
            .expect("기록")
            .1
            .clone();
        assert_eq!(first.window, window());
        assert_eq!(second.window, wider);
        assert_ne!(first, second);
        assert_eq!(first.summary().outcome(), &domain::SyncOutcome::Failed);
    }
    #[test]
    fn concurrent_success_is_preserved_when_an_older_failure_finishes_late() {
        let trace = Arc::new(RwLock::new(Trace::default()));
        let gate = Arc::new(RwLock::new(Gate::Held));
        let repository = Repository {
            state: initial(),
            failure: None,
            trace: Arc::clone(&trace),
            gate: Arc::clone(&gate),
        };
        let failing = ScriptedBrokerRead::default();
        failing.script_transactions().push(Err(network_error()));
        let succeeding = ScriptedBrokerRead::default();
        succeeding
            .script_transactions()
            .push(Ok(response(vec![row(3)])));
        let first_clock = SteppedClock::new(at(40));
        let second_clock = SteppedClock::new(at(41));
        let events = sync_events(&trace);
        let first = SyncTransactions {
            broker: &failing,
            repository: &repository,
            clock: &first_clock,
            events: &events,
        };
        let second = SyncTransactions {
            broker: &succeeding,
            repository: &repository,
            clock: &second_clock,
            events: &events,
        };
        let account = AccountId::new(91);
        let mut pending = std::pin::pin!(first.sync_window(&account, window()));
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(pending.as_mut().poll(&mut context), Poll::Pending);
        *gate.write().expect("gate") = Gate::Open;
        assert_eq!(
            run(second.sync_window(&account, window())),
            Ok(DeliveryReport::default())
        );
        assert_eq!(
            pending.as_mut().poll(&mut context),
            Poll::Ready(Err(TransactionSyncError::Repository(SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::InvalidData
            ))))
        );
        let latest = run(repository.latest_sync(&account))
            .expect("조회")
            .expect("기록");
        assert_eq!(latest.recorded_at, at(41));
        assert_eq!(run(repository.state(&account)), Ok(latest.state.clone()));
        assert_eq!(run(repository.latest(&account)), Ok(Some(latest.summary())));
        let stored = trace.read().expect("잠금");
        assert_eq!(stored.records.len(), 1);
        assert_eq!(
            stored.events,
            vec![DomainEvent::TransactionsSyncRecorded(account.clone())]
        );
    }
}
