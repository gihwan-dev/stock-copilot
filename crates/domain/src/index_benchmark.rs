use crate::{
    BenchmarkKind, BenchmarkSeries, CandlePeriod, Currency, IndicatorCandleStatus, IndicatorCode,
    Market, MarketIndicatorCandle, MarketSession, MarketState, OffsetSeconds, PerformanceCurrency,
    PerformanceError, ValuationSnapshot,
};
use std::collections::BTreeMap;

pub fn finalize_index_day(
    mut candle: MarketIndicatorCandle,
    session: &MarketSession,
) -> Result<MarketIndicatorCandle, PerformanceError> {
    let start = candle.start;
    let date = start
        .date_at_offset(OffsetSeconds::new(32400))
        .or(Err(PerformanceError::InvalidBenchmark))?;
    let regular = session
        .sessions()
        .iter()
        .find(|part| part.state() == MarketState::RegularMarket)
        .ok_or(PerformanceError::InvalidBenchmark)?;
    if candle.period() != CandlePeriod::Daily
        || index_kind(candle.code()).is_err()
        || session.market() != Market::Kr
        || session.date() != date
        || start > regular.session().interval().start()
        || regular.session().interval().end() >= candle.observation().collected_at()
    {
        return Err(PerformanceError::InvalidBenchmark);
    }
    candle.status = IndicatorCandleStatus::Finalized;
    Ok(candle)
}

pub fn index_benchmark_series(
    candles: &[MarketIndicatorCandle],
    snapshots: &[ValuationSnapshot],
    currency: PerformanceCurrency,
) -> Result<BenchmarkSeries, PerformanceError> {
    if candles.len() > 4097 || snapshots.len() > 4097 {
        return Err(PerformanceError::TooManyInputs);
    }
    let first = candles.first().ok_or(PerformanceError::InvalidBenchmark)?;
    let kind = index_kind(first.code())?;
    if currency == PerformanceCurrency::Native(Currency::Usd) {
        return Err(PerformanceError::BenchmarkMismatch);
    }
    let prices = index_prices(candles, first)?;
    let mut ordered = snapshots.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|snapshot| snapshot.valuation_at());
    if ordered.len() < 2 {
        return Err(PerformanceError::InvalidPeriod);
    }
    let mut dates = Vec::new();
    let mut values = Vec::new();
    for snapshot in ordered {
        dates.push(snapshot.settlement_date());
        values.push(
            *prices
                .get(&snapshot.price_date(Currency::Krw))
                .ok_or(PerformanceError::BenchmarkMismatch)?,
        );
    }
    if dates
        .iter()
        .zip(dates.iter().skip(1))
        .any(|(old, new)| old >= new)
    {
        return Err(PerformanceError::InvalidPeriod);
    }
    let returns = values
        .iter()
        .zip(values.iter().skip(1))
        .map(|(old, new)| crate::performance_arithmetic::price_return(*old, *new))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BenchmarkSeries {
        kind,
        currency,
        dates,
        returns,
    })
}

