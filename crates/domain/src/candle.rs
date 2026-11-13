use crate::{CalendarDate, Currency, InstrumentId, Price, Quantity, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdjustmentBasis(Timestamp);
impl AdjustmentBasis {
    #[must_use]
    pub const fn new(value: Timestamp) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_timestamp(&self) -> &Timestamp {
        &self.0
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceAdjustment {
    Raw,
    Adjusted(AdjustmentBasis),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandlePeriod {
    Daily,
    Minute,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandleError {
    CurrencyMismatch,
    InvalidRange,
    InvalidTimestamp,
    FutureTimestamp,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ohlcv {
    open: Price,
    high: Price,
    low: Price,
    close: Price,
    volume: Quantity,
}
impl Ohlcv {
    pub fn try_new(
        open: Price,
        high: Price,
        low: Price,
        close: Price,
        volume: Quantity,
    ) -> Result<Self, CandleError> {
        let currency = open.as_money().currency();
        if [high, low, close]
            .iter()
            .any(|price| price.as_money().currency() != currency)
        {
            return Err(CandleError::CurrencyMismatch);
        }
        let high_value = high.as_money().amount().as_decimal();
        let low_value = low.as_money().amount().as_decimal();
        if [open, close].iter().any(|price| {
            let value = price.as_money().amount().as_decimal();
            value > high_value || value < low_value
        }) {
            return Err(CandleError::InvalidRange);
        }
        Ok(Self {
            open,
            high,
            low,
            close,
            volume,
        })
    }
    #[must_use]
    pub const fn open(&self) -> Price {
        self.open
    }
    #[must_use]
    pub const fn high(&self) -> Price {
        self.high
    }
    #[must_use]
    pub const fn low(&self) -> Price {
        self.low
    }
    #[must_use]
    pub const fn close(&self) -> Price {
        self.close
    }
    #[must_use]
    pub const fn volume(&self) -> Quantity {
        self.volume
    }
    #[must_use]
    pub fn currency(&self) -> Currency {
        self.open.as_money().currency()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandleTime {
    timestamp: Timestamp,
    period: CandlePeriod,
}
impl CandleTime {
    pub fn try_new(timestamp: Timestamp, period: CandlePeriod) -> Result<Self, CandleError> {
        let unit = match period {
            CandlePeriod::Daily => 86_400_i64,
            CandlePeriod::Minute => 60_i64,
        };
        let local = timestamp
            .utc_seconds()
            .as_i64()
            .checked_add(i64::from(*timestamp.offset_seconds().as_i32()))
            .and_then(|seconds| seconds.checked_rem(unit));
        if local != Some(0) || *timestamp.nanosecond().as_u32() != 0 {
            return Err(CandleError::InvalidTimestamp);
        }
        Ok(Self { timestamp, period })
    }
    #[must_use]
    pub const fn timestamp(&self) -> Timestamp {
        self.timestamp
    }
    #[must_use]
    pub const fn period(&self) -> CandlePeriod {
        self.period
    }
    #[must_use]
    pub const fn date(&self) -> CalendarDate {
        self.timestamp.offset_calendar_date()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candle {
    instrument: InstrumentId,
    time: CandleTime,
    adjustment: PriceAdjustment,
    values: Ohlcv,
    collected_at: Timestamp,
}
impl Candle {
    pub fn try_new(
        instrument: InstrumentId,
        time: CandleTime,
        adjustment: PriceAdjustment,
        values: Ohlcv,
        collected_at: Timestamp,
    ) -> Result<Self, CandleError> {
        if time.timestamp() > collected_at {
            return Err(CandleError::FutureTimestamp);
        }
        Ok(Self {
            instrument,
            time,
            adjustment,
            values,
            collected_at,
        })
    }
    #[must_use]
    pub fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }
    #[must_use]
    pub const fn time(&self) -> CandleTime {
        self.time
    }
    #[must_use]
    pub const fn adjustment(&self) -> PriceAdjustment {
        self.adjustment
    }
    #[must_use]
    pub const fn values(&self) -> Ohlcv {
        self.values
    }
    #[must_use]
    pub const fn collected_at(&self) -> Timestamp {
        self.collected_at
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::{
        AdjustmentBasis, Candle, CandleError, CandlePeriod, CandleTime, Ohlcv, PriceAdjustment,
    };
    use crate::{Currency, InstrumentId, Price, Quantity, Timestamp};
    use crate::{Nanosecond, NumericInput, OffsetSeconds, UnixSeconds};
    pub(crate) fn at(day: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(
                1_767_225_600_i64
                    .checked_add(day.checked_mul(86_400).expect("일"))
                    .expect("시각"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    pub(crate) fn price(text: &str, currency: Currency) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("길이"), currency).expect("가격")
    }
    pub(crate) fn series(values: &[&str]) -> Vec<Candle> {
        values
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let close = price(text, Currency::Usd);
                Candle::try_new(
                    InstrumentId::try_new("sample").expect("식별자"),
                    CandleTime::try_new(
                        at(i64::try_from(index).expect("크기")),
                        CandlePeriod::Daily,
                    )
                    .expect("일봉"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(at(10_001))),
                    Ohlcv::try_new(
                        close,
                        close,
                        close,
                        close,
                        Quantity::parse(&NumericInput::try_new("0").expect("길이")).expect("수량"),
                    )
                    .expect("OHLCV"),
                    at(10_001),
                )
                .expect("봉")
            })
            .collect()
    }
    #[test]
    fn ohlcv_rejects_inconsistent_high_low_and_currency_without_normalizing() {
        let one = price("1", Currency::Usd);
        let two = price("2", Currency::Usd);
        let three = price("3", Currency::Usd);
        let volume = Quantity::parse(&NumericInput::try_new("0").expect("길이")).expect("거래량");
        for (open, high, low, close) in [
            (three, two, one, two),
            (two, two, one, three),
            (one, three, two, two),
            (two, three, two, one),
            (two, one, three, two),
        ] {
            assert_eq!(
                Ohlcv::try_new(open, high, low, close, volume),
                Err(CandleError::InvalidRange)
            );
        }
        for values in [
            [price("1", Currency::Krw), two, one, two],
            [one, price("3", Currency::Krw), one, two],
            [one, three, price("1", Currency::Krw), two],
            [one, three, one, price("2", Currency::Krw)],
        ] {
            let [open, high, low, close] = values;
            assert_eq!(
                Ohlcv::try_new(open, high, low, close, volume),
                Err(CandleError::CurrencyMismatch)
            );
        }
        let candle = series(&["2"]).into_iter().next().expect("봉");
        assert_eq!(candle.instrument().as_str(), "sample");
        assert_eq!(candle.values().open(), two);
        assert_eq!(candle.values().high(), two);
        assert_eq!(candle.values().low(), two);
        assert_eq!(candle.values().close(), two);
        assert_eq!(candle.values().volume(), volume);
        assert_eq!(candle.values().currency(), Currency::Usd);
        assert_eq!(candle.time().date(), at(0).offset_calendar_date());
        assert_eq!(candle.time().timestamp(), at(0));
        assert_eq!(candle.time().period(), CandlePeriod::Daily);
        assert_eq!(candle.collected_at(), at(10_001));
        assert_eq!(
            candle.adjustment(),
            PriceAdjustment::Adjusted(AdjustmentBasis::new(at(10_001)))
        );
        assert_eq!(AdjustmentBasis::new(at(3)).as_timestamp(), &at(3));
        assert_eq!(
            Candle::try_new(
                candle.instrument().clone(),
                candle.time(),
                PriceAdjustment::Raw,
                candle.values(),
                at(-1)
            ),
            Err(CandleError::FutureTimestamp)
        );
    }
    #[test]
    fn candle_time_requires_local_midnight_or_minute_end_and_zero_nanoseconds() {
        for (seconds, nanos, period, expected) in [
            (60, 0, CandlePeriod::Minute, Ok(())),
            (
                60,
                0,
                CandlePeriod::Daily,
                Err(CandleError::InvalidTimestamp),
            ),
            (
                1,
                0,
                CandlePeriod::Minute,
                Err(CandleError::InvalidTimestamp),
            ),
            (
                0,
                1,
                CandlePeriod::Daily,
                Err(CandleError::InvalidTimestamp),
            ),
            (
                0,
                1,
                CandlePeriod::Minute,
                Err(CandleError::InvalidTimestamp),
            ),
        ] {
            let stamp = Timestamp::from_parts(
                UnixSeconds::new(
                    at(0)
                        .utc_seconds()
                        .as_i64()
                        .checked_add(seconds)
                        .expect("초"),
                ),
                Nanosecond::new(nanos),
                OffsetSeconds::new(0),
            )
            .expect("시각");
            assert_eq!(
                CandleTime::try_new(stamp, period)
                    .map(|time| time.period())
                    .map(|_| ()),
                expected
            );
        }
    }

    #[test]
    fn local_midnight_and_price_volume_domains_are_validated() {
        let offset = Timestamp::from_parts(
            UnixSeconds::new(
                at(0)
                    .utc_seconds()
                    .as_i64()
                    .checked_sub(32_400)
                    .expect("KST"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("시각");
        assert_eq!(
            CandleTime::try_new(offset, CandlePeriod::Daily)
                .expect("현지 자정")
                .date(),
            at(0).offset_calendar_date()
        );
        assert_eq!(
            price("0.1", Currency::Usd).as_money().currency(),
            Currency::Usd
        );
        for text in ["0", "-1"] {
            assert!(
                Price::parse(&NumericInput::try_new(text).expect("길이"), Currency::Usd).is_err(),
                "로그 정의역 밖의 가격 거부"
            );
        }
        assert!(
            Quantity::parse(&NumericInput::try_new("-1").expect("길이")).is_err(),
            "음수 거래량 거부"
        );
    }
}
