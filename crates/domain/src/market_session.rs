use crate::{CalendarDate, Market, TimeInterval, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketState {
    Unknown,
    Holiday,
    BeforeOpen,
    DayMarket,
    PreMarket,
    RegularMarket,
    AfterMarket,
    BetweenSessions,
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionInformation {
    Unavailable,
    Kr {
        pre_market: Option<TimeInterval>,
        regular_market: Option<TimeInterval>,
        after_market: Option<TimeInterval>,
    },
    Us {
        day_market: Option<TimeInterval>,
        pre_market: Option<TimeInterval>,
        regular_market: Option<TimeInterval>,
        after_market: Option<TimeInterval>,
    },
}

impl SessionInformation {
    const fn sessions(self) -> [(MarketState, Option<TimeInterval>); 4] {
        let (day, pre, regular, after) = match self {
            Self::Unavailable => (None, None, None, None),
            Self::Kr {
                pre_market,
                regular_market,
                after_market,
            } => (None, pre_market, regular_market, after_market),
            Self::Us {
                day_market,
                pre_market,
                regular_market,
                after_market,
            } => (day_market, pre_market, regular_market, after_market),
        };
        [
            (MarketState::DayMarket, day),
            (MarketState::PreMarket, pre),
            (MarketState::RegularMarket, regular),
            (MarketState::AfterMarket, after),
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketSession {
    market: Market,
    date: CalendarDate,
    information: SessionInformation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketSessionError {
    UnsupportedMarket,
    MarketMismatch,
    OverlappingOrUnordered,
}

impl MarketSession {
    pub fn try_new(
        market: Market,
        date: CalendarDate,
        information: SessionInformation,
    ) -> Result<Self, MarketSessionError> {
        match market {
            Market::Unknown(_) => return Err(MarketSessionError::UnsupportedMarket),
            Market::Kr | Market::Us => {}
        }
        let expected = match information {
            SessionInformation::Unavailable => None,
            SessionInformation::Kr { .. } => Some(Market::Kr),
            SessionInformation::Us { .. } => Some(Market::Us),
        };
        if expected.is_some_and(|value| value != market) {
            return Err(MarketSessionError::MarketMismatch);
        }
        let mut previous_end = None;
        for (_, interval) in information.sessions() {
            let Some(value) = interval else {
                continue;
            };
            if previous_end.is_some_and(|end| end > value.start()) {
                return Err(MarketSessionError::OverlappingOrUnordered);
            }
            previous_end = Some(value.end());
        }
        Ok(Self {
            market,
            date,
            information,
        })
    }

    #[must_use]
    pub const fn market(&self) -> &Market {
        &self.market
    }

    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.date
    }

    #[must_use]
    pub fn state_at(&self, instant: Timestamp) -> MarketState {
        if self.information == SessionInformation::Unavailable {
            return MarketState::Unknown;
        }
        let mut previous_end = None;
        for (state, interval) in self.information.sessions() {
            let Some(value) = interval else {
                continue;
            };
            if instant < value.start() {
                return previous_end
                    .map_or(MarketState::BeforeOpen, |_| MarketState::BetweenSessions);
            }
            if instant < value.end() {
                return state;
            }
            previous_end = Some(value.end());
        }
        if previous_end.is_some() {
            MarketState::Ended
        } else {
            MarketState::Holiday
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MarketSession, MarketSessionError, MarketState, SessionInformation};
    use crate::{CalendarDate, Market, MarketCode, TimeInterval, TimeIntervalError, Timestamp};
    use proptest::test_runner::{Config, TestRunner};
    use time::{Date, Month, UtcOffset};

    fn date(month: Month, day: u8) -> CalendarDate {
        CalendarDate::new(Date::from_calendar_date(2026, month, day).expect("고정한 날짜"))
    }

    fn at(month: Month, day: u8, hour: u8, minute: u8, second: u8) -> Timestamp {
        Timestamp::new(
            date(month, day)
                .as_date()
                .with_hms(hour, minute, second)
                .expect("유효한 시각")
                .assume_offset(UtcOffset::from_hms(9, 0, 0).expect("KST")),
        )
    }

    fn interval(start: Timestamp, end: Timestamp) -> TimeInterval {
        TimeInterval::try_new(start, end).expect("앞으로 흐르는 구간")
    }

    fn us(regular: TimeInterval, day: CalendarDate) -> MarketSession {
        MarketSession::try_new(
            Market::Us,
            day,
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(regular),
                after_market: None,
            },
        )
        .expect("미국 세션")
    }

    #[test]
    fn dst_fridays_and_mondays_use_literal_korean_boundaries_across_midnight() {
        for (month, day, next, hour, close) in [
            (Month::March, 6, 7, 23, 6),
            (Month::March, 9, 10, 22, 5),
            (Month::October, 30, 31, 22, 5),
            (Month::November, 2, 3, 23, 6),
        ] {
            let start = at(month, day, hour, 30, 0);
            let end = at(month, next, close, 0, 0);
            let session = us(interval(start, end), date(month, day));
            assert_eq!(session.date(), date(month, day));
            assert_eq!(session.market(), &Market::Us);
            assert_eq!(
                session.state_at(at(month, day, hour, 29, 59)),
                MarketState::BeforeOpen
            );
            assert_eq!(session.state_at(start), MarketState::RegularMarket);
            assert_eq!(
                session.state_at(at(month, next, 0, 0, 0)),
                MarketState::RegularMarket
            );
            assert_eq!(
                session.state_at(at(
                    month,
                    next,
                    close.checked_sub(1).expect("직전 시"),
                    59,
                    59
                )),
                MarketState::RegularMarket
            );
            assert_eq!(session.state_at(end), MarketState::Ended);
        }
    }

    #[test]
    fn known_holidays_and_dst_sundays_are_closed() {
        for (market, day, information, instant) in [
            (
                Market::Kr,
                date(Month::May, 5),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: None,
                    after_market: None,
                },
                at(Month::May, 5, 9, 0, 0),
            ),
            (
                Market::Us,
                date(Month::July, 3),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: None,
                    after_market: None,
                },
                at(Month::July, 3, 22, 30, 0),
            ),
            (
                Market::Us,
                date(Month::March, 8),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: None,
                    after_market: None,
                },
                at(Month::March, 8, 22, 30, 0),
            ),
            (
                Market::Us,
                date(Month::November, 1),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: None,
                    after_market: None,
                },
                at(Month::November, 1, 23, 30, 0),
            ),
        ] {
            assert_eq!(
                MarketSession::try_new(market, day, information)
                    .expect("휴장 응답")
                    .state_at(instant),
                MarketState::Holiday
            );
        }
    }

    #[test]
    fn missing_information_and_partial_closures_are_distinct() {
        let missing = MarketSession::try_new(
            Market::Us,
            date(Month::March, 9),
            SessionInformation::Unavailable,
        )
        .expect("정보 없음");
        assert_eq!(
            missing.state_at(at(Month::March, 9, 23, 0, 0)),
            MarketState::Unknown
        );
        let regular = interval(
            at(Month::March, 25, 9, 0, 0),
            at(Month::March, 25, 15, 30, 0),
        );
        let partial = MarketSession::try_new(
            Market::Kr,
            date(Month::March, 25),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(regular),
                after_market: None,
            },
        )
        .expect("부분 휴장");
        assert_eq!(
            partial.state_at(regular.start()),
            MarketState::RegularMarket
        );
        assert_eq!(partial.state_at(regular.end()), MarketState::Ended);
    }

    #[test]
    fn korean_session_boundaries_are_start_inclusive_end_exclusive() {
        let day = date(Month::March, 25);
        let kr = MarketSession::try_new(
            Market::Kr,
            day,
            SessionInformation::Kr {
                pre_market: Some(interval(
                    at(Month::March, 25, 8, 0, 0),
                    at(Month::March, 25, 9, 0, 0),
                )),
                regular_market: Some(interval(
                    at(Month::March, 25, 9, 0, 0),
                    at(Month::March, 25, 15, 30, 0),
                )),
                after_market: Some(interval(
                    at(Month::March, 25, 15, 30, 0),
                    at(Month::March, 25, 20, 0, 0),
                )),
            },
        )
        .expect("국내 통합 세션");
        for (hour, minute, expected) in [
            (7, 59, MarketState::BeforeOpen),
            (8, 0, MarketState::PreMarket),
            (9, 0, MarketState::RegularMarket),
            (15, 30, MarketState::AfterMarket),
            (20, 0, MarketState::Ended),
        ] {
            assert_eq!(kr.state_at(at(Month::March, 25, hour, minute, 0)), expected);
        }
    }

    #[test]
    fn us_session_boundaries_and_gap_are_start_inclusive_end_exclusive() {
        let day = date(Month::March, 25);
        let us = MarketSession::try_new(
            Market::Us,
            day,
            SessionInformation::Us {
                day_market: Some(interval(
                    at(Month::March, 25, 9, 0, 0),
                    at(Month::March, 25, 16, 50, 0),
                )),
                pre_market: Some(interval(
                    at(Month::March, 25, 17, 0, 0),
                    at(Month::March, 25, 22, 30, 0),
                )),
                regular_market: Some(interval(
                    at(Month::March, 25, 22, 30, 0),
                    at(Month::March, 26, 5, 0, 0),
                )),
                after_market: Some(interval(
                    at(Month::March, 26, 5, 0, 0),
                    at(Month::March, 26, 7, 0, 0),
                )),
            },
        )
        .expect("미국 네 세션");
        for (day_number, hour, minute, expected) in [
            (25, 9, 0, MarketState::DayMarket),
            (25, 16, 50, MarketState::BetweenSessions),
            (25, 17, 0, MarketState::PreMarket),
            (25, 22, 30, MarketState::RegularMarket),
            (26, 5, 0, MarketState::AfterMarket),
            (26, 7, 0, MarketState::Ended),
        ] {
            assert_eq!(
                us.state_at(at(Month::March, day_number, hour, minute, 0)),
                expected
            );
        }
    }

    #[test]
    fn early_close_uses_the_supplied_end_instead_of_a_normal_day_constant() {
        let session = us(
            interval(
                at(Month::November, 27, 23, 30, 0),
                at(Month::November, 28, 3, 0, 0),
            ),
            date(Month::November, 27),
        );
        assert_eq!(
            session.state_at(at(Month::November, 28, 2, 59, 59)),
            MarketState::RegularMarket
        );
        assert_eq!(
            session.state_at(at(Month::November, 28, 3, 0, 0)),
            MarketState::Ended
        );
        assert_eq!(
            session.state_at(at(Month::November, 28, 5, 0, 0)),
            MarketState::Ended
        );
    }

    #[test]
    fn malformed_intervals_overlaps_and_market_mismatches_are_rejected() {
        let start = at(Month::March, 25, 9, 0, 0);
        let end = at(Month::March, 25, 15, 30, 0);
        assert_eq!(
            TimeInterval::try_new(end, start),
            Err(TimeIntervalError::NonIncreasing)
        );
        assert_eq!(
            TimeInterval::try_new(start, start),
            Err(TimeIntervalError::NonIncreasing)
        );
        let hours = SessionInformation::Us {
            day_market: None,
            pre_market: None,
            regular_market: Some(interval(start, end)),
            after_market: None,
        };
        assert_eq!(
            MarketSession::try_new(Market::Kr, date(Month::March, 25), hours),
            Err(MarketSessionError::MarketMismatch)
        );
        let unknown = Market::new(MarketCode::try_new("JP").expect("외부 시장"));
        assert_eq!(
            MarketSession::try_new(
                unknown,
                date(Month::March, 25),
                SessionInformation::Unavailable
            ),
            Err(MarketSessionError::UnsupportedMarket)
        );
        for pre in [
            interval(start, end),
            interval(end, at(Month::March, 25, 20, 0, 0)),
        ] {
            assert_eq!(
                MarketSession::try_new(
                    Market::Kr,
                    date(Month::March, 25),
                    SessionInformation::Kr {
                        pre_market: Some(pre),
                        regular_market: Some(interval(start, end)),
                        after_market: None
                    }
                ),
                Err(MarketSessionError::OverlappingOrUnordered)
            );
        }
        assert_eq!(
            MarketSession::try_new(
                Market::Us,
                date(Month::March, 25),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: None,
                    after_market: None
                }
            ),
            Err(MarketSessionError::MarketMismatch)
        );
    }

    #[test]
    fn arbitrary_instants_are_deterministic_and_outside_is_never_regular() {
        TestRunner::new(Config::with_cases(4096))
            .run(
                &(0_i64..200_000, 1_i64..100_000, -100_000_i64..400_000),
                |(start, length, point)| {
                    let end = start.checked_add(length).expect("범위 내 합");
                    let timestamp = |value| {
                        Timestamp::new(
                            time::OffsetDateTime::from_unix_timestamp(value).expect("범위 내 초"),
                        )
                    };
                    let session = us(
                        interval(timestamp(start), timestamp(end)),
                        date(Month::January, 1),
                    );
                    let actual = session.state_at(timestamp(point));
                    proptest::prop_assert_eq!(actual, session.state_at(timestamp(point)));
                    if point < start || point >= end {
                        proptest::prop_assert_ne!(actual, MarketState::RegularMarket);
                    } else {
                        proptest::prop_assert_eq!(actual, MarketState::RegularMarket);
                    }
                    Ok(())
                },
            )
            .expect("순수 함수와 구간 속성");
    }
}
