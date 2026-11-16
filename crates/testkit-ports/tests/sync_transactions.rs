#[cfg(test)]
mod tests {
    use application::{
        DeliveryReport, EventBusBuilder, EventSubscriber, HttpStatus, IncompleteReason,
        IncompleteTransactions, PortError, PortErrorKind, ReadEndpoint, RetryDelay,
        SubscriberFailure, SyncTransactions, SystemFailure, SystemPort, SystemPortError,
        TransactionRead, TransactionSyncError, TransactionSyncRecord, TransactionSyncRepository,
        UnavailableCause, WriteCompleted,
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
        events: Vec<DomainEvent>,
    }
    #[derive(Debug)]
    struct Repository {
        state: SettledRowSet,
        failure: Option<SystemPortError>,
        trace: Arc<RwLock<Trace>>,
    }
    impl TransactionSyncRepository for Repository {
        fn state(
            &self,
            _account: &AccountId,
        ) -> impl Future<Output = Result<SettledRowSet, SystemPortError>> + Send {
            std::future::ready(Ok(self.state.clone()))
        }
        fn record(
            &self,
            account: &AccountId,
            previous: &SettledRowSet,
            record: &TransactionSyncRecord,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            assert_eq!(previous, &self.state);
            if let Some(error) = self.failure {
                return std::future::ready(Err(error));
            }
            self.trace
                .write()
                .expect("잠금")
                .records
                .push((account.clone(), record.clone()));
            std::future::ready(Ok(WriteCompleted))
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
    fn execute(
        state: SettledRowSet,
        observation: Result<TransactionRead, PortError>,
        failure: Option<SystemPortError>,
        now: Timestamp,
    ) -> (
        Result<DeliveryReport, TransactionSyncError>,
        Arc<RwLock<Trace>>,
    ) {
        let trace = Arc::new(RwLock::new(Trace::default()));
        let repository = Repository {
            state,
            failure,
            trace: Arc::clone(&trace),
        };
        let broker = ScriptedBrokerRead::default();
        broker.script_transactions().push(observation);
        let clock = SteppedClock::new(now);
        let mut builder = EventBusBuilder::default();
        let _subscription = builder.subscribe(
            [DomainEventKind::TransactionsSyncRecorded].into(),
            Subscriber(Arc::clone(&trace)),
        );
        let events = builder.build();
        let sync = SyncTransactions {
            broker: &broker,
            repository: &repository,
            clock: &clock,
            events: &events,
        };
        let result = run(sync.sync_window(&AccountId::new(91), window()));
        if result.is_ok() {
            assert_eq!(
                broker.calls(),
                vec![BrokerReadCall::Transactions(
                    AccountId::new(91),
                    window(),
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
    #[test]
    fn duplicate_disappearance_preserves_both_boundaries_and_outside_rows() {
        let (result, trace) = execute(initial(), Ok(response(vec![row(3)])), None, at(20));
        assert_eq!(result, Ok(DeliveryReport::default()));
        let stored = trace.write().expect("잠금");
        let (account, record) = stored.records.first().expect("저장");
        assert_eq!(account, &AccountId::new(91));
        assert_eq!(record.recorded_at, at(20));
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
            record.reconciliation.as_ref().expect("대조").state(),
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
        assert_eq!(record.reconciliation, None);
    }
    #[test]
    fn broker_failure_and_access_denial_are_recorded_without_erasing_state() {
        let causes = [
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
            PortErrorKind::AccessDenied(application::AccessDenial::Forbidden),
        ];
        for cause in causes {
            let error = PortError::new(ReadEndpoint::Transactions, HttpStatus::NotReceived, cause);
            let (result, trace) = execute(initial(), Err(error), None, at(22));
            assert_eq!(result, Ok(DeliveryReport::default()));
            let stored = trace.write().expect("잠금");
            let record = &stored.records.first().expect("저장").1;
            assert_eq!(record.state, initial());
            assert_eq!(record.observation, Err(error));
            assert_eq!(record.reconciliation, None);
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
    fn rejects_wrong_response_account_window_or_filter_without_save_or_event() {
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
            assert_eq!(result, Err(TransactionSyncError::ResponseScopeMismatch));
            let stored = trace.write().expect("잠금");
            assert_eq!(stored.records, []);
            assert_eq!(stored.events, []);
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
            assert_eq!(result, Err(TransactionSyncError::ResponseScopeMismatch));
            let stored = trace.write().expect("잠금");
            assert_eq!(stored.records, []);
            assert_eq!(stored.events, []);
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
            record.reconciliation.as_ref().expect("대조").state(),
            &record.state
        );
    }
}
