use super::{
    BacktestError, BacktestHorizon, BaseProbabilityBacktest, Outcome, historical_brier, one_third,
};
use crate::candle::tests::{at, price, series};
use crate::{Candle, RiskAppetite, VolatilityError};
use crate::{CandleObservation, CandleStatus, Currency, Ohlcv};
use rust_decimal::Decimal;

fn prices(history: usize, tail: &[&str]) -> Vec<Candle> {
    let mut values: Vec<_> = ["100", "102"].into_iter().cycle().take(history).collect();
    values.extend_from_slice(tail);
    series(&values)
}

fn run(candles: &[Candle]) -> BaseProbabilityBacktest {
    BaseProbabilityBacktest::evaluate(candles, BacktestHorizon::FewDays, RiskAppetite::Neutral)
        .expect("일봉 평가")
}

#[test]
fn one_prediction_uses_sixty_returns_and_five_subsequent_bars() {
    let report = run(&prices(61, &["110", "60", "100", "100", "100"]));
    assert_eq!(
        (report.evaluated.0, report.hits.0, report.stops.0),
        (1, 1, 0)
    );
    let summary = report.summary.expect("표본 1개");
    assert_eq!(summary.hit_rate.0, Decimal::ONE);
    let error = Decimal::ONE
        .checked_sub(summary.mean_probability.0)
        .expect("오차");
    assert_eq!(summary.brier.0, error.checked_mul(error).expect("제곱"));
    assert_eq!(format!("{:.4}", summary.reference_brier.0), "0.4444");
}

#[test]
fn changing_label_days_does_not_change_forecast_and_incomplete_horizons_are_excluded() {
    let first = run(&prices(61, &["110", "60", "100", "100", "100"]));
    let second = run(&prices(
        61,
        &["60", "110", "100", "100", "100", "90", "90", "90", "90"],
    ));
    assert_eq!(
        first.summary.expect("표본").mean_probability,
        second.summary.expect("표본").mean_probability
    );
    assert_eq!(
        (second.evaluated.0, second.hits.0, second.stops.0),
        (1, 0, 1)
    );
    assert_eq!(run(&prices(61, &["100"; 10])).evaluated.0, 2);
}

#[test]
fn expiry_and_two_barriers_on_one_day_are_misses_and_flat_history_is_skipped() {
    let candles = prices(61, &["100"; 5]);
    assert_eq!(run(&candles).expired.0, 1);
    let mut ambiguous = candles;
    let day = ambiguous.get_mut(61).expect("첫 미래 봉");
    *day = Candle::try_new(
        day.instrument().clone(),
        day.time(),
        day.adjustment(),
        Ohlcv::try_new(
            day.values().open(),
            price("120", Currency::Usd),
            price("80", Currency::Usd),
            day.values().close(),
            day.values().volume(),
        )
        .expect("일중 두 경계 통과"),
        CandleObservation::new(at(10_001), CandleStatus::Finalized),
    )
    .expect("일봉");
    let report = run(&ambiguous);
    assert_eq!(
        (report.ambiguous.0, report.summary.expect("표본").hit_rate.0),
        (1, Decimal::ZERO)
    );
    let flat = run(&series(&["100"; 66]));
    assert_eq!((flat.skipped.0, flat.summary), (1, None));
}

#[test]
fn expanding_hit_rate_scores_before_observing_each_new_label() {
    let outcomes = [Outcome::Hit, Outcome::Stop, Outcome::Hit];
    let brier = historical_brier(outcomes.into_iter(), one_third().expect("초기 기준"))
        .expect("과거 적중률");
    assert_eq!(format!("{:.4}", brier.0), "0.5648");
}

#[test]
fn incomplete_periods_and_invalid_date_order_never_enter_the_denominator() {
    let insufficient = run(&prices(61, &["100"; 4]));
    assert_eq!((insufficient.evaluated.0, insufficient.summary), (0, None));
    let mut invalid = prices(61, &["100"; 5]);
    let prior = invalid.get(63).expect("이전 관측").clone();
    *invalid.get_mut(64).expect("중복 날짜") = prior;
    assert_eq!(
        BaseProbabilityBacktest::evaluate(
            &invalid,
            BacktestHorizon::FewDays,
            RiskAppetite::Neutral
        ),
        Err(BacktestError::Series(VolatilityError::Unordered))
    );
}

#[test]
fn longer_policy_horizons_use_completed_non_overlapping_windows() {
    for (horizon, length) in [
        (BacktestHorizon::WeeksToMonths, 40_usize),
        (BacktestHorizon::HalfYearOrMore, 126),
    ] {
        let series = prices(
            61_usize
                .saturating_add(length.saturating_mul(3))
                .saturating_sub(1),
            &[],
        );
        let report = BaseProbabilityBacktest::evaluate(&series, horizon, RiskAppetite::Neutral)
            .expect("완료 구간");
        assert_eq!(report.days.0, length);
        assert_eq!(report.evaluated.0, 2);
        assert_eq!(report.bins.iter().map(|bin| bin.count.0).sum::<usize>(), 2);
    }
}
