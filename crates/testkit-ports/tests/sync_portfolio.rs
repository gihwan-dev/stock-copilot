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
            vec![holding()],
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
        let rows = [41_i64, 67]
            .into_iter()
            .map(|id| {
                BrokerAccountRow::from_broker_account_result(
                    AccountId::new(id),
                    BrokerAccountTypeCode::new("BROKERAGE"),
                    BrokerAccountNumberInput::new("0000001234".to_owned()),
                    at(),
                )
                .expect("행")
            })
            .collect();
        BrokerAccountRows::try_new(rows).expect("목록")
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
                assert_eq!(portfolio.holdings(), holdings().holdings());
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
    fn observed_case(
        broker: &ScriptedBrokerRead,
        account: &AccountId,
    ) -> application::PortfolioSyncReport {
        let (calls, received) = mpsc::channel();
        let store = Store {
            calls: calls.clone(),
            result: Ok(WriteCompleted),
        };
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            [DomainEventKind::PortfolioSyncRecorded].into(),
            Subscriber(calls),
        );
        let bus = builder.build();
        let report = run(
            SyncPortfolio::new(broker, &store, &SteppedClock::new(at()), &bus).execute(account),
        )
        .expect("기록 성공");
        assert_eq!(
            received.try_iter().collect::<Vec<_>>(),
            vec![
                Call::Store(account.clone(), Box::new(report.record.clone())),
                Call::Event(DomainEvent::PortfolioSyncRecorded(account.clone()))
            ]
        );
        report
    }
    fn cash_reply(
        requested: Currency,
        failed: Currency,
        error: PortError,
    ) -> Result<BuyingPower, PortError> {
        if requested == failed {
            Err(error)
        } else {
            Ok(power(requested))
        }
    }
    fn failure_case(kind: PortErrorKind, expected: domain::CashObservationFailure) {
        for currency in [Currency::Krw, Currency::Usd] {
            let broker = ScriptedBrokerRead::default();
            broker.script_accounts().push(Ok(accounts()));
            broker.script_holdings().push(Ok(holdings()));
            let original = PortError::new(
                ReadEndpoint::BuyingPower,
                HttpStatus::OtherClientError,
                kind,
            );
            for requested in [Currency::Krw, Currency::Usd] {
                let response = cash_reply(requested, currency, original);
                broker.script_buying_power().push(response);
            }
            let report = observed_case(&broker, &AccountId::new(41));
            assert_eq!(report.failures, vec![original]);
            let SyncOutcome::Partial(portfolio) = report.record.outcome() else {
                panic!("부분 실패")
            };
            assert_eq!(portfolio.reported(), Some(*holdings().reported()));
            assert_eq!(portfolio.holdings(), holdings().holdings());
            assert_eq!(
                portfolio.buying_power(currency).money(),
                Err(domain::PortfolioError::CashLookupFailed(expected))
            );
            assert_eq!(portfolio.buying_power(currency).currency(), currency);
            assert_eq!(portfolio.buying_power(currency).observed_at(), at());
            assert_eq!(portfolio.holdings(), holdings().holdings());
        }
    }
    #[test]
    fn cash_error_reasons_group_0() {
        failure_case(
            PortErrorKind::NotFound(application::MissingData::Stock),
            domain::CashObservationFailure::NotFound(domain::CashMissingData::Stock),
        );
        failure_case(
            PortErrorKind::NotFound(application::MissingData::ExchangeRate),
            domain::CashObservationFailure::NotFound(domain::CashMissingData::ExchangeRate),
        );
        failure_case(
            PortErrorKind::NotFound(application::MissingData::Sector),
            domain::CashObservationFailure::NotFound(domain::CashMissingData::Sector),
        );
        failure_case(
            PortErrorKind::InvalidResponse(application::ResponseMismatch::Schema),
            domain::CashObservationFailure::InvalidResponse(domain::CashResponseMismatch::Schema),
        );
        failure_case(
            PortErrorKind::InvalidResponse(application::ResponseMismatch::Validation),
            domain::CashObservationFailure::InvalidResponse(
                domain::CashResponseMismatch::Validation,
            ),
        );
    }
    #[test]
    fn cash_error_reasons_group_1() {
        failure_case(
            PortErrorKind::InvalidRequest(application::RequestRejection::MalformedRequest),
            domain::CashObservationFailure::InvalidRequest(
                domain::CashRequestRejection::MalformedRequest,
            ),
        );
        failure_case(
            PortErrorKind::InvalidRequest(application::RequestRejection::MissingAccountHeader),
            domain::CashObservationFailure::InvalidRequest(
                domain::CashRequestRejection::MissingAccountHeader,
            ),
        );
        failure_case(
            PortErrorKind::InvalidRequest(application::RequestRejection::InvalidToken),
            domain::CashObservationFailure::InvalidRequest(
                domain::CashRequestRejection::InvalidToken,
            ),
        );
        failure_case(
            PortErrorKind::InvalidRequest(application::RequestRejection::EdgeBlocked),
            domain::CashObservationFailure::InvalidRequest(
                domain::CashRequestRejection::EdgeBlocked,
            ),
        );
        failure_case(
            PortErrorKind::InvalidRequest(application::RequestRejection::UnsupportedQuery),
            domain::CashObservationFailure::InvalidRequest(
                domain::CashRequestRejection::UnsupportedQuery,
            ),
        );
    }
    #[test]
    fn cash_error_reasons_group_2() {
        failure_case(
            PortErrorKind::InvalidRequest(application::RequestRejection::UnsupportedContentType),
            domain::CashObservationFailure::InvalidRequest(
                domain::CashRequestRejection::UnsupportedContentType,
            ),
        );
        failure_case(
            PortErrorKind::InvalidRequest(application::RequestRejection::BatchLimitExceeded),
            domain::CashObservationFailure::InvalidRequest(
                domain::CashRequestRejection::BatchLimitExceeded,
            ),
        );
        failure_case(
            PortErrorKind::Unavailable {
                cause: UnavailableCause::RateLimited,
                retry: RetryDelay::Unknown,
            },
            domain::CashObservationFailure::Unavailable {
                cause: domain::CashUnavailableCause::RateLimited,
                retry: domain::CashRetryDelay::Unknown,
            },
        );
        failure_case(
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Maintenance,
                retry: RetryDelay::Unknown,
            },
            domain::CashObservationFailure::Unavailable {
                cause: domain::CashUnavailableCause::Maintenance,
                retry: domain::CashRetryDelay::Unknown,
            },
        );
        failure_case(
            PortErrorKind::Unavailable {
                cause: UnavailableCause::ServerError,
                retry: RetryDelay::Unknown,
            },
            domain::CashObservationFailure::Unavailable {
                cause: domain::CashUnavailableCause::ServerError,
                retry: domain::CashRetryDelay::Unknown,
            },
        );
    }
    #[test]
    fn cash_error_reasons_group_3() {
        failure_case(
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
            domain::CashObservationFailure::Unavailable {
                cause: domain::CashUnavailableCause::Network,
                retry: domain::CashRetryDelay::Unknown,
            },
        );
        failure_case(
            PortErrorKind::Unavailable {
                cause: UnavailableCause::ExpiredToken,
                retry: RetryDelay::Unknown,
            },
            domain::CashObservationFailure::Unavailable {
                cause: domain::CashUnavailableCause::ExpiredToken,
                retry: domain::CashRetryDelay::Unknown,
            },
        );
    }
    #[test]
    fn account_lookup_errors_and_unselected_accounts_save_without_reading_holdings() {
        for response in [
            Err(error()),
            Err(PortError::new(
                ReadEndpoint::Accounts,
                HttpStatus::Forbidden,
                PortErrorKind::AccessDenied(application::AccessDenial::Forbidden),
            )),
            Ok(BrokerAccountRows::try_new(vec![]).expect("빈 목록")),
            Ok(accounts()),
        ] {
            let broker = ScriptedBrokerRead::default();
            broker.script_accounts().push(response.clone());
            let report = observed_case(&broker, &AccountId::new(99));
            assert_eq!(
                broker.calls(),
                vec![testkit_ports::scripted::BrokerReadCall::Accounts()]
            );
            let expected_failures = response
                .as_ref()
                .err()
                .copied()
                .into_iter()
                .collect::<Vec<_>>();
            assert_eq!(report.failures, expected_failures);
            match response {
                Err(failure) if failure == error() => {
                    assert_eq!(report.record.outcome(), &SyncOutcome::Failed);
                }
                Err(_) | Ok(_) => assert_eq!(report.record.outcome(), &SyncOutcome::AccessBlocked),
            }
        }
    }

    fn holding() -> domain::Holding {
        let master = domain::MasterInstrument::try_new(domain::MasterInstrumentRow {
            listing_market: domain::MasterCode::try_new("NASDAQ").expect("시장"),
            instrument_id: domain::InstrumentId::try_new("fixture-holding").expect("ID"),
            symbol: domain::Symbol::try_new("AAPL").expect("심볼"),
            name: domain::DisplayInstrumentName::try_new("고정 보유").expect("이름"),
            security_type: domain::MasterSecurityType::from_code(
                domain::MasterCode::try_new("STOCK").expect("종류"),
            ),
            currency: Currency::Usd,
            listing_status: domain::ListingStatus::Active,
            common_share: domain::CommonShare::Common,
            korean_detail: None,
            observed_at: at(),
        })
        .expect("마스터");
        domain::Holding::new(
            master.order_instrument().expect("종목"),
            domain::Quantity::parse(&NumericInput::try_new("3").expect("수량 입력")).expect("수량"),
            domain::AcquisitionPrice::try_new(money("11", Currency::Usd)).expect("원가"),
            domain::HoldingObservation::new(None, None, at()),
        )
        .expect("보유")
    }
    #[test]
    fn known_retry_delay_is_not_replaced_with_unknown_or_zero() {
        for delay in [domain::TimeSpan::NANOSECOND, domain::TimeSpan::DAY] {
            failure_case(
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::RateLimited,
                    retry: RetryDelay::Known(delay),
                },
                domain::CashObservationFailure::Unavailable {
                    cause: domain::CashUnavailableCause::RateLimited,
                    retry: domain::CashRetryDelay::Known(delay),
                },
            );
        }
    }
    #[test]
    fn one_account_failure_does_not_prevent_another_account_success() {
        let broker = ScriptedBrokerRead::default();
        for _ in 0_u8..2 {
            broker.script_accounts().push(Ok(accounts()));
            for currency in [Currency::Krw, Currency::Usd] {
                broker.script_buying_power().push(Ok(power(currency)));
            }
        }
        broker.script_holdings().push(Err(error()));
        broker.script_holdings().push(Ok(holdings()));
        let (calls, received) = mpsc::channel();
        let store = Store {
            calls: calls.clone(),
            result: Ok(WriteCompleted),
        };
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            [DomainEventKind::PortfolioSyncRecorded].into(),
            Subscriber(calls),
        );
        let bus = builder.build();
        let clock = SteppedClock::new(at());
        let use_case = SyncPortfolio::new(&broker, &store, &clock, &bus);
        let failed = run(use_case.execute(&AccountId::new(41))).expect("실패 기록");
        let complete = run(use_case.execute(&AccountId::new(67))).expect("다른 계좌 성공 기록");
        assert_eq!(failed.record.outcome(), &SyncOutcome::Failed);
        let SyncOutcome::Complete(portfolio) = complete.record.outcome() else {
            panic!("다른 계좌 정상")
        };
        assert_eq!(portfolio.holdings(), holdings().holdings());
        assert_eq!(portfolio.buying_power(Currency::Krw), power(Currency::Krw));
        assert_eq!(portfolio.buying_power(Currency::Usd), power(Currency::Usd));
        assert_eq!(
            received.try_iter().collect::<Vec<_>>(),
            vec![
                Call::Store(AccountId::new(41), Box::new(failed.record)),
                Call::Event(DomainEvent::PortfolioSyncRecorded(AccountId::new(41))),
                Call::Store(AccountId::new(67), Box::new(complete.record)),
                Call::Event(DomainEvent::PortfolioSyncRecorded(AccountId::new(67)))
            ]
        );
    }
    #[test]
    fn invalid_cash_cannot_be_reported_as_complete() {
        for currency in [Currency::Krw, Currency::Usd] {
            let broker = ScriptedBrokerRead::default();
            broker.script_accounts().push(Ok(accounts()));
            broker.script_holdings().push(Ok(holdings()));
            for requested in [Currency::Krw, Currency::Usd] {
                let cash = invalid_power(requested, currency);
                broker.script_buying_power().push(Ok(cash));
            }
            let report = observed_case(&broker, &AccountId::new(41));
            let SyncOutcome::Partial(portfolio) = report.record.outcome() else {
                panic!("오류가 있는 현금은 부분 결과")
            };
            assert_eq!(
                portfolio.buying_power(currency).money(),
                Err(domain::PortfolioError::NegativeBuyingPower)
            );
            assert_eq!(report.failures, vec![]);
        }
    }
    fn invalid_power(requested: Currency, invalid: Currency) -> BuyingPower {
        if requested == invalid {
            BuyingPower::new(money("-3", requested), at())
        } else {
            power(requested)
        }
    }
    #[test]
    fn unmapped_domain_cash_failure_is_failed_and_retains_original_error() {
        for kind in [
            PortErrorKind::NotFound(application::MissingData::Order),
            PortErrorKind::Unsupported(application::BrokerCapability::Sectors),
        ] {
            let broker = ScriptedBrokerRead::default();
            broker.script_accounts().push(Ok(accounts()));
            broker.script_holdings().push(Ok(holdings()));
            let original = PortError::new(ReadEndpoint::BuyingPower, HttpStatus::NotFound, kind);
            broker.script_buying_power().push(Err(original));
            broker.script_buying_power().push(Ok(power(Currency::Usd)));
            let report = observed_case(&broker, &AccountId::new(41));
            assert_eq!(report.record.outcome(), &SyncOutcome::Failed);
            assert_eq!(report.failures, vec![original]);
        }
    }
    fn denial(endpoint: ReadEndpoint) -> PortError {
        PortError::new(
            endpoint,
            HttpStatus::Forbidden,
            PortErrorKind::AccessDenied(application::AccessDenial::Forbidden),
        )
    }
    #[test]
    fn access_denial_at_every_read_position_wins_and_preserves_all_errors() {
        for (holding_error, krw_error, usd_error) in [
            (denial(ReadEndpoint::Holdings), error(), error()),
            (error(), denial(ReadEndpoint::BuyingPower), error()),
            (error(), error(), denial(ReadEndpoint::BuyingPower)),
        ] {
            let broker = ScriptedBrokerRead::default();
            broker.script_accounts().push(Ok(accounts()));
            broker.script_holdings().push(Err(holding_error));
            broker.script_buying_power().push(Err(krw_error));
            broker.script_buying_power().push(Err(usd_error));
            let report = observed_case(&broker, &AccountId::new(41));
            assert_eq!(report.record.outcome(), &SyncOutcome::AccessBlocked);
            assert_eq!(report.failures, vec![holding_error, krw_error, usd_error]);
        }
    }
    #[test]
    fn ordinary_holdings_failure_is_preserved_without_changing_it_to_success() {
        let original = PortError::new(
            ReadEndpoint::Holdings,
            HttpStatus::ServerError,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::ServerError,
                retry: RetryDelay::Unknown,
            },
        );
        let broker = ScriptedBrokerRead::default();
        broker.script_accounts().push(Ok(accounts()));
        broker.script_holdings().push(Err(original));
        broker.script_buying_power().push(Ok(power(Currency::Krw)));
        broker.script_buying_power().push(Ok(power(Currency::Usd)));
        let report = observed_case(&broker, &AccountId::new(41));
        assert_eq!(report.record.outcome(), &SyncOutcome::Failed);
        assert_eq!(report.failures, vec![original]);
    }
    struct ObservationClock(std::sync::RwLock<std::collections::VecDeque<Timestamp>>);
    impl application::Clock for ObservationClock {
        fn now(&self) -> Timestamp {
            self.0
                .write()
                .expect("시각 잠금")
                .pop_front()
                .expect("관측 시각")
        }
    }
    #[test]
    fn record_uses_completion_clock_and_cash_failure_keeps_its_observation_time() {
        let completed_at = Timestamp::from_parts(
            UnixSeconds::new(124),
            Nanosecond::new(31),
            OffsetSeconds::new(0),
        )
        .expect("완료 시각");
        let clock = ObservationClock(std::sync::RwLock::new([at(), completed_at].into()));
        let broker = broker_for(1);
        let (calls, received) = mpsc::channel();
        let store = Store {
            calls: calls.clone(),
            result: Ok(WriteCompleted),
        };
        let mut builder = EventBusBuilder::default();
        builder.subscribe(
            [DomainEventKind::PortfolioSyncRecorded].into(),
            Subscriber(calls),
        );
        let bus = builder.build();
        let report =
            run(SyncPortfolio::new(&broker, &store, &clock, &bus).execute(&AccountId::new(41)))
                .expect("기록");
        assert_eq!(report.record.recorded_at(), completed_at);
        let SyncOutcome::Partial(portfolio) = report.record.outcome() else {
            panic!("부분 결과")
        };
        assert_eq!(portfolio.buying_power(Currency::Krw).observed_at(), at());
        assert_eq!(
            received.try_iter().collect::<Vec<_>>(),
            vec![
                Call::Store(AccountId::new(41), Box::new(report.record)),
                Call::Event(DomainEvent::PortfolioSyncRecorded(AccountId::new(41)))
            ]
        );
    }
}
