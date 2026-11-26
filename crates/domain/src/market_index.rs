use crate::quote::{TimestampAllowance, assess_time};
use crate::{CandlePeriod, ClockAlignment, MasterCode, NumericInput, Quantity, Ratio, Timestamp};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarketDataError {
    InvalidNumber,
    NonPositiveIndex,
    FutureTimestamp,
    InvalidRange,
    UnsupportedIndicator,
    UnsupportedPeriod,
    InvalidRank,
    DuplicateInstrument,
    InvalidCurrency,
    InvalidSymbol,
    InvalidCount,
    UnsupportedRanking,
    InvalidFlow,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorCode {
    Kospi,
    Kosdaq,
    KrBond2Y,
    KrBond3Y,
    KrBond5Y,
    KrBond10Y,
    KrBond20Y,
    KrBond30Y,
    Unknown,
}
impl IndicatorCode {
    #[must_use]
    pub fn from_code(code: &MasterCode) -> Self {
        match code.as_str() {
            "KOSPI" => Self::Kospi,
            "KOSDAQ" => Self::Kosdaq,
            "KR_BOND_2Y" => Self::KrBond2Y,
            "KR_BOND_3Y" => Self::KrBond3Y,
            "KR_BOND_5Y" => Self::KrBond5Y,
            "KR_BOND_10Y" => Self::KrBond10Y,
            "KR_BOND_20Y" => Self::KrBond20Y,
            "KR_BOND_30Y" => Self::KrBond30Y,
            _ => Self::Unknown,
        }
    }
    #[must_use]
    pub const fn unit(self) -> IndicatorUnit {
        match self {
            Self::Kospi | Self::Kosdaq => IndicatorUnit::IndexPoints,
            Self::KrBond2Y
            | Self::KrBond3Y
            | Self::KrBond5Y
            | Self::KrBond10Y
            | Self::KrBond20Y
            | Self::KrBond30Y => IndicatorUnit::YieldPercent,
            Self::Unknown => IndicatorUnit::Unknown,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorUnit {
    IndexPoints,
    YieldPercent,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorMarket {
    Korea,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorSource {
    TossOpenApi,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorChange {
    NotProvided,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationAlignment {
    Unknown,
    Aligned,
    AheadWithinTolerance,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketObservation {
    server: Option<Timestamp>,
    collected: Timestamp,
    alignment: ObservationAlignment,
}
impl MarketObservation {
    pub(crate) const fn collected_only(collected: Timestamp) -> Self {
        Self {
            server: None,
            collected,
            alignment: ObservationAlignment::Unknown,
        }
    }
    pub(crate) fn from_response(
        server: Option<Timestamp>,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        let alignment = match server {
            None => ObservationAlignment::Unknown,
            Some(stamp) => match assess_time(stamp, collected, TimestampAllowance::Observation)
                .or(Err(MarketDataError::FutureTimestamp))?
            {
                ClockAlignment::Aligned => ObservationAlignment::Aligned,
                ClockAlignment::AheadWithinTolerance => ObservationAlignment::AheadWithinTolerance,
            },
        };
        Ok(Self {
            server,
            collected,
            alignment,
        })
    }
    #[must_use]
    pub const fn server_at(self) -> Option<Timestamp> {
        self.server
    }
    #[must_use]
    pub const fn collected_at(self) -> Timestamp {
        self.collected
    }
    #[must_use]
    pub const fn alignment(self) -> ObservationAlignment {
        self.alignment
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexPoints(Ratio);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndicatorYieldPercent(Ratio);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnclassifiedIndicatorValue(Ratio);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorValue {
    Points(IndexPoints),
    YieldPercent(IndicatorYieldPercent),
    Unknown(UnclassifiedIndicatorValue),
}
impl IndicatorValue {
    fn parse(input: &NumericInput, unit: IndicatorUnit) -> Result<Self, MarketDataError> {
        let value = Ratio::parse(input).or(Err(MarketDataError::InvalidNumber))?;
        match unit {
            IndicatorUnit::IndexPoints => {
                if value.as_decimal() <= Decimal::ZERO {
                    return Err(MarketDataError::NonPositiveIndex);
                }
                Ok(Self::Points(IndexPoints(value)))
            }
            IndicatorUnit::YieldPercent => Ok(Self::YieldPercent(IndicatorYieldPercent(value))),
            IndicatorUnit::Unknown => Ok(Self::Unknown(UnclassifiedIndicatorValue(value))),
        }
    }
    fn number(self) -> Ratio {
        match self {
            Self::Points(value) => value.0,
            Self::YieldPercent(value) => value.0,
            Self::Unknown(value) => value.0,
        }
    }

    #[must_use]
    pub fn as_ratio(self) -> Ratio {
        self.number()
    }

    #[must_use]
    pub fn display(self) -> IndicatorDisplay {
        let text = IndicatorDisplayText(self.number().as_decimal().to_string());
        match self {
            Self::Points(_) => IndicatorDisplay::Points(text),
            Self::YieldPercent(_) => IndicatorDisplay::YieldPercent(text),
            Self::Unknown(_) => IndicatorDisplay::Unknown(text),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorDisplayText(String);
impl IndicatorDisplayText {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndicatorDisplay {
    Points(IndicatorDisplayText),
    YieldPercent(IndicatorDisplayText),
    Unknown(IndicatorDisplayText),
    VolumeShares(IndicatorDisplayText),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndicatorVolume(Quantity);
impl IndicatorVolume {
    #[must_use]
    pub fn display(self) -> IndicatorDisplay {
        IndicatorDisplay::VolumeShares(IndicatorDisplayText(self.0.as_decimal().to_string()))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorCandleStatus {
    Finalized,
    InProgress,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketIndicatorQuote {
    raw_code: MasterCode,
    code: IndicatorCode,
    value: IndicatorValue,
    observation: MarketObservation,
    change: IndicatorChange,
    market: IndicatorMarket,
    source: IndicatorSource,
}
impl MarketIndicatorQuote {
    pub fn from_response(
        raw_code: MasterCode,
        value: &NumericInput,
        timestamp: Option<Timestamp>,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        let code = IndicatorCode::from_code(&raw_code);
        Ok(Self {
            value: IndicatorValue::parse(value, code.unit())?,
            raw_code,
            code,
            observation: MarketObservation::from_response(timestamp, collected)?,
            change: IndicatorChange::NotProvided,
            market: match code {
                IndicatorCode::Unknown => IndicatorMarket::Unknown,
                IndicatorCode::Kospi
                | IndicatorCode::Kosdaq
                | IndicatorCode::KrBond2Y
                | IndicatorCode::KrBond3Y
                | IndicatorCode::KrBond5Y
                | IndicatorCode::KrBond10Y
                | IndicatorCode::KrBond20Y
                | IndicatorCode::KrBond30Y => IndicatorMarket::Korea,
            },
            source: IndicatorSource::TossOpenApi,
        })
    }
    #[must_use]
    pub const fn code(&self) -> IndicatorCode {
        self.code
    }
    #[must_use]
    pub const fn raw_code(&self) -> &MasterCode {
        &self.raw_code
    }
    #[must_use]
    pub const fn value(&self) -> IndicatorValue {
        self.value
    }
    #[must_use]
    pub const fn unit(&self) -> IndicatorUnit {
        self.code.unit()
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
    #[must_use]
    pub const fn change(&self) -> IndicatorChange {
        self.change
    }
    #[must_use]
    pub const fn market(&self) -> IndicatorMarket {
        self.market
    }
    #[must_use]
    pub const fn source(&self) -> IndicatorSource {
        self.source
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorCandleRow {
    pub timestamp: Timestamp,
    pub open: NumericInput,
    pub high: NumericInput,
    pub low: NumericInput,
    pub close: NumericInput,
    pub volume: NumericInput,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketIndicatorCandle {
    pub(crate) start: Timestamp,
    raw_code: MasterCode,
    code: IndicatorCode,
    period: CandlePeriod,
    open: IndicatorValue,
    high: IndicatorValue,
    low: IndicatorValue,
    close: IndicatorValue,
    volume: IndicatorVolume,
    observation: MarketObservation,
    pub(crate) status: IndicatorCandleStatus,
}
impl MarketIndicatorCandle {
    pub fn from_response(
        raw_code: MasterCode,
        period: CandlePeriod,
        row: &IndicatorCandleRow,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        let code = IndicatorCode::from_code(&raw_code);
        if code == IndicatorCode::Unknown {
            return Err(MarketDataError::UnsupportedIndicator);
        }
        if period == CandlePeriod::Minute && code.unit() != IndicatorUnit::IndexPoints {
            return Err(MarketDataError::UnsupportedPeriod);
        }
        let open = IndicatorValue::parse(&row.open, code.unit())?;
        let high = IndicatorValue::parse(&row.high, code.unit())?;
        let low = IndicatorValue::parse(&row.low, code.unit())?;
        let close = IndicatorValue::parse(&row.close, code.unit())?;
        if low.number() > high.number()
            || [open, close]
                .iter()
                .any(|value| value.number() < low.number() || value.number() > high.number())
        {
            return Err(MarketDataError::InvalidRange);
        }
        Ok(Self {
            raw_code,
            code,
            period,
            open,
            high,
            low,
            close,
            volume: IndicatorVolume(
                Quantity::parse(&row.volume).or(Err(MarketDataError::InvalidNumber))?,
            ),
            start: row.timestamp,
            status: indicator_candle_status(row.timestamp, period, collected),
            observation: MarketObservation::from_response(Some(row.timestamp), collected)?,
        })
    }
    #[must_use]
    pub const fn code(&self) -> IndicatorCode {
        self.code
    }
    #[must_use]
    pub const fn raw_code(&self) -> &MasterCode {
        &self.raw_code
    }
    #[must_use]
    pub const fn period(&self) -> CandlePeriod {
        self.period
    }
    #[must_use]
    pub const fn status(&self) -> IndicatorCandleStatus {
        self.status
    }
    #[must_use]
    pub const fn open(&self) -> IndicatorValue {
        self.open
    }
    #[must_use]
    pub const fn high(&self) -> IndicatorValue {
        self.high
    }
    #[must_use]
    pub const fn low(&self) -> IndicatorValue {
        self.low
    }
    #[must_use]
    pub const fn close(&self) -> IndicatorValue {
        self.close
    }
    #[must_use]
    pub const fn volume(&self) -> IndicatorVolume {
        self.volume
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
}

fn indicator_candle_status(
    start: Timestamp,
    period: CandlePeriod,
    collected: Timestamp,
) -> IndicatorCandleStatus {
    match period {
        CandlePeriod::Daily => IndicatorCandleStatus::Unknown,
        CandlePeriod::Minute => {
            let possible_end = start
                .utc_seconds()
                .as_i64()
                .checked_add(60)
                .and_then(|seconds| {
                    Timestamp::from_parts(
                        crate::UnixSeconds::new(seconds),
                        start.nanosecond(),
                        start.offset_seconds(),
                    )
                    .ok()
                });
            match possible_end {
                Some(end) if end < collected => IndicatorCandleStatus::Finalized,
                Some(_) => IndicatorCandleStatus::InProgress,
                None => IndicatorCandleStatus::Unknown,
            }
        }
    }
}
pub(crate) fn index_close_number(
    candle: &MarketIndicatorCandle,
) -> Result<crate::performance_arithmetic::Number, crate::PerformanceError> {
    match candle.close {
        IndicatorValue::Points(points) => Ok(crate::performance_arithmetic::Number::new(
            points.0.as_decimal(),
        )),
        IndicatorValue::YieldPercent(_) | IndicatorValue::Unknown(_) => {
            Err(crate::PerformanceError::InvalidBenchmark)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::market_index::{
        IndicatorCandleRow, IndicatorChange, IndicatorCode, IndicatorMarket, IndicatorSource,
        MarketDataError, MarketIndicatorCandle, MarketIndicatorQuote, ObservationAlignment,
    };
    use crate::{CandlePeriod, Timestamp};
    use crate::{MasterCode, Nanosecond, NumericInput, OffsetSeconds, UnixSeconds};
    fn display_text(display: crate::IndicatorDisplay) -> String {
        match display {
            crate::IndicatorDisplay::Points(value)
            | crate::IndicatorDisplay::YieldPercent(value)
            | crate::IndicatorDisplay::Unknown(value)
            | crate::IndicatorDisplay::VolumeShares(value) => value.as_str().to_owned(),
        }
    }
    fn code(value: &str) -> MasterCode {
        MasterCode::try_new(value).expect("코드")
    }
    fn num(value: &str) -> NumericInput {
        NumericInput::try_new(value).expect("수치")
    }
    fn at(seconds: i64, nanos: u32) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(nanos),
            OffsetSeconds::new(32400),
        )
        .expect("시각")
    }
    #[test]
    fn index_response_preserves_units_identity_and_unknown_time() {
        for (raw, expected) in [
            ("KOSPI", IndicatorCode::Kospi),
            ("KOSDAQ", IndicatorCode::Kosdaq),
            ("KR_BOND_2Y", IndicatorCode::KrBond2Y),
            ("KR_BOND_3Y", IndicatorCode::KrBond3Y),
            ("KR_BOND_5Y", IndicatorCode::KrBond5Y),
            ("KR_BOND_10Y", IndicatorCode::KrBond10Y),
            ("KR_BOND_20Y", IndicatorCode::KrBond20Y),
            ("KR_BOND_30Y", IndicatorCode::KrBond30Y),
            ("NEW", IndicatorCode::Unknown),
        ] {
            let quote =
                MarketIndicatorQuote::from_response(code(raw), &num("2812.45"), None, at(100, 0))
                    .expect("지표");
            assert_eq!(quote.code(), expected);
            assert_eq!(quote.raw_code().as_str(), raw);
            assert_eq!(
                quote.observation().alignment(),
                ObservationAlignment::Unknown
            );
            assert_eq!(quote.observation().server_at(), None);
            assert_eq!(quote.observation().collected_at(), at(100, 0));
            assert_eq!(quote.change(), IndicatorChange::NotProvided);
            assert_eq!(
                quote.market(),
                if expected == IndicatorCode::Unknown {
                    IndicatorMarket::Unknown
                } else {
                    IndicatorMarket::Korea
                }
            );
            assert_eq!(quote.source(), IndicatorSource::TossOpenApi);
            assert_eq!(display_text(quote.value().display()), "2812.45");
            assert_eq!(quote.unit(), expected.unit());
        }
    }
    #[test]
    fn indicator_clock_matches_d11_two_second_nanosecond_boundary() {
        for (text, number, expected) in [
            (100, 0, Ok(ObservationAlignment::Aligned)),
            (100, 1, Ok(ObservationAlignment::AheadWithinTolerance)),
            (102, 0, Ok(ObservationAlignment::AheadWithinTolerance)),
            (102, 1, Err(MarketDataError::FutureTimestamp)),
        ] {
            assert_eq!(
                MarketIndicatorQuote::from_response(
                    code("KOSPI"),
                    &num("1"),
                    Some(at(text, number)),
                    at(100, 0)
                )
                .map(|value| value.observation().alignment()),
                expected
            );
        }
        for text in ["0", "-1", "broken"] {
            MarketIndicatorQuote::from_response(code("KOSPI"), &num(text), None, at(0, 0))
                .expect_err("거부");
        }
    }
    #[test]
    fn indicator_candles_preserve_start_time_and_reject_invalid_ohlc() {
        let row = || IndicatorCandleRow {
            timestamp: at(0, 0),
            open: num("2"),
            high: num("4"),
            low: num("1"),
            close: num("3"),
            volume: num("10"),
        };
        let candle = MarketIndicatorCandle::from_response(
            code("KOSPI"),
            CandlePeriod::Daily,
            &row(),
            at(100, 0),
        )
        .expect("봉");
        assert_eq!(candle.code(), IndicatorCode::Kospi);
        assert_eq!(candle.raw_code().as_str(), "KOSPI");
        assert_eq!(candle.period(), CandlePeriod::Daily);
        assert_eq!(candle.observation().server_at(), Some(at(0, 0)));
        assert_eq!(display_text(candle.open().display()), "2");
        assert_eq!(display_text(candle.high().display()), "4");
        assert_eq!(display_text(candle.low().display()), "1");
        assert_eq!(display_text(candle.close().display()), "3");
        assert_eq!(display_text(candle.volume().display()), "10");
    }
    #[test]
    fn invalid_indicator_candles_are_rejected() {
        let row = || IndicatorCandleRow {
            timestamp: at(0, 0),
            open: num("2"),
            high: num("4"),
            low: num("1"),
            close: num("3"),
            volume: num("10"),
        };
        for (open, high, low, close, volume) in [
            ("0", "4", "1", "3", "10"),
            ("5", "4", "1", "3", "10"),
            ("2", "4", "1", "5", "10"),
            ("2", "0", "1", "3", "10"),
            ("2", "4", "0", "3", "10"),
            ("2", "4", "1", "0", "10"),
            ("2", "4", "1", "3", "-1"),
        ] {
            let mut bad = row();
            bad.open = num(open);
            bad.high = num(high);
            bad.low = num(low);
            bad.close = num(close);
            bad.volume = num(volume);
            MarketIndicatorCandle::from_response(
                code("KOSPI"),
                CandlePeriod::Daily,
                &bad,
                at(100, 0),
            )
            .expect_err("거부");
        }
        MarketIndicatorCandle::from_response(
            code("KR_BOND_2Y"),
            CandlePeriod::Minute,
            &row(),
            at(100, 0),
        )
        .expect_err("거부");
        MarketIndicatorCandle::from_response(code("NEW"), CandlePeriod::Daily, &row(), at(100, 0))
            .expect_err("거부");
    }

    #[test]
    fn candle_low_high_and_both_open_close_range_boundaries() {
        let input = || IndicatorCandleRow {
            timestamp: at(0, 0),
            open: num("2"),
            high: num("4"),
            low: num("1"),
            close: num("3"),
            volume: num("0"),
        };
        for (open, high, low, close) in [
            ("0.5", "4", "1", "3"),
            ("2", "4", "1", "0.5"),
            ("2", "1", "4", "3"),
        ] {
            let mut row = input();
            row.open = num(open);
            row.high = num(high);
            row.low = num(low);
            row.close = num(close);
            MarketIndicatorCandle::from_response(
                code("KOSPI"),
                CandlePeriod::Daily,
                &row,
                at(100, 0),
            )
            .expect_err("범위");
        }
        let mut future = input();
        future.timestamp = at(103, 0);
        MarketIndicatorCandle::from_response(
            code("KOSPI"),
            CandlePeriod::Daily,
            &future,
            at(100, 0),
        )
        .expect_err("미래");
        let mut bad = input();
        bad.volume = num("bad");
        MarketIndicatorCandle::from_response(code("KOSPI"), CandlePeriod::Daily, &bad, at(100, 0))
            .expect_err("수량 문법");
        let bond = MarketIndicatorCandle::from_response(
            code("KR_BOND_2Y"),
            CandlePeriod::Daily,
            &input(),
            at(100, 0),
        )
        .expect("국채 일봉");
        assert_eq!(display_text(bond.close().display()), "3");
    }

    #[test]
    fn indicator_display_is_tagged_and_volume_is_not_order_quantity() {
        let quote =
            MarketIndicatorQuote::from_response(code("KOSPI"), &num("2812.45"), None, at(0, 0))
                .expect("지수");
        assert_eq!(
            quote.value().display(),
            crate::IndicatorDisplay::Points(super::IndicatorDisplayText("2812.45".to_owned()))
        );
        let row = IndicatorCandleRow {
            timestamp: at(0, 0),
            open: num("1"),
            high: num("1"),
            low: num("1"),
            close: num("1"),
            volume: num("42"),
        };
        assert_eq!(
            MarketIndicatorCandle::from_response(
                code("KOSPI"),
                CandlePeriod::Daily,
                &row,
                at(0, 0)
            )
            .expect("봉")
            .volume()
            .display(),
            crate::IndicatorDisplay::VolumeShares(super::IndicatorDisplayText("42".to_owned()))
        );
    }
    #[test]
    fn minute_finality_requires_a_strictly_past_end_daily_without_end_is_unknown() {
        let row = IndicatorCandleRow {
            timestamp: at(0, 0),
            open: num("1"),
            high: num("1"),
            low: num("1"),
            close: num("1"),
            volume: num("0"),
        };
        for (collected, expected) in [
            (59, crate::IndicatorCandleStatus::InProgress),
            (60, crate::IndicatorCandleStatus::InProgress),
            (61, crate::IndicatorCandleStatus::Finalized),
        ] {
            assert_eq!(
                MarketIndicatorCandle::from_response(
                    code("KOSPI"),
                    CandlePeriod::Minute,
                    &row,
                    at(collected, 0)
                )
                .expect("분봉")
                .status(),
                expected
            );
        }
        assert_eq!(
            MarketIndicatorCandle::from_response(
                code("KOSPI"),
                CandlePeriod::Daily,
                &row,
                at(86400, 0)
            )
            .expect("일봉")
            .status(),
            crate::IndicatorCandleStatus::Unknown
        );
        let mut boundary = row;
        boundary.timestamp = Timestamp::from_parts(
            UnixSeconds::new(253_402_300_799),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("최대 시각");
        assert_eq!(
            MarketIndicatorCandle::from_response(
                code("KOSPI"),
                CandlePeriod::Minute,
                &boundary,
                boundary.timestamp
            )
            .expect("종료 범위 없음")
            .status(),
            crate::IndicatorCandleStatus::Unknown
        );
    }
}
