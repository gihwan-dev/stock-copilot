use crate::market_read::{currency, invalid, price, timestamp};
use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossAccountRead};
use application::{Clock, PortError, ReadEndpoint, SecretStore};
use domain::{
    Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime, NumericInput, Ohlcv,
    PriceAdjustment, Quantity, QuoteInstrument, TimeInterval, TimeSpan, Timestamp,
};
use std::collections::BTreeMap;
const ENDPOINT: ReadEndpoint = ReadEndpoint::Candles;
const PAGE_LIMIT: usize = 64;

pub(crate) fn format_time(value: Timestamp) -> Result<String, PortError> {
    let parsed = time::OffsetDateTime::from_unix_timestamp(*value.utc_seconds().as_i64())
        .and_then(|value_time| value_time.replace_nanosecond(*value.nanosecond().as_u32()))
        .map_err(|_error| invalid(ENDPOINT))?;
    Ok(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}Z",
        parsed.year(),
        u8::from(parsed.month()),
        parsed.day(),
        parsed.hour(),
        parsed.minute(),
        parsed.second(),
        parsed.nanosecond()
    ))
}

fn candle_request(
    instrument: &QuoteInstrument,
    period: CandlePeriod,
    adjustment: PriceAdjustment,
    before: &str,
) -> ReadRequest {
    ReadRequest::new(ReadRoute::Candles)
        .with_query(QueryKey::Symbol, instrument.symbol().as_str().to_owned())
        .with_query(
            QueryKey::Interval,
            match period {
                CandlePeriod::Daily => "1d",
                CandlePeriod::Minute => "1m",
            }
            .to_owned(),
        )
        .with_query(QueryKey::Count, "200".to_owned())
        .with_query(QueryKey::Before, before.to_owned())
        .with_query(
            QueryKey::Adjusted,
            match adjustment {
                PriceAdjustment::Raw => "false",
                PriceAdjustment::Adjusted(_) => "true",
            }
            .to_owned(),
        )
}
pub(crate) async fn candles<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    window: TimeInterval,
    period: CandlePeriod,
    adjustment: PriceAdjustment,
) -> Result<Vec<Candle>, PortError> {
    let mut before = format_time(window.end())?;
    let mut boundary = window.end();
    let mut result = BTreeMap::new();
    for _page in 0..PAGE_LIMIT {
        let value = reader
            .market_read(
                candle_request(instrument, period, adjustment, &before),
                ENDPOINT,
            )
            .await?;
        let decoded: broker_toss_schema::types::CandlePageResponse =
            serde_json::from_value(value).map_err(|_error| invalid(ENDPOINT))?;
        if decoded.candles.len() > 200 {
            return Err(invalid(ENDPOINT));
        }
        let mut previous = None;
        for row in decoded.candles {
            let parsed = parse(reader, &row, instrument, period)?;
            let at = parsed.time.timestamp();
            if at > boundary || previous.is_some_and(|newer| newer <= at) {
                return Err(invalid(ENDPOINT));
            }
            previous = Some(at);
            if at < window.start() || at >= window.end() {
                continue;
            }
            let candle = assemble(reader, parsed, instrument, adjustment)?;
            if result.get(&at).is_some_and(|old| !same(old, &candle)) {
                return Err(invalid(ENDPOINT));
            }
            result.entry(at).or_insert(candle);
        }
        if previous.is_some_and(|at| at <= window.start()) || decoded.next_before.is_none() {
            let candles = result.into_values().collect::<Vec<_>>();
            validate_gaps(reader, instrument, period, &candles)?;
            validate_adjusted_moves(reader, instrument, period, adjustment, &candles)?;
            return Ok(candles);
        }
        let next = decoded.next_before.ok_or_else(|| invalid(ENDPOINT))?;
        let next_time = timestamp(&next, ENDPOINT)?;
        if next_time >= boundary || previous.is_none_or(|oldest| next_time > oldest) {
            return Err(invalid(ENDPOINT));
        }
        before = next;
        boundary = next_time;
    }
    Err(invalid(ENDPOINT))
}
fn same(left: &Candle, right: &Candle) -> bool {
    left.time() == right.time()
        && left.values() == right.values()
        && left.adjustment() == right.adjustment()
        && left.status() == right.status()
}
#[derive(Clone, Copy)]
struct ParsedCandle {
    time: CandleTime,
    values: Ohlcv,
    collected_at: Timestamp,
}
fn parse<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    row: &broker_toss_schema::types::Candle,
    instrument: &QuoteInstrument,
    period: CandlePeriod,
) -> Result<ParsedCandle, PortError> {
    let at = timestamp(&row.timestamp, ENDPOINT)?;
    let observed = reader.market_now();
    let unit = currency(&row.currency.to_string(), ENDPOINT)?;
    if unit != instrument.currency() || at > observed {
        return Err(invalid(ENDPOINT));
    }
    let values = Ohlcv::try_new(
        price(&row.open_price, unit, ENDPOINT)?,
        price(&row.high_price, unit, ENDPOINT)?,
        price(&row.low_price, unit, ENDPOINT)?,
        price(&row.close_price, unit, ENDPOINT)?,
        NumericInput::try_new(&row.volume)
            .and_then(|input| Quantity::parse(&input))
            .map_err(|_error| invalid(ENDPOINT))?,
    )
    .map_err(|_error| invalid(ENDPOINT))?;
    let time = CandleTime::try_new(at, period).map_err(|_error| invalid(ENDPOINT))?;
    Ok(ParsedCandle {
        time,
        values,
        collected_at: observed,
    })
}
fn assemble<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    parsed: ParsedCandle,
    instrument: &QuoteInstrument,
    adjustment: PriceAdjustment,
) -> Result<Candle, PortError> {
    let status = match parsed.time.period() {
        CandlePeriod::Minute => CandleStatus::Finalized,
        CandlePeriod::Daily => {
            let session = session_for(reader, instrument, parsed.time.date())?;
            let end = session
                .sessions()
                .iter()
                .map(|segment| segment.session().interval().end())
                .max()
                .ok_or_else(|| invalid(ENDPOINT))?;
            if parsed.collected_at >= end {
                CandleStatus::Finalized
            } else {
                CandleStatus::InProgress
            }
        }
    };
    Candle::try_new(
        instrument.instrument_id().clone(),
        parsed.time,
        adjustment,
        parsed.values,
        CandleObservation::new(parsed.collected_at, status),
    )
    .map_err(|_error| invalid(ENDPOINT))
}
fn validate_gaps<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    period: CandlePeriod,
    candles: &[Candle],
) -> Result<(), PortError> {
    for [left, right] in candles.array_windows::<2>() {
        let next = left.time().timestamp();
        match period {
            CandlePeriod::Minute => {
                let span =
                    TimeSpan::try_new(domain::SpanSeconds::new(60), domain::Nanosecond::new(0))
                        .map_err(|_error| invalid(ENDPOINT))?;
                if next.checked_add(span).map_err(|_error| invalid(ENDPOINT))?
                    != right.time().timestamp()
                {
                    return Err(invalid(ENDPOINT));
                }
            }
            CandlePeriod::Daily => {
                validate_daily_gap(reader, instrument, next, right.time().timestamp())?;
            }
        }
    }
    Ok(())
}

