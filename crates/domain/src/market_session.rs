use crate::{CalendarDate, Market, OffsetSeconds, TimeInterval, TimeSpan, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketState {
    Unknown,
    Closed,
    DayMarket,
    PreMarket,
    RegularMarket,
    AfterMarket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuctionBoundary {
    StartsAt(Timestamp),
    EndsAt(Timestamp),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionData {
    interval: TimeInterval,
    auction: Option<AuctionBoundary>,
}
impl SessionData {
    pub fn try_new(
        interval: TimeInterval,
        auction: Option<AuctionBoundary>,
    ) -> Result<Self, MarketSessionError> {
        let point = match auction {
            Some(AuctionBoundary::StartsAt(value) | AuctionBoundary::EndsAt(value)) => Some(value),
            None => None,
        };
        if point.is_some_and(|value| value < interval.start() || value > interval.end()) {
            return Err(MarketSessionError::AuctionOutsideSession);
        }
        Ok(Self { interval, auction })
    }
    #[must_use]
    pub const fn interval(self) -> TimeInterval {
        self.interval
    }
    #[must_use]
    pub const fn auction(self) -> Option<AuctionBoundary> {
        self.auction
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionInformation {
    Kr {
        pre_market: Option<SessionData>,
        regular_market: Option<SessionData>,
        after_market: Option<SessionData>,
    },
    Us {
        day_market: Option<SessionData>,
        pre_market: Option<SessionData>,
        regular_market: Option<SessionData>,
        after_market: Option<SessionData>,
    },
}
impl SessionInformation {
    const fn market(self) -> Market {
        match self {
            Self::Kr { .. } => Market::Kr,
            Self::Us { .. } => Market::Us,
        }
    }
    fn segments(self) -> Vec<MarketSegment> {
        let (day, pre, regular, after) = match self {
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
        .into_iter()
        .filter_map(|(state, session)| {
            session.map(|value| MarketSegment {
                state,
                session: value,
            })
        })
        .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketSegment {
    state: MarketState,
    session: SessionData,
}
impl MarketSegment {
    #[must_use]
    pub const fn state(self) -> MarketState {
        self.state
    }
    #[must_use]
    pub const fn session(self) -> SessionData {
        self.session
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketSession {
    date: CalendarDate,
    information: SessionInformation,
    sessions: Vec<MarketSegment>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketSessionError {
    OverlappingSessions,
    SessionTooLong,
    DateMismatch,
    AuctionOutsideSession,
    UnexpectedAuctionBoundary,
}
impl MarketSession {
    pub fn try_new(
        date: CalendarDate,
        information: SessionInformation,
    ) -> Result<Self, MarketSessionError> {
        let mut sessions = information.segments();
        sessions.sort_by_key(|entry| entry.session.interval.start());
        let mut previous_end = None;
        for entry in &sessions {
            if previous_end.is_some_and(|end| end > entry.session.interval.start()) {
                return Err(MarketSessionError::OverlappingSessions);
            }
            Self::validate_segment(date, information, *entry)?;
            previous_end = Some(entry.session.interval.end());
        }
        Ok(Self {
            date,
            information,
            sessions,
        })
    }
    fn validate_segment(
        date: CalendarDate,
        information: SessionInformation,
        entry: MarketSegment,
    ) -> Result<(), MarketSessionError> {
        let interval = entry.session.interval;
        if !interval
            .start()
            .elapsed_until(interval.end())
            .is_ok_and(|length| length <= TimeSpan::DAY)
        {
            return Err(MarketSessionError::SessionTooLong);
        }
        let start_date = interval
            .start()
            .date_at_offset(OffsetSeconds::new(32400))
            .or(Err(MarketSessionError::DateMismatch))?;
        match information {
            SessionInformation::Kr { .. } => {
                if start_date != date {
                    return Err(MarketSessionError::DateMismatch);
                }
                Self::validate_auction(entry)
            }
            SessionInformation::Us { .. } => {
                Self::validate_us_date(date, start_date)?;
                let end_date = interval
                    .end()
                    .date_at_offset(OffsetSeconds::new(32400))
                    .or(Err(MarketSessionError::DateMismatch))?;
                Self::validate_us_date(date, end_date)?;
                if entry.session.auction.is_some() {
                    return Err(MarketSessionError::UnexpectedAuctionBoundary);
                }
                Ok(())
            }
        }
    }
    fn validate_us_date(
        date: CalendarDate,
        candidate: CalendarDate,
    ) -> Result<(), MarketSessionError> {
        date.days_until(candidate)
            .ok()
            .filter(|distance| (-1..=2).contains(distance.as_i64()))
            .map(|_distance| ())
            .ok_or(MarketSessionError::DateMismatch)
    }
    fn validate_auction(entry: MarketSegment) -> Result<(), MarketSessionError> {
        match (entry.state, entry.session.auction) {
            (_, None)
            | (
                MarketState::PreMarket | MarketState::RegularMarket,
                Some(AuctionBoundary::StartsAt(_)),
            )
            | (MarketState::AfterMarket, Some(AuctionBoundary::EndsAt(_))) => Ok(()),
            (MarketState::Unknown | MarketState::Closed | MarketState::DayMarket, Some(_))
            | (
                MarketState::PreMarket | MarketState::RegularMarket,
                Some(AuctionBoundary::EndsAt(_)),
            )
            | (MarketState::AfterMarket, Some(AuctionBoundary::StartsAt(_))) => {
                Err(MarketSessionError::UnexpectedAuctionBoundary)
            }
        }
    }
    #[must_use]
    pub const fn market(&self) -> Market {
        self.information.market()
    }
    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.date
    }
    #[must_use]
    pub fn sessions(&self) -> &[MarketSegment] {
        &self.sessions
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketCalendar {
    rows: [MarketSession; 3],
    sessions: Vec<MarketSegment>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketCalendarError {
    DateOrder,
    MarketMismatch,
    OverlappingSessions,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketTransition {
    at: Timestamp,
    state: MarketState,
}
impl MarketTransition {
    #[must_use]
    pub const fn at(self) -> Timestamp {
        self.at
    }
    #[must_use]
    pub const fn state(self) -> MarketState {
        self.state
    }
}
impl MarketCalendar {
    pub fn try_new(
        previous_business_day: MarketSession,
        today: MarketSession,
        next_business_day: MarketSession,
    ) -> Result<Self, MarketCalendarError> {
        if previous_business_day.date >= today.date || today.date >= next_business_day.date {
            return Err(MarketCalendarError::DateOrder);
        }
        if previous_business_day.market() != today.market()
            || today.market() != next_business_day.market()
        {
            return Err(MarketCalendarError::MarketMismatch);
        }
        let rows = [previous_business_day, today, next_business_day];
        let mut sessions: Vec<MarketSegment> = rows
            .iter()
            .flat_map(|row| row.sessions.iter().copied())
            .collect();
        sessions.sort_by_key(|entry| entry.session.interval.start());
        let mut previous_end = None;
        for entry in &sessions {
            if previous_end.is_some_and(|end| end > entry.session.interval.start()) {
                return Err(MarketCalendarError::OverlappingSessions);
            }
            previous_end = Some(entry.session.interval.end());
        }
        Ok(Self { rows, sessions })
    }
    #[must_use]
    pub fn previous_business_day(&self) -> &MarketSession {
        let [previous, _, _] = &self.rows;
        previous
    }
    #[must_use]
    pub fn today(&self) -> &MarketSession {
        let [_, today, _] = &self.rows;
        today
    }
    #[must_use]
    pub fn next_business_day(&self) -> &MarketSession {
        let [_, _, next] = &self.rows;
        next
    }
    #[must_use]
    pub fn sessions(&self) -> &[MarketSegment] {
        &self.sessions
    }
    #[must_use]
    pub fn state_at(&self, instant: Timestamp) -> MarketState {
        let Some((first, last)) = self.sessions.first().zip(self.sessions.last()) else {
            return MarketState::Unknown;
        };
        if instant < first.session.interval.start() || instant >= last.session.interval.end() {
            return MarketState::Unknown;
        }
        self.sessions
            .iter()
            .find(|entry| {
                instant >= entry.session.interval.start() && instant < entry.session.interval.end()
            })
            .map_or(MarketState::Closed, |entry| entry.state)
    }
    #[must_use]
    pub fn next_transition(&self, after: Timestamp) -> Option<MarketTransition> {
        let mut boundaries: Vec<Timestamp> = self
            .sessions
            .iter()
            .flat_map(|entry| [entry.session.interval.start(), entry.session.interval.end()])
            .collect();
        boundaries.sort_unstable();
        boundaries.dedup();
        let nano = TimeSpan::NANOSECOND;
        for at in boundaries.into_iter().filter(|at| *at > after) {
            let before = at
                .checked_sub(nano)
                .map_or(MarketState::Unknown, |instant| self.state_at(instant));
            let state = self.state_at(at);
            if before != state {
                return Some(MarketTransition { at, state });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuctionBoundary, MarketCalendar, MarketCalendarError, MarketSession, MarketSessionError,
        MarketState, SessionData, SessionInformation,
    };
    use crate::{
        CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond, OffsetSeconds, SpanSeconds,
        TimeInterval, TimeSpan, Timestamp, UnixSeconds,
    };
    use proptest::test_runner::{Config, TestRunner};
    use time::{Date, Month, UtcOffset};

    fn date(month: u8, day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(month),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn at(month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
        let raw = Date::from_calendar_date(2026, Month::try_from(month).expect("월"), day)
            .expect("날짜")
            .with_hms(hour, minute, 0)
            .expect("시각")
            .assume_offset(UtcOffset::from_hms(9, 0, 0).expect("KST"));
        Timestamp::from_parts(
            UnixSeconds::new(raw.unix_timestamp()),
            Nanosecond::new(0),
            OffsetSeconds::new(32400),
        )
        .expect("시점")
    }
    fn span(seconds: u64, nanos: u32) -> TimeSpan {
        TimeSpan::try_new(SpanSeconds::new(seconds), Nanosecond::new(nanos)).expect("기간")
    }
    fn session(start: Timestamp, end: Timestamp) -> SessionData {
        SessionData::try_new(TimeInterval::try_new(start, end).expect("구간"), None).expect("세션")
    }
    fn us_row(month: u8, day: u8, next: u8, open_hour: u8, close_hour: u8) -> MarketSession {
        MarketSession::try_new(
            date(month, day),
            SessionInformation::Us {
                day_market: Some(session(at(month, day, 9, 0), at(month, day, 16, 50))),
                pre_market: Some(session(
                    at(month, day, 17, 0),
                    at(month, day, open_hour, 30),
                )),
                regular_market: Some(session(
                    at(month, day, open_hour, 30),
                    at(month, next, close_hour, 0),
                )),
                after_market: Some(session(
                    at(month, next, close_hour, 0),
                    at(month, next, 7, 0),
                )),
            },
        )
        .expect("미국 행")
    }
    fn holiday(month: u8, day: u8) -> MarketSession {
        MarketSession::try_new(
            date(month, day),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
        )
        .expect("휴장 행")
    }
    fn calendar() -> MarketCalendar {
        MarketCalendar::try_new(
            us_row(3, 24, 25, 22, 5),
            us_row(3, 25, 26, 22, 5),
            us_row(3, 26, 27, 22, 5),
        )
        .expect("세 행")
    }

    #[test]
    fn each_date_and_market_adjacency_is_validated_independently() {
        for (previous, today, next) in [(24, 24, 26), (24, 26, 26), (25, 24, 26), (24, 26, 25)] {
            assert_eq!(
                MarketCalendar::try_new(holiday(3, previous), holiday(3, today), holiday(3, next)),
                Err(MarketCalendarError::DateOrder)
            );
        }
        let kr = |day| {
            MarketSession::try_new(
                date(3, day),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: None,
                    after_market: None,
                },
            )
            .expect("국내 행")
        };
        for (previous, today, next) in [
            (kr(24), holiday(3, 25), holiday(3, 26)),
            (holiday(3, 24), holiday(3, 25), kr(26)),
        ] {
            assert_eq!(
                MarketCalendar::try_new(previous, today, next),
                Err(MarketCalendarError::MarketMismatch)
            );
        }
    }

    #[test]
    fn auction_markers_allow_both_interval_endpoints() {
        let start = at(3, 25, 9, 0);
        let end = at(3, 25, 15, 30);
        let interval = TimeInterval::try_new(start, end).expect("정규장");
        for boundary in [
            AuctionBoundary::StartsAt(start),
            AuctionBoundary::StartsAt(end),
            AuctionBoundary::EndsAt(start),
            AuctionBoundary::EndsAt(end),
        ] {
            assert_eq!(
                SessionData::try_new(interval, Some(boundary))
                    .expect("구간 경계 표식")
                    .auction(),
                Some(boundary)
            );
        }
    }

    #[test]
    fn calendar_selects_by_actual_sessions_including_us_local_previous_evening() {
        let calendar = calendar();
        assert_eq!(calendar.state_at(at(3, 25, 10, 0)), MarketState::DayMarket);
        assert_eq!(
            calendar.state_at(at(3, 25, 4, 0)),
            MarketState::RegularMarket
        );
        assert_eq!(
            calendar.state_at(at(3, 25, 23, 0)),
            MarketState::RegularMarket
        );
        let kst = at(3, 25, 10, 0);
        let eastern = Timestamp::from_parts(
            kst.utc_seconds(),
            kst.nanosecond(),
            OffsetSeconds::new(-14400),
        )
        .expect("미국 오프셋");
        assert_eq!(eastern.offset_calendar_date(), date(3, 24));
        assert_eq!(calendar.state_at(eastern), MarketState::DayMarket);
        assert_eq!(calendar.previous_business_day().date(), date(3, 24));
        assert_eq!(calendar.today().date(), date(3, 25));
        assert_eq!(calendar.next_business_day().date(), date(3, 26));
        assert_eq!(calendar.sessions().len(), 12);
        assert_eq!(calendar.today().sessions().len(), 4);
        assert_eq!(
            calendar.today().sessions().first().expect("데이").state(),
            MarketState::DayMarket
        );
    }

    #[test]
    fn stale_rows_holidays_and_missing_bounds_never_extrapolate_a_closed_state() {
        let old = MarketCalendar::try_new(
            us_row(3, 18, 19, 22, 5),
            us_row(3, 19, 20, 22, 5),
            us_row(3, 20, 21, 22, 5),
        )
        .expect("낡은 응답");
        assert_eq!(old.state_at(at(3, 25, 23, 0)), MarketState::Unknown);
        let current = calendar();
        assert_eq!(current.state_at(at(3, 24, 8, 59)), MarketState::Unknown);
        assert_eq!(current.state_at(at(3, 27, 7, 0)), MarketState::Unknown);
        assert_eq!(current.state_at(at(3, 25, 16, 55)), MarketState::Closed);
        let holidays =
            MarketCalendar::try_new(holiday(7, 2), holiday(7, 3), holiday(7, 6)).expect("빈 세 행");
        assert_eq!(holidays.state_at(at(7, 3, 10, 0)), MarketState::Unknown);
        assert_eq!(holidays.next_transition(at(7, 3, 10, 0)), None);
        let bounded = MarketCalendar::try_new(
            us_row(7, 2, 3, 22, 5),
            holiday(7, 3),
            us_row(7, 6, 7, 22, 5),
        )
        .expect("주말과 휴장");
        assert_eq!(bounded.state_at(at(7, 3, 23, 0)), MarketState::Closed);
        assert_eq!(bounded.state_at(at(7, 5, 12, 0)), MarketState::Closed);
        assert_eq!(bounded.state_at(at(8, 3, 12, 0)), MarketState::Unknown);
    }

    #[test]
    fn dst_friday_monday_boundaries_and_sunday_gaps_use_literal_calendar_facts() {
        for (
            before_month,
            before_day,
            before_next,
            month,
            monday,
            tuesday,
            wednesday,
            old_open,
            old_close,
            new_open,
            new_close,
            sunday,
        ) in [
            (3, 6, 7, 3, 9, 10, 11, 23, 6, 22, 5, 8),
            (10, 30, 31, 11, 2, 3, 4, 22, 5, 23, 6, 1),
        ] {
            let calendar = MarketCalendar::try_new(
                us_row(before_month, before_day, before_next, old_open, old_close),
                us_row(month, monday, tuesday, new_open, new_close),
                us_row(month, tuesday, wednesday, new_open, new_close),
            )
            .expect("DST 전환 주");
            for (session_month, day, next, open, close) in [
                (before_month, before_day, before_next, old_open, old_close),
                (month, monday, tuesday, new_open, new_close),
            ] {
                let start = at(session_month, day, open, 30);
                let end = at(session_month, next, close, 0);
                assert_eq!(
                    calendar.state_at(start.checked_sub(span(0, 1)).expect("직전")),
                    MarketState::PreMarket
                );
                assert_eq!(calendar.state_at(start), MarketState::RegularMarket);
                assert_eq!(
                    calendar.state_at(at(session_month, next, 0, 0)),
                    MarketState::RegularMarket
                );
                assert_eq!(
                    calendar.state_at(end.checked_sub(span(0, 1)).expect("직전")),
                    MarketState::RegularMarket
                );
                assert_eq!(calendar.state_at(end), MarketState::AfterMarket);
            }
            assert_eq!(
                calendar.state_at(at(month, sunday, 12, 0)),
                MarketState::Closed
            );
        }
    }

    #[test]
    fn half_day_and_touching_sessions_produce_real_next_transitions() {
        let row = |day, end_day, end_hour| {
            MarketSession::try_new(
                date(11, day),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: Some(session(
                        at(11, day, 23, 30),
                        at(11, end_day, end_hour, 0),
                    )),
                    after_market: None,
                },
            )
            .expect("정규장")
        };
        let calendar = MarketCalendar::try_new(row(23, 24, 6), row(25, 26, 6), row(27, 28, 3))
            .expect("반일장 응답");
        let open = calendar.next_transition(at(11, 27, 23, 29)).expect("개장");
        assert_eq!(open.at(), at(11, 27, 23, 30));
        assert_eq!(open.state(), MarketState::RegularMarket);
        let close = calendar.next_transition(open.at()).expect("조기 종료");
        assert_eq!(close.at(), at(11, 28, 3, 0));
        assert_eq!(
            calendar.state_at(close.at().checked_sub(span(0, 1)).expect("직전")),
            MarketState::RegularMarket
        );
        assert_eq!(close.state(), MarketState::Unknown);
        assert_eq!(calendar.next_transition(close.at()), None);
        let adjacent = super::tests::calendar()
            .next_transition(at(3, 25, 22, 30))
            .expect("인접 애프터");
        assert_eq!(adjacent.at(), at(3, 26, 5, 0));
        assert_eq!(adjacent.state(), MarketState::AfterMarket);
    }

    #[test]
    fn invalid_calendar_order_market_and_cross_row_overlap_are_rejected() {
        let same = us_row(3, 25, 26, 22, 5);
        assert_eq!(
            MarketCalendar::try_new(same.clone(), same.clone(), same),
            Err(MarketCalendarError::DateOrder)
        );
        let kr = MarketSession::try_new(
            date(3, 25),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
        )
        .expect("국내 휴장");
        assert_eq!(
            MarketCalendar::try_new(holiday(3, 24), kr, holiday(3, 26)),
            Err(MarketCalendarError::MarketMismatch)
        );
        let row = |day, start, end| {
            MarketSession::try_new(
                date(3, day),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: Some(session(at(3, 25, start, 0), at(3, 25, end, 0))),
                    after_market: None,
                },
            )
            .expect("보수적 날짜 범위")
        };
        assert_eq!(
            MarketCalendar::try_new(row(24, 10, 12), row(25, 11, 13), holiday(3, 26)),
            Err(MarketCalendarError::OverlappingSessions)
        );
    }

    #[test]
    fn rows_sort_actual_times_and_reject_bad_dates_lengths_and_overlaps() {
        let early = session(at(3, 25, 8, 0), at(3, 25, 9, 0));
        let late = session(at(3, 25, 10, 0), at(3, 25, 11, 0));
        let information = SessionInformation::Kr {
            pre_market: Some(late),
            regular_market: Some(early),
            after_market: None,
        };
        let row =
            MarketSession::try_new(date(3, 25), information).expect("필드 순서가 시각 순서는 아님");
        assert_eq!(
            row.sessions()
                .first()
                .expect("가장 이른 세션")
                .session()
                .interval(),
            early.interval()
        );
        assert_eq!(
            MarketSession::try_new(date(1, 1), information),
            Err(MarketSessionError::DateMismatch)
        );
        let overlapping = SessionInformation::Kr {
            pre_market: Some(early),
            regular_market: Some(early),
            after_market: None,
        };
        assert_eq!(
            MarketSession::try_new(date(3, 25), overlapping),
            Err(MarketSessionError::OverlappingSessions)
        );
        let huge = SessionInformation::Us {
            day_market: None,
            pre_market: None,
            regular_market: Some(session(at(3, 25, 9, 0), at(3, 30, 9, 0))),
            after_market: None,
        };
        assert_eq!(
            MarketSession::try_new(date(3, 25), huge),
            Err(MarketSessionError::SessionTooLong)
        );
        let wrong_us = SessionInformation::Us {
            day_market: None,
            pre_market: None,
            regular_market: Some(late),
            after_market: None,
        };
        assert_eq!(
            MarketSession::try_new(date(1, 1), wrong_us),
            Err(MarketSessionError::DateMismatch)
        );
    }

    #[test]
    fn auctions_are_preserved_validated_and_available_to_order_gates() {
        let regular = TimeInterval::try_new(at(3, 25, 9, 0), at(3, 25, 15, 30)).expect("정규장");
        let auction = AuctionBoundary::StartsAt(at(3, 25, 15, 20));
        let data = SessionData::try_new(regular, Some(auction)).expect("종가 단일가");
        assert_eq!(data.auction(), Some(auction));
        for point in [at(3, 25, 8, 59), at(3, 25, 15, 31)] {
            assert_eq!(
                SessionData::try_new(regular, Some(AuctionBoundary::StartsAt(point))),
                Err(MarketSessionError::AuctionOutsideSession)
            );
        }
        let pre = SessionData::try_new(
            TimeInterval::try_new(at(3, 25, 8, 0), at(3, 25, 9, 0)).expect("프리"),
            Some(AuctionBoundary::StartsAt(at(3, 25, 8, 50))),
        )
        .expect("시가 단일가");
        let after = SessionData::try_new(
            TimeInterval::try_new(at(3, 25, 15, 30), at(3, 25, 20, 0)).expect("애프터"),
            Some(AuctionBoundary::EndsAt(at(3, 25, 15, 40))),
        )
        .expect("시가 단일가 종료");
        let row = MarketSession::try_new(
            date(3, 25),
            SessionInformation::Kr {
                pre_market: Some(pre),
                regular_market: Some(data),
                after_market: Some(after),
            },
        )
        .expect("세 단일가 시각");
        assert_eq!(
            row.sessions().last().expect("애프터").session().auction(),
            Some(AuctionBoundary::EndsAt(at(3, 25, 15, 40)))
        );
        let wrong = SessionData::try_new(regular, Some(AuctionBoundary::EndsAt(at(3, 25, 15, 20))))
            .expect("구간 안");
        assert_eq!(
            MarketSession::try_new(
                date(3, 25),
                SessionInformation::Kr {
                    pre_market: None,
                    regular_market: Some(wrong),
                    after_market: None
                }
            ),
            Err(MarketSessionError::UnexpectedAuctionBoundary)
        );
        assert_eq!(
            MarketSession::try_new(
                date(3, 25),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: Some(data),
                    after_market: None
                }
            ),
            Err(MarketSessionError::UnexpectedAuctionBoundary)
        );
    }

    #[test]
    fn adjacent_equal_states_are_skipped_and_session_length_limit_is_exact() {
        let row = |day, start, end| {
            MarketSession::try_new(
                date(3, day),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: Some(session(start, end)),
                    after_market: None,
                },
            )
        };
        let calendar = MarketCalendar::try_new(
            row(24, at(3, 24, 9, 0), at(3, 25, 9, 0)).expect("정확히 24시간"),
            row(25, at(3, 25, 9, 0), at(3, 25, 10, 0)).expect("같은 상태의 인접 구간"),
            holiday(3, 26),
        )
        .expect("캘린더");
        let transition = calendar
            .next_transition(at(3, 24, 9, 0))
            .expect("실제 상태 전이");
        assert_eq!(transition.at(), at(3, 25, 10, 0));
        assert_eq!(transition.state(), MarketState::Unknown);
        assert_eq!(
            row(
                24,
                at(3, 24, 9, 0),
                at(3, 25, 9, 0)
                    .checked_add(span(0, 1))
                    .expect("24시간 초과")
            ),
            Err(MarketSessionError::SessionTooLong)
        );
    }

    #[test]
    fn us_date_window_checks_both_endpoints_and_kst_projection_limits() {
        let row = |start, end| {
            MarketSession::try_new(
                date(3, 25),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: Some(session(start, end)),
                    after_market: None,
                },
            )
        };
        row(at(3, 24, 10, 0), at(3, 24, 11, 0)).expect("전날 경계");
        row(at(3, 27, 10, 0), at(3, 27, 11, 0)).expect("이틀 뒤 경계");
        assert_eq!(
            row(at(3, 23, 10, 0), at(3, 23, 11, 0)),
            Err(MarketSessionError::DateMismatch)
        );
        assert_eq!(
            row(at(3, 27, 23, 0), at(3, 28, 1, 0)),
            Err(MarketSessionError::DateMismatch)
        );
        let last_date = CalendarDate::from_parts(
            CalendarYear::new(9999),
            MonthNumber::new(12),
            DayNumber::new(31),
        )
        .expect("마지막 날짜");
        let stamp = |seconds| {
            Timestamp::from_parts(
                UnixSeconds::new(seconds),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("상한 근처 UTC")
        };
        for (start, end) in [
            (253_402_293_600, 253_402_297_200),
            (253_402_264_800, 253_402_272_000),
        ] {
            let information = SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(session(stamp(start), stamp(end))),
                after_market: None,
            };
            assert_eq!(
                MarketSession::try_new(last_date, information),
                Err(MarketSessionError::DateMismatch)
            );
        }
    }

    fn raw_nanos(seconds: i64, nanos: u32) -> i128 {
        i128::from(seconds)
            .checked_mul(1_000_000_000)
            .and_then(|value| value.checked_add(i128::from(nanos)))
            .expect("유한 시각")
    }
    fn wrap(seconds: i64, nanos: u32, offset_minutes: i32) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(nanos),
            OffsetSeconds::new(offset_minutes.checked_mul(60).expect("오프셋 초")),
        )
        .expect("유효 시각")
    }
    fn generated(
        lengths: &[(i64, i64)],
        nanos: u32,
        offset: i32,
    ) -> (MarketCalendar, Vec<(i128, i128, MarketState)>) {
        let mut rows = Vec::new();
        let mut reference = Vec::new();
        for (day, chunk) in [24_u8, 25, 26].into_iter().zip(lengths.as_chunks::<4>().0) {
            let delta = i64::from(day)
                .checked_sub(24)
                .and_then(|value| value.checked_mul(86400))
                .expect("날짜 간격");
            let mut cursor = 1_774_310_400_i64
                .checked_add(delta)
                .expect("2026-03-24 00:00 UTC");
            let mut sessions = Vec::new();
            for ((length, gap), state) in chunk.iter().zip([
                MarketState::DayMarket,
                MarketState::PreMarket,
                MarketState::RegularMarket,
                MarketState::AfterMarket,
            ]) {
                let end = cursor.checked_add(*length).expect("종료");
                sessions.push(session(
                    wrap(cursor, nanos, offset),
                    wrap(end, nanos, offset),
                ));
                reference.push((raw_nanos(cursor, nanos), raw_nanos(end, nanos), state));
                cursor = end.checked_add(*gap).expect("공백");
            }
            let [day_market, pre_market, regular_market, after_market]: [SessionData; 4] =
                sessions.try_into().expect("네 세션");
            rows.push(
                MarketSession::try_new(
                    date(3, day),
                    SessionInformation::Us {
                        day_market: Some(day_market),
                        pre_market: Some(pre_market),
                        regular_market: Some(regular_market),
                        after_market: Some(after_market),
                    },
                )
                .expect("생성 행"),
            );
        }
        let [previous, today, next]: [MarketSession; 3] = rows.try_into().expect("세 행");
        (
            MarketCalendar::try_new(previous, today, next).expect("생성 캘린더"),
            reference,
        )
    }
    fn reference_state(entries: &[(i128, i128, MarketState)], point: i128) -> MarketState {
        let first = entries.first().expect("첫 구간").0;
        let last = entries.last().expect("끝 구간").1;
        if point < first || point >= last {
            return MarketState::Unknown;
        }
        for (start, end, state) in entries {
            if *start <= point && point < *end {
                return *state;
            }
        }
        MarketState::Closed
    }

    fn assert_reference_boundaries(
        calendar: &MarketCalendar,
        reference: &[(i128, i128, MarketState)],
        offset: i32,
    ) {
        for (start, end, _) in reference {
            for point in [
                start.checked_sub(1).expect("직전"),
                *start,
                end.checked_sub(1).expect("직전"),
                *end,
            ] {
                let seconds = i64::try_from(point.div_euclid(1_000_000_000)).expect("초");
                let fraction = u32::try_from(point.rem_euclid(1_000_000_000)).expect("나노초");
                assert_eq!(
                    calendar.state_at(wrap(seconds, fraction, offset)),
                    reference_state(reference, point)
                );
            }
        }
    }

    #[test]
    fn generated_offsets_nanoseconds_sessions_and_gaps_match_independent_reference() {
        let strategy = (
            proptest::collection::vec((1_i64..3600, 0_i64..600), 12),
            0_u32..1_000_000_000,
            -720_i32..841_i32,
            -3600_i64..259_200_i64,
        );
        TestRunner::new(Config::with_cases(4096))
            .run(&strategy, |(lengths, nanos, offset, point)| {
                let (calendar, reference) = generated(&lengths, nanos, offset);
                let seconds = 1_774_310_400_i64.checked_add(point).expect("표본 시각");
                proptest::prop_assert_eq!(
                    calendar.state_at(wrap(seconds, nanos, offset)),
                    reference_state(&reference, raw_nanos(seconds, nanos))
                );
                assert_reference_boundaries(&calendar, &reference, offset);
                Ok(())
            })
            .expect("독립 기준 대조");
    }

    #[test]
    fn generated_transitions_change_state_and_traverse_every_session_boundary() {
        let strategy = (
            proptest::collection::vec((1_i64..3600, 0_i64..600), 12),
            0_u32..1_000_000_000,
            -720_i32..841_i32,
        );
        TestRunner::new(Config::with_cases(4096))
            .run(&strategy, |(lengths, nanos, offset)| {
                let (calendar, reference) = generated(&lengths, nanos, offset);
                let mut expected: Vec<i128> = reference
                    .iter()
                    .flat_map(|(start, end, _)| [*start, *end])
                    .collect();
                expected.sort_unstable();
                expected.dedup();
                let mut after = wrap(1_774_306_800, nanos, offset);
                let mut observed = Vec::new();
                while let Some(transition) = calendar.next_transition(after) {
                    proptest::prop_assert!(transition.at() > after);
                    let before = transition.at().checked_sub(span(0, 1)).expect("전이 직전");
                    proptest::prop_assert_ne!(
                        calendar.state_at(before),
                        calendar.state_at(transition.at())
                    );
                    proptest::prop_assert_eq!(
                        calendar.state_at(transition.at()),
                        transition.state()
                    );
                    observed.push(raw_nanos(
                        *transition.at().utc_seconds().as_i64(),
                        *transition.at().nanosecond().as_u32(),
                    ));
                    after = transition.at();
                    proptest::prop_assert!(observed.len() <= expected.len());
                }
                proptest::prop_assert_eq!(observed, expected);
                Ok(())
            })
            .expect("전이 경계 완주");
    }
}
