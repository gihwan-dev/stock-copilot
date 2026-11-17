use super::SettlementWindow;
use crate::{CalendarDate, Timestamp};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InspectionRangeDays(u32);

impl InspectionRangeDays {
    pub fn try_new(value: u32) -> Result<Self, FullInspectionError> {
        if value == 0 {
            Err(FullInspectionError::ZeroLimit)
        } else {
            Ok(Self(value))
        }
    }
    #[must_use]
    pub const fn as_u32(&self) -> &u32 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullInspectionError {
    ZeroLimit,
    InvalidPeriod,
    OutOfRange,
    UnexpectedRange,
    DuplicateRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullInspectionPlan(Vec<SettlementWindow>);

impl FullInspectionPlan {
    pub fn new(
        opened_on: Option<CalendarDate>,
        now: Timestamp,
        limit: InspectionRangeDays,
    ) -> Result<Self, FullInspectionError> {
        let end = crate::transaction_sync::kst_date(now)
            .map_err(|_error| FullInspectionError::OutOfRange)?;
        let start = opened_on.unwrap_or_else(CalendarDate::inspection_earliest);
        let period = SettlementWindow::try_new(start, end)
            .map_err(|_error| FullInspectionError::InvalidPeriod)?;
        Ok(split_full_inspection(period, limit))
    }
    #[must_use]
    pub fn ranges(&self) -> &[SettlementWindow] {
        &self.0
    }
    #[must_use]
    pub fn begin(&self) -> FullInspectionProgress {
        FullInspectionProgress {
            results: self
                .0
                .iter()
                .map(|range| ((range.from, range.to), None))
                .collect(),
            invalid: None,
        }
    }
}

#[must_use]
pub fn split_full_inspection(
    period: SettlementWindow,
    limit: InspectionRangeDays,
) -> FullInspectionPlan {
    let mut ranges = Vec::new();
    let mut start = period.from;
    loop {
        let end = start.inspection_chunk_end(period.to, limit);
        ranges.push(SettlementWindow {
            from: start,
            to: end,
        });
        if end == period.to {
            return FullInspectionPlan(ranges);
        }
        start = end.inspection_successor();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InspectionRangeResult {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullInspectionStatus {
    Incomplete,
    Complete,
    Invalid(FullInspectionError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FullInspectionProgress {
    results: BTreeMap<(CalendarDate, CalendarDate), Option<InspectionRangeResult>>,
    invalid: Option<FullInspectionError>,
}

impl FullInspectionProgress {
    pub fn report(
        &mut self,
        range: SettlementWindow,
        result: InspectionRangeResult,
    ) -> Result<(), FullInspectionError> {
        if let Some(error) = self.invalid {
            return Err(error);
        }
        let error = match self.results.get_mut(&(range.from, range.to)) {
            Some(slot @ None) => {
                *slot = Some(result);
                return Ok(());
            }
            Some(Some(_result)) => FullInspectionError::DuplicateRange,
            None => FullInspectionError::UnexpectedRange,
        };
        self.invalid = Some(error);
        Err(error)
    }
    #[must_use]
    pub fn status(&self) -> FullInspectionStatus {
        if let Some(error) = self.invalid {
            return FullInspectionStatus::Invalid(error);
        }
        if self
            .results
            .values()
            .all(|result| *result == Some(InspectionRangeResult::Succeeded))
        {
            FullInspectionStatus::Complete
        } else {
            FullInspectionStatus::Incomplete
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FullInspectionError, FullInspectionPlan, FullInspectionStatus, InspectionRangeDays,
        InspectionRangeResult, SettlementWindow, split_full_inspection,
    };
    use crate::{CalendarDate, Timestamp};
    use crate::{CalendarYear, DayNumber, MonthNumber, Nanosecond, OffsetSeconds, UnixSeconds};
    use proptest::{prop_assert, prop_assert_eq, proptest, test_runner::Config as ProptestConfig};
    use time::{Date, Month};

    fn date(year: i32, month: u8, day: u8) -> CalendarDate {
        CalendarDate::from_parts(
            CalendarYear::new(year),
            MonthNumber::new(month),
            DayNumber::new(day),
        )
        .expect("날짜")
    }
    fn window(first: (i32, u8, u8), second: (i32, u8, u8)) -> SettlementWindow {
        SettlementWindow::try_new(
            date(first.0, first.1, first.2),
            date(second.0, second.1, second.2),
        )
        .expect("창")
    }
    fn limit(count: u32) -> InspectionRangeDays {
        InspectionRangeDays::try_new(count).expect("상한")
    }
    fn now(seconds: i64, nanos: u32) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(nanos),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    #[test]
    fn exact_calendar_examples() {
        for (period, cap, expected) in [
            (
                window((2024_i32, 2_u8, 28_u8), (2024_i32, 3_u8, 2_u8)),
                2,
                vec![
                    window((2024_i32, 2_u8, 28_u8), (2024_i32, 2_u8, 29_u8)),
                    window((2024_i32, 3_u8, 1_u8), (2024_i32, 3_u8, 2_u8)),
                ],
            ),
            (
                window((2026_i32, 12_u8, 31_u8), (2027_i32, 1_u8, 2_u8)),
                2,
                vec![
                    window((2026_i32, 12_u8, 31_u8), (2027_i32, 1_u8, 1_u8)),
                    window((2027_i32, 1_u8, 2_u8), (2027_i32, 1_u8, 2_u8)),
                ],
            ),
            (
                window((2026_i32, 1_u8, 31_u8), (2026_i32, 2_u8, 2_u8)),
                1,
                vec![
                    window((2026_i32, 1_u8, 31_u8), (2026_i32, 1_u8, 31_u8)),
                    window((2026_i32, 2_u8, 1_u8), (2026_i32, 2_u8, 1_u8)),
                    window((2026_i32, 2_u8, 2_u8), (2026_i32, 2_u8, 2_u8)),
                ],
            ),
            (
                window((1_i32, 1_u8, 1_u8), (1_i32, 1_u8, 1_u8)),
                1,
                vec![window((1_i32, 1_u8, 1_u8), (1_i32, 1_u8, 1_u8))],
            ),
            (
                window((9999_i32, 12_u8, 30_u8), (9999_i32, 12_u8, 31_u8)),
                u32::MAX,
                vec![window((9999_i32, 12_u8, 30_u8), (9999_i32, 12_u8, 31_u8))],
            ),
        ] {
            assert_eq!(split_full_inspection(period, limit(cap)).ranges(), expected);
        }
        assert_eq!(
            InspectionRangeDays::try_new(0),
            Err(FullInspectionError::ZeroLimit)
        );
        assert_eq!(*limit(19).as_u32(), 19);
    }
    #[test]
    fn start_policy_and_kst_midnight() {
        let midnight = Date::from_calendar_date(2026, Month::February, 1)
            .expect("日")
            .midnight()
            .assume_utc()
            .unix_timestamp()
            .checked_sub(32400)
            .expect("KST");
        for (seconds, nanos, end) in [
            (
                midnight.checked_sub(1).expect("이전"),
                999_999_999,
                date(2026_i32, 1_u8, 31_u8),
            ),
            (midnight, 0, date(2026_i32, 2_u8, 1_u8)),
            (midnight, 1, date(2026_i32, 2_u8, 1_u8)),
        ] {
            let plan = FullInspectionPlan::new(
                Some(date(2024_i32, 2_u8, 29_u8)),
                now(seconds, nanos),
                limit(u32::MAX),
            )
            .expect("計画");
            assert_eq!(
                plan.ranges(),
                [SettlementWindow::try_new(date(2024_i32, 2_u8, 29_u8), end).expect("창")]
            );
            let unknown =
                FullInspectionPlan::new(None, now(seconds, nanos), limit(u32::MAX)).expect("창");
            assert_eq!(
                unknown.ranges(),
                [SettlementWindow::try_new(date(1_i32, 1_u8, 1_u8), end).expect("창")]
            );
        }
        assert_eq!(
            FullInspectionPlan::new(Some(date(2026_i32, 2_u8, 2_u8)), now(midnight, 0), limit(2)),
            Err(FullInspectionError::InvalidPeriod)
        );
        assert_eq!(
            FullInspectionPlan::new(None, now(253_402_268_400, 0), limit(2)),
            Err(FullInspectionError::OutOfRange)
        );
    }
    #[test]
    fn completion_missing_failed_duplicates_and_foreign_ranges() {
        let plan = split_full_inspection(
            window((2024_i32, 2_u8, 28_u8), (2024_i32, 3_u8, 2_u8)),
            limit(2),
        );
        let first = plan.ranges()[0];
        let second = plan.ranges()[1];
        let mut progress = plan.begin();
        assert_eq!(progress.status(), FullInspectionStatus::Incomplete);
        progress
            .report(second, InspectionRangeResult::Succeeded)
            .expect("보고");
        assert_eq!(progress.status(), FullInspectionStatus::Incomplete);
        progress
            .report(first, InspectionRangeResult::Succeeded)
            .expect("보고");
        assert_eq!(progress.status(), FullInspectionStatus::Complete);
        assert_eq!(
            progress.report(second, InspectionRangeResult::Failed),
            Err(FullInspectionError::DuplicateRange)
        );
        assert_eq!(
            progress.status(),
            FullInspectionStatus::Invalid(FullInspectionError::DuplicateRange)
        );
        assert_eq!(
            progress.report(first, InspectionRangeResult::Succeeded),
            Err(FullInspectionError::DuplicateRange)
        );
        let mut failed = plan.begin();
        failed
            .report(first, InspectionRangeResult::Failed)
            .expect("보고");
        failed
            .report(second, InspectionRangeResult::Succeeded)
            .expect("보고");
        assert_eq!(failed.status(), FullInspectionStatus::Incomplete);
        assert_eq!(
            failed.report(first, InspectionRangeResult::Succeeded),
            Err(FullInspectionError::DuplicateRange)
        );
    }
    #[test]
    fn foreign_and_duplicate_reports_are_permanent() {
        let plan = split_full_inspection(
            window((2024_i32, 2_u8, 28_u8), (2024_i32, 3_u8, 2_u8)),
            limit(2),
        );
        let first = plan.ranges()[0];
        let mut foreign = plan.begin();
        assert_eq!(
            foreign.report(
                window((2024_i32, 2_u8, 28_u8), (2024_i32, 3_u8, 2_u8)),
                InspectionRangeResult::Succeeded
            ),
            Err(FullInspectionError::UnexpectedRange)
        );
        assert_eq!(
            foreign.status(),
            FullInspectionStatus::Invalid(FullInspectionError::UnexpectedRange)
        );
        assert_eq!(
            foreign.report(first, InspectionRangeResult::Succeeded),
            Err(FullInspectionError::UnexpectedRange)
        );
        let mut duplicate_success = plan.begin();
        duplicate_success
            .report(first, InspectionRangeResult::Succeeded)
            .expect("보고");
        assert_eq!(
            duplicate_success.report(first, InspectionRangeResult::Succeeded),
            Err(FullInspectionError::DuplicateRange)
        );
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn partition_and_completion_equivalence(start in 1_721_426_i32..5_373_000_i32, length in 1_i32..300_i32, cap in 1_u32..400_u32, outcomes in proptest::collection::vec(0_u8..3,1..300), reverse in proptest::bool::ANY) {
            let raw=Date::from_julian_day(start).expect("시작");
            let end=Date::from_julian_day(start.checked_add(length).expect("합").checked_sub(1).expect("끝")).expect("끝");
            let from=date(raw.year(),raw.month().into(),raw.day());
            let to=date(end.year(),end.month().into(),end.day());
            let plan=split_full_inspection(SettlementWindow::try_new(from,to).expect("창"),limit(cap));
            let mut cursor=start;
            for range in plan.ranges() {
                let first=Date::from_calendar_date(*range.from().year().as_i32(),Month::try_from(*range.from().month().as_u8()).expect("월"),*range.from().day().as_u8()).expect("날짜").to_julian_day();
                let second=Date::from_calendar_date(*range.to().year().as_i32(),Month::try_from(*range.to().month().as_u8()).expect("월"),*range.to().day().as_u8()).expect("날짜").to_julian_day();
                prop_assert_eq!(first,cursor);
                prop_assert_eq!(second,(first.checked_add(i32::try_from(cap).expect("상한")).expect("합").checked_sub(1).expect("끝")).min(start.checked_add(length).expect("합").checked_sub(1).expect("끝")));
                prop_assert!(second>=first);
                cursor=second.checked_add(1).expect("다음");
            }
            prop_assert_eq!(cursor,start.checked_add(length).expect("합"));
            let mut reports:Vec<_>=plan.ranges().iter().enumerate().map(|(index,range)|(*range,outcomes[index.checked_rem(outcomes.len()).expect("나머지")])).collect();
            if reverse { reports.reverse(); }
            let expected=reports.iter().all(|(_,outcome)|*outcome==1);
            let mut progress=plan.begin();
            for (range,outcome) in reports {
                match outcome {
                    1=>progress.report(range,InspectionRangeResult::Succeeded).expect("성공"),
                    2=>progress.report(range,InspectionRangeResult::Failed).expect("실패"),
                    _=>(),
                }
            }
            prop_assert_eq!(progress.status()==FullInspectionStatus::Complete,expected);
        }
    }
}
