#[cfg(test)]
mod tests {
    use crate::signal_return_contract::tests::{instant, key};
    use crate::signal_return_scenarios::tests::sample;
    use crate::{BootstrapInterval, BootstrapSeed, WithheldCount, WithheldReason};
    #[test]
    fn review_cost_market_mismatch_is_rejected_even_when_task_and_maturity_match() {
        let source = crate::signal_return_scenarios::tests::task(0);
        let zero = crate::CostRate::try_new(
            crate::Ratio::parse(&crate::NumericInput::try_new("0").expect("영")).expect("비율"),
        )
        .expect("요율");
        let cost = crate::signal_return_cost(
            Some(crate::CommissionRateFact::from_commissions_result(
                crate::Market::Us,
                zero,
                crate::CommissionValidity::try_new(None, None).expect("기간"),
            )),
            Some(crate::TradeTaxFact::from_schedule_result(
                crate::Market::Us,
                zero,
                zero,
            )),
            crate::EstimateCostPeriod::from_calendar_result(&source, instant(67, 4)).expect("만기"),
        )
        .expect("미국 비용 사실");
        let exit = crate::virtual_exit::tests::virtual_exit_fixture_fill(
            &source,
            crate::candle::tests::price("100", crate::Currency::Krw),
            instant(67, 4),
        );
        let benchmark = crate::signal_return_scenarios::tests::benchmark(
            0,
            "100",
            crate::AdjustmentBasis::new(instant(67, 4)),
        );
        assert_eq!(
            crate::signal_return(&source, exit, &cost, &benchmark),
            Err(crate::SignalReturnError::Cost)
        );
    }
    #[test]
    fn review_one_missing_comparison_withholds_thirty_available_positive_samples() {
        let pairs = (0_i64..31)
            .map(|index| sample(index.checked_mul(7).expect("비중첩"), "99"))
            .collect::<Vec<_>>();
        let records = pairs
            .iter()
            .map(|(record, _)| record.clone())
            .collect::<Vec<_>>();
        let mut values = pairs
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        values.pop().expect("비교 불가 한 건");
        let edge = crate::scorecard_edge(
            &key(),
            &records,
            &values,
            instant(500, 4),
            BootstrapSeed::new(42),
        )
        .expect("보류 결과");
        assert_eq!(
            edge.confidence(),
            &BootstrapInterval::Withheld(
                WithheldReason::IncomparableSamples,
                WithheldCount::new(1)
            )
        );
        assert_eq!(edge.unavailable().as_usize(), 1);
        assert_eq!(edge.confidence().clone().into_ready(), None);
    }
    #[test]
    fn review_missing_overlapping_judged_signal_still_withholds() {
        let shifts = std::iter::once(1_i64)
            .chain((0_i64..30).map(|index| index.checked_mul(7).expect("간격")));
        let pairs = shifts.map(|shift| sample(shift, "99")).collect::<Vec<_>>();
        let records = pairs
            .iter()
            .map(|(record, _)| record.clone())
            .collect::<Vec<_>>();
        let values = pairs
            .into_iter()
            .skip(1)
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        let edge = crate::scorecard_edge(
            &key(),
            &records,
            &values,
            instant(500, 4),
            BootstrapSeed::new(42),
        )
        .expect("겹치는 미비교 표본");
        assert_eq!(
            edge.confidence(),
            &BootstrapInterval::Withheld(
                WithheldReason::IncomparableSamples,
                WithheldCount::new(1)
            )
        );
    }
}