fn session_for<'a, S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &'a TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    date: domain::CalendarDate,
) -> Result<&'a domain::MarketSession, PortError> {
    let mut matching = reader.market_sessions().iter().filter(|entry| {
        entry.market() == domain::Market::from(*instrument.market()) && entry.date() == date
    });
    let session = matching
        .next()
        .ok_or_else(|| unsupported(application::BrokerCapability::DailyCandles))?;
    if matching.next().is_some() {
        return Err(invalid(ENDPOINT));
    }
    Ok(session)
}
fn validate_daily_gap<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    left: Timestamp,
    right: Timestamp,
) -> Result<(), PortError> {
    if left.offset_calendar_date() >= right.offset_calendar_date() {
        return Err(invalid(ENDPOINT));
    }
    let mut next = left
        .checked_add(TimeSpan::DAY)
        .map_err(|_error| invalid(ENDPOINT))?;
    while next.offset_calendar_date() < right.offset_calendar_date() {
        if !session_for(reader, instrument, next.offset_calendar_date())?
            .sessions()
            .is_empty()
        {
            return Err(invalid(ENDPOINT));
        }
        next = next
            .checked_add(TimeSpan::DAY)
            .map_err(|_error| invalid(ENDPOINT))?;
    }
    Ok(())
}

fn unsupported(capability: application::BrokerCapability) -> PortError {
    PortError::new(
        ENDPOINT,
        application::HttpStatus::Success,
        application::PortErrorKind::Unsupported(capability),
    )
}
fn validate_adjusted_moves<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    period: CandlePeriod,
    adjustment: PriceAdjustment,
    candles: &[Candle],
) -> Result<(), PortError> {
    match period {
        CandlePeriod::Minute => return Ok(()),
        CandlePeriod::Daily => {}
    }
    match adjustment {
        PriceAdjustment::Raw => return Ok(()),
        PriceAdjustment::Adjusted(_) => {}
    }
    let master = reader
        .market_master()
        .instruments()
        .iter()
        .find(|entry| {
            entry
                .quote_instrument()
                .is_ok_and(|quoted| &quoted == instrument)
        })
        .ok_or_else(|| unsupported(application::BrokerCapability::AdjustedHistory))?;
    for [previous, current] in candles.array_windows::<2>() {
        check_move(
            master,
            current.time().date(),
            previous.values().close(),
            current.values().low(),
        )?;
        check_move(
            master,
            current.time().date(),
            previous.values().close(),
            current.values().high(),
        )?;
    }
    Ok(())
}
fn check_move(
    master: &domain::MasterInstrument,
    date: domain::CalendarDate,
    previous: domain::Price,
    current: domain::Price,
) -> Result<(), PortError> {
    let result = domain::PriceLimits::check_adjusted_move(master, date, previous, current)
        .map_err(|_error| unsupported(application::BrokerCapability::AdjustedHistory))?;
    match result {
        domain::PriceLimitCheck::Within | domain::PriceLimitCheck::Unlimited => Ok(()),
        domain::PriceLimitCheck::BelowLower | domain::PriceLimitCheck::AboveUpper => {
            Err(invalid(ENDPOINT))
        }
    }
}
