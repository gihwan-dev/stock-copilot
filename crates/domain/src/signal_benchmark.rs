use crate::{
    BenchmarkKind, BenchmarkSeries, Candle, Currency, Market, MarketCalendar, MarketState,
    PerformanceCurrency, PerformanceValue, SignalReturnError, Timestamp, ValuationSnapshot,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct BenchmarkSegment {
    start: Timestamp,
    end: Timestamp,
    value: PerformanceValue,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalBenchmark {
    series: BenchmarkSeries,
    segments: Vec<BenchmarkSegment>,
    dates: std::collections::BTreeMap<crate::CalendarDate, Timestamp>,
}
impl SignalBenchmark {
    pub fn from_candles(
        kind: BenchmarkKind,
        candles: &[Candle],
        snapshots: &[ValuationSnapshot],
        calendars: &[MarketCalendar],
        currency: Currency,
    ) -> Result<Self, SignalReturnError> {
        if calendars.len() > 4097 {
            return Err(SignalReturnError::TooManyInputs);
        }
        let series = BenchmarkSeries::from_candles(
            kind,
            candles,
            snapshots,
            PerformanceCurrency::Native(currency),
        )
        .map_err(|_error| SignalReturnError::BenchmarkMismatch)?;
        let closes = native_closes(calendars, currency)?;
        let mut ordered = snapshots.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|snapshot| snapshot.valuation_at());
        let points = ordered
            .iter()
            .map(|snapshot| {
                closes
                    .get(&snapshot.price_date(currency))
                    .copied()
                    .ok_or(SignalReturnError::BenchmarkMismatch)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if points
            .iter()
            .zip(points.iter().skip(1))
            .any(|(start, end)| start > end)
        {
            return Err(SignalReturnError::BenchmarkMismatch);
        }
        let segments = points
            .iter()
            .zip(points.iter().skip(1))
            .zip(series.observations())
            .map(|((start, end), value)| BenchmarkSegment {
                start: *start,
                end: *end,
                value: value.value(),
            })
            .collect();
        let dates = ordered
            .iter()
            .zip(&points)
            .map(|(snapshot, at)| (snapshot.price_date(currency), *at))
            .collect();
        Ok(Self {
            series,
            segments,
            dates,
        })
    }
    #[must_use]
    pub const fn series(&self) -> &BenchmarkSeries {
        &self.series
    }
    pub(crate) fn between_dates(
        &self,
        start: crate::CalendarDate,
        end: crate::CalendarDate,
        currency: Currency,
    ) -> Result<PerformanceValue, SignalReturnError> {
        if self.series.currency() != PerformanceCurrency::Native(currency) {
            return Err(SignalReturnError::BenchmarkMismatch);
        }
        let first = self
            .dates
            .get(&start)
            .copied()
            .ok_or(SignalReturnError::BenchmarkMismatch)?;
        let last = self
            .dates
            .get(&end)
            .copied()
            .ok_or(SignalReturnError::BenchmarkMismatch)?;
        if start == end {
            return Ok(PerformanceValue::from_number(
                crate::performance_arithmetic::Number::new(rust_decimal::Decimal::ZERO),
            ));
        }
        self.between(first, last, currency)
    }
    pub(crate) fn between(
        &self,
        start: Timestamp,
        end: Timestamp,
        currency: Currency,
    ) -> Result<PerformanceValue, SignalReturnError> {
        if self.series.currency() != PerformanceCurrency::Native(currency) {
            return Err(SignalReturnError::BenchmarkMismatch);
        }
        let selected = self
            .segments
            .iter()
            .filter(|segment| segment.start >= start && segment.end <= end)
            .collect::<Vec<_>>();
        if selected.first().map(|segment| segment.start) != Some(start)
            || selected.last().map(|segment| segment.end) != Some(end)
        {
            return Err(SignalReturnError::BenchmarkMismatch);
        }
        crate::performance_arithmetic::linked(
            &selected
                .iter()
                .map(|segment| segment.value)
                .collect::<Vec<_>>(),
        )
        .map_err(|_error| SignalReturnError::Arithmetic)
    }
}

fn native_closes(
    calendars: &[MarketCalendar],
    currency: Currency,
) -> Result<std::collections::BTreeMap<crate::CalendarDate, Timestamp>, SignalReturnError> {
    let market = match currency {
        Currency::Krw => Market::Kr,
        Currency::Usd => Market::Us,
    };
    let mut closes = std::collections::BTreeMap::new();
    let rows = calendars
        .iter()
        .flat_map(|calendar| {
            [
                calendar.previous_business_day(),
                calendar.today(),
                calendar.next_business_day(),
            ]
        })
        .filter(|row| row.market() == market);
    for (date, at) in rows.flat_map(|row| {
        row.sessions()
            .iter()
            .filter(|part| part.state() == MarketState::RegularMarket)
            .map(|part| (row.date(), part.session().interval().end()))
    }) {
        if let Some(previous) = closes.insert(date, at)
            && previous != at
        {
            return Err(SignalReturnError::BenchmarkMismatch);
        }
    }
    Ok(closes)
}

#[cfg(test)]
mod tests {
    use super::{SignalBenchmark, native_closes};
    use crate::signal_return_contract::tests::{instant, row};
    use crate::signal_return_scenarios::tests::{bar, calendar, snapshot};
    use crate::{
        AdjustmentBasis, BenchmarkKind, Candle, CandleObservation, CandlePeriod, Currency,
        MarketCalendar, MarketSession, Ohlcv, SessionInformation, SignalReturnError,
    };
    fn us_calendar(day: i64) -> MarketCalendar {
        let session = |index: i64| {
            MarketSession::try_new(
                row(index).date(),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: row(index).sessions().first().map(|part| part.session()),
                    after_market: None,
                },
            )
            .expect("미국 장")
        };
        MarketCalendar::try_new(
            session(day.checked_sub(1).expect("전일")),
            session(day),
            session(day.checked_add(1).expect("익일")),
        )
        .expect("달력")
    }
    fn candles(currency: Currency) -> Vec<Candle> {
        prices(currency, &["100", "100"])
    }
    fn prices(currency: Currency, values: &[&str]) -> Vec<Candle> {
        values
            .iter()
            .enumerate()
            .map(|(index, amount)| {
                let day = 61_i64
                    .checked_add(i64::try_from(index).expect("날짜"))
                    .expect("날짜");
                let original = bar(
                    day,
                    0,
                    CandlePeriod::Daily,
                    "100",
                    AdjustmentBasis::new(instant(67, 4)),
                );
                let price = crate::candle::tests::price(amount, currency);
                Candle::try_new(
                    original.instrument().clone(),
                    original.time(),
                    original.adjustment(),
                    Ohlcv::try_new(price, price, price, price, original.values().volume())
                        .expect("OHLC"),
                    CandleObservation::new(original.collected_at(), original.status()),
                )
                .expect("지수")
            })
            .collect()
    }
    #[test]
    fn review_daily_close_same_day_is_zero_and_missing_end_is_unavailable() {
        let benchmark = SignalBenchmark::from_candles(
            BenchmarkKind::Kospi,
            &prices(Currency::Krw, &["100", "110", "220"]),
            &[snapshot(61), snapshot(62), snapshot(63)],
            &[calendar(61), calendar(62)],
            Currency::Krw,
        )
        .expect("세 종가");
        assert_eq!(
            benchmark
                .between_dates(row(61).date(), row(61).date(), Currency::Krw)
                .expect("당일")
                .to_string(),
            "0.000000"
        );
        assert_eq!(
            benchmark
                .between_dates(row(61).date(), row(62).date(), Currency::Krw)
                .expect("하루")
                .to_string(),
            "0.100000"
        );
        assert_eq!(
            benchmark.between_dates(row(61).date(), row(64).date(), Currency::Krw),
            Err(SignalReturnError::BenchmarkMismatch)
        );
        assert_eq!(
            benchmark.between_dates(row(60).date(), row(62).date(), Currency::Krw),
            Err(SignalReturnError::BenchmarkMismatch)
        );
        assert_eq!(
            benchmark.between_dates(row(61).date(), row(62).date(), Currency::Usd),
            Err(SignalReturnError::BenchmarkMismatch)
        );
    }
    #[test]
    fn d29_benchmark_requires_native_currency_and_exact_close_instants() {
        let dates = [snapshot(61), snapshot(62)];
        for (currency, kind, calendars) in [
            (Currency::Krw, BenchmarkKind::Kospi, vec![calendar(61)]),
            (
                Currency::Usd,
                BenchmarkKind::Sp500Etf,
                vec![us_calendar(61)],
            ),
        ] {
            let benchmark = SignalBenchmark::from_candles(
                kind,
                &candles(currency),
                &dates,
                &calendars,
                currency,
            )
            .expect("같은 가격 시각");
            assert_eq!(benchmark.series().kind(), kind);
            assert_eq!(
                benchmark
                    .between(instant(61, 4), instant(62, 4), currency)
                    .expect("정확한 종가")
                    .to_string(),
                "0.000000"
            );
            let wrong = match currency {
                Currency::Krw => Currency::Usd,
                Currency::Usd => Currency::Krw,
            };
            assert_eq!(
                benchmark.between(instant(61, 4), instant(62, 4), wrong),
                Err(SignalReturnError::BenchmarkMismatch)
            );
            assert_eq!(
                benchmark.between(instant(61, 3), instant(62, 4), currency),
                Err(SignalReturnError::BenchmarkMismatch)
            );
        }
    }
    #[test]
    fn d29_benchmark_rejects_missing_conflicting_and_excess_calendar_facts() {
        let dates = [snapshot(61), snapshot(62)];
        let bars = candles(Currency::Krw);
        assert_eq!(
            SignalBenchmark::from_candles(BenchmarkKind::Kospi, &bars, &dates, &[], Currency::Krw),
            Err(SignalReturnError::BenchmarkMismatch)
        );
        assert_eq!(
            SignalBenchmark::from_candles(BenchmarkKind::Kospi, &[], &dates, &[], Currency::Krw),
            Err(SignalReturnError::BenchmarkMismatch)
        );
        assert_eq!(
            SignalBenchmark::from_candles(
                BenchmarkKind::Kospi,
                &bars,
                &dates,
                &vec![calendar(61); 4098],
                Currency::Krw
            ),
            Err(SignalReturnError::TooManyInputs)
        );
        let altered = MarketSession::try_new(
            row(61).date(),
            SessionInformation::Kr {
                pre_market: None,
                regular_market: Some(
                    crate::SessionData::try_new(
                        crate::TimeInterval::try_new(instant(61, 0), instant(61, 3)).expect("장"),
                        None,
                    )
                    .expect("장"),
                ),
                after_market: None,
            },
        )
        .expect("서로 다른 사실");
        let conflict = MarketCalendar::try_new(row(60), altered, row(62)).expect("별도 달력");
        assert_eq!(
            SignalBenchmark::from_candles(
                BenchmarkKind::Kospi,
                &bars,
                &dates,
                &[calendar(61), conflict],
                Currency::Krw
            ),
            Err(SignalReturnError::BenchmarkMismatch)
        );
        assert_eq!(
            native_closes(&[calendar(61)], Currency::Usd),
            Ok(std::collections::BTreeMap::new())
        );
    }
    #[test]
    fn d29_benchmark_rejects_reversed_native_points_and_linked_overflow() {
        let empty = |day: i64| {
            MarketSession::try_new(
                row(day).date(),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: None,
                    after_market: None,
                },
            )
            .expect("휴장")
        };
        let sparse = |date: i64, actual: i64| {
            let regular = MarketSession::try_new(
                row(date).date(),
                SessionInformation::Us {
                    day_market: None,
                    pre_market: None,
                    regular_market: row(actual).sessions().first().map(|part| part.session()),
                    after_market: None,
                },
            )
            .expect("거래일 사실");
            MarketCalendar::try_new(empty(60), regular, empty(64)).expect("별도 달력")
        };
        assert_eq!(
            SignalBenchmark::from_candles(
                BenchmarkKind::Sp500Etf,
                &candles(Currency::Usd),
                &[snapshot(61), snapshot(62)],
                &[sparse(61, 63), sparse(62, 61)],
                Currency::Usd
            ),
            Err(SignalReturnError::BenchmarkMismatch)
        );
        let extreme = prices(
            Currency::Krw,
            &[
                "0.0000000000000000000000000001",
                "1",
                "79228162514264337593543950335",
            ],
        );
        let dates = [snapshot(61), snapshot(62), snapshot(63)];
        let benchmark = SignalBenchmark::from_candles(
            BenchmarkKind::Kospi,
            &extreme,
            &dates,
            &[calendar(61), calendar(62)],
            Currency::Krw,
        )
        .expect("각 구간은 유효");
        assert_eq!(
            benchmark.between(instant(61, 4), instant(63, 4), Currency::Krw),
            Err(SignalReturnError::Arithmetic)
        );
    }
}
