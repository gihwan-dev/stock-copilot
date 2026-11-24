use crate::{
    CalendarDate, MarketCalendar, SnapshotCalendar, SnapshotError, Timestamp, ValuationSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValuationDayEligibility {
    Candidate,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValuationCalendarDay {
    date: CalendarDate,
    calendar: Result<SnapshotCalendar, SnapshotError>,
    eligibility: ValuationDayEligibility,
}
impl ValuationCalendarDay {
    pub fn new(kr: &MarketCalendar, us: &MarketCalendar) -> Result<Self, SnapshotError> {
        let date = kr.today().date();
        let (calendar, eligibility) = match SnapshotCalendar::new(kr, us) {
            Ok(value) => (Ok(value), ValuationDayEligibility::Candidate),
            Err(SnapshotError::ClosedDay) => (
                Err(SnapshotError::ClosedDay),
                ValuationDayEligibility::Closed,
            ),
            Err(error) => return Err(error),
        };
        Ok(Self {
            date,
            calendar,
            eligibility,
        })
    }
    #[must_use]
    pub const fn unavailable(date: CalendarDate, reason: SnapshotError) -> Self {
        Self {
            date,
            calendar: Err(reason),
            eligibility: ValuationDayEligibility::Candidate,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationGapPlan {
    days: Vec<ValuationCalendarDay>,
}
impl ValuationGapPlan {
    pub fn new(
        last: CalendarDate,
        now: Timestamp,
        calendars: &[ValuationCalendarDay],
    ) -> Result<Self, SnapshotError> {
        let today = crate::valuation_snapshot_calendar::kst_date(now)?;
        if last > today {
            return Err(SnapshotError::WrongPeriod);
        }
        let mut previous = last;
        let mut days = Vec::new();
        for row in calendars {
            if row.date > today
                || previous
                    .days_until(row.date)
                    .map(|distance| *distance.as_i64())
                    != Ok(1)
            {
                return Err(SnapshotError::CalendarMismatch);
            }
            previous = row.date;
            if row.eligibility == ValuationDayEligibility::Candidate {
                days.push(*row);
            }
        }
        if previous != today {
            return Err(SnapshotError::CalendarMismatch);
        }
        Ok(Self { days })
    }
    pub fn dates(&self) -> impl Iterator<Item = CalendarDate> + '_ {
        self.days.iter().map(|row| row.date)
    }
    pub fn resolve<F>(&self, mut fill: F) -> Vec<ValuationDayResult>
    where
        F: FnMut(CalendarDate, SnapshotCalendar) -> Result<ValuationSnapshot, SnapshotError>,
    {
        self.days
            .iter()
            .map(|row| {
                let result = row.calendar.and_then(|calendar| {
                    let snapshot = fill(row.date, calendar)?;
                    snapshot_on_date(snapshot, row.date, calendar)
                });
                ValuationDayResult {
                    date: row.date,
                    result,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationDayResult {
    date: CalendarDate,
    result: Result<ValuationSnapshot, SnapshotError>,
}
impl ValuationDayResult {
    pub fn restore_from_saved_state(
        date: CalendarDate,
        result: Result<ValuationSnapshot, SnapshotError>,
    ) -> Result<Self, SnapshotError> {
        if let Ok(snapshot) = &result
            && snapshot.settlement_date() != date
        {
            return Err(SnapshotError::WrongPeriod);
        }
        Ok(Self { date, result })
    }
    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.date
    }
    pub const fn result(&self) -> Result<&ValuationSnapshot, &SnapshotError> {
        self.result.as_ref()
    }
}

fn snapshot_on_date(
    snapshot: ValuationSnapshot,
    date: CalendarDate,
    calendar: SnapshotCalendar,
) -> Result<ValuationSnapshot, SnapshotError> {
    if snapshot.settlement_date() != date {
        return Err(SnapshotError::WrongPeriod);
    }
    if snapshot.dates() != calendar.dates() {
        return Err(SnapshotError::CalendarMismatch);
    }
    for currency in [crate::Currency::Krw, crate::Currency::Usd] {
        if calendar.price(currency, snapshot.valuation_at())
            != Ok((
                snapshot.price_date(currency),
                snapshot.price_basis(currency),
            ))
        {
            return Err(SnapshotError::CalendarMismatch);
        }
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use crate::valuation_snapshot::tests::{at, date, observed};
    use crate::{
        Market, MarketCalendar, MarketSession, SessionData, SessionInformation, SnapshotError,
        TimeInterval, TimeSpan, ValuationCalendarDay, ValuationGapPlan,
    };
    use proptest::prelude::{ProptestConfig, any, prop};
    use proptest::{prop_assert_eq, proptest};

    fn trading(number: i64) -> SessionData {
        SessionData::try_new(
            TimeInterval::try_new(at(number, 0), at(number, 1)).expect("구간"),
            None,
        )
        .expect("세션")
    }

    #[test]
    fn unavailable_calendar_dates_keep_reasons_without_skipping_other_days() {
        let rows = [
            row(2, Open, Open),
            ValuationCalendarDay::unavailable(date(3), SnapshotError::MissingPrice),
            row(4, Closed, Closed),
            ValuationCalendarDay::unavailable(date(5), SnapshotError::ClosedDay),
            row(6, Open, Open),
        ];
        let plan = ValuationGapPlan::new(date(1), at(6, 2), &rows).expect("계획");
        assert_eq!(
            plan.dates().collect::<Vec<_>>(),
            vec![date(2), date(3), date(5), date(6)]
        );
        let mut called = Vec::new();
        let results = plan.resolve(|day, _calendar| {
            called.push(day);
            if day == date(2) {
                Ok(observed(2, "101", "203"))
            } else {
                Ok(observed(6, "307", "409"))
            }
        });
        assert_eq!(called, vec![date(2), date(6)]);
        let mut items = results.iter();
        assert_eq!(
            items.next().expect("2일").result(),
            Ok(&observed(2, "101", "203"))
        );
        assert_eq!(
            items.next().expect("3일").result(),
            Err(&SnapshotError::MissingPrice)
        );
        assert_eq!(
            items.next().expect("5일").result(),
            Err(&SnapshotError::ClosedDay)
        );
        assert_eq!(
            items.next().expect("6일").result(),
            Ok(&observed(6, "307", "409"))
        );
        assert_eq!(items.next(), None);
    }

    #[test]
    fn returned_snapshot_must_use_planned_market_dates_and_price_bases() {
        let plan = ValuationGapPlan::new(date(2), at(3, 2), &[row(3, Open, Open)]).expect("계획");
        let other = crate::SnapshotCalendar::new(
            &market_calendar(3, &Market::Kr, Open),
            &market_calendar(2, &Market::Us, Open),
        )
        .expect("다른 미국 날짜");
        let empty = crate::Portfolio::new(
            Vec::new(),
            crate::BuyingPower::new(
                crate::valuation_snapshot::tests::money("0", crate::Currency::Krw),
                at(3, 7),
            ),
            crate::BuyingPower::new(
                crate::valuation_snapshot::tests::money("0", crate::Currency::Usd),
                at(3, 7),
            ),
        )
        .expect("포트폴리오");
        let mismatched = plan.resolve(|_day, _calendar| {
            crate::ValuationSnapshot::observe(&other, &empty.evaluate(None), at(3, 7), None)
        });
        assert_eq!(
            mismatched.first().expect("결과").result(),
            Err(&SnapshotError::CalendarMismatch)
        );
        for (kr, us) in [(Closed, Open), (Open, Closed)] {
            let closed_plan =
                ValuationGapPlan::new(date(2), at(3, 2), &[row(3, kr, us)]).expect("계획");
            let result = closed_plan.resolve(|_day, _calendar| Ok(observed(3, "509", "601")));
            assert_eq!(
                result.first().expect("결과").result(),
                Err(&SnapshotError::CalendarMismatch)
            );
        }
    }

    #[test]
    fn wrong_market_and_kst_projection_overflow_are_errors() {
        let kr = market_calendar(3, &Market::Kr, Open);
        let us = market_calendar(3, &Market::Us, Open);
        assert_eq!(
            ValuationCalendarDay::new(&us, &kr),
            Err(SnapshotError::CalendarMismatch)
        );
        assert_eq!(
            ValuationCalendarDay::new(&kr, &kr),
            Err(SnapshotError::CalendarMismatch)
        );
        let max = crate::Timestamp::from_parts(
            crate::UnixSeconds::new(253_402_300_799),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(0),
        )
        .expect("시각 상한");
        assert_eq!(
            ValuationGapPlan::new(date(1), max, &[]),
            Err(SnapshotError::Time(crate::TimeError::OutOfRange))
        );
    }

    #[test]
    fn holiday_carry_preserves_last_close_without_accepting_intraday() {
        use crate::valuation_snapshot::tests::money;
        use crate::{Currency, Price, ValuationPriceEvidence};
        let plan =
            ValuationGapPlan::new(date(2), at(3, 2), &[row(3, Closed, Open)]).expect("편측 휴장");
        let price = Price::try_new(money("317", Currency::Krw)).expect("가격");
        let results = plan.resolve(|_day, calendar| {
            let carry = ValuationPriceEvidence::HolidayCarry {
                price,
                date: date(2),
            };
            assert_eq!(
                carry.closing_price(calendar, Currency::Krw, at(3, 2)),
                Ok(price)
            );
            let wrong = ValuationPriceEvidence::SessionClose {
                price,
                date: date(2),
            };
            assert_eq!(
                wrong.closing_price(calendar, Currency::Krw, at(3, 2)),
                Err(SnapshotError::InvalidCandle)
            );
            Err(SnapshotError::MissingCash)
        });
        assert_eq!(
            results.first().expect("결과").result(),
            Err(&SnapshotError::MissingCash)
        );
    }

    #[derive(Clone, Copy)]
    enum Active {
        Open,
        Closed,
    }
    use Active::{Closed, Open};

    fn row(day: i64, kr_open: Active, us_open: Active) -> ValuationCalendarDay {
        ValuationCalendarDay::new(
            &market_calendar(day, &Market::Kr, kr_open),
            &market_calendar(day, &Market::Us, us_open),
        )
        .expect("평가일")
    }
    fn market_calendar(day: i64, market: &Market, open: Active) -> MarketCalendar {
        let session = |number, active: Active| {
            let data = match active {
                Open => Some(trading(number)),
                Closed => None,
            };
            let information = match market {
                Market::Kr => SessionInformation::Kr {
                    pre_market: None,
                    regular_market: data,
                    after_market: None,
                },
                Market::Us | Market::Unknown(_) => SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: data,
                    after_market: None,
                },
            };
            MarketSession::try_new(date(number), information).expect("시장일")
        };
        MarketCalendar::try_new(
            session(day.checked_sub(1).expect("전일"), Open),
            session(day, open),
            session(day.checked_add(1).expect("익일"), Open),
        )
        .expect("달력")
    }

    #[test]
    fn weekend_holidays_year_end_and_last_date_boundaries() {
        let rows = [
            row(1, Closed, Closed),
            row(2, Open, Closed),
            row(3, Closed, Closed),
            row(4, Closed, Closed),
            row(5, Closed, Open),
        ];
        let plan = ValuationGapPlan::new(date(0), at(5, 2), &rows).expect("연말부터 연휴와 주말");
        assert_eq!(plan.dates().collect::<Vec<_>>(), vec![date(2), date(5)]);
        assert_eq!(
            ValuationGapPlan::new(date(5), at(5, 2), &[])
                .expect("오늘")
                .dates()
                .count(),
            0
        );
        assert_eq!(
            ValuationGapPlan::new(date(4), at(5, 2), &[row(5, Open, Open)])
                .expect("어제")
                .dates()
                .collect::<Vec<_>>(),
            vec![date(5)]
        );
        assert_eq!(
            ValuationGapPlan::new(date(6), at(5, 2), &[]),
            Err(SnapshotError::WrongPeriod)
        );
        let long: Vec<_> = (1..=400).map(|day| row(day, Open, Open)).collect();
        assert_eq!(
            ValuationGapPlan::new(date(0), at(400, 2), &long)
                .expect("오래전")
                .dates()
                .count(),
            400
        );
    }

    #[test]
    fn consecutive_midweek_holidays_are_not_missing_evaluations() {
        let rows = [
            row(6, Closed, Closed),
            row(7, Closed, Closed),
            row(8, Open, Closed),
        ];
        let plan = ValuationGapPlan::new(date(5), at(8, 2), &rows).expect("연휴");
        assert_eq!(plan.dates().collect::<Vec<_>>(), vec![date(8)]);
    }

    #[test]
    fn midnight_and_calendar_coverage_are_exact() {
        let before = at(2, 15).checked_sub(TimeSpan::NANOSECOND).expect("직전");
        assert_eq!(
            ValuationGapPlan::new(date(2), before, &[])
                .expect("KST23:59")
                .dates()
                .count(),
            0
        );
        assert_eq!(
            ValuationGapPlan::new(date(2), at(2, 15), &[]),
            Err(SnapshotError::CalendarMismatch)
        );
        assert_eq!(
            ValuationGapPlan::new(date(2), at(2, 15), &[row(3, Open, Closed)])
                .expect("KST00:00")
                .dates()
                .collect::<Vec<_>>(),
            vec![date(3)]
        );
        for rows in [
            vec![],
            vec![row(3, Open, Open)],
            vec![row(2, Open, Open), row(2, Open, Open)],
            vec![row(2, Open, Open), row(4, Open, Open)],
            vec![row(3, Open, Open), row(2, Open, Open)],
        ] {
            assert_eq!(
                ValuationGapPlan::new(date(1), at(3, 2), &rows),
                Err(SnapshotError::CalendarMismatch)
            );
        }
    }

    #[test]
    fn results_keep_success_failure_and_wrong_snapshot_date() {
        let plan = ValuationGapPlan::new(
            date(1),
            at(4, 2),
            &[row(2, Open, Open), row(3, Open, Open), row(4, Open, Open)],
        )
        .expect("계획");
        let results = plan.resolve(|day, _calendar| {
            if day == date(2) {
                Ok(observed(2, "101", "203"))
            } else if day == date(3) {
                Err(SnapshotError::MissingFx)
            } else {
                Ok(observed(5, "307", "409"))
            }
        });
        assert_eq!(
            results
                .iter()
                .map(crate::ValuationDayResult::date)
                .collect::<Vec<_>>(),
            vec![date(2), date(3), date(4)]
        );
        let mut items = results.iter();
        assert_eq!(
            items.next().expect("성공").result(),
            Ok(&observed(2, "101", "203"))
        );
        assert_eq!(
            items.next().expect("누락").result(),
            Err(&SnapshotError::MissingFx)
        );
        assert_eq!(
            items.next().expect("다른 날짜").result(),
            Err(&SnapshotError::WrongPeriod)
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn unavailable_and_closed_dates_have_distinct_complete_outcomes(states in prop::collection::vec(0_u8..3, 1..80)) {
            let rows: Vec<_> = states.iter().enumerate().map(|(index, state)| {
                let day = i64::try_from(index).expect("인덱스").checked_add(2).expect("날짜");
                match state {
                    0 => row(day, Closed, Closed),
                    1 => ValuationCalendarDay::unavailable(date(day), SnapshotError::ClosedDay),
                    _ => row(day, Open, Open),
                }
            }).collect();
            let end = i64::try_from(states.len()).expect("길이").checked_add(1).expect("마지막");
            let plan = ValuationGapPlan::new(date(1), at(end, 2), &rows).expect("계획");
            let actual = plan.resolve(|_day, _calendar| Err(SnapshotError::MissingFx));
            let expected: Vec<_> = states.iter().enumerate().filter(|(_index, state)| **state != 0).map(|(index, state)| (date(i64::try_from(index).expect("인덱스").checked_add(2).expect("날짜")), if *state == 1 { SnapshotError::ClosedDay } else { SnapshotError::MissingFx })).collect();
            prop_assert_eq!(actual.iter().map(|item| (item.date(), *item.result().expect_err("실패 사유"))).collect::<Vec<_>>(), expected);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn every_open_date_has_exactly_one_result(open in prop::collection::vec((any::<bool>(), any::<bool>()), 1..80)) {
            let rows: Vec<_> = open.iter().enumerate().map(|(index, (kr, us))| row(i64::try_from(index).expect("인덱스").checked_add(2).expect("일"), if *kr { Open } else { Closed }, if *us { Open } else { Closed })).collect();
            let end = i64::try_from(open.len()).expect("길이").checked_add(1).expect("일");
            let plan = ValuationGapPlan::new(date(1), at(end, 2), &rows).expect("계획");
            let expected: Vec<_> = open.iter().enumerate().filter(|(_index, (kr, us))| *kr || *us).map(|(index, _)| date(i64::try_from(index).expect("인덱스").checked_add(2).expect("일"))).collect();
            prop_assert_eq!(&plan.dates().collect::<Vec<_>>(), &expected);
            let results = plan.resolve(|_day, _calendar| Err(SnapshotError::MissingPrice));
            prop_assert_eq!(results.iter().map(crate::ValuationDayResult::date).collect::<Vec<_>>(), expected);
            for result in results { prop_assert_eq!(result.result(), Err(&SnapshotError::MissingPrice)); }
        }
    }
    fn extended_only_calendar(market: &Market) -> MarketCalendar {
        let baseline = market_calendar(3, market, Open);
        let information = match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: None,
                after_market: Some(trading(3)),
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: None,
                after_market: Some(trading(3)),
            },
        };
        MarketCalendar::try_new(
            baseline.previous_business_day().clone(),
            MarketSession::try_new(date(3), information).expect("연장 거래"),
            baseline.next_business_day().clone(),
        )
        .expect("달력")
    }

    #[test]
    fn extended_only_and_opposite_holiday_remain_failed_planned_days() {
        let kr_extended = extended_only_calendar(&Market::Kr);
        let us_extended = extended_only_calendar(&Market::Us);
        let kr_closed = market_calendar(3, &Market::Kr, Closed);
        let us_closed = market_calendar(3, &Market::Us, Closed);
        let resolve = |plan: &ValuationGapPlan, currency| {
            plan.resolve(|_, day| {
                day.price_date(currency, at(3, 7))
                    .map(|_| observed(3, "0", "0"))
            })
        };
        for (kr, us, currency) in [
            (&kr_extended, &us_closed, crate::Currency::Krw),
            (&kr_closed, &us_extended, crate::Currency::Usd),
        ] {
            let calendar = crate::SnapshotCalendar::new(kr, us).expect("확인되지 않은 장");
            assert_eq!(
                calendar.price_date(currency, at(3, 7)),
                Err(SnapshotError::MissingPrice)
            );
            let row = ValuationCalendarDay::new(kr, us).expect("평가일");
            let plan = ValuationGapPlan::new(date(2), at(3, 7), &[row]).expect("계획");
            assert_eq!(plan.dates().collect::<Vec<_>>(), vec![date(3)]);
            let results = resolve(&plan, currency);
            assert_eq!(
                results.first().expect("결과").result(),
                Err(&SnapshotError::MissingPrice)
            );
        }
        let control =
            ValuationGapPlan::new(date(2), at(3, 7), &[row(3, Open, Open)]).expect("정규장 계획");
        let successful = resolve(&control, crate::Currency::Krw);
        let snapshot = successful
            .first()
            .expect("정규장 결과")
            .result()
            .expect("완료");
        assert_eq!(snapshot.price_date(crate::Currency::Krw), date(3));
    }
}
