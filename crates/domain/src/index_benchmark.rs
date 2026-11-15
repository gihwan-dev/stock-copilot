use crate::{
    BenchmarkKind, BenchmarkSeries, CandlePeriod, Currency, IndicatorCandleStatus, IndicatorCode,
    Market, MarketCalendar, MarketIndicatorCandle, MarketState, OffsetSeconds, PerformanceCurrency,
    PerformanceError, ValuationSnapshot,
};
use std::collections::BTreeMap;

pub fn finalize_index_day(
    mut candle: MarketIndicatorCandle,
    calendar: &MarketCalendar,
) -> Result<MarketIndicatorCandle, PerformanceError> {
    let start = candle.start;
    let date = start
        .date_at_offset(OffsetSeconds::new(32400))
        .or(Err(PerformanceError::InvalidBenchmark))?;
    let session = calendar.today();
    let regular = session
        .sessions()
        .iter()
        .find(|part| part.state() == MarketState::RegularMarket)
        .ok_or(PerformanceError::InvalidBenchmark)?;
    if candle.period() != CandlePeriod::Daily
        || index_kind(candle.code()).is_err()
        || session.market() != Market::Kr
        || session.date() != date
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
    BenchmarkSeries::from_snapshots(kind, currency, snapshots, |snapshot| {
        prices
            .get(&snapshot.price_date(Currency::Krw))
            .copied()
            .ok_or(PerformanceError::BenchmarkMismatch)
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
pub(crate) mod tests {
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
    fn calendar(day: i64) -> crate::MarketCalendar {
        crate::MarketCalendar::try_new(
            session(day.checked_sub(1).expect("전일")),
            session(day),
            session(day.checked_add(1).expect("익일")),
        )
        .expect("달력")
    }
    pub(crate) fn index_candles(
        kind: BenchmarkKind,
        candles: &[crate::Candle],
    ) -> Vec<MarketIndicatorCandle> {
        let code = if kind == BenchmarkKind::Kosdaq {
            "KOSDAQ"
        } else {
            "KOSPI"
        };
        candles
            .iter()
            .map(|bar| {
                let number = |price: crate::Price| {
                    NumericInput::try_new(&price.as_money().amount().as_decimal().to_string())
                        .expect("지수 fixture 값")
                };
                let row = IndicatorCandleRow {
                    timestamp: bar.time().timestamp(),
                    open: number(bar.values().open()),
                    high: number(bar.values().high()),
                    low: number(bar.values().low()),
                    close: number(bar.values().close()),
                    volume: num("7"),
                };
                let mut value = MarketIndicatorCandle::from_response(
                    MasterCode::try_new(code).expect("지수"),
                    bar.time().period(),
                    &row,
                    bar.collected_at(),
                )
                .expect("지수 fixture");
                assert_eq!(bar.status(), crate::CandleStatus::Finalized);
                assert_eq!(bar.values().currency(), Currency::Krw);
                value.status = IndicatorCandleStatus::Finalized;
                value
            })
            .collect()
    }
    pub(crate) fn series_from_candles(
        kind: BenchmarkKind,
        candles: &[crate::Candle],
        snapshots: &[crate::ValuationSnapshot],
        currency: PerformanceCurrency,
    ) -> Result<crate::BenchmarkSeries, PerformanceError> {
        index_benchmark_series(&index_candles(kind, candles), snapshots, currency)
    }
    fn finalized(day: i64, price: &str) -> MarketIndicatorCandle {
        finalize_index_day(
            candle(day, price, "KOSPI", CandlePeriod::Daily),
            &calendar(day),
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
                finalize_index_day(bars[0].clone(), &calendar(2))
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
            &calendar(5),
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
                &calendar(day),
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
                finalize_index_day(bar, &calendar(day)),
                Err(PerformanceError::InvalidBenchmark)
            );
        }
        let raw = candle(20, "100", "KOSPI", CandlePeriod::Daily);
        assert_eq!(
            finalize_index_day(raw, &calendar(20)),
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
            finalize_index_day(
                candle(2, "100", "KOSPI", CandlePeriod::Daily),
                &crate::MarketCalendar::try_new(session(1), closed, session(3)).expect("휴장 달력")
            ),
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
    fn finality_rejects_equal_close_and_accepts_same_day_labels() {
        for (start, collected) in [(at(2, 0), at(2, 1)), (at(2, 0), at(2, 2))] {
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
                finalize_index_day(bar, &calendar(2)),
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
            finalize_index_day(extreme.clone(), &calendar(2)),
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
        #![proptest_config(proptest::test_runner::Config::with_cases(testkit::property_cases()))]
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
        let fixture = crate::valuation_snapshot::tests::fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            crate::SettlementWindow::try_new(date(2), date(4)).expect("기간"),
        );
        let snapshots = [2, 3, 4]
            .map(|day| crate::valuation_snapshot::tests::snapshot(&fixture, day, &[], "1000"));
        let series = BenchmarkSeries::from_snapshots(
            BenchmarkKind::Kospi,
            PerformanceCurrency::ConvertedKrw,
            &snapshots,
            |_| {
                Ok(crate::performance_arithmetic::Number::new(
                    rust_decimal::Decimal::ONE,
                ))
            },
        )
        .expect("시계열");
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

#[cfg(test)]
mod path_regression_tests {
    #[test]
    fn legacy_domestic_candle_path_is_closed() {
        let bar = crate::signal_return_scenarios::tests::bar(
            2,
            0,
            crate::CandlePeriod::Daily,
            "100",
            crate::AdjustmentBasis::new(crate::signal_return_contract::tests::instant(20, 4)),
        );
        for kind in [crate::BenchmarkKind::Kospi, crate::BenchmarkKind::Kosdaq] {
            assert_eq!(
                crate::BenchmarkSeries::from_candles(
                    kind,
                    std::slice::from_ref(&bar),
                    &[],
                    crate::PerformanceCurrency::Native(crate::Currency::Krw)
                ),
                Err(crate::PerformanceError::InvalidBenchmark)
            );
        }
    }
}
#[cfg(test)]
mod review_tests {
    use crate::valuation_snapshot::tests::{at, date};
    use crate::{
        BenchmarkKind, BenchmarkSeries, Candle, CandleObservation, CandlePeriod, CandleStatus,
        CandleTime, Currency, IndicatorCandleRow, IndicatorCandleStatus, MarketCalendar,
        MarketIndicatorCandle, MarketSession, MasterCode, NumericInput, Ohlcv, PerformanceCurrency,
        PerformanceError, SessionData, SessionInformation, SignalBenchmark, SignalReturnError,
        TimeInterval,
    };
    fn num(value: &str) -> NumericInput {
        NumericInput::try_new(value).expect("수치")
    }
    fn row(day: i64, market: crate::KnownMarket, closed: Option<i64>) -> MarketSession {
        let part = |start, end| {
            Some(
                SessionData::try_new(
                    TimeInterval::try_new(at(day, start), at(day, end)).expect("구간"),
                    None,
                )
                .expect("세션"),
            )
        };
        let information = match (market, closed == Some(day)) {
            (crate::KnownMarket::Kr, false) => SessionInformation::Kr {
                pre_market: part(0, 1),
                regular_market: part(1, 2),
                after_market: part(2, 3),
            },
            (crate::KnownMarket::Kr, true) => SessionInformation::Kr {
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
            (crate::KnownMarket::Us, false) => SessionInformation::Us {
                day_market: None,
                pre_market: part(0, 1),
                regular_market: part(1, 2),
                after_market: part(2, 3),
            },
            (crate::KnownMarket::Us, true) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: None,
                after_market: None,
            },
        };
        MarketSession::try_new(date(day), information).expect("시장 세션")
    }
    fn calendar(day: i64, market: crate::KnownMarket, closed: Option<i64>) -> MarketCalendar {
        let previous_day = day.checked_sub(1).expect("전일");
        let next_day = day.checked_add(1).expect("다음날");
        let previous = if Some(previous_day) == closed {
            previous_day.checked_sub(1).expect("휴장 전일")
        } else {
            previous_day
        };
        let next = if Some(next_day) == closed {
            next_day.checked_add(1).expect("휴장 다음날")
        } else {
            next_day
        };
        MarketCalendar::try_new(
            row(previous, market, closed),
            row(day, market, closed),
            row(next, market, closed),
        )
        .expect("달력")
    }
    fn raw(day: i64, hour: i64, collected: crate::Timestamp, close: &str) -> MarketIndicatorCandle {
        raw_at(at(day, hour), collected, close)
    }
    fn raw_at(
        timestamp: crate::Timestamp,
        collected: crate::Timestamp,
        close: &str,
    ) -> MarketIndicatorCandle {
        MarketIndicatorCandle::from_response(
            MasterCode::try_new("KOSPI").expect("지수"),
            CandlePeriod::Daily,
            &IndicatorCandleRow {
                timestamp,
                open: num("99"),
                high: num("900"),
                low: num("1"),
                close: num(close),
                volume: num("17"),
            },
            collected,
        )
        .expect("일봉")
    }
    fn legacy() -> Candle {
        let price = crate::candle::tests::price("100", Currency::Krw);
        Candle::try_new(
            crate::InstrumentId::try_new("index-disguised").expect("ID"),
            CandleTime::try_new(at(2, 0), CandlePeriod::Daily).expect("일봉"),
            crate::PriceAdjustment::Raw,
            Ohlcv::try_new(
                price,
                price,
                price,
                price,
                crate::Quantity::parse(&num("7")).expect("수량"),
            )
            .expect("가격"),
            CandleObservation::new(at(20, 0), CandleStatus::Finalized),
        )
        .expect("종목 봉")
    }
    #[test]
    fn legacy_candle_paths_reject_both_domestic_indices() {
        for kind in [BenchmarkKind::Kospi, BenchmarkKind::Kosdaq] {
            assert_eq!(
                BenchmarkSeries::from_candles(
                    kind,
                    &[legacy()],
                    &[],
                    PerformanceCurrency::Native(Currency::Krw)
                ),
                Err(PerformanceError::InvalidBenchmark)
            );
            assert_eq!(
                SignalBenchmark::from_candles(kind, &[legacy()], &[], &[], Currency::Krw),
                Err(SignalReturnError::BenchmarkMismatch)
            );
        }
    }
    #[test]
    fn regular_close_controls_finality_and_same_day_labels_are_accepted() {
        let kr = calendar(2, crate::KnownMarket::Kr, None);
        assert_eq!(
            crate::finalize_index_day(raw(2, 0, at(2, 1), "100"), &kr),
            Err(PerformanceError::InvalidBenchmark)
        );
        assert_eq!(
            crate::finalize_index_day(raw(2, 0, at(2, 2), "100"), &kr),
            Err(PerformanceError::InvalidBenchmark)
        );
        for hour in [0, 1, 2, 3, 10] {
            assert_eq!(
                crate::finalize_index_day(raw(2, hour, at(20, 0), "100"), &kr)
                    .expect("같은 거래일 표기")
                    .status(),
                IndicatorCandleStatus::Finalized
            );
        }
        assert_eq!(
            crate::finalize_index_day(raw(2, 0, at(2, 3), "100"), &kr)
                .expect("시간외 종료 이전 수집도 정규장 뒤")
                .status(),
            IndicatorCandleStatus::Finalized
        );
        let us = calendar(2, crate::KnownMarket::Us, None);
        assert_eq!(
            crate::finalize_index_day(raw(2, 0, at(20, 0), "100"), &us),
            Err(PerformanceError::InvalidBenchmark)
        );
        assert_eq!(
            crate::finalize_index_day(
                raw(2, 0, at(20, 0), "100"),
                &calendar(3, crate::KnownMarket::Kr, None)
            ),
            Err(PerformanceError::InvalidBenchmark)
        );
        assert_eq!(
            crate::finalize_index_day(
                raw(2, 0, at(20, 0), "100"),
                &calendar(2, crate::KnownMarket::Kr, Some(2))
            ),
            Err(PerformanceError::InvalidBenchmark)
        );
    }
    fn holiday_fixture() -> (
        Vec<MarketCalendar>,
        Vec<crate::ValuationSnapshot>,
        [MarketIndicatorCandle; 3],
    ) {
        let calendars = (2..=5)
            .map(|day| calendar(day, crate::KnownMarket::Kr, Some(3)))
            .collect::<Vec<_>>();
        let snapshots = (2..=5)
            .map(|day| {
                let kr = calendar(day, crate::KnownMarket::Kr, Some(3));
                let us = calendar(day, crate::KnownMarket::Us, Some(4));
                let dates = crate::SnapshotCalendar::new(&kr, &us).expect("혼합 휴장 달력");
                let now = at(day, 7);
                let portfolio = crate::Portfolio::new(
                    vec![],
                    crate::BuyingPower::new(
                        crate::candle::tests::price("100", Currency::Krw).as_money(),
                        now,
                    ),
                    crate::BuyingPower::new(
                        crate::candle::tests::price("1", Currency::Usd).as_money(),
                        now,
                    ),
                )
                .expect("잔고");
                crate::ValuationSnapshot::observe(&dates, &portfolio.evaluate(None), now, None)
                    .expect("평가")
            })
            .collect::<Vec<_>>();
        let bars = [(2, "100"), (4, "120"), (5, "150")].map(|(day, value)| {
            crate::finalize_index_day(
                raw(day, 2, at(20, 0), value),
                &calendar(day, crate::KnownMarket::Kr, Some(3)),
            )
            .expect("지수 확정")
        });
        (calendars, snapshots, bars)
    }
    #[test]
    fn holiday_carry_uses_krw_price_dates_and_preserves_settlement_dates() {
        let (calendars, snapshots, bars) = holiday_fixture();
        let kr_dates = snapshots
            .iter()
            .map(|snapshot| snapshot.price_date(Currency::Krw))
            .collect::<Vec<_>>();
        let us_dates = snapshots
            .iter()
            .map(|snapshot| snapshot.price_date(Currency::Usd))
            .collect::<Vec<_>>();
        assert_eq!(kr_dates, vec![date(2), date(2), date(4), date(5)]);
        assert_eq!(us_dates, vec![date(2), date(3), date(3), date(5)]);
        let series = crate::index_benchmark_series(
            &bars,
            &snapshots,
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("국내 비교");
        assert_eq!(
            series
                .observations()
                .map(|observation| (
                    observation.start(),
                    observation.end(),
                    observation.value().to_string()
                ))
                .collect::<Vec<_>>(),
            vec![
                (date(2), date(3), "0.000000".into()),
                (date(3), date(4), "0.200000".into()),
                (date(4), date(5), "0.250000".into())
            ]
        );
        let mut reversed = snapshots.clone();
        reversed.reverse();
        assert_eq!(
            crate::index_benchmark_series(
                &bars,
                &reversed,
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Ok(series.clone())
        );
        let signal = SignalBenchmark::from_index_candles(&bars, &snapshots, &calendars)
            .expect("국내 신호 경로");
        assert_eq!(signal.series(), &series);
        assert_eq!(
            signal
                .between_dates(date(2), date(5), Currency::Krw)
                .expect("휴장 포함 연결")
                .to_string(),
            "0.500000"
        );
        assert_eq!(
            signal
                .between(at(2, 2), at(5, 2), Currency::Krw)
                .expect("정규장 종가 구간")
                .to_string(),
            "0.500000"
        );
    }
    #[test]
    fn unconfirmed_index_candles_cannot_form_a_signal() {
        let (calendars, snapshots, _bars) = holiday_fixture();
        let raw_bars = [raw(2, 2, at(20, 0), "100"), raw(5, 2, at(20, 0), "150")];
        assert_eq!(
            SignalBenchmark::from_index_candles(&raw_bars, &snapshots, &calendars),
            Err(SignalReturnError::BenchmarkMismatch)
        );
    }
    #[test]
    fn etf_candle_path_requires_usd_even_with_adjusted_prices() {
        let original = legacy();
        let adjusted = Candle::try_new(
            original.instrument().clone(),
            original.time(),
            crate::PriceAdjustment::Adjusted(crate::AdjustmentBasis::new(at(20, 0))),
            original.values(),
            CandleObservation::new(at(20, 0), CandleStatus::Finalized),
        )
        .expect("조정 봉");
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Sp500Etf,
                &[adjusted],
                &[],
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Err(PerformanceError::InvalidBenchmark)
        );
    }
    #[test]
    fn same_trading_day_uses_exact_kst_midnight_boundaries() {
        let market = calendar(2, crate::KnownMarket::Kr, None);
        let last = at(2, 14)
            .checked_add(
                crate::TimeSpan::try_new(crate::SpanSeconds::new(3599), crate::Nanosecond::new(0))
                    .expect("자정 직전"),
            )
            .expect("시각");
        for timestamp in [at(1, 15), last] {
            assert_eq!(
                crate::finalize_index_day(raw_at(timestamp, at(20, 0), "100"), &market)
                    .expect("거래일 내부")
                    .status(),
                IndicatorCandleStatus::Finalized
            );
        }
        for timestamp in [
            at(1, 15)
                .checked_sub(crate::TimeSpan::NANOSECOND)
                .expect("전일"),
            at(2, 15),
        ] {
            assert_eq!(
                crate::finalize_index_day(raw_at(timestamp, at(20, 0), "100"), &market),
                Err(PerformanceError::InvalidBenchmark)
            );
        }
    }
}
