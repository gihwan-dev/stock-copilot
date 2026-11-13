use crate::{
    AdjustmentBasis, AnnualVolatility, CalendarDate, Candle, CandlePeriod, CandleStatus,
    DailyVolatility, InstrumentId, PriceAdjustment,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolatilitySamples(usize);
impl VolatilitySamples {
    #[must_use]
    pub const fn as_usize(&self) -> &usize {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolatilityError {
    SampleCount,
    Unadjusted,
    MixedSeries,
    Unordered,
    NotDaily,
    Arithmetic,
    NonConvergence,
    Unfinalized,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Volatility {
    daily: DailyVolatility,
    instrument: InstrumentId,
    as_of: CalendarDate,
    adjustment_basis: AdjustmentBasis,
    samples: VolatilitySamples,
}
impl Volatility {
    pub fn calculate(candles: &[Candle]) -> Result<Self, VolatilityError> {
        let (first, remaining) = candles.split_first().ok_or(VolatilityError::SampleCount)?;
        if !(2..=10_000).contains(&remaining.len()) {
            return Err(VolatilityError::SampleCount);
        }
        let adjustment_basis = match first.adjustment() {
            PriceAdjustment::Raw => return Err(VolatilityError::Unadjusted),
            PriceAdjustment::Adjusted(basis) => basis,
        };
        let mut as_of = first.time().date();
        for candle in candles {
            if candle.status() != CandleStatus::Finalized {
                return Err(VolatilityError::Unfinalized);
            }
            as_of = candle.time().date();
            if candle.time().period() != CandlePeriod::Daily {
                return Err(VolatilityError::NotDaily);
            }
            if candle.instrument() != first.instrument()
                || candle.values().currency() != first.values().currency()
                || candle.adjustment() != first.adjustment()
            {
                return Err(VolatilityError::MixedSeries);
            }
        }
        for (previous, current) in candles.iter().zip(candles.iter().skip(1)) {
            if previous.time().date() >= current.time().date()
                || previous.time().timestamp() >= current.time().timestamp()
            {
                return Err(VolatilityError::Unordered);
            }
        }
        let daily = crate::volatility_arithmetic::calculate(candles)?;
        Ok(Self {
            daily,
            instrument: first.instrument().clone(),
            as_of,
            adjustment_basis,
            samples: VolatilitySamples(remaining.len()),
        })
    }
    #[must_use]
    pub const fn daily(&self) -> DailyVolatility {
        self.daily
    }
    pub fn annualized(&self) -> Result<AnnualVolatility, VolatilityError> {
        crate::volatility_arithmetic::annualize(self.daily)
    }
    #[must_use]
    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }
    #[must_use]
    pub const fn as_of(&self) -> CalendarDate {
        self.as_of
    }
    #[must_use]
    pub const fn adjustment_basis(&self) -> AdjustmentBasis {
        self.adjustment_basis
    }
    #[must_use]
    pub const fn samples(&self) -> VolatilitySamples {
        self.samples
    }
}

#[cfg(test)]
mod tests {
    use super::{Volatility, VolatilityError};
    use crate::candle::tests::series;
    use crate::{
        Candle, CandleObservation, CandlePeriod, CandleStatus, NumericInput, PriceAdjustment, Ratio,
    };
    use rust_decimal::Decimal;

    #[test]
    fn r11_example_nine_uses_simple_returns_and_sample_denominator() {
        let candles = series(&["100", "102", "99", "101", "104", "100", "98", "103"]);
        let value = Volatility::calculate(&candles).expect("예제 9");
        assert_eq!(value.samples().as_usize(), &7);
        for (actual, expected) in [
            (value.daily().as_decimal(), "0.033867"),
            (value.annualized().expect("연율").as_decimal(), "0.537618"),
        ] {
            let reference = Ratio::parse(&NumericInput::try_new(expected).expect("길이"))
                .expect("R-11 숫자")
                .as_decimal();
            assert!(
                actual.checked_sub(reference).expect("차이").abs() < Decimal::new(5, 7),
                "소수 여섯 자리 일치"
            );
        }
        let reference =
            Ratio::parse(&NumericInput::try_new("0.0338667707234930947423863745").expect("길이"))
                .expect("독립 80자리 기준")
                .as_decimal();
        assert!(
            value
                .daily()
                .as_decimal()
                .checked_sub(reference)
                .expect("차이")
                .abs()
                < Decimal::new(1, 18),
            "독립 기준 오차"
        );
    }
    #[test]
    fn samples_bounds_zero_variance_and_sixty_return_boundary_are_preserved() {
        for count in [0, 1, 2, 10_002] {
            assert_eq!(
                Volatility::calculate(&series(&vec!["100"; count])),
                Err(VolatilityError::SampleCount)
            );
        }
        for count in [3, 60, 61, 10_001] {
            let result = Volatility::calculate(&series(&vec!["100"; count])).expect("상수 가격");
            assert_eq!(
                *result.samples().as_usize(),
                count.checked_sub(1).expect("표본")
            );
            assert_eq!(result.daily().as_decimal(), Decimal::ZERO);
            assert_eq!(
                result.annualized().expect("연율").as_decimal(),
                Decimal::ZERO
            );
        }
        let constant_return =
            Volatility::calculate(&series(&["1", "2", "4", "8"])).expect("모두 같은 수익률");
        assert_eq!(constant_return.daily().as_decimal(), Decimal::ZERO);
    }

    fn replace(candles: &[Candle], replacement: Candle, expected: VolatilityError) {
        let mut changed = candles.to_vec();
        *changed.get_mut(1).expect("둘째 봉") = replacement;
        assert_eq!(Volatility::calculate(&changed), Err(expected));
    }
    #[test]
    fn mixed_adjustment_currency_and_instrument_are_rejected() {
        use crate::candle::tests::{at, price};
        use crate::{AdjustmentBasis, Currency, InstrumentId, Ohlcv};
        let original = series(&["100", "101", "99"]);
        let second = original.get(1).expect("둘째 봉");
        for adjustment in [
            PriceAdjustment::Raw,
            PriceAdjustment::Adjusted(AdjustmentBasis::new(at(10_000))),
        ] {
            let replacement = Candle::try_new(
                second.instrument().clone(),
                second.time(),
                adjustment,
                second.values(),
                CandleObservation::new(second.collected_at(), CandleStatus::Finalized),
            )
            .expect("봉");
            replace(&original, replacement, VolatilityError::MixedSeries);
        }
        let other = Candle::try_new(
            InstrumentId::try_new("other").expect("종목"),
            second.time(),
            second.adjustment(),
            second.values(),
            CandleObservation::new(second.collected_at(), CandleStatus::Finalized),
        )
        .expect("봉");
        replace(&original, other, VolatilityError::MixedSeries);
        let krw = price("1", Currency::Krw);
        let values = Ohlcv::try_new(krw, krw, krw, krw, second.values().volume()).expect("KRW");
        let other_currency = Candle::try_new(
            second.instrument().clone(),
            second.time(),
            second.adjustment(),
            values,
            CandleObservation::new(second.collected_at(), CandleStatus::Finalized),
        )
        .expect("봉");
        replace(&original, other_currency, VolatilityError::MixedSeries);
        let raw = original
            .iter()
            .map(|candle| {
                Candle::try_new(
                    candle.instrument().clone(),
                    candle.time(),
                    PriceAdjustment::Raw,
                    candle.values(),
                    CandleObservation::new(candle.collected_at(), CandleStatus::Finalized),
                )
                .expect("원주가")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            Volatility::calculate(&raw),
            Err(VolatilityError::Unadjusted)
        );
    }
    #[test]
    fn minute_duplicate_reversed_and_overflow_are_rejected() {
        use crate::CandleTime;
        use crate::candle::tests::at;
        let original = series(&["100", "101", "99"]);
        let second = original.get(1).expect("둘째 봉");
        for (time, expected) in [
            (
                CandleTime::try_new(at(1), CandlePeriod::Minute).expect("분봉"),
                VolatilityError::NotDaily,
            ),
            (
                CandleTime::try_new(at(0), CandlePeriod::Daily).expect("중복"),
                VolatilityError::Unordered,
            ),
            (
                CandleTime::try_new(at(-1), CandlePeriod::Daily).expect("이전"),
                VolatilityError::Unordered,
            ),
        ] {
            let replacement = Candle::try_new(
                second.instrument().clone(),
                time,
                second.adjustment(),
                second.values(),
                CandleObservation::new(second.collected_at(), CandleStatus::Finalized),
            )
            .expect("봉");
            replace(&original, replacement, expected);
        }
        let mut reversed = original;
        reversed.reverse();
        assert_eq!(
            Volatility::calculate(&reversed),
            Err(VolatilityError::Unordered)
        );
        for values in [
            [
                "0.0000000000000000000000000001",
                "79228162514264337593543950335",
                "1",
            ],
            ["1", "79228162514264337593543950335", "1"],
        ] {
            assert_eq!(
                Volatility::calculate(&series(&values)),
                Err(VolatilityError::Arithmetic)
            );
        }
    }
    #[test]
    fn increasing_local_dates_cannot_hide_reversed_absolute_timestamps() {
        use crate::candle::tests::at;
        use crate::{CandleTime, Nanosecond, OffsetSeconds, UnixSeconds};
        let mut candles = series(&["100", "101", "99"]);
        for (index, delta, offset) in [(0, 82_800_i64, -82_800_i32), (1, 3_600_i64, 82_800_i32)] {
            let stamp = crate::Timestamp::from_parts(
                UnixSeconds::new(
                    at(0)
                        .utc_seconds()
                        .as_i64()
                        .checked_add(delta)
                        .expect("시각"),
                ),
                Nanosecond::new(0),
                OffsetSeconds::new(offset),
            )
            .expect("오프셋");
            let original = candles.get(index).expect("봉");
            let replacement = Candle::try_new(
                original.instrument().clone(),
                CandleTime::try_new(stamp, CandlePeriod::Daily).expect("현지 자정"),
                original.adjustment(),
                original.values(),
                CandleObservation::new(original.collected_at(), CandleStatus::Finalized),
            )
            .expect("봉");
            *candles.get_mut(index).expect("봉") = replacement;
        }
        assert!(
            candles.first().expect("첫날").time().date()
                < candles.get(1).expect("둘째날").time().date(),
            "현지 날짜는 증가"
        );
        assert_eq!(
            Volatility::calculate(&candles),
            Err(VolatilityError::Unordered)
        );
    }
    #[test]
    fn provenance_is_preserved_and_any_live_candle_is_rejected() {
        use crate::candle::tests::at;
        let original = series(&["100", "101", "99"]);
        let value = Volatility::calculate(&original).expect("확정");
        assert_eq!(
            value.instrument(),
            original.first().expect("봉").instrument()
        );
        assert_eq!(value.as_of(), at(2).offset_calendar_date());
        assert_eq!(
            value.adjustment_basis(),
            crate::AdjustmentBasis::new(at(10_001))
        );
        for index in 0..original.len() {
            let mut changed = original.clone();
            let candle = changed.get(index).expect("봉");
            let live = Candle::try_new(
                candle.instrument().clone(),
                candle.time(),
                candle.adjustment(),
                candle.values(),
                CandleObservation::new(candle.collected_at(), CandleStatus::InProgress),
            )
            .expect("진행 중");
            *changed.get_mut(index).expect("봉") = live;
            assert_eq!(
                Volatility::calculate(&changed),
                Err(VolatilityError::Unfinalized)
            );
        }
        let extreme =
            Volatility::calculate(&series(&["1", "200000000000000", "1"])).expect("일간 유효");
        assert!(
            extreme.daily().as_decimal() > Decimal::ZERO,
            "일간은 연율 실패와 독립"
        );
        assert_eq!(extreme.annualized(), Err(VolatilityError::Arithmetic));
    }
}
