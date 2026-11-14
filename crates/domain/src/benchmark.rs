use crate::performance_arithmetic::{
    Number, benchmark_weight, blended_return, linked, price_return,
};
use crate::{
    CalendarDate, Candle, CandlePeriod, CandleStatus, Currency, Performance, PerformanceCurrency,
    PerformanceError, PerformanceValue, ValuationSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkKind {
    Kospi,
    Kosdaq,
    Sp500Etf,
    Mixed,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkDividends {
    Excluded,
    Unverified,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchmarkSeries {
    kind: BenchmarkKind,
    currency: PerformanceCurrency,
    dates: Vec<CalendarDate>,
    pub(crate) returns: Vec<PerformanceValue>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BenchmarkObservation {
    start: CalendarDate,
    end: CalendarDate,
    value: PerformanceValue,
}
impl BenchmarkObservation {
    #[must_use]
    pub const fn start(self) -> CalendarDate {
        self.start
    }
    #[must_use]
    pub const fn end(self) -> CalendarDate {
        self.end
    }
    #[must_use]
    pub const fn value(self) -> PerformanceValue {
        self.value
    }
}
impl BenchmarkSeries {
    pub fn from_candles(
        kind: BenchmarkKind,
        candles: &[Candle],
        snapshots: &[ValuationSnapshot],
        currency: PerformanceCurrency,
    ) -> Result<Self, PerformanceError> {
        if candles.len() > 4097 || snapshots.len() > 4097 {
            return Err(PerformanceError::TooManyInputs);
        }
        let first = candles.first().ok_or(PerformanceError::InvalidBenchmark)?;
        let native = first.values().currency();
        if kind == BenchmarkKind::Sp500Etf && first.adjustment() == crate::PriceAdjustment::Raw {
            return Err(PerformanceError::InvalidBenchmark);
        }
        match (kind, native) {
            (BenchmarkKind::Kospi | BenchmarkKind::Kosdaq, Currency::Krw)
            | (BenchmarkKind::Sp500Etf, Currency::Usd) => (),
            (BenchmarkKind::Kospi | BenchmarkKind::Kosdaq, Currency::Usd)
            | (BenchmarkKind::Sp500Etf, Currency::Krw)
            | (BenchmarkKind::Mixed, _) => return Err(PerformanceError::InvalidBenchmark),
        }
        if let PerformanceCurrency::Native(unit) = currency
            && unit != native
        {
            return Err(PerformanceError::BenchmarkMismatch);
        }
        let prices = candle_prices(candles, first)?;
        let mut ordered = snapshots.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|snapshot| snapshot.valuation_at());
        if ordered.len() < 2 {
            return Err(PerformanceError::InvalidPeriod);
        }
        let mut values = Vec::new();
        let mut dates = Vec::new();
        for snapshot in ordered {
            let price = *prices
                .get(&snapshot.price_date(native))
                .ok_or(PerformanceError::BenchmarkMismatch)?;
            let amount = benchmark_money(price, snapshot, currency)?;
            dates.push(snapshot.settlement_date());
            values.push(Number::new(amount.amount().as_decimal()));
        }
        if dates
            .iter()
            .zip(dates.iter().skip(1))
            .any(|(old, today)| old >= today)
        {
            return Err(PerformanceError::InvalidPeriod);
        }
        let returns = values
            .iter()
            .zip(values.iter().skip(1))
            .map(|(old, today)| price_return(*old, *today))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            kind,
            currency,
            dates,
            returns,
        })
    }
    pub fn mixed(
        kr: &Self,
        us: &Self,
        performance: &Performance,
    ) -> Result<Self, PerformanceError> {
        alignment(kr, performance)?;
        mixed_snapshots(kr, us, performance.snapshots())
    }
    #[must_use]
    pub const fn weights(&self) -> BenchmarkWeights {
        match self.kind {
            BenchmarkKind::Mixed => BenchmarkWeights::StocksExcludingCash,
            BenchmarkKind::Kospi | BenchmarkKind::Kosdaq | BenchmarkKind::Sp500Etf => {
                BenchmarkWeights::NotMixed
            }
        }
    }
    #[must_use]
    pub const fn kind(&self) -> BenchmarkKind {
        self.kind
    }
    #[must_use]
    pub const fn currency(&self) -> PerformanceCurrency {
        self.currency
    }
    #[must_use]
    pub const fn dividends(&self) -> BenchmarkDividends {
        match self.kind {
            BenchmarkKind::Kospi | BenchmarkKind::Kosdaq => BenchmarkDividends::Excluded,
            BenchmarkKind::Sp500Etf | BenchmarkKind::Mixed => BenchmarkDividends::Unverified,
        }
    }
    pub fn observations(&self) -> impl Iterator<Item = BenchmarkObservation> + '_ {
        self.dates
            .iter()
            .zip(self.dates.iter().skip(1))
            .zip(&self.returns)
            .map(|((start, end), value)| BenchmarkObservation {
                start: *start,
                end: *end,
                value: *value,
            })
    }
    pub fn excess_return(
        &self,
        performance: &Performance,
    ) -> Result<PerformanceValue, PerformanceError> {
        alignment(self, performance)?;
        let benchmark = linked(&self.returns)?;
        performance
            .twr()
            .number()
            .sub(benchmark.number())
            .map(PerformanceValue::from_number)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkWeights {
    NotMixed,
    StocksExcludingCash,
}
fn mixed_snapshots(
    kr: &BenchmarkSeries,
    us: &BenchmarkSeries,
    snapshots: &[ValuationSnapshot],
) -> Result<BenchmarkSeries, PerformanceError> {
    if kr.dates != us.dates
        || kr.currency != PerformanceCurrency::ConvertedKrw
        || us.currency != kr.currency
        || kr.kind != BenchmarkKind::Kospi
        || us.kind != BenchmarkKind::Sp500Etf
    {
        return Err(PerformanceError::BenchmarkMismatch);
    }
    let mut ordered = snapshots.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|snapshot| snapshot.valuation_at());
    if ordered
        .iter()
        .map(|snapshot| snapshot.settlement_date())
        .collect::<Vec<_>>()
        != kr.dates
    {
        return Err(PerformanceError::BenchmarkMismatch);
    }
    let returns = kr
        .returns
        .iter()
        .zip(&us.returns)
        .zip(ordered)
        .map(|((domestic, foreign), snapshot)| {
            let native = snapshot
                .stock_value(Currency::Krw)
                .map(|money| Number::new(money.amount().as_decimal()))
                .map_err(PerformanceError::Snapshot)?;
            let total = snapshot
                .stock_krw_value()
                .map(|value| Number::new(value.clone().into_parts().0.amount().as_decimal()))
                .map_err(PerformanceError::Snapshot)?;
            let weight = benchmark_weight(native, total)?;
            blended_return(weight, domestic.number(), foreign.number())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BenchmarkSeries {
        kind: BenchmarkKind::Mixed,
        currency: kr.currency,
        dates: kr.dates.clone(),
        returns,
    })
}
fn candle_prices(
    candles: &[Candle],
    first: &Candle,
) -> Result<std::collections::BTreeMap<CalendarDate, crate::Money>, PerformanceError> {
    let mut prices = std::collections::BTreeMap::new();
    for candle in candles {
        if candle.instrument() != first.instrument()
            || candle.adjustment() != first.adjustment()
            || candle.values().currency() != first.values().currency()
            || candle.status() != CandleStatus::Finalized
            || candle.time().period() != CandlePeriod::Daily
            || prices
                .insert(candle.time().date(), candle.values().close().as_money())
                .is_some()
        {
            return Err(PerformanceError::InvalidBenchmark);
        }
    }
    Ok(prices)
}
fn benchmark_money(
    price: crate::Money,
    snapshot: &ValuationSnapshot,
    currency: PerformanceCurrency,
) -> Result<crate::Money, PerformanceError> {
    match (currency, price.currency()) {
        (PerformanceCurrency::Native(_), _)
        | (PerformanceCurrency::ConvertedKrw, Currency::Krw) => Ok(price),
        (PerformanceCurrency::ConvertedKrw, Currency::Usd) => {
            let rate = snapshot.fx().ok_or(PerformanceError::MissingFx)?;
            if rate.validity_at(snapshot.valuation_at()) != crate::FxValidity::Valid {
                return Err(PerformanceError::MissingFx);
            }
            rate.convert_for_aggregation(price)
                .map(|converted| converted.into_parts().0)
                .map_err(|_cause| PerformanceError::MissingFx)
        }
    }
}

pub(crate) fn alignment(
    benchmark: &BenchmarkSeries,
    performance: &Performance,
) -> Result<(), PerformanceError> {
    if benchmark.currency != performance.currency() || benchmark.dates != performance.dates {
        return Err(PerformanceError::BenchmarkMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BenchmarkKind, BenchmarkSeries};
    use crate::valuation_snapshot::tests::{at, date, fixture_window, money, snapshot};
    use crate::{
        AdjustmentBasis, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime,
        Currency, InstrumentId, NumericInput, Ohlcv, Performance, PerformanceCurrency, Price,
        PriceAdjustment, Quantity, SettlementWindow,
    };
    fn candles(unit: Currency, prices: &[&str]) -> Vec<Candle> {
        prices
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let price = Price::try_new(money(text, unit)).expect("가격");
                let day = i64::try_from(index)
                    .expect("크기")
                    .checked_add(2)
                    .expect("날짜");
                Candle::try_new(
                    InstrumentId::try_new("index").expect("ID"),
                    CandleTime::try_new(at(day, 0), CandlePeriod::Daily).expect("시간"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(at(20, 0))),
                    Ohlcv::try_new(
                        price,
                        price,
                        price,
                        price,
                        Quantity::parse(&NumericInput::try_new("0").expect("입력")).expect("수량"),
                    )
                    .expect("OHLC"),
                    CandleObservation::new(at(20, 0), CandleStatus::Finalized),
                )
                .expect("봉")
            })
            .collect()
    }
    fn equity_fixture() -> crate::valuation_snapshot::tests::Fixture {
        let trades = [
            (Currency::Krw, "kr", "600", "60000"),
            (Currency::Usd, "us", "1", "40"),
        ]
        .map(|(unit, symbol, quantity, amount)| {
            let mut data = crate::valuation_snapshot::tests::tx(
                crate::TransactionKind::Buy,
                unit,
                1,
                1,
                amount,
            )
            .data()
            .clone();
            data.symbol = Some(crate::Symbol::try_new(symbol).expect("심볼"));
            data.asset_class = Some(crate::AssetClass::Stock);
            data.quantity = Some(
                Quantity::parse(&NumericInput::try_new(quantity).expect("입력")).expect("수량"),
            );
            crate::Transaction::try_new(data).expect("매수")
        });
        fixture_window(
            trades.into(),
            ("0", "0"),
            ("0", "0"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        )
    }
    fn equity_snapshots(
        data: &crate::valuation_snapshot::tests::Fixture,
    ) -> [crate::ValuationSnapshot; 2] {
        let histories = [(Currency::Krw, "kr", "100"), (Currency::Usd, "us", "40")].map(
            |(unit, symbol, price)| {
                let id = InstrumentId::try_new(symbol).expect("ID");
                let market = match unit {
                    Currency::Krw => crate::Market::Kr,
                    Currency::Usd => crate::Market::Us,
                };
                let identity = crate::LotIdentity::new(
                    id.clone(),
                    crate::Symbol::try_new(symbol).expect("심볼"),
                    market,
                    unit,
                );
                let values = candles(unit, &[price; 4])
                    .into_iter()
                    .map(|candle| {
                        Candle::try_new(
                            id.clone(),
                            candle.time(),
                            PriceAdjustment::Raw,
                            candle.values(),
                            CandleObservation::new(at(20, 0), CandleStatus::Finalized),
                        )
                        .expect("봉")
                    })
                    .collect();
                crate::SnapshotInstrumentHistory::new(identity, vec![], values).expect("주식 이력")
            },
        );
        [
            snapshot(data, 2, &histories, "1000"),
            snapshot(data, 5, &histories, "1002"),
        ]
    }
    #[test]
    fn research_example_ten_matches_converted_and_mixed_benchmark_legs() {
        let data = equity_fixture();
        let snapshots = equity_snapshots(&data);
        let kr = BenchmarkSeries::from_candles(
            BenchmarkKind::Kospi,
            &candles(Currency::Krw, &["100", "100", "100", "101"]),
            &snapshots,
            PerformanceCurrency::ConvertedKrw,
        )
        .expect("국내");
        let us = BenchmarkSeries::from_candles(
            BenchmarkKind::Sp500Etf,
            &candles(Currency::Usd, &["100", "100", "100", "100.5"]),
            &snapshots,
            PerformanceCurrency::ConvertedKrw,
        )
        .expect("미국");
        assert_eq!(us.returns.first().expect("수익률").to_string(), "0.007010");
        let mixed = super::mixed_snapshots(&kr, &us, &snapshots).expect("혼합");
        assert_eq!(
            mixed.returns.first().expect("수익률").to_string(),
            "0.008804"
        );
        let cash = fixture_window(
            vec![],
            ("60000", "40"),
            ("60000", "40"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        assert_eq!(
            super::mixed_snapshots(
                &kr,
                &us,
                &[
                    snapshot(&cash, 2, &[], "1000"),
                    snapshot(&cash, 5, &[], "1002")
                ]
            ),
            Err(crate::PerformanceError::NonPositiveCapital)
        );
        let point = mixed.observations().next().expect("수익률 관측");
        assert_eq!(point.start(), date(2));
        assert_eq!(point.end(), date(5));
        assert_eq!(point.value().to_string(), "0.008804");
        assert_eq!(mixed.kind(), BenchmarkKind::Mixed);
        assert_eq!(mixed.dividends(), super::BenchmarkDividends::Unverified);
        assert_eq!(us.dividends(), super::BenchmarkDividends::Unverified);
        assert_eq!(kr.dividends(), super::BenchmarkDividends::Excluded);
        assert_eq!(kr.weights(), crate::BenchmarkWeights::NotMixed);
        assert_eq!(mixed.currency(), PerformanceCurrency::ConvertedKrw);
        let performance = Performance::calculate(
            &snapshots,
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::ConvertedKrw,
        )
        .expect("성과");
        assert_eq!(
            mixed.excess_return(&performance).expect("초과").to_string(),
            "-0.008004"
        );
    }
    #[test]
    fn invalid_benchmark_sources_are_closed() {
        use crate::PerformanceError;
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Mixed,
                &candles(Currency::Krw, &["1"]),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::InvalidBenchmark)
        );
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Kospi,
                &candles(Currency::Usd, &["1"]),
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::InvalidBenchmark)
        );
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Sp500Etf,
                &[],
                &[],
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::InvalidBenchmark)
        );
    }
    #[test]
    fn weekly_beta_checks_52_complete_weeks() {
        let performance = crate::performance::tests::series(364);
        let benchmark = BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: performance.currency(),
            dates: performance.dates.clone(),
            returns: performance.returns.clone(),
        };
        let extract = |result| match result {
            crate::BetaMetric::Value(value) => Some(value),
            crate::BetaMetric::InsufficientSamples | crate::BetaMetric::Unavailable(_) => None,
        };
        assert_eq!(extract(crate::BetaMetric::InsufficientSamples), None);
        assert_eq!(
            extract(crate::BetaMetric::Unavailable(
                crate::PerformanceError::Arithmetic
            )),
            None
        );
        let actual = extract(crate::performance_beta(&performance, &benchmark)).expect("52주");
        assert_eq!(actual.beta().to_string(), "1.000000");
        assert_eq!(actual.r_squared().to_string(), "1.000000");
        let shorter = crate::performance::tests::series(357);
        let short_benchmark = BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: shorter.currency(),
            dates: shorter.dates.clone(),
            returns: shorter.returns.clone(),
        };
        assert_eq!(
            crate::performance_beta(&shorter, &short_benchmark),
            crate::BetaMetric::InsufficientSamples
        );
        assert_eq!(
            crate::performance_beta(&shorter, &benchmark),
            crate::BetaMetric::Unavailable(crate::PerformanceError::BenchmarkMismatch)
        );
    }
    #[test]
    fn series_rejects_misalignment_duplicates_and_input_limits() {
        use crate::PerformanceError;
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&data, 2, &[], "1000"),
            snapshot(&data, 5, &[], "1000"),
        ];
        let prices = candles(Currency::Krw, &["100", "100", "100", "101"]);
        let native = PerformanceCurrency::Native(Currency::Krw);
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Kospi,
                &vec![prices.first().expect("봉").clone(); 4098],
                &snapshots,
                native
            ),
            Err(PerformanceError::TooManyInputs)
        );
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Kospi,
                &prices,
                &vec![snapshots.first().expect("스냅샷").clone(); 4098],
                native
            ),
            Err(PerformanceError::TooManyInputs)
        );
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Kospi,
                &prices,
                &snapshots,
                PerformanceCurrency::Native(Currency::Usd)
            ),
            Err(PerformanceError::BenchmarkMismatch)
        );
        assert_eq!(
            BenchmarkSeries::from_candles(BenchmarkKind::Kospi, &prices, &[], native),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Kospi,
                &prices,
                std::slice::from_ref(snapshots.first().expect("스냅샷")),
                native
            ),
            Err(PerformanceError::InvalidPeriod)
        );
    }
    #[test]
    fn duplicate_dates_and_missing_prices_are_rejected() {
        use crate::PerformanceError;
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&data, 2, &[], "1000"),
            snapshot(&data, 5, &[], "1000"),
        ];
        let prices = candles(Currency::Krw, &["100", "100", "100", "101"]);
        let native = PerformanceCurrency::Native(Currency::Krw);
        let duplicate = [
            snapshots.first().expect("시작").clone(),
            snapshots.first().expect("시작").clone(),
        ];
        assert_eq!(
            BenchmarkSeries::from_candles(BenchmarkKind::Kospi, &prices, &duplicate, native),
            Err(PerformanceError::InvalidPeriod)
        );
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Kospi,
                &[
                    prices.first().expect("봉").clone(),
                    prices.first().expect("봉").clone()
                ],
                &snapshots,
                native
            ),
            Err(PerformanceError::InvalidBenchmark)
        );
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Kospi,
                std::slice::from_ref(prices.first().expect("봉")),
                &snapshots,
                native
            ),
            Err(PerformanceError::BenchmarkMismatch)
        );
        assert_eq!(
            BenchmarkSeries::from_candles(BenchmarkKind::Kosdaq, &prices, &snapshots, native)
                .expect("코스닥")
                .kind(),
            BenchmarkKind::Kosdaq
        );
        assert_eq!(
            super::benchmark_money(
                money("1", Currency::Usd),
                &crate::valuation_snapshot::tests::observed(2, "0", "0"),
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::MissingFx)
        );
    }
    #[test]
    fn mixed_and_excess_reject_mismatched_facts() {
        use crate::PerformanceError;
        let performance = crate::performance::tests::series(60);
        let kr = BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: PerformanceCurrency::ConvertedKrw,
            dates: performance.dates.clone(),
            returns: performance.returns.clone(),
        };
        let us = BenchmarkSeries {
            kind: BenchmarkKind::Sp500Etf,
            ..kr.clone()
        };
        assert_eq!(
            super::mixed_snapshots(&kr, &us, &[]),
            Err(PerformanceError::BenchmarkMismatch)
        );
        assert_eq!(
            super::mixed_snapshots(&us, &kr, &[]),
            Err(PerformanceError::BenchmarkMismatch)
        );
        assert_eq!(
            kr.excess_return(&performance),
            Err(PerformanceError::BenchmarkMismatch)
        );
        let data = fixture_window(
            vec![],
            ("0", "0"),
            ("0", "0"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&data, 2, &[], "1000"),
            snapshot(&data, 5, &[], "1000"),
        ];
        let mut domestic = kr;
        domestic.dates = snapshots
            .iter()
            .map(crate::ValuationSnapshot::settlement_date)
            .collect();
        let foreign = BenchmarkSeries {
            kind: BenchmarkKind::Sp500Etf,
            ..domestic.clone()
        };
        assert_eq!(
            super::mixed_snapshots(&domestic, &foreign, &snapshots),
            Err(PerformanceError::NonPositiveCapital)
        );
    }
    #[test]
    fn flat_market_variance_is_not_a_beta_value() {
        let performance = crate::performance::tests::series(364);
        let zero = crate::PerformanceValue::from_number(
            crate::performance_arithmetic::Number::new(rust_decimal::Decimal::ZERO),
        );
        let benchmark = BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: performance.currency(),
            dates: performance.dates.clone(),
            returns: vec![zero; performance.returns.len()],
        };
        assert_eq!(
            crate::performance_beta(&performance, &benchmark),
            crate::BetaMetric::Unavailable(crate::PerformanceError::Arithmetic)
        );
    }
    #[test]
    fn public_benchmark_paths_reject_missing_fx_raw_etf_and_overflow() {
        use crate::PerformanceError;
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&data, 2, &[], "1000"),
            snapshot(&data, 5, &[], "1000"),
        ];
        let observed = [
            crate::valuation_snapshot::tests::observed(2, "100", "100"),
            crate::valuation_snapshot::tests::observed(5, "100", "100"),
        ];
        let usd = candles(Currency::Usd, &["1", "1", "1", "1"]);
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Sp500Etf,
                &usd,
                &observed,
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::MissingFx)
        );
        let source = usd.first().expect("봉");
        let raw = Candle::try_new(
            source.instrument().clone(),
            source.time(),
            PriceAdjustment::Raw,
            source.values(),
            CandleObservation::new(at(20, 0), CandleStatus::Finalized),
        )
        .expect("원주가");
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Sp500Etf,
                &[raw],
                &snapshots,
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::InvalidBenchmark)
        );
    }
    #[test]
    fn price_overflow_and_expired_fx_never_form_benchmark_values() {
        use crate::PerformanceError;
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&data, 2, &[], "1000"),
            snapshot(&data, 5, &[], "1000"),
        ];
        let maximum = rust_decimal::Decimal::MAX.to_string();
        let prices = candles(
            Currency::Krw,
            &[
                "0.0000000000000000000000000001",
                maximum.as_str(),
                maximum.as_str(),
                maximum.as_str(),
            ],
        );
        assert_eq!(
            BenchmarkSeries::from_candles(
                BenchmarkKind::Kospi,
                &prices,
                &snapshots,
                PerformanceCurrency::Native(Currency::Krw)
            ),
            Err(PerformanceError::Arithmetic)
        );
        let expired = crate::ValuationSnapshot::recalculate(
            &crate::valuation_snapshot::tests::calendar(2),
            &data.rows,
            &[],
            Some(crate::valuation_snapshot::tests::ledgers(&data)),
            crate::SnapshotObservation::new(
                at(2, 7),
                at(20, 0),
                Some(crate::valuation_snapshot::tests::fx(at(1, 7), "1000")),
            ),
        )
        .expect("만료 환율");
        assert_eq!(
            super::benchmark_money(
                money("1", Currency::Usd),
                &expired,
                PerformanceCurrency::ConvertedKrw
            ),
            Err(PerformanceError::MissingFx)
        );
    }
    #[test]
    fn excess_return_and_mixed_inputs_propagate_period_errors() {
        use crate::PerformanceError;
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&data, 2, &[], "1000"),
            snapshot(&data, 3, &[], "1000"),
            snapshot(&data, 5, &[], "1000"),
        ];
        let prices = candles(
            Currency::Krw,
            &["0.0000000000000000000000000001", "1", "1", "100"],
        );
        let benchmark = BenchmarkSeries::from_candles(
            BenchmarkKind::Kospi,
            &prices,
            &snapshots,
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("시계열");
        let portfolio = Performance::calculate(
            &snapshots,
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("계좌");
        assert_eq!(
            benchmark.excess_return(&portfolio),
            Err(PerformanceError::Arithmetic)
        );
    }
    #[test]
    fn mixed_benchmark_rejects_incomplete_account_facts() {
        use crate::PerformanceError;
        let data = fixture_window(
            vec![],
            ("100", "100"),
            ("100", "100"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = [
            snapshot(&data, 2, &[], "1000"),
            snapshot(&data, 3, &[], "1000"),
            snapshot(&data, 5, &[], "1000"),
        ];
        let benchmark = BenchmarkSeries::from_candles(
            BenchmarkKind::Kospi,
            &candles(Currency::Krw, &["100", "100", "100", "100"]),
            &snapshots,
            PerformanceCurrency::ConvertedKrw,
        )
        .expect("기준");
        let mut domestic = benchmark;
        domestic.currency = PerformanceCurrency::ConvertedKrw;
        let foreign = BenchmarkSeries {
            kind: BenchmarkKind::Sp500Etf,
            ..domestic.clone()
        };
        let invalid = [
            crate::valuation_snapshot::tests::observed(2, "100", "100"),
            crate::valuation_snapshot::tests::observed(3, "100", "100"),
            crate::valuation_snapshot::tests::observed(5, "100", "100"),
        ];
        assert_eq!(
            super::mixed_snapshots(&domestic, &foreign, &invalid),
            Err(PerformanceError::NonPositiveCapital)
        );
        let partial = snapshots
            .iter()
            .map(|item| {
                crate::ValuationSnapshot::recalculate(
                    &crate::valuation_snapshot::tests::calendar(i64::from(
                        *item.settlement_date().day().as_u8(),
                    )),
                    &data.rows,
                    &[],
                    Some(crate::valuation_snapshot::tests::ledgers(&data)),
                    crate::SnapshotObservation::new(item.valuation_at(), at(20, 0), None),
                )
                .expect("불완전")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            super::mixed_snapshots(&domestic, &foreign, &partial),
            Err(PerformanceError::NonPositiveCapital)
        );
    }
    #[test]
    fn weekly_link_overflow_does_not_produce_a_beta_value() {
        use crate::performance_arithmetic::Number;
        use crate::{BetaMetric, PerformanceError, PerformanceValue};
        let portfolio = crate::performance::tests::adverse_series();
        let zero = PerformanceValue::from_number(Number::new(rust_decimal::Decimal::ZERO));
        let flat = BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: portfolio.currency(),
            dates: portfolio.dates.clone(),
            returns: vec![zero; portfolio.returns.len()],
        };
        assert_eq!(
            crate::performance_beta(&portfolio, &flat),
            BetaMetric::Unavailable(PerformanceError::Arithmetic)
        );
        let stable = crate::performance::tests::series(364);
        let volatile = BenchmarkSeries {
            returns: portfolio.returns,
            ..flat
        };
        assert_eq!(
            crate::performance_beta(&stable, &volatile),
            BetaMetric::Unavailable(PerformanceError::Arithmetic)
        );
    }
    #[test]
    fn benchmark_annualization_cannot_publish_an_uncertified_root() {
        use crate::PerformanceValue;
        use crate::performance_arithmetic::Number;
        let mut performance = crate::performance::tests::series(366);
        performance.returns =
            vec![PerformanceValue::from_number(Number::new(rust_decimal::Decimal::ZERO)); 366];
        let mut returns = performance.returns.clone();
        for index in [0, 100, 200, 300] {
            *returns.get_mut(index).expect("거래일") =
                PerformanceValue::from_number(Number::new(rust_decimal::Decimal::new(-99, 2)));
        }
        let benchmark = BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: performance.currency(),
            dates: performance.dates.clone(),
            returns,
        };
        assert_eq!(
            benchmark
                .excess_return(&performance)
                .expect("누적 초과")
                .to_string(),
            "0.981866"
        );
    }
    #[test]
    fn partially_covered_weeks_never_claim_full_period_beta() {
        let trailing = crate::performance::tests::series(365);
        let benchmark = BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: trailing.currency(),
            dates: trailing.dates.clone(),
            returns: trailing.returns.clone(),
        };
        let result =
            beta_value(crate::performance_beta(&trailing, &benchmark)).expect("충분한 주간 표본");
        assert_eq!(result.weeks().to_string(), "52");
        assert_eq!(
            result.start(),
            trailing.dates.get(1).copied().expect("시작")
        );
        assert_eq!(result.end(), trailing.end());
        let mut gap = crate::performance::tests::series(378);
        let joined = crate::performance_arithmetic::linked(&[
            *gap.returns.get(6).expect("전"),
            *gap.returns.get(7).expect("후"),
        ])
        .expect("연결");
        *gap.returns.get_mut(6).expect("교체") = joined;
        gap.returns = gap
            .returns
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| if index == 7 { None } else { Some(value) })
            .collect();
        gap.dates = gap
            .dates
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| if index == 7 { None } else { Some(value) })
            .collect();
        gap.offsets = gap
            .offsets
            .into_iter()
            .enumerate()
            .filter_map(|(index, value)| if index == 7 { None } else { Some(value) })
            .collect();
        let missing = BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: gap.currency(),
            dates: gap.dates.clone(),
            returns: gap.returns.clone(),
        };
        let gap_result =
            beta_value(crate::performance_beta(&gap, &missing)).expect("충분한 주간 표본");
        assert_eq!(gap_result.weeks().to_string(), "54");
    }
    #[test]
    fn public_mixed_uses_only_its_performance_equities_even_with_cash() {
        let stock = equity_fixture();
        let rows = stock
            .rows
            .active_settled_rows()
            .map(crate::StoredTransaction::transaction)
            .cloned()
            .collect();
        let data = fixture_window(
            rows,
            ("30000", "30"),
            ("30000", "30"),
            SettlementWindow::try_new(date(2), date(5)).expect("기간"),
        );
        let snapshots = equity_snapshots(&data);
        let kr = BenchmarkSeries::from_candles(
            BenchmarkKind::Kospi,
            &candles(Currency::Krw, &["100", "100", "100", "101"]),
            &snapshots,
            PerformanceCurrency::ConvertedKrw,
        )
        .expect("국내");
        let us = BenchmarkSeries::from_candles(
            BenchmarkKind::Sp500Etf,
            &candles(Currency::Usd, &["100", "100", "100", "100.5"]),
            &snapshots,
            PerformanceCurrency::ConvertedKrw,
        )
        .expect("미국");
        let performance = Performance::calculate(
            &snapshots,
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::ConvertedKrw,
        )
        .expect("성과");
        let mixed = BenchmarkSeries::mixed(&kr, &us, &performance).expect("현금 제외");
        assert_eq!(
            mixed
                .observations()
                .next()
                .expect("구간")
                .value()
                .to_string(),
            "0.008804"
        );
        assert_eq!(
            mixed.weights(),
            crate::BenchmarkWeights::StocksExcludingCash
        );
        let native = Performance::calculate(
            &snapshots,
            (&data.kp, &data.up),
            &[],
            PerformanceCurrency::Native(Currency::Krw),
        )
        .expect("원통화");
        assert_eq!(
            BenchmarkSeries::mixed(&kr, &us, &native),
            Err(crate::PerformanceError::BenchmarkMismatch)
        );
    }
    fn beta_value(metric: crate::BetaMetric) -> Option<crate::PerformanceBeta> {
        match metric {
            crate::BetaMetric::Value(value) => Some(value),
            crate::BetaMetric::InsufficientSamples | crate::BetaMetric::Unavailable(_) => None,
        }
    }
    #[test]
    fn unavailable_beta_has_no_numeric_payload() {
        assert_eq!(beta_value(crate::BetaMetric::InsufficientSamples), None);
        assert_eq!(
            beta_value(crate::BetaMetric::Unavailable(
                crate::PerformanceError::InvalidPeriod
            )),
            None
        );
    }
    fn matching_series(performance: &Performance) -> BenchmarkSeries {
        BenchmarkSeries {
            kind: BenchmarkKind::Kospi,
            currency: performance.currency(),
            dates: performance.dates.clone(),
            returns: performance.returns.clone(),
        }
    }
    #[test]
    fn end_anchored_years_expose_the_actual_fifty_two_week_range() {
        for (count, start_day) in [(365, 6), (366, 7)] {
            let performance = crate::performance::tests::series(count);
            let value = beta_value(crate::performance_beta(
                &performance,
                &matching_series(&performance),
            ))
            .expect("주간 표본");
            assert_eq!(value.weeks().to_string(), "52");
            assert_eq!(value.start(), date(start_day));
            assert_eq!(value.end(), performance.end());
            assert_eq!(value.beta().to_string(), "1.000000");
        }
    }
    #[test]
    fn an_empty_week_does_not_bridge_two_weeks_into_one_sample() {
        let mut performance = crate::performance::tests::series(392);
        let kept = (0..=392)
            .filter(|index| !(8..=14).contains(index))
            .collect::<Vec<_>>();
        let returns = kept
            .iter()
            .zip(kept.iter().skip(1))
            .map(|(start, end)| {
                crate::performance_arithmetic::linked(
                    &performance
                        .returns
                        .iter()
                        .skip(*start)
                        .take(end.wrapping_sub(*start))
                        .copied()
                        .collect::<Vec<_>>(),
                )
                .expect("재묶음")
            })
            .collect();
        performance.dates = kept
            .iter()
            .map(|index| *performance.dates.get(*index).expect("날짜"))
            .collect();
        performance.offsets = kept
            .iter()
            .map(|index| *performance.offsets.get(*index).expect("간격"))
            .collect();
        performance.returns = returns;
        let value = beta_value(crate::performance_beta(
            &performance,
            &matching_series(&performance),
        ))
        .expect("주간 표본");
        assert_eq!(value.weeks().to_string(), "54");
        assert_eq!(value.start(), performance.start());
        assert_eq!(value.end(), performance.end());
    }
    #[test]
    fn missing_stock_history_cannot_be_used_for_mixed_weights() {
        let source = equity_fixture();
        for unit in [None, Some(Currency::Usd)] {
            let rows = source
                .rows
                .active_settled_rows()
                .map(crate::StoredTransaction::transaction)
                .filter(|row| unit.is_none_or(|expected| row.data().currency == expected))
                .cloned()
                .collect();
            let data = fixture_window(
                rows,
                ("0", "0"),
                ("0", "0"),
                SettlementWindow::try_new(date(2), date(5)).expect("기간"),
            );
            let missing = [
                snapshot(&data, 2, &[], "1000"),
                snapshot(&data, 5, &[], "1000"),
            ];
            let good = equity_snapshots(&data);
            let kr = BenchmarkSeries::from_candles(
                BenchmarkKind::Kospi,
                &candles(Currency::Krw, &["1"; 4]),
                &good,
                PerformanceCurrency::ConvertedKrw,
            )
            .expect("국내");
            let us = BenchmarkSeries::from_candles(
                BenchmarkKind::Sp500Etf,
                &candles(Currency::Usd, &["1"; 4]),
                &good,
                PerformanceCurrency::ConvertedKrw,
            )
            .expect("미국");
            assert_eq!(
                super::mixed_snapshots(&kr, &us, &missing),
                Err(crate::PerformanceError::Snapshot(
                    crate::SnapshotError::MissingHistory
                ))
            );
        }
    }
}
