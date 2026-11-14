use crate::{CalendarDate, Currency, FxRate, Market, MarketCalendar, OffsetSeconds, Timestamp};
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
struct MarketDay {
    date: CalendarDate,
    previous: CalendarDate,
    close: Option<Timestamp>,
    previous_close: Option<Timestamp>,
}
impl MarketDay {
    fn from_calendar(calendar: &MarketCalendar) -> Self {
        let end = |row: &crate::MarketSession| {
            row.sessions()
                .last()
                .map(|segment| segment.session().interval().end())
        };
        Self {
            date: calendar.today().date(),
            previous: calendar.previous_business_day().date(),
            close: end(calendar.today()),
            previous_close: end(calendar.previous_business_day()),
        }
    }
    fn price(self, at: Timestamp) -> Result<(CalendarDate, SnapshotPriceBasis), SnapshotError> {
        match self.close {
            Some(close) if close <= at => Ok((self.date, SnapshotPriceBasis::CurrentSession)),
            current => {
                if self.previous_close.is_none_or(|close| close > at) {
                    return Err(SnapshotError::MissingPrice);
                }
                Ok((
                    self.previous,
                    match current {
                        Some(_) => SnapshotPriceBasis::AwaitingSession,
                        None => SnapshotPriceBasis::HolidayCarry,
                    },
                ))
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
        if kr.close.is_none() && us.close.is_none() {
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
