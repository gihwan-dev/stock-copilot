use super::{UiPerformanceSpan, performance_view};

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
