use super::{UiPerformanceSpan, UiPerformanceStatus, performance_view, period_line};
use crate::performance_book::PeriodReading;
use application::PerformanceRunError;
use domain::{PerformanceError, PerformanceSpan};

#[test]
fn every_period_is_listed_even_before_the_first_calculation() {
    let view = performance_view(None);
    crate::ipc::tests::assert_response_snapshot("get_performance", &view);
    let spans: Vec<UiPerformanceSpan> = view.lines.iter().map(|line| line.span).collect();
    assert_eq!(
        spans,
        [
            UiPerformanceSpan::OneMonth,
            UiPerformanceSpan::ThreeMonths,
            UiPerformanceSpan::SixMonths,
            UiPerformanceSpan::OneYear,
        ]
    );
    assert!(view.lines.iter().all(|line| line.period_return.is_none()));
}

#[test]
fn the_six_month_failed_result_keeps_its_reason_and_has_no_number() {
    for (error, status) in [
        (
            PerformanceRunError::NoValuedDays,
            UiPerformanceStatus::InsufficientData,
        ),
        (
            PerformanceRunError::Performance(PerformanceError::MissingFx),
            UiPerformanceStatus::MissingFx,
        ),
        (
            PerformanceRunError::Performance(PerformanceError::NonPositiveCapital),
            UiPerformanceStatus::NonPositiveCapital,
        ),
    ] {
        let item = period_line(
            PerformanceSpan::SixMonths,
            Some(PeriodReading::Finished(Err(error))),
        );
        assert_eq!(item.status, status);
        assert_eq!(item.period_return, None);
    }
}
