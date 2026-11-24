use crate::CalendarDate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BriefingKind {
    Morning,
    ClosingReview,
    PreUsMarket,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BriefingId {
    date: CalendarDate,
    kind: BriefingKind,
}

impl BriefingId {
    #[must_use]
    pub const fn new(kind: BriefingKind, date: CalendarDate) -> Self {
        Self { date, kind }
    }

    #[must_use]
    pub const fn kind(self) -> BriefingKind {
        self.kind
    }

    #[must_use]
    pub const fn date(self) -> CalendarDate {
        self.date
    }
}

#[cfg(test)]
mod tests {
    use super::{BriefingId, BriefingKind};
    use crate::{CalendarDate, CalendarYear, DayNumber, MonthNumber};

    fn date(day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(2026),
            MonthNumber::new(11),
            DayNumber::new(day),
        )
        .expect("날짜")
    }

    #[test]
    fn briefing_id_is_one_per_kind_and_day() {
        let morning = BriefingId::new(BriefingKind::Morning, date(25));
        assert_eq!(morning, BriefingId::new(BriefingKind::Morning, date(25)));
        assert_ne!(
            morning,
            BriefingId::new(BriefingKind::ClosingReview, date(25))
        );
        assert_ne!(morning, BriefingId::new(BriefingKind::Morning, date(26)));
        assert!(morning < BriefingId::new(BriefingKind::Morning, date(26)));
        assert_eq!(morning.kind(), BriefingKind::Morning);
        assert_eq!(morning.date(), date(25));
    }
}
