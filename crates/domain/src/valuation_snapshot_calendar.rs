use crate::{
    CalendarDate, Currency, FxRate, Market, MarketCalendar, MarketSession, MarketState,
    OffsetSeconds, Timestamp,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    CalendarMismatch,
    ClosedDay,
    MissingPrice,
    InvalidCandle,
    MissingFx,
    InvalidFx,
    MissingCash,
    IncompleteValuation,
    MissingPrevious,
    WrongPeriod,
    MissingFlows,
    DuplicateInstrument,
    TooManyInputs,
    PendingCorporateCash,
    MissingHistory,
    OutsideConfirmedPeriod,
    WrongCashBasis,
    MissingExecutionDate,
    MissingReportedValue,
    MissingFlowTime,
    UnsupportedStockTime,
    Lot(crate::LotError),
    Cash(crate::CashUnavailable),
    Arithmetic(crate::MoneyError),
    Conversion(crate::FxConversionError),
    Time(crate::TimeError),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotSource {
    Observed,
    Recalculated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotPriceBasis {
    CurrentSession,
    HolidayCarry,
    AwaitingSession,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotObservation {
    valuation_at: Timestamp,
    recorded_at: Timestamp,
    fx: Option<FxRate>,
}
impl SnapshotObservation {
    #[must_use]
    pub const fn new(valuation_at: Timestamp, recorded_at: Timestamp, fx: Option<FxRate>) -> Self {
        Self {
            valuation_at,
            recorded_at,
            fx,
        }
    }
    #[must_use]
    pub const fn valuation_at(self) -> Timestamp {
        self.valuation_at
    }
    #[must_use]
    pub const fn recorded_at(self) -> Timestamp {
        self.recorded_at
    }
    #[must_use]
    pub const fn fx(self) -> Option<FxRate> {
        self.fx
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotDates {
    krw: CalendarDate,
    usd: CalendarDate,
}
impl SnapshotDates {
    pub(crate) const fn new(krw: CalendarDate, usd: CalendarDate) -> Self {
        Self { krw, usd }
    }
    #[must_use]
    pub const fn date(self, currency: Currency) -> CalendarDate {
        match currency {
            Currency::Krw => self.krw,
            Currency::Usd => self.usd,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RegularClose {
    Scheduled(Timestamp),
    Holiday,
    Missing,
}
impl RegularClose {
    fn from_session(row: &MarketSession) -> Self {
        match row
            .sessions()
            .iter()
            .find(|segment| segment.state() == MarketState::RegularMarket)
        {
            Some(segment) => Self::Scheduled(segment.session().interval().end()),
            None if row.sessions().is_empty() => Self::Holiday,
            None => Self::Missing,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ObservationSessionScope {
    NoExtended,
    HasExtended,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MarketDay {
    date: CalendarDate,
    previous: CalendarDate,
    close: RegularClose,
    previous_close: RegularClose,
    observation_sessions: ObservationSessionScope,
}
impl MarketDay {
    fn from_calendar(calendar: &MarketCalendar) -> Self {
        Self {
            date: calendar.today().date(),
            previous: calendar.previous_business_day().date(),
            close: RegularClose::from_session(calendar.today()),
            previous_close: RegularClose::from_session(calendar.previous_business_day()),
            observation_sessions: match calendar
                .today()
                .sessions()
                .iter()
                .find(|segment| segment.state() != MarketState::RegularMarket)
            {
                Some(_) => ObservationSessionScope::HasExtended,
                None => ObservationSessionScope::NoExtended,
            },
        }
    }
    fn price(self, at: Timestamp) -> Result<(CalendarDate, SnapshotPriceBasis), SnapshotError> {
        let basis = match self.close {
            RegularClose::Scheduled(close) if close <= at => {
                return Ok((self.date, SnapshotPriceBasis::CurrentSession));
            }
            RegularClose::Scheduled(_) => SnapshotPriceBasis::AwaitingSession,
            RegularClose::Holiday => SnapshotPriceBasis::HolidayCarry,
            RegularClose::Missing => return Err(SnapshotError::MissingPrice),
        };
        match self.previous_close {
            RegularClose::Scheduled(close) if close <= at => Ok((self.previous, basis)),
            RegularClose::Scheduled(_) | RegularClose::Holiday | RegularClose::Missing => {
                Err(SnapshotError::MissingPrice)
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotCalendar {
    kr: MarketDay,
    us: MarketDay,
}
impl SnapshotCalendar {
    pub fn new(
        kr_calendar: &MarketCalendar,
        us_calendar: &MarketCalendar,
    ) -> Result<Self, SnapshotError> {
        if kr_calendar.today().market() != Market::Kr || us_calendar.today().market() != Market::Us
        {
            return Err(SnapshotError::CalendarMismatch);
        }
        let kr = MarketDay::from_calendar(kr_calendar);
        let us = MarketDay::from_calendar(us_calendar);
        if kr.close == RegularClose::Holiday && us.close == RegularClose::Holiday {
            return Err(SnapshotError::ClosedDay);
        }
        Ok(Self { kr, us })
    }
    #[must_use]
    pub const fn dates(self) -> SnapshotDates {
        SnapshotDates::new(self.kr.date, self.us.date)
    }
    pub fn price_date(
        self,
        currency: Currency,
        at: Timestamp,
    ) -> Result<CalendarDate, SnapshotError> {
        self.price(currency, at).map(|(day, _basis)| day)
    }
    pub(crate) fn price(
        self,
        currency: Currency,
        at: Timestamp,
    ) -> Result<(CalendarDate, SnapshotPriceBasis), SnapshotError> {
        match currency {
            Currency::Krw => self.kr,
            Currency::Usd => self.us,
        }
        .price(at)
    }
    pub(crate) const fn observation_sessions(self, currency: Currency) -> ObservationSessionScope {
        match currency {
            Currency::Krw => self.kr.observation_sessions,
            Currency::Usd => self.us.observation_sessions,
        }
    }
    pub(crate) fn settlement_date(self, at: Timestamp) -> Result<CalendarDate, SnapshotError> {
        let date = kst_date(at)?;
        if date != self.kr.date {
            return Err(SnapshotError::WrongPeriod);
        }
        Ok(date)
    }
}
pub(crate) fn kst_date(at: Timestamp) -> Result<CalendarDate, SnapshotError> {
    at.date_at_offset(OffsetSeconds::new(32_400))
        .map_err(SnapshotError::Time)
}

#[cfg(test)]
mod regular_close_tests {
    use crate::valuation_snapshot::tests::{at, date, money};
    use crate::{
        Currency, MarketCalendar, MarketSession, MarketState, Nanosecond, OffsetSeconds, Price,
        SessionData, SessionInformation, SnapshotCalendar, SnapshotError, SnapshotPriceBasis,
        TimeInterval, Timestamp, UnixSeconds, ValuationPriceEvidence,
    };
    use proptest::prelude::{ProptestConfig, any};
    use proptest::{prop_assert_eq, proptest};

    fn instant(day: i64, seconds: i64) -> Timestamp {
        let start = *at(day, 0).utc_seconds().as_i64();
        Timestamp::from_parts(
            UnixSeconds::new(start.checked_add(seconds).expect("초")),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn interval(day: i64, start: i64, end: i64) -> SessionData {
        SessionData::try_new(
            TimeInterval::try_new(instant(day, start), instant(day, end)).expect("구간"),
            None,
        )
        .expect("세션")
    }

    fn kr(
        day: i64,
        regular: Option<SessionData>,
        pre: Option<SessionData>,
        after: Option<SessionData>,
    ) -> MarketSession {
        MarketSession::try_new(
            date(day),
            SessionInformation::Kr {
                pre_market: pre,
                regular_market: regular,
                after_market: after,
            },
        )
        .expect("한국 세션")
    }

    fn kr_regular(day: i64) -> SessionData {
        interval(day, 0, 23_400)
    }

    fn us(day: i64) -> MarketSession {
        MarketSession::try_new(
            date(day),
            SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: Some(interval(day, 52_200, 75_600)),
                after_market: Some(interval(day, 75_600, 82_800)),
            },
        )
        .expect("미국 세션")
    }

    fn pair(today: MarketSession, previous: MarketSession) -> (MarketCalendar, MarketCalendar) {
        let kr = MarketCalendar::try_new(previous, today, kr(4, Some(kr_regular(4)), None, None))
            .expect("한국 달력");
        let us = MarketCalendar::try_new(us(2), us(3), us(4)).expect("미국 달력");
        (kr, us)
    }

    fn calendar(today: MarketSession, previous: MarketSession) -> SnapshotCalendar {
        let (kr, us) = pair(today, previous);
        SnapshotCalendar::new(&kr, &us).expect("평가 달력")
    }

    #[test]
    fn regular_close_uses_korean_close_during_after_market_and_rejects_intraday() {
        let previous = kr(2, Some(kr_regular(2)), None, None);
        let today = kr(
            3,
            Some(kr_regular(3)),
            None,
            Some(interval(3, 23_400, 39_600)),
        );
        let (kr_calendar, us_calendar) = pair(today, previous);
        let cal = SnapshotCalendar::new(&kr_calendar, &us_calendar).expect("달력");
        let current = Price::try_new(money("123", Currency::Krw)).expect("가격");
        let after_trade = Price::try_new(money("117", Currency::Krw)).expect("가격");
        assert_eq!(
            kr_calendar.state_at(instant(3, 25_200)),
            MarketState::AfterMarket
        );
        assert_eq!(
            cal.price(Currency::Krw, instant(3, 25_200)),
            Ok((date(3), SnapshotPriceBasis::CurrentSession))
        );
        assert_eq!(
            cal.price(Currency::Usd, instant(3, 25_200)),
            Ok((date(2), SnapshotPriceBasis::AwaitingSession))
        );
        assert_eq!(
            ValuationPriceEvidence::SessionClose {
                price: current,
                date: date(3)
            }
            .closing_price(cal, Currency::Krw, instant(3, 25_200)),
            Ok(current)
        );
        assert_eq!(
            ValuationPriceEvidence::Intraday {
                price: after_trade,
                observed_at: instant(3, 25_200)
            }
            .closing_price(cal, Currency::Krw, instant(3, 25_200)),
            Err(SnapshotError::InvalidCandle)
        );
    }

    #[test]
    fn regular_close_rejects_today_before_close_and_accepts_the_boundary() {
        let cal = calendar(
            kr(
                3,
                Some(kr_regular(3)),
                None,
                Some(interval(3, 23_400, 39_600)),
            ),
            kr(2, Some(kr_regular(2)), None, None),
        );
        let prior = Price::try_new(money("101", Currency::Krw)).expect("가격");
        let current = Price::try_new(money("123", Currency::Krw)).expect("가격");
        assert_eq!(
            cal.price(Currency::Krw, instant(3, 21_600)),
            Ok((date(2), SnapshotPriceBasis::AwaitingSession))
        );
        assert_eq!(
            ValuationPriceEvidence::SessionClose {
                price: current,
                date: date(3)
            }
            .closing_price(cal, Currency::Krw, instant(3, 21_600)),
            Err(SnapshotError::InvalidCandle)
        );
        assert_eq!(
            ValuationPriceEvidence::AwaitingSession {
                price: prior,
                date: date(2)
            }
            .closing_price(cal, Currency::Krw, instant(3, 21_600)),
            Ok(prior)
        );
        assert_eq!(
            cal.price(Currency::Krw, instant(3, 23_399)),
            Ok((date(2), SnapshotPriceBasis::AwaitingSession))
        );
        assert_eq!(
            cal.price(Currency::Krw, instant(3, 23_400)),
            Ok((date(3), SnapshotPriceBasis::CurrentSession))
        );
    }

    #[test]
    fn regular_close_handles_short_session_holiday_and_missing_regular_session() {
        let previous = kr(
            2,
            Some(kr_regular(2)),
            None,
            Some(interval(2, 23_400, 39_600)),
        );
        let short = kr(
            3,
            Some(interval(3, 0, 12_600)),
            None,
            Some(interval(3, 12_600, 39_600)),
        );
        let cal = calendar(short, previous.clone());
        assert_eq!(
            cal.price(Currency::Krw, instant(3, 12_599)),
            Ok((date(2), SnapshotPriceBasis::AwaitingSession))
        );
        assert_eq!(
            cal.price(Currency::Krw, instant(3, 12_600)),
            Ok((date(3), SnapshotPriceBasis::CurrentSession))
        );
        let holiday = calendar(kr(3, None, None, None), previous);
        assert_eq!(
            holiday.price(Currency::Krw, instant(3, 25_200)),
            Ok((date(2), SnapshotPriceBasis::HolidayCarry))
        );
        let unknown = calendar(
            kr(3, None, None, Some(interval(3, 23_400, 39_600))),
            kr(2, Some(kr_regular(2)), None, None),
        );
        assert_eq!(
            unknown.price(Currency::Krw, instant(3, 25_200)),
            Err(SnapshotError::MissingPrice)
        );
        let unknown_previous = calendar(
            kr(3, None, None, None),
            kr(2, None, None, Some(interval(2, 23_400, 39_600))),
        );
        assert_eq!(
            unknown_previous.price(Currency::Krw, instant(3, 25_200)),
            Err(SnapshotError::MissingPrice)
        );
    }

    #[test]
    fn regular_close_prior_session_is_usable_at_its_exact_end() {
        let cal = calendar(
            kr(3, Some(kr_regular(3)), None, None),
            kr(
                2,
                Some(kr_regular(2)),
                None,
                Some(interval(2, 23_400, 39_600)),
            ),
        );
        assert_eq!(
            cal.price(Currency::Krw, instant(2, 23_399)),
            Err(SnapshotError::MissingPrice)
        );
        assert_eq!(
            cal.price(Currency::Krw, instant(2, 23_400)),
            Ok((date(2), SnapshotPriceBasis::AwaitingSession))
        );
    }

    fn us_extra(
        day: i64,
        day_market: Option<SessionData>,
        pre: Option<SessionData>,
        after: Option<SessionData>,
    ) -> MarketSession {
        MarketSession::try_new(
            date(day),
            SessionInformation::Us {
                day_market,
                pre_market: pre,
                regular_market: Some(interval(day, 52_200, 75_600)),
                after_market: after,
            },
        )
        .expect("미국 세션")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn regular_close_is_invariant_to_us_extended_sessions(day_market in any::<bool>(), pre in any::<bool>(), after in any::<bool>(), second in 0_i64..86_400) {
            let kr = MarketCalendar::try_new(
                kr(2, Some(kr_regular(2)), None, None),
                kr(3, Some(kr_regular(3)), None, None),
                kr(4, Some(kr_regular(4)), None, None),
            ).expect("한국 달력");
            let build = |today| MarketCalendar::try_new(us(2), today, us(4)).expect("미국 달력");
            let plain_us = build(us_extra(3, None, None, None));
            let extended_us = build(us_extra(3,
                day_market.then(|| interval(3, 36_000, 43_200)),
                pre.then(|| interval(3, 43_200, 52_200)),
                after.then(|| interval(3, 75_600, 82_800)),
            ));
            let plain = SnapshotCalendar::new(&kr, &plain_us).expect("평가 달력");
            let extended = SnapshotCalendar::new(&kr, &extended_us).expect("평가 달력");
            let moment = instant(3, second);
            prop_assert_eq!(extended.price(Currency::Usd, moment), plain.price(Currency::Usd, moment));
            let today = ValuationPriceEvidence::SessionClose { price: Price::try_new(money("83", Currency::Usd)).expect("가격"), date: date(3) };
            let previous = ValuationPriceEvidence::AwaitingSession { price: Price::try_new(money("71", Currency::Usd)).expect("가격"), date: date(2) };
            for evidence in [today, previous] {
                prop_assert_eq!(evidence.closing_price(extended, Currency::Usd, moment), evidence.closing_price(plain, Currency::Usd, moment));
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn regular_close_is_invariant_to_extended_sessions(pre in any::<bool>(), after in any::<bool>(), second in 0_i64..43_200) {
            let regular = kr(3, Some(kr_regular(3)), None, None);
            let previous = kr(2, Some(kr_regular(2)), None, None);
            let extra = kr(3, Some(kr_regular(3)), pre.then(|| interval(3, -7_200, 0)), after.then(|| interval(3, 23_400, 39_600)));
            let plain = calendar(regular, previous.clone());
            let extended = calendar(extra, previous);
            let moment = instant(3, second);
            prop_assert_eq!(extended.price(Currency::Krw, moment), plain.price(Currency::Krw, moment));
            let today = ValuationPriceEvidence::SessionClose { price: Price::try_new(money("123", Currency::Krw)).expect("가격"), date: date(3) };
            let prior_evidence = ValuationPriceEvidence::AwaitingSession { price: Price::try_new(money("101", Currency::Krw)).expect("가격"), date: date(2) };
            for evidence in [today, prior_evidence] {
                prop_assert_eq!(evidence.closing_price(extended, Currency::Krw, moment), evidence.closing_price(plain, Currency::Krw, moment));
            }
        }
    }
    #[test]
    fn observation_market_sessions_detect_pre_day_and_after_trading() {
        let previous = kr(2, Some(kr_regular(2)), None, None);
        let regular = kr(3, Some(kr_regular(3)), None, None);
        let regular_calendar = calendar(regular, previous.clone());
        assert_eq!(
            regular_calendar.observation_sessions(Currency::Krw),
            super::ObservationSessionScope::NoExtended
        );
        let pre = kr(3, Some(kr_regular(3)), Some(interval(3, -7_200, 0)), None);
        let after = kr(
            3,
            Some(kr_regular(3)),
            None,
            Some(interval(3, 23_400, 39_600)),
        );
        for row in [pre, after] {
            assert_eq!(
                calendar(row, previous.clone()).observation_sessions(Currency::Krw),
                super::ObservationSessionScope::HasExtended
            );
        }
        let us_day = us_extra(3, Some(interval(3, 36_000, 43_200)), None, None);
        let us_regular = us_extra(3, None, None, None);
        let kr_calendar = MarketCalendar::try_new(
            previous,
            kr(3, Some(kr_regular(3)), None, None),
            kr(4, Some(kr_regular(4)), None, None),
        )
        .expect("한국");
        for (row, expected) in [
            (us_day, super::ObservationSessionScope::HasExtended),
            (us_regular, super::ObservationSessionScope::NoExtended),
        ] {
            let us_calendar = MarketCalendar::try_new(us(2), row, us(4)).expect("미국");
            assert_eq!(
                SnapshotCalendar::new(&kr_calendar, &us_calendar)
                    .expect("평가")
                    .observation_sessions(Currency::Usd),
                expected
            );
        }
    }
}
