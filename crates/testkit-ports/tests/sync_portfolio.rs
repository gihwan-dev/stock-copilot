#[cfg(test)]
mod tests {
    use application::{
        EventBusBuilder, EventSubscriber, HoldingsObservation, HttpStatus, PortError,
        PortErrorKind, PortfolioRepository, ReadEndpoint, RetryDelay, SubscriberFailure,
        SyncPortfolio, SystemPortError, UnavailableCause, WriteCompleted,
    };
    use domain::{
        AccountId, BrokerAccountNumberInput, BrokerAccountRow, BrokerAccountRows,
        BrokerAccountTypeCode, BuyingPower, Currency, CurrencyAmounts, DomainEvent,
        DomainEventKind, Money, Nanosecond, NumericInput, OffsetSeconds, Portfolio,
        ReportedPortfolio, SyncOutcome, SyncRecord, Timestamp, UnixSeconds,
    };
    use std::{
        future::Future,
        sync::mpsc::{self, Sender},
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{SteppedClock, scripted::ScriptedBrokerRead};
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Call {
        Store(AccountId, Box<SyncRecord<Portfolio>>),
        Event(DomainEvent),
    }
    struct Store {
        calls: Sender<Call>,
        result: Result<WriteCompleted, SystemPortError>,
    }
    impl PortfolioRepository for Store {
        fn record(
            &self,
            account: &AccountId,
            record: &SyncRecord<Portfolio>,
        ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
            self.calls
                .send(Call::Store(account.clone(), Box::new(record.clone())))
                .expect("기록");
            std::future::ready(self.result)
        }
        fn latest(
            &self,
            _: &AccountId,
        ) -> impl Future<Output = Result<Option<SyncRecord<Portfolio>>, SystemPortError>> + Send
        {
            std::future::ready(Err(SystemPortError::new(
                application::SystemPort::Repository,
                application::SystemFailure::InvalidData,
            )))
        }
    }
    struct Subscriber(Sender<Call>);
    impl EventSubscriber for Subscriber {
        fn on_event(&self, event: &DomainEvent) -> Result<(), SubscriberFailure> {
            self.0
                .send(Call::Event(event.clone()))
                .map_err(|_send_error| SubscriberFailure::Unavailable)
        }
    }
    fn run<F: Future>(f: F) -> F::Output {
        let mut pinned = std::pin::pin!(f);
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
            UnixSeconds::new(123),
            Nanosecond::new(17),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn money(value: &str, currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("입력"), currency).expect("금액")
    }
    fn power(currency: Currency) -> BuyingPower {
        BuyingPower::new(
            money(
                match currency {
                    Currency::Krw => "137",
                    Currency::Usd => "29",
                },
                currency,
            ),
            at(),
        )
    }
    fn holdings() -> HoldingsObservation {
        HoldingsObservation::from_broker_result(
            vec![],
            ReportedPortfolio::new(
                CurrencyAmounts::new(money("31", Currency::Krw), Some(money("7", Currency::Usd)))
                    .expect("평가"),
                CurrencyAmounts::new(money("23", Currency::Krw), Some(money("5", Currency::Usd)))
                    .expect("원가"),
                CurrencyAmounts::new(money("8", Currency::Krw), Some(money("2", Currency::Usd)))
                    .expect("손익"),
                at(),
            ),
        )
    }
    fn accounts() -> BrokerAccountRows {
        BrokerAccountRows::try_new(vec![
            BrokerAccountRow::from_broker_account_result(
                AccountId::new(41),
                BrokerAccountTypeCode::new("BROKERAGE"),
                BrokerAccountNumberInput::new("0000001234".to_owned()),
                at(),
            )
            .expect("행"),
        ])
        .expect("목록")
    }
    fn error() -> PortError {
        PortError::new(
            ReadEndpoint::BuyingPower,
            HttpStatus::NotReceived,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
        )
    }
    fn assert_outcome(path: u8, outcome: &SyncOutcome<Portfolio>) {
        match (path, outcome) {
            (0 | 4, SyncOutcome::Complete(portfolio)) => {
                assert_eq!(portfolio.reported(), Some(*holdings().reported()));
                assert_eq!(portfolio.buying_power(Currency::Krw), power(Currency::Krw));
                assert_eq!(portfolio.buying_power(Currency::Usd), power(Currency::Usd));
            }
            (1, SyncOutcome::Partial(portfolio)) => {
                assert_eq!(portfolio.buying_power(Currency::Usd), power(Currency::Usd));
                assert_eq!(
                    portfolio.buying_power(Currency::Krw).money(),
                    Err(domain::PortfolioError::CashLookupFailed(
                        domain::CashObservationFailure::Unavailable {
                            cause: domain::CashUnavailableCause::Network,
                            retry: domain::CashRetryDelay::Unknown
                        }
                    ))
                );
            }
            (2, SyncOutcome::Failed) | (3, SyncOutcome::AccessBlocked) => (),
            _ => panic!("결과 불일치"),
        }
    }
    fn broker_for(path: u8) -> ScriptedBrokerRead {
        let broker = ScriptedBrokerRead::default();
        broker.script_accounts().push(Ok(accounts()));
        broker.script_holdings().push(if path == 2 {
            Err(error())
        } else {
            Ok(holdings())
        });
        broker.script_buying_power().push(if path == 1 {
            Err(error())
        } else if path == 3 {
            Err(PortError::new(
                ReadEndpoint::BuyingPower,
                HttpStatus::Forbidden,
                PortErrorKind::AccessDenied(application::AccessDenial::Forbidden),
            ))
        } else {
            Ok(power(Currency::Krw))
        });
        broker.script_buying_power().push(Ok(power(Currency::Usd)));
        broker
    }
    #[test]
    fn success_partial_failure_blocked_and_storage_failure_record_before_one_event() {
        for path in 0_u8..5 {
            let broker = broker_for(path);
            let (calls, received) = mpsc::channel();
            let store = Store {
                calls: calls.clone(),
                result: if path == 4 {
                    Err(SystemPortError::new(
                        application::SystemPort::Repository,
                        application::SystemFailure::Unavailable(RetryDelay::Unknown),
                    ))
                } else {
                    Ok(WriteCompleted)
                },
            };
            let mut builder = EventBusBuilder::default();
            builder.subscribe(
                [DomainEventKind::PortfolioSyncRecorded].into(),
                Subscriber(calls.clone()),
            );
            let bus = builder.build();
            let result = run(
                SyncPortfolio::new(&broker, &store, &SteppedClock::new(at()), &bus)
                    .execute(&AccountId::new(41)),
            );
            let recorded: Vec<Call> = received.try_iter().collect();
            let Call::Store(account, record) = recorded.first().expect("저장") else {
                panic!("먼저 저장")
            };
            assert_eq!(account, &AccountId::new(41));
            assert_eq!(record.recorded_at(), at());
            assert_outcome(path, record.outcome());
            if path == 4 {
                assert_eq!(
                    result,
                    Err(SystemPortError::new(
                        application::SystemPort::Repository,
                        application::SystemFailure::Unavailable(RetryDelay::Unknown)
                    ))
                );
                assert_eq!(recorded.len(), 1);
            } else {
                let report = result.expect("저장 완료");
                assert_eq!(&report.record, record.as_ref());
                assert_eq!(report.delivery.failures, vec![]);
                assert_eq!(recorded.len(), 2);
                assert_eq!(
                    recorded.get(1),
                    Some(&Call::Event(DomainEvent::PortfolioSyncRecorded(
                        AccountId::new(41)
                    )))
                );
            }
        }
    }
}
