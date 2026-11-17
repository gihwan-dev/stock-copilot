use domain::{
    CalendarDate, CalendarYear, DayNumber, MonthNumber, Nanosecond, OffsetSeconds, Timestamp,
    UnixSeconds,
};
use std::{collections::VecDeque, str::FromStr};
pub(super) fn invalid<E>(_error: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}
#[derive(Default)]
pub(super) struct Fields(VecDeque<String>);
impl Fields {
    pub(super) fn push(&mut self, value: &(impl ToString + ?Sized)) {
        self.0.push_back(value.to_string());
    }
    pub(super) fn take(&mut self) -> rusqlite::Result<String> {
        self.0.pop_front().ok_or_else(|| invalid(()))
    }
    pub(super) fn number<T: FromStr>(&mut self) -> rusqlite::Result<T> {
        self.take()?.parse().map_err(invalid)
    }
    pub(super) fn encode(self) -> String {
        let mut encoded = String::new();
        for value in self.0 {
            encoded.push_str(&value.len().to_string());
            encoded.push(':');
            encoded.push_str(&value);
        }
        encoded
    }
    pub(super) fn decode(mut source: &str) -> rusqlite::Result<Self> {
        let mut fields = Self::default();
        while !source.is_empty() {
            let (size_text, rest) = source.split_once(':').ok_or_else(|| invalid(()))?;
            let size: usize = size_text.parse().map_err(invalid)?;
            let (value, tail) = rest.split_at_checked(size).ok_or_else(|| invalid(()))?;
            fields.push(&(value));
            source = tail;
        }
        Ok(fields)
    }
    pub(super) fn finish(self) -> rusqlite::Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(invalid(()))
        }
    }
    pub(super) fn put_time(&mut self, at: Timestamp) {
        self.push(&(at.utc_seconds().as_i64()));
        self.push(&(at.nanosecond().as_u32()));
        self.push(&(at.offset_seconds().as_i32()));
    }
    pub(super) fn time(&mut self) -> rusqlite::Result<Timestamp> {
        Timestamp::from_parts(
            UnixSeconds::new(self.number()?),
            Nanosecond::new(self.number()?),
            OffsetSeconds::new(self.number()?),
        )
        .map_err(invalid)
    }
    pub(super) fn put_optional_time(&mut self, at: Option<Timestamp>) {
        match at {
            Some(value) => {
                self.push(&("some"));
                self.put_time(value);
            }
            None => self.push(&("none")),
        }
    }
    pub(super) fn optional_time(&mut self) -> rusqlite::Result<Option<Timestamp>> {
        match self.take()?.as_str() {
            "some" => self.time().map(Some),
            "none" => Ok(None),
            _ => Err(invalid(())),
        }
    }
    pub(super) fn put_date(&mut self, date: CalendarDate) {
        self.push(&(date.year().as_i32()));
        self.push(&(date.month().as_u8()));
        self.push(&(date.day().as_u8()));
    }
    pub(super) fn date(&mut self) -> rusqlite::Result<CalendarDate> {
        CalendarDate::from_parts(
            CalendarYear::new(self.number()?),
            MonthNumber::new(self.number()?),
            DayNumber::new(self.number()?),
        )
        .map_err(invalid)
    }
}
pub(super) fn time_text(at: Timestamp) -> String {
    let mut fields = Fields::default();
    fields.put_time(at);
    fields.encode()
}
pub(super) fn read_time(text: &str) -> rusqlite::Result<Timestamp> {
    let mut fields = Fields::decode(text)?;
    let at = fields.time()?;
    fields.finish()?;
    Ok(at)
}
pub(super) fn date_text(date: CalendarDate) -> String {
    let mut fields = Fields::default();
    fields.put_date(date);
    fields.encode()
}
pub(super) fn read_date(text: &str) -> rusqlite::Result<CalendarDate> {
    let mut fields = Fields::decode(text)?;
    let date = fields.date()?;
    fields.finish()?;
    Ok(date)
}
