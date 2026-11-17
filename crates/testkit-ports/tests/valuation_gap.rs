#[cfg(test)]
mod tests {
    use application::{
        AccessDenial, BrokerCapability, HttpStatus, PlanValuationGap, PortError, PortErrorKind,
        ReadEndpoint, ResponseMismatch, RetryDelay, UnavailableCause, ValuationCalendarSource,
        ValuationGapError,
    };
    use domain::{
        CalendarDate, CalendarYear, DayNumber, MarketCalendar, MarketSession, MonthNumber,
        Nanosecond, OffsetSeconds, SessionData, SessionInformation, SnapshotError, TimeInterval,
        Timestamp, UnixSeconds, ValuationCalendarDay,
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
    fn session(day: u8, state: MarketState, market: &domain::Market) -> MarketSession {
        let regular_market = match state {
            MarketState::Open => Some(
                SessionData::try_new(
                    TimeInterval::try_new(
                        at(i64::from(day)
                            .checked_mul(86_400)
                            .expect("곱")
                            .checked_sub(86_400)
                            .expect("차")),
                        at(i64::from(day)
                            .checked_mul(86_400)
                            .expect("곱")
                            .checked_sub(82_800)
                            .expect("차")),
                    )
                    .expect("구간"),
                    None,
                )
                .expect("세션"),
            ),
            MarketState::Closed => None,
        };
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
    #[derive(Clone, Copy)]
    enum MarketState {
        Open,
        Closed,
    }
    fn row(day: u8, state: MarketState) -> ValuationCalendarDay {
        let calendar = |market: &domain::Market| {
            MarketCalendar::try_new(
                session(day.checked_sub(1).expect("전일"), MarketState::Open, market),
                session(day, state, market),
                session(day.checked_add(1).expect("익일"), MarketState::Open, market),
            )
            .expect("달력")
        };
        ValuationCalendarDay::new(
            &calendar(&domain::Market::Kr),
            &calendar(&domain::Market::Us),
        )
        .expect("평가일")
    }
    type CalendarReply = Result<Vec<ValuationCalendarDay>, PortError>;
    #[derive(Default)]
    struct CalendarSource(Script<(CalendarDate, Timestamp), CalendarReply>);
    impl ValuationCalendarSource for CalendarSource {
        async fn days(&self, last: CalendarDate, now: Timestamp) -> CalendarReply {
            self.0.run((last, now)).await
        }
    }
    #[test]
    fn plans_all_candidates_and_keeps_unavailable_separate_from_confirmed_holidays() {
        let source = CalendarSource::default();
        source.0.push(Ok(vec![
            row(2, MarketState::Open),
            ValuationCalendarDay::unavailable(date(3), SnapshotError::ClosedDay),
            row(4, MarketState::Closed),
            row(5, MarketState::Open),
        ]));
        let clock = SteppedClock::new(at(350_000));
        let plan = run(PlanValuationGap {
            calendars: &source,
            clock: &clock,
        }
        .execute(date(1)))
        .expect("계획");
        assert_eq!(
            plan.dates().collect::<Vec<_>>(),
            vec![date(2), date(3), date(5)]
        );
        let results = plan.resolve(|_, _| Err(SnapshotError::MissingHistory));
        assert_eq!(
            results
                .iter()
                .map(|result| (
                    result.date(),
                    result.result().cloned().map_err(|error| *error)
                ))
                .collect::<Vec<_>>(),
            vec![
                (date(2), Err(SnapshotError::MissingHistory)),
                (date(3), Err(SnapshotError::ClosedDay)),
                (date(5), Err(SnapshotError::MissingHistory))
            ]
        );
        assert_eq!(source.0.calls(), vec![(date(1), at(350_000))]);
        assert_eq!(clock.calls(), vec![at(350_000)]);
    }
    #[test]
    fn source_failures_are_not_relabelled_as_holidays() {
        for (status, kind) in [
            (
                HttpStatus::Success,
                PortErrorKind::InvalidResponse(ResponseMismatch::Validation),
            ),
            (
                HttpStatus::NotReceived,
                PortErrorKind::Unavailable {
                    cause: UnavailableCause::Network,
                    retry: RetryDelay::Unknown,
                },
            ),
            (
                HttpStatus::Forbidden,
                PortErrorKind::AccessDenied(AccessDenial::IpNotAllowed),
            ),
            (
                HttpStatus::BadRequest,
                PortErrorKind::Unsupported(BrokerCapability::Calendar),
            ),
        ] {
            let source = CalendarSource::default();
            let error = PortError::new(ReadEndpoint::Calendar, status, kind);
            source.0.push(Err(error));
            let clock = SteppedClock::new(at(100_000));
            assert_eq!(
                run(PlanValuationGap {
                    calendars: &source,
                    clock: &clock
                }
                .execute(date(1))),
                Err(ValuationGapError::Calendar(error))
            );
            assert_eq!(source.0.calls(), vec![(date(1), at(100_000))]);
            assert_eq!(clock.calls(), vec![at(100_000)]);
        }
    }
    #[test]
    fn incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain() {
        for (last, rows, expected) in [
            (date(1), vec![], SnapshotError::CalendarMismatch),
            (date(3), vec![], SnapshotError::WrongPeriod),
            (
                date(1),
                vec![row(3, MarketState::Open)],
                SnapshotError::CalendarMismatch,
            ),
        ] {
            let source = CalendarSource::default();
            source.0.push(Ok(rows));
            let clock = SteppedClock::new(at(100_000));
            assert_eq!(
                run(PlanValuationGap {
                    calendars: &source,
                    clock: &clock
                }
                .execute(last)),
                Err(ValuationGapError::Plan(expected))
            );
        }
    }
    #[test]
    fn same_date_is_empty_and_kst_midnight_changes_candidate_range() {
        let source = CalendarSource::default();
        source.0.push(Ok(vec![]));
        source.0.push(Ok(vec![row(3, MarketState::Open)]));
        let clock = SteppedClock::new(at(140_399));
        let use_case = PlanValuationGap {
            calendars: &source,
            clock: &clock,
        };
        assert_eq!(
            run(use_case.execute(date(2)))
                .expect("계획")
                .dates()
                .collect::<Vec<_>>(),
            vec![]
        );
        clock.advance_to(at(140_400)).expect("자정");
        assert_eq!(
            run(use_case.execute(date(2)))
                .expect("계획")
                .dates()
                .collect::<Vec<_>>(),
            vec![date(3)]
        );
        assert_eq!(
            source.0.calls(),
            vec![(date(2), at(140_399)), (date(2), at(140_400))]
        );
        assert_eq!(clock.calls(), vec![at(140_399), at(140_400)]);
    }
}