fn index_prices(
    candles: &[MarketIndicatorCandle],
    first: &MarketIndicatorCandle,
) -> Result<BTreeMap<crate::CalendarDate, crate::performance_arithmetic::Number>, PerformanceError>
{
    let mut prices = BTreeMap::new();
    for candle in candles {
        let close = crate::market_index::index_close_number(candle)?;
        if candle.code() != first.code()
            || candle.period() != CandlePeriod::Daily
            || candle.status() != IndicatorCandleStatus::Finalized
        {
            return Err(PerformanceError::InvalidBenchmark);
        }
        let date = candle
            .start
            .date_at_offset(OffsetSeconds::new(32400))
            .or(Err(PerformanceError::InvalidBenchmark))?;
        if prices.insert(date, close).is_some() {
            return Err(PerformanceError::InvalidBenchmark);
        }
    }
    Ok(prices)
}
fn index_kind(code: IndicatorCode) -> Result<BenchmarkKind, PerformanceError> {
    match code {
        IndicatorCode::Kospi => Ok(BenchmarkKind::Kospi),
        IndicatorCode::Kosdaq => Ok(BenchmarkKind::Kosdaq),
        IndicatorCode::KrBond2Y
        | IndicatorCode::KrBond3Y
        | IndicatorCode::KrBond5Y
        | IndicatorCode::KrBond10Y
        | IndicatorCode::KrBond20Y
        | IndicatorCode::KrBond30Y
        | IndicatorCode::Unknown => Err(PerformanceError::InvalidBenchmark),
    }
}
pub(crate) fn validate_signal_benchmark_length(
    series: &BenchmarkSeries,
    points: &[crate::Timestamp],
) -> Result<(), crate::SignalReturnError> {
    if series.observations().count().saturating_add(1) != points.len() {
        return Err(crate::SignalReturnError::BenchmarkMismatch);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{finalize_index_day, index_benchmark_series};
    use crate::valuation_snapshot::tests::{at, date, fixture_window, snapshot};
    use crate::{
        BenchmarkKind, CandlePeriod, Currency, IndicatorCandleStatus, MarketIndicatorCandle,
        MarketSession, PerformanceCurrency, PerformanceError,
    };
    use crate::{
        IndicatorCandleRow, MasterCode, NumericInput, SessionData, SessionInformation,
        SettlementWindow, TimeInterval,
    };
    fn num(text: &str) -> NumericInput {
        NumericInput::try_new(text).expect("입력")
    }
    fn candle(day: i64, price: &str, code: &str, period: CandlePeriod) -> MarketIndicatorCandle {
        MarketIndicatorCandle::from_response(
            MasterCode::try_new(code).expect("코드"),
            period,
            &IndicatorCandleRow {
                timestamp: at(day, 0),
                open: num("100"),
                high: num("79228162514264337593543950335"),
                low: num("0.0000000000000000000000000001"),
                close: num(price),
                volume: num("7"),
            },
            at(20, 0),
        )
        .expect("봉")
    }
    fn session(day: i64) -> MarketSession {
        MarketSession::try_new(
            date(day),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(
                    SessionData::try_new(
                        TimeInterval::try_new(at(day, 1), at(day, 2)).expect("구간"),
                        None,
                    )
                    .expect("정규장"),
                ),
                after_market: None,
            },
        )
        .expect("장")
    }
    fn finalized(day: i64, price: &str) -> MarketIndicatorCandle {
        finalize_index_day(
            candle(day, price, "KOSPI", CandlePeriod::Daily),
            &session(day),
        )
        .expect("확정")
    }
    #[test]
    fn index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs() {
        let fixture = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&fixture, 2, &[], "1000"),
            snapshot(&fixture, 5, &[], "1000"),
        ];
        let bars = [finalized(2, "2000"), finalized(5, "2200")];
        for currency in [
            PerformanceCurrency::Native(Currency::Krw),
            PerformanceCurrency::ConvertedKrw,
        ] {
            let series = index_benchmark_series(&bars, &snapshots, currency).expect("시리즈");
            assert_eq!(series.kind(), BenchmarkKind::Kospi);
            assert_eq!(series.currency(), currency);
            let point = series.observations().next().expect("관측");
            assert_eq!(point.start(), date(2));
            assert_eq!(point.end(), date(5));
            assert_eq!(point.value().to_string(), "0.100000");
            assert_eq!(
                finalize_index_day(bars[0].clone(), &session(2))
                    .expect("멱등")
                    .status(),
                IndicatorCandleStatus::Finalized
            );
        }
    }
    #[test]
    fn index_path_rejects_invalid_sources() {
        let fixture = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&fixture, 2, &[], "1000"),
            snapshot(&fixture, 5, &[], "1000"),
        ];
        let bars = [finalized(2, "2000"), finalized(5, "2200")];
        let err = PerformanceError::InvalidBenchmark;
        for bad in [
            vec![],
            vec![bars[0].clone(); 2],
            vec![candle(2, "2000", "KOSPI", CandlePeriod::Daily)],
            vec![candle(2, "2000", "KOSPI", CandlePeriod::Minute)],
            vec![candle(2, "2", "KR_BOND_2Y", CandlePeriod::Daily)],
            vec![bars[0].clone(), {
                let mut bar = bars[1].clone();
                bar.status = IndicatorCandleStatus::InProgress;
                bar
            }],
        ] {
            assert_eq!(
                index_benchmark_series(&bad, &snapshots, PerformanceCurrency::ConvertedKrw),
                Err(err)
            );
        }
        let other = finalize_index_day(
            candle(5, "2300", "KOSDAQ", CandlePeriod::Daily),
            &session(5),
        )
        .expect("코스닥");
        assert_eq!(
            index_benchmark_series(
                &[bars[0].clone(), other.clone()],
                &snapshots,
                PerformanceCurrency::ConvertedKrw
            ),
            Err(err)
        );
        assert_eq!(
            index_benchmark_series(&[other], &snapshots, PerformanceCurrency::ConvertedKrw),
            Err(PerformanceError::BenchmarkMismatch)
        );
        assert_eq!(
            index_benchmark_series(
                &bars,
                &snapshots,
                PerformanceCurrency::Native(Currency::Usd)
            ),
            Err(PerformanceError::BenchmarkMismatch)
        );
    }
    #[test]
    fn index_path_rejects_invalid_period_and_limits() {
        let fixture = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&fixture, 2, &[], "1000"),
            snapshot(&fixture, 5, &[], "1000"),
        ];
        let bars = [finalized(2, "2000"), finalized(5, "2200")];
        assert_eq!(
            index_benchmark_series(&bars, &[], PerformanceCurrency::ConvertedKrw),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            index_benchmark_series(
                &bars,
                &vec![snapshots[0].clone(); 2],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            index_benchmark_series(
                &vec![bars[0].clone(); 4098],
                &snapshots,
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::TooManyInputs)
        );
        assert_eq!(
            index_benchmark_series(
                &bars,
                &vec![snapshots[0].clone(); 4098],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::TooManyInputs)
        );
        let kosdaq = [2, 5].map(|day| {
            finalize_index_day(
                candle(day, "2000", "KOSDAQ", CandlePeriod::Daily),
                &session(day),
            )
            .expect("확정")
        });
        assert_eq!(
            index_benchmark_series(&kosdaq, &snapshots, PerformanceCurrency::ConvertedKrw)
                .expect("코스닥")
                .kind(),
            BenchmarkKind::Kosdaq
        );
    }
    #[test]
    fn finality_requires_matching_regular_session_strictly_before_collection() {
        for (bar, day) in [
            (candle(2, "100", "KOSPI", CandlePeriod::Daily), 3),
            (candle(2, "100", "KOSPI", CandlePeriod::Minute), 2),
            (candle(2, "2", "KR_BOND_2Y", CandlePeriod::Daily), 2),
        ] {
            assert_eq!(
                finalize_index_day(bar, &session(day)),
                Err(PerformanceError::InvalidBenchmark)
            );
        }
        let raw = candle(20, "100", "KOSPI", CandlePeriod::Daily);
        assert_eq!(
            finalize_index_day(raw, &session(20)),
            Err(PerformanceError::InvalidBenchmark)
        );
        let closed = MarketSession::try_new(
            date(2),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
        )
        .expect("휴장");
        assert_eq!(
            finalize_index_day(candle(2, "100", "KOSPI", CandlePeriod::Daily), &closed),
            Err(PerformanceError::InvalidBenchmark)
        );
    }
    #[test]
    fn index_rejects_a_single_snapshot() {
        let fixture = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        assert_eq!(
            index_benchmark_series(
                &[finalized(2, "100")],
                &[snapshot(&fixture, 2, &[], "1000")],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            crate::market_index::index_close_number(&candle(
                2,
                "2",
                "KR_BOND_2Y",
                CandlePeriod::Daily
            )),
            Err(PerformanceError::InvalidBenchmark)
        );
    }
    #[test]
    fn finality_rejects_equal_close_and_late_bar_start() {
        for (start, collected) in [(at(2, 0), at(2, 2)), (at(2, 3), at(20, 0))] {
            let row = IndicatorCandleRow {
                timestamp: start,
                open: num("100"),
                high: num("100"),
                low: num("100"),
                close: num("100"),
                volume: num("1"),
            };
            let bar = MarketIndicatorCandle::from_response(
                MasterCode::try_new("KOSPI").expect("지수"),
                CandlePeriod::Daily,
                &row,
                collected,
            )
            .expect("봉");
            assert_eq!(
                finalize_index_day(bar, &session(2)),
                Err(PerformanceError::InvalidBenchmark)
            );
        }
    }
    #[test]
    fn index_range_and_arithmetic_errors_are_closed() {
        let maximum = crate::Timestamp::from_parts(
            crate::UnixSeconds::new(253_402_300_799),
            crate::Nanosecond::new(0),
            crate::OffsetSeconds::new(0),
        )
        .expect("최대 시각");
        let row = IndicatorCandleRow {
            timestamp: maximum,
            open: num("100"),
            high: num("100"),
            low: num("100"),
            close: num("100"),
            volume: num("1"),
        };
        let mut extreme = MarketIndicatorCandle::from_response(
            MasterCode::try_new("KOSPI").expect("지수"),
            CandlePeriod::Daily,
            &row,
            maximum,
        )
        .expect("봉");
        assert_eq!(
            finalize_index_day(extreme.clone(), &session(2)),
            Err(PerformanceError::InvalidBenchmark)
        );
        extreme.status = IndicatorCandleStatus::Finalized;
        assert_eq!(
            index_benchmark_series(&[extreme], &[], PerformanceCurrency::ConvertedKrw),
            Err(PerformanceError::InvalidBenchmark)
        );
        assert_eq!(
            index_benchmark_series(
                &[
                    finalized(2, "100"),
                    candle(5, "2", "KR_BOND_2Y", CandlePeriod::Daily)
                ],
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::InvalidBenchmark)
        );
        let fixture = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&fixture, 2, &[], "1000"),
            snapshot(&fixture, 5, &[], "1000"),
        ];
        let bars = [
            finalized(2, "0.0000000000000000000000000001"),
            finalized(5, "79228162514264337593543950335"),
        ];
        assert_eq!(
            index_benchmark_series(&bars, &snapshots, PerformanceCurrency::ConvertedKrw),
            Err(PerformanceError::Arithmetic)
        );
    }
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(4096))]
        #[test]
        fn index_returns_match_independent_integer_ratio(old in 1_u32..10000, new in 1_u32..10000) {
            let fixture=fixture_window(vec![],("100","100"),("100","100"),SettlementWindow::try_new(date(2),date(5)).expect("기간"));
            let snapshots=[snapshot(&fixture,2,&[],"1000"),snapshot(&fixture,5,&[],"1000")];
            let bars=[finalized(2,&old.to_string()),finalized(5,&new.to_string())];
            let series=index_benchmark_series(&bars,&snapshots,PerformanceCurrency::ConvertedKrw).expect("시리즈");
            let actual=crate::Ratio::parse(&num(&series.observations().next().expect("관측").value().to_string())).expect("표시").as_decimal();
            let expected=rust_decimal::Decimal::from(new).checked_sub(rust_decimal::Decimal::from(old)).expect("차").checked_div(rust_decimal::Decimal::from(old)).expect("몫");
            proptest::prop_assert!(actual.checked_sub(expected).expect("오차").abs()<=rust_decimal::Decimal::new(5,7));
        }
    }
}

#[cfg(test)]
mod length_tests {
    use super::validate_signal_benchmark_length;
    use crate::valuation_snapshot::tests::{at, date};
    use crate::{BenchmarkKind, BenchmarkSeries, PerformanceCurrency};
    #[test]
    fn signal_zip_rejects_both_shorter_and_longer_point_lists() {
        let zero = crate::PerformanceValue::from_number(
            crate::performance_arithmetic::Number::new(rust_decimal::Decimal::ZERO),
        );
        let series = BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: PerformanceCurrency::ConvertedKrw,
            dates: vec![date(2), date(3), date(4)],
            returns: vec![zero; 2],
        };
        for count in [0, 1, 2, 4] {
            assert_eq!(
                validate_signal_benchmark_length(&series, &vec![at(2, 0); count]),
                Err(crate::SignalReturnError::BenchmarkMismatch)
            );
        }
        assert_eq!(
            validate_signal_benchmark_length(&series, &[at(2, 0); 3]),
            Ok(())
        );
    }
}
