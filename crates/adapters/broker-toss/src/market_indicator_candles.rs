use crate::{QueryKey, ReadRequest, ReadRoute, RouteId, TokenRenewal, TossAccountRead};
use application::{Clock, PortError, ReadEndpoint, RequestRejection, SecretStore};
use domain::{
    CandlePeriod, IndicatorCandleRow, IndicatorCode, IndicatorUnit, Market, MarketIndicatorCandle,
    MasterCode, Nanosecond, NumericInput, OffsetSeconds, SpanSeconds, TimeInterval, TimeSpan,
    Timestamp,
};
use std::collections::BTreeMap;

const ENDPOINT: ReadEndpoint = ReadEndpoint::IndicatorCandles;
const PAGE_LIMIT: usize = 64;
const PAGE_SIZE: usize = 200;
const GAP_LIMIT: usize = 4096;

fn invalid() -> PortError {
    crate::market_read::invalid(ENDPOINT)
}
fn unsupported() -> PortError {
    crate::market_read::rejected(ENDPOINT, RequestRejection::UnsupportedQuery)
}
fn candle_query(
    symbol: &str,
    period: CandlePeriod,
    before: &str,
) -> Result<ReadRequest, PortError> {
    let id = RouteId::new(symbol).map_err(|_error| unsupported())?;
    let interval = match period {
        CandlePeriod::Daily => "1d",
        CandlePeriod::Minute => "1m",
    };
    Ok(ReadRequest::new(ReadRoute::IndicatorCandles(id))
        .with_query(QueryKey::Interval, interval.to_owned())
        .with_query(QueryKey::Count, "200".to_owned())
        .with_query(QueryKey::Before, before.to_owned()))
}
fn parse(
    value: &broker_toss_schema::types::MarketIndicatorCandle,
    symbol: &str,
    period: CandlePeriod,
    collected: Timestamp,
) -> Result<MarketIndicatorCandle, PortError> {
    let stamp = crate::market_read::timestamp(&value.timestamp, ENDPOINT)?;
    if stamp > collected {
        return Err(invalid());
    }
    let row = IndicatorCandleRow {
        timestamp: stamp,
        open: NumericInput::try_new(&value.open_price).map_err(|_error| invalid())?,
        high: NumericInput::try_new(&value.high_price).map_err(|_error| invalid())?,
        low: NumericInput::try_new(&value.low_price).map_err(|_error| invalid())?,
        close: NumericInput::try_new(&value.close_price).map_err(|_error| invalid())?,
        volume: NumericInput::try_new(&value.volume).map_err(|_error| invalid())?,
    };
    MarketIndicatorCandle::from_response(
        MasterCode::try_new(symbol).map_err(|_error| invalid())?,
        period,
        &row,
        collected,
    )
    .map_err(|_error| invalid())
}
fn same(left: &MarketIndicatorCandle, right: &MarketIndicatorCandle) -> bool {
    left.observation()
        .server_at()
        .zip(right.observation().server_at())
        .is_some_and(|(first, second)| first.offset_seconds() == second.offset_seconds())
        && left.code() == right.code()
        && left.period() == right.period()
        && left.open() == right.open()
        && left.high() == right.high()
        && left.low() == right.low()
        && left.close() == right.close()
        && left.volume() == right.volume()
}
pub(crate) async fn fetch<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    code: IndicatorCode,
    window: TimeInterval,
    period: CandlePeriod,
) -> Result<Vec<MarketIndicatorCandle>, PortError> {
    let symbol = crate::market_indicators::code(code).map_err(|_error| unsupported())?;
    if period == CandlePeriod::Minute && code.unit() != IndicatorUnit::IndexPoints {
        return Err(unsupported());
    }
    let mut boundary = window.end();
    let mut before =
        crate::market_read::candles::format_time(boundary).map_err(|_error| invalid())?;
    let mut result = BTreeMap::new();
    for _page in 0..PAGE_LIMIT {
        let value = reader
            .market_read(candle_query(symbol, period, &before)?, ENDPOINT)
            .await?;
        if value.get("nextBefore").is_none() {
            return Err(invalid());
        }
        let decoded: broker_toss_schema::types::MarketIndicatorCandlePageResponse =
            serde_json::from_value(value).map_err(|_error| invalid())?;
        if decoded.candles.len() > PAGE_SIZE {
            return Err(invalid());
        }
        let mut previous = None;
        for row in decoded.candles {
            let candle = parse(&row, symbol, period, reader.market_now())?;
            let stamp = candle.observation().server_at().ok_or_else(invalid)?;
            if stamp > boundary || previous.is_some_and(|newer| newer <= stamp) {
                return Err(invalid());
            }
            previous = Some(stamp);
            if stamp < window.start() || stamp >= window.end() {
                continue;
            }
            if result.get(&stamp).is_some_and(|old| !same(old, &candle)) {
                return Err(invalid());
            }
            result.entry(stamp).or_insert(candle);
        }
        if previous.is_some_and(|stamp| stamp <= window.start()) || decoded.next_before.is_none() {
            let candles = result.into_values().collect::<Vec<_>>();
            validate_gaps(reader, period, &candles)?;
            return Ok(candles);
        }
        let next = decoded.next_before.ok_or_else(invalid)?;
        let next_time = crate::market_read::timestamp(&next, ENDPOINT)?;
        if next_time >= boundary || previous.is_none_or(|oldest| next_time > oldest) {
            return Err(invalid());
        }
        before = next;
        boundary = next_time;
    }
    Err(invalid())
}
fn validate_gaps<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    period: CandlePeriod,
    candles: &[MarketIndicatorCandle],
) -> Result<(), PortError> {
    for [left, right] in candles.array_windows::<2>() {
        let earlier = left.observation().server_at().ok_or_else(invalid)?;
        let later = right.observation().server_at().ok_or_else(invalid)?;
        match period {
            CandlePeriod::Minute => validate_minute_spacing(reader, earlier, later)?,
            CandlePeriod::Daily => validate_daily_gap(reader, earlier, later)?,
        }
    }
    Ok(())
}
fn validate_minute_spacing<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    earlier: Timestamp,
    later: Timestamp,
) -> Result<(), PortError> {
    let minute =
        TimeSpan::try_new(SpanSeconds::new(60), Nanosecond::new(0)).map_err(|_error| invalid())?;
    let next = earlier.checked_add(minute).map_err(|_error| invalid())?;
    if next > later {
        return Err(invalid());
    }
    if next == later {
        return Ok(());
    }
    let (seconds, nanos) = earlier
        .elapsed_until(later)
        .map_err(|_error| invalid())?
        .as_parts();
    if *seconds.as_u64() % 60 != 0 || *nanos.as_u32() != 0 {
        return Err(invalid());
    }
    validate_minute_gap(reader, earlier, next, later)
}
fn missing_calendar() -> PortError {
    PortError::new(
        ENDPOINT,
        application::HttpStatus::Success,
        application::PortErrorKind::Unsupported(application::BrokerCapability::IndicatorCandles),
    )
}
fn validate_minute_gap<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    earlier: Timestamp,
    first_missing: Timestamp,
    later: Timestamp,
) -> Result<(), PortError> {
    let offset = OffsetSeconds::new(32_400);
    let later_date = later.date_at_offset(offset).map_err(|_error| invalid())?;
    let mut cursor = earlier
        .checked_sub(TimeSpan::DAY)
        .map_err(|_error| missing_calendar())?;
    for _day in 0..GAP_LIMIT {
        let date = cursor.date_at_offset(offset).map_err(|_error| invalid())?;
        let mut matching = reader
            .market_sessions()
            .iter()
            .filter(|session| session.market() == Market::Kr && session.date() == date);
        let session = matching.next().ok_or_else(missing_calendar)?;
        if matching.next().is_some() {
            return Err(invalid());
        }
        if session.sessions().iter().any(|segment| {
            let interval = segment.session().interval();
            interval.start() < later && interval.end() > first_missing
        }) {
            return Err(invalid());
        }
        if date == later_date {
            return Ok(());
        }
        cursor = cursor
            .checked_add(TimeSpan::DAY)
            .map_err(|_error| invalid())?;
    }
    Err(invalid())
}
fn validate_daily_gap<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    earlier: Timestamp,
    later: Timestamp,
) -> Result<(), PortError> {
    let offset = OffsetSeconds::new(32_400);
    let previous_date = earlier.date_at_offset(offset).map_err(|_error| invalid())?;
    let later_date = later.date_at_offset(offset).map_err(|_error| invalid())?;
    if previous_date >= later_date {
        return Err(invalid());
    }
    let mut next = earlier
        .checked_add(TimeSpan::DAY)
        .map_err(|_error| invalid())?;
    for _day in 0..GAP_LIMIT {
        let date = next.date_at_offset(offset).map_err(|_error| invalid())?;
        if date >= later_date {
            return Ok(());
        }
        let mut matching = reader
            .market_sessions()
            .iter()
            .filter(|session| session.market() == Market::Kr && session.date() == date);
        let session = matching.next().ok_or_else(|| {
            PortError::new(
                ENDPOINT,
                application::HttpStatus::Success,
                application::PortErrorKind::Unsupported(
                    application::BrokerCapability::IndicatorCandles,
                ),
            )
        })?;
        if matching.next().is_some() || !session.sessions().is_empty() {
            return Err(invalid());
        }
        next = next
            .checked_add(TimeSpan::DAY)
            .map_err(|_error| invalid())?;
    }
    Err(invalid())
}
