#[cfg(test)]
mod tests {
    use application::{
        BackfillValuationDays, HttpStatus, PortError, PortErrorKind, ReadEndpoint, RecordOutcome,
        RetryDelay, SystemFailure, SystemPort, SystemPortError, UnavailableCause,
        ValuationBackfillError, ValuationCalendarSource, ValuationDayInputSource,
        ValuationDayRepository, ValuationDaySuccessSource, ValuationInputError,
    };
    use domain::{
        AccountId, BuyingPower, CalendarDate, CalendarYear, CashLedger, CashLedgerInput,
        CompleteTransactionResponse, CompletionAttestation, Currency, DayCashLedger, DayNumber,
        ExternalFlows, FlowPeriod, FxPair, FxRate, FxRateKind, FxSource, LedgerEvidence,
        MarketCalendar, MarketSession, Money, MonthNumber, Nanosecond, NumericInput, OffsetSeconds,
        SessionData, SessionInformation, SettledRowSet, SettlementWindow, SnapshotError,
        SnapshotObservation, TimeInterval, Timestamp, TransactionBatch, TransactionFilter,
        UnixSeconds, ValuationCalendarDay, ValuationDayHistory, ValuationDayInputs,
        ValuationDayResult, valuation_time,
    };
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    use testkit_ports::{Script, SteppedClock};

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
    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(1970),
            MonthNumber::new(1),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    fn start_of(day: u8) -> i64 {
        i64::from(day)
            .checked_sub(1)
            .expect("일")
            .checked_mul(86_400)
            .expect("초")
    }
    fn session(day: u8, market: &domain::Market) -> MarketSession {
        let regular_market = Some(
            SessionData::try_new(
                TimeInterval::try_new(
                    at(start_of(day)),
                    at(start_of(day).checked_add(3_600).expect("끝")),
                )
                .expect("구간"),
                None,
            )
            .expect("세션"),
        );
        MarketSession::try_new(
            date(day),
            match market {
                domain::Market::Kr => SessionInformation::Kr {
                    pre_market: None,
                    regular_market,
                    after_market: None,
                },
                domain::Market::Us | domain::Market::Unknown(_) => SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market,
                    after_market: None,
                },
            },
        )
        .expect("세션")
    }
    fn row(day: u8) -> ValuationCalendarDay {
        let calendar = |market: &domain::Market| {
            MarketCalendar::try_new(
                session(day.checked_sub(1).expect("전일"), market),
                session(day, market),
                session(day.checked_add(1).expect("익일"), market),
            )
            .expect("달력")
        };
        ValuationCalendarDay::new(
            &calendar(&domain::Market::Kr),
            &calendar(&domain::Market::Us),
        )
        .expect("평가일")
    }
    fn account() -> AccountId {
        AccountId::new(1)
    }
    fn zero(currency: Currency) -> Money {
        Money::parse(&NumericInput::try_new("0").expect("입력"), currency).expect("금액")
    }
    fn now() -> Timestamp {
        at(start_of(4).checked_add(36_000).expect("시각"))
    }
    fn empty_inputs(day: u8) -> ValuationDayInputs {
        let rows = SettledRowSet::try_restore(
            account(),
            Vec::new(),
            TransactionBatch::try_new(Vec::new()).expect("목록"),
            Some(now()),
        )
        .expect("행 집합");
        let window = SettlementWindow::try_new(date(1), date(day)).expect("기간");
        let cash = |currency| {
            let evidence = LedgerEvidence::from_complete_response(
                CompleteTransactionResponse::after_last_page_without_quarantine(
                    account(),
                    window,
                    TransactionFilter::all(),
                    TransactionBatch::try_new(Vec::new()).expect("목록"),
                    CompletionAttestation::AllPagesAndAllMatchingPendingReceived,
                )
                .expect("완전 응답"),
            )
            .expect("증거");
            let ledger = CashLedger::reconstruct(
                &rows,
                CashLedgerInput {
                    window,
                    currency,
                    opening: Some(zero(currency)),
                    evidence: Some(evidence),
                    reported: Some(BuyingPower::new(zero(currency), now())),
                },
            );
            let period = match ledger.external_flows(&[]) {
                ExternalFlows::Reconciled(flows) => match flows.period(window) {
                    FlowPeriod::Confirmed(period) => period,
                    FlowPeriod::Uncertain(_) => panic!("확정 기간"),
                },
                ExternalFlows::Unavailable(_) => panic!("대사"),
            };
            DayCashLedger::new(ledger, period)
        };
        let valued = valuation_time(date(day));
        let fx = FxRate::parse(
            &NumericInput::try_new("1300").expect("입력"),
            FxPair::UsdKrw,
            FxRateKind::Mid,
            TimeInterval::try_new(valued, now()).expect("구간"),
            FxSource::TossOpenApi,
        )
        .expect("환율");
        let ledgers = (cash(Currency::Krw), cash(Currency::Usd));
        ValuationDayInputs::new(
            date(day),
            rows,
            ValuationDayHistory::new(Vec::new(), Vec::new()),
            ledgers,
            SnapshotObservation::new(valued, now(), Some(fx)),
        )
    }

    type CalendarReply = Result<Vec<ValuationCalendarDay>, PortError>;
    #[derive(Default)]
    struct Calendars(Script<(CalendarDate, Timestamp), CalendarReply>);
    impl ValuationCalendarSource for Calendars {
        async fn days(&self, last: CalendarDate, now: Timestamp) -> CalendarReply {
            self.0.run((last, now)).await
        }
    }
    type InputReply = Result<ValuationDayInputs, ValuationInputError>;
    #[derive(Default)]
    struct Inputs(Script<(AccountId, CalendarDate), InputReply>);
    impl ValuationDayInputSource for Inputs {
        async fn inputs(&self, account: &AccountId, date: CalendarDate) -> InputReply {
            self.0.run((account.clone(), date)).await
        }
    }
    type SuccessReply = Result<Vec<CalendarDate>, SystemPortError>;
    #[derive(Default)]
    struct Successes(Script<(AccountId, Vec<CalendarDate>), SuccessReply>);
    impl ValuationDaySuccessSource for Successes {
        async fn succeeded(&self, account: &AccountId, dates: &[CalendarDate]) -> SuccessReply {
            self.0.run((account.clone(), dates.to_vec())).await
        }
    }
    type RecordReply = Result<RecordOutcome, SystemPortError>;
    #[derive(Default)]
    struct Days(Script<(AccountId, ValuationDayResult), RecordReply>);
    impl ValuationDayRepository for Days {
        async fn record(&self, account: &AccountId, result: &ValuationDayResult) -> RecordReply {
            self.0.run((account.clone(), result.clone())).await
        }
    }

    struct Fixture {
        calendars: Calendars,
        clock: SteppedClock,
        inputs: Inputs,
        successes: Successes,
        days: Days,
    }
    impl Fixture {
        fn new() -> Self {
            let calendars = Calendars::default();
            calendars.0.push(Ok(vec![row(2), row(3), row(4)]));
            Self {
                calendars,
                clock: SteppedClock::new(now()),
                inputs: Inputs::default(),
                successes: Successes::default(),
                days: Days::default(),
            }
        }
        fn execute(&self) -> Result<Vec<ValuationDayResult>, ValuationBackfillError> {
            run(BackfillValuationDays {
                calendars: &self.calendars,
                clock: &self.clock,
                inputs: &self.inputs,
                successes: &self.successes,
                days: &self.days,
            }
            .execute(&account(), date(1)))
        }
        fn recorded(&self) -> Vec<(CalendarDate, Result<CalendarDate, SnapshotError>)> {
            self.days
                .0
                .calls()
                .into_iter()
                .map(|(_, result)| {
                    (
                        result.date(),
                        result
                            .result()
                            .map(domain::ValuationSnapshot::settlement_date)
                            .map_err(|error| *error),
                    )
                })
                .collect()
        }
    }

    #[test]
    fn missing_daily_candles_fail_only_that_date_and_filled_dates_are_not_rebuilt() {
        let fixture = Fixture::new();
        fixture.successes.0.push(Ok(vec![date(3)]));
        fixture.inputs.0.push(Ok(empty_inputs(2)));
        fixture.inputs.0.push(Err(ValuationInputError::Missing(
            SnapshotError::MissingPrice,
        )));
        for _ in 0_u8..2 {
            fixture.days.0.push(Ok(RecordOutcome::Recorded));
        }
        let results = fixture.execute().expect("기록");
        assert_eq!(results.len(), 2);
        assert_eq!(
            fixture.successes.0.calls(),
            vec![(account(), vec![date(2), date(3), date(4)])]
        );
        assert_eq!(
            fixture.inputs.0.calls(),
            vec![(account(), date(2)), (account(), date(4))]
        );
        assert_eq!(
            fixture.recorded(),
            vec![
                (date(2), Ok(date(2))),
                (date(4), Err(SnapshotError::MissingPrice))
            ]
        );
    }

    #[test]
    fn failed_dates_are_retried_on_the_next_run_because_only_successes_are_excluded() {
        let fixture = Fixture::new();
        fixture.calendars.0.push(Ok(vec![row(2), row(3), row(4)]));
        fixture.successes.0.push(Ok(vec![date(2), date(3)]));
        fixture.successes.0.push(Ok(vec![date(2), date(3)]));
        fixture.inputs.0.push(Err(ValuationInputError::Missing(
            SnapshotError::MissingHistory,
        )));
        fixture.inputs.0.push(Ok(empty_inputs(4)));
        fixture.days.0.push(Ok(RecordOutcome::Recorded));
        fixture.days.0.push(Ok(RecordOutcome::Recorded));
        fixture.execute().expect("첫 실행");
        fixture.execute().expect("재시도");
        assert_eq!(
            fixture.inputs.0.calls(),
            vec![(account(), date(4)), (account(), date(4))]
        );
        assert_eq!(
            fixture.recorded(),
            vec![
                (date(4), Err(SnapshotError::MissingHistory)),
                (date(4), Ok(date(4)))
            ]
        );
    }

    #[test]
    fn inputs_for_another_date_are_recorded_as_a_failure_for_the_planned_date() {
        let fixture = Fixture::new();
        fixture.successes.0.push(Ok(vec![date(2), date(3)]));
        fixture.inputs.0.push(Ok(empty_inputs(3)));
        fixture.days.0.push(Ok(RecordOutcome::AlreadyRecorded));
        fixture.execute().expect("기록");
        assert_eq!(
            fixture.recorded(),
            vec![(date(4), Err(SnapshotError::WrongPeriod))]
        );
    }

    #[test]
    fn everything_filled_or_nothing_planned_touches_no_input_or_record() {
        let fixture = Fixture::new();
        fixture
            .successes
            .0
            .push(Ok(vec![date(2), date(3), date(4)]));
        assert_eq!(fixture.execute(), Ok(Vec::new()));
        let empty = Fixture::new();
        let calendars = Calendars::default();
        calendars.0.push(Ok(Vec::new()));
        let clock = SteppedClock::new(at(start_of(1).checked_add(3_600).expect("시각")));
        assert_eq!(
            run(BackfillValuationDays {
                calendars: &calendars,
                clock: &clock,
                inputs: &empty.inputs,
                successes: &empty.successes,
                days: &empty.days,
            }
            .execute(&account(), date(1))),
            Ok(Vec::new())
        );
        for item in [&fixture, &empty] {
            assert_eq!(item.inputs.0.calls().len(), 0);
            assert_eq!(item.days.0.calls().len(), 0);
        }
        assert_eq!(empty.successes.0.calls().len(), 0);
    }

    #[test]
    fn port_failures_stop_before_recording_and_keep_their_cause() {
        let port = PortError::new(
            ReadEndpoint::Candles,
            HttpStatus::NotReceived,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
        );
        let fixture = Fixture::new();
        fixture.successes.0.push(Ok(Vec::new()));
        fixture.inputs.0.push(Ok(empty_inputs(2)));
        fixture.inputs.0.push(Err(ValuationInputError::Port(port)));
        assert_eq!(fixture.execute(), Err(ValuationBackfillError::Inputs(port)));
        assert_eq!(fixture.days.0.calls().len(), 0);

        let storage = SystemPortError::new(SystemPort::Repository, SystemFailure::InvalidData);
        let lookup = Fixture::new();
        lookup.successes.0.push(Err(storage));
        assert_eq!(
            lookup.execute(),
            Err(ValuationBackfillError::Repository(storage))
        );
        assert_eq!(lookup.inputs.0.calls().len(), 0);

        let write = Fixture::new();
        write.successes.0.push(Ok(vec![date(3), date(4)]));
        write.inputs.0.push(Ok(empty_inputs(2)));
        write.days.0.push(Err(storage));
        assert_eq!(
            write.execute(),
            Err(ValuationBackfillError::Repository(storage))
        );

        let calendar = Fixture {
            calendars: Calendars::default(),
            ..Fixture::new()
        };
        let calendar_error = PortError::new(
            ReadEndpoint::Calendar,
            HttpStatus::NotReceived,
            PortErrorKind::Unavailable {
                cause: UnavailableCause::Network,
                retry: RetryDelay::Unknown,
            },
        );
        calendar.calendars.0.push(Err(calendar_error));
        assert_eq!(
            calendar.execute(),
            Err(ValuationBackfillError::Gap(
                application::ValuationGapError::Calendar(calendar_error)
            ))
        );
    }
}
