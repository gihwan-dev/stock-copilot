use crate::{
    CalendarDate, CheckResult, EstimateTask, OffsetSeconds, TimeInterval, Timestamp, TradeCostError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommissionValidity {
    start: Option<CalendarDate>,
    end: Option<CalendarDate>,
}
impl CommissionValidity {
    pub fn try_new(
        start: Option<CalendarDate>,
        end: Option<CalendarDate>,
    ) -> Result<Self, TradeCostError> {
        if start.zip(end).is_some_and(|(first, last)| first > last) {
            return Err(TradeCostError::InvalidPeriod);
        }
        Ok(Self { start, end })
    }
    pub(crate) fn covers(self, start: Timestamp, end: Timestamp) -> CheckResult {
        let dates = start
            .date_at_offset(OffsetSeconds::new(32_400))
            .ok()
            .zip(end.date_at_offset(OffsetSeconds::new(32_400)).ok());
        if dates.is_some_and(|(first, last)| {
            self.start.is_none_or(|begin| begin <= first)
                && self.end.is_none_or(|until| last <= until)
        }) {
            CheckResult::Passed
        } else {
            CheckResult::Failed
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateCostPeriod {
    task: EstimateTask,
    interval: TimeInterval,
}
impl EstimateCostPeriod {
    pub fn from_calendar_result(
        task: &EstimateTask,
        maturity: Timestamp,
    ) -> Result<Self, TradeCostError> {
        TimeInterval::try_new(task.created_at(), maturity)
            .map(|interval| Self {
                task: task.clone(),
                interval,
            })
            .map_err(|_error| TradeCostError::InvalidPeriod)
    }
    pub(crate) fn task(&self) -> &EstimateTask {
        &self.task
    }
    pub(crate) fn interval(&self) -> TimeInterval {
        self.interval
    }
}

#[cfg(test)]
mod tests {
    use super::{CommissionValidity, EstimateCostPeriod};
    use crate::{
        CommissionRateFact, CostRate, Market, Ratio, TradeCost, TradeCostError, TradeTaxFact,
    };
    use rust_decimal::Decimal;
    fn day(index: i64) -> crate::CalendarDate {
        crate::candle::tests::at(index)
            .date_at_offset(crate::OffsetSeconds::new(32_400))
            .expect("한국 날짜")
    }
    #[test]
    fn commission_dates_must_cover_creation_through_calendar_maturity() {
        let task = crate::thresholds::tests::task_for(crate::Horizon::new(
            crate::HorizonSelection::HalfYearOrMore,
        ));
        let period = EstimateCostPeriod::from_calendar_result(&task, crate::candle::tests::at(321))
            .expect("반년 과제 만기 사실");
        let zero = CostRate::try_new(Ratio::new(Decimal::ZERO)).expect("요율");
        for (start, end, expected) in [
            (None, None, Ok(())),
            (Some(day(61)), Some(day(321)), Ok(())),
            (Some(day(62)), None, Err(TradeCostError::UnknownComponent)),
            (None, Some(day(200)), Err(TradeCostError::UnknownComponent)),
            (
                Some(day(60)),
                Some(day(320)),
                Err(TradeCostError::UnknownComponent),
            ),
        ] {
            let validity = CommissionValidity::try_new(start, end).expect("적용 기간");
            let result = TradeCost::from_facts(
                Some(CommissionRateFact::from_commissions_result(
                    Market::Kr,
                    zero,
                    validity,
                )),
                Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
                period.clone(),
            );
            assert_eq!(result.map(|_cost| ()), expected);
        }
        assert_eq!(
            CommissionValidity::try_new(Some(day(2)), Some(day(1))),
            Err(TradeCostError::InvalidPeriod)
        );
        assert_eq!(
            EstimateCostPeriod::from_calendar_result(&task, task.created_at()),
            Err(TradeCostError::InvalidPeriod)
        );
    }
}
