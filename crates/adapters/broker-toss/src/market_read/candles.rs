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

#[derive(Clone, Copy)]
pub(crate) struct CandleMode {
    pub adjustment: PriceAdjustment,
    pub master_errors: crate::account_read::MasterErrorMode,
}

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
    mode: CandleMode,
) -> Result<Vec<Candle>, PortError> {
    let mut before = format_time(window.end())?;
    let mut boundary = window.end();
    let mut result = BTreeMap::new();
    for _page in 0..PAGE_LIMIT {
        let value = reader
            .market_read(
                candle_request(instrument, period, mode.adjustment, &before),
                ENDPOINT,
            )
            .await?;
        let decoded: broker_toss_schema::types::CandlePageResponse =
            serde_json::from_value(value).map_err(|_error| rejected_at("candles:candles:1"))?;
        if decoded.candles.len() > 200 {
            return Err(rejected_at("candles:candles:2"));
        }
        let mut previous = None;
        for row in decoded.candles {
            let parsed = parse(reader, &row, instrument, period)?;
            let at = parsed.time.timestamp();
            if at > boundary || previous.is_some_and(|newer| newer <= at) {
                return Err(rejected_at("candles:candles:3"));
            }
            previous = Some(at);
            if at < window.start() || at >= window.end() {
                continue;
            }
            let candle = assemble(reader, parsed, instrument, mode.adjustment)?;
            if result.get(&at).is_some_and(|old| !same(old, &candle)) {
                return Err(rejected_at("candles:candles:4"));
            }
            result.entry(at).or_insert(candle);
        }
        if previous.is_some_and(|at| at <= window.start()) || decoded.next_before.is_none() {
            let candles = result.into_values().collect::<Vec<_>>();
            validate_gaps(reader, instrument, period, &candles)?;
            validate_adjusted_moves(reader, instrument, period, mode, &candles)?;
            return Ok(candles);
        }
        let next = decoded
            .next_before
            .ok_or_else(|| rejected_at("candles:candles:5"))?;
        let next_time = timestamp(&next, ENDPOINT)?;
        if next_time >= boundary || previous.is_none_or(|oldest| next_time > oldest) {
            return Err(rejected_at("candles:candles:6"));
        }
        before = next;
        boundary = next_time;
    }
    Err(rejected_at("candles:candles:7"))
}
fn history_status<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    parsed: &ParsedCandle,
) -> Option<CandleStatus> {
    let settled = parsed
        .time
        .timestamp()
        .checked_add(TimeSpan::DAY)
        .is_ok_and(|after_a_day| after_a_day <= parsed.collected_at);
    if settled {
        return Some(CandleStatus::Finalized);
    }
    let session = session_for(reader, instrument, parsed.time.date()).ok()?;
    domain::regular_session_candle_status(session, parsed.collected_at).ok()
}

#[derive(Default)]
struct HistorySkips {
    unreadable: usize,
    future: usize,
    open_session: usize,
    candle: usize,
    shape: Option<String>,
}

impl HistorySkips {
    fn record(&self) {
        let skipped = self
            .unreadable
            .saturating_add(self.future)
            .saturating_add(self.open_session)
            .saturating_add(self.candle);
        if skipped == 0 && self.shape.is_none() {
            return;
        }
        crate::diagnostics::validation_failed(
            ENDPOINT,
            &format!(
                "history-skipped unreadable={} future={} open-session={} candle={} daily-time={}",
                self.unreadable,
                self.future,
                self.open_session,
                self.candle,
                self.shape.as_deref().unwrap_or("midnight")
            ),
        );
    }
}

fn local_midnight(at: Timestamp) -> Option<(Timestamp, Option<String>)> {
    let offset = i64::from(*at.offset_seconds().as_i32());
    let local = at.utc_seconds().as_i64().checked_add(offset)?;
    let into_day = local.rem_euclid(86_400);
    let midnight = Timestamp::from_parts(
        domain::UnixSeconds::new(at.utc_seconds().as_i64().checked_sub(into_day)?),
        domain::Nanosecond::new(0),
        at.offset_seconds(),
    )
    .ok()?;
    let shape = (into_day != 0 || *at.nanosecond().as_u32() != 0)
        .then(|| format!("{into_day}s-into-day@offset{offset:+}s"));
    Some((midnight, shape))
}

fn history_parse<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    row: &broker_toss_schema::types::Candle,
    instrument: &QuoteInstrument,
    skips: &mut HistorySkips,
) -> Option<ParsedCandle> {
    let observed = reader.market_now();
    let Some((day, shape)) = timestamp(&row.timestamp, ENDPOINT)
        .ok()
        .and_then(local_midnight)
    else {
        skips.unreadable = skips.unreadable.saturating_add(1);
        return None;
    };
    if skips.shape.is_none() {
        skips.shape = shape;
    }
    if day > observed {
        skips.future = skips.future.saturating_add(1);
        return None;
    }
    let unit = currency(&row.currency.to_string(), ENDPOINT).ok()?;
    let values = (unit == instrument.currency())
        .then(|| {
            Ohlcv::try_new(
                price(&row.open_price, unit, ENDPOINT).ok()?,
                price(&row.high_price, unit, ENDPOINT).ok()?,
                price(&row.low_price, unit, ENDPOINT).ok()?,
                price(&row.close_price, unit, ENDPOINT).ok()?,
                NumericInput::try_new(&row.volume)
                    .and_then(|input| Quantity::parse(&input))
                    .ok()?,
            )
            .ok()
        })
        .flatten();
    let parsed = values.and_then(|ohlcv| {
        CandleTime::try_new(day, CandlePeriod::Daily)
            .ok()
            .map(|time| ParsedCandle {
                time,
                values: ohlcv,
                collected_at: observed,
            })
    });
    if parsed.is_none() {
        skips.unreadable = skips.unreadable.saturating_add(1);
    }
    parsed
}

fn history_candle<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    row: &broker_toss_schema::types::Candle,
    skips: &mut HistorySkips,
) -> Option<Candle> {
    let parsed = history_parse(reader, row, instrument, skips)?;
    let Some(status) = history_status(reader, instrument, &parsed) else {
        skips.open_session = skips.open_session.saturating_add(1);
        return None;
    };
    let candle = Candle::try_new(
        instrument.instrument_id().clone(),
        parsed.time,
        PriceAdjustment::Raw,
        parsed.values,
        CandleObservation::new(parsed.collected_at, status),
    )
    .ok();
    if candle.is_none() {
        skips.candle = skips.candle.saturating_add(1);
    }
    candle
}

pub(crate) async fn history<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instrument: &QuoteInstrument,
    window: TimeInterval,
) -> Result<Vec<Candle>, PortError> {
    let mut before = format_time(window.end())?;
    let mut boundary = window.end();
    let mut result = BTreeMap::new();
    let mut skips = HistorySkips::default();
    for _page in 0..PAGE_LIMIT {
        let value = reader
            .market_read(
                candle_request(
                    instrument,
                    CandlePeriod::Daily,
                    PriceAdjustment::Raw,
                    &before,
                ),
                ENDPOINT,
            )
            .await?;
        let decoded: broker_toss_schema::types::CandlePageResponse = serde_json::from_value(value)
            .map_err(|error| {
                crate::diagnostics::decode_failed(ENDPOINT, &error);
                invalid(ENDPOINT)
            })?;
        let mut oldest: Option<Timestamp> = None;
        for row in &decoded.candles {
            let Some(candle) = history_candle(reader, instrument, row, &mut skips) else {
                continue;
            };
            let at = candle.time().timestamp();
            oldest = Some(oldest.map_or(at, |known| known.min(at)));
            if at >= window.start() && at < window.end() {
                let _kept = result.entry(at).or_insert(candle);
            }
        }
        let reached = oldest.is_none_or(|at| at <= window.start());
        let Some(next) = decoded.next_before.filter(|_more| !reached) else {
            skips.record();
            return Ok(result.into_values().collect());
        };
        let next_time = timestamp(&next, ENDPOINT)?;
        if next_time >= boundary {
            crate::diagnostics::validation_failed(ENDPOINT, "history-cursor-not-older");
            skips.record();
            return Ok(result.into_values().collect());
        }
        before = next;
        boundary = next_time;
    }
    skips.record();
    Ok(result.into_values().collect())
}

fn rejected_at(stage: &str) -> PortError {
    crate::diagnostics::validation_failed(ENDPOINT, stage);
    invalid(ENDPOINT)
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
        return Err(rejected_at("candles:parse:1"));
    }
    let values = Ohlcv::try_new(
        price(&row.open_price, unit, ENDPOINT)?,
        price(&row.high_price, unit, ENDPOINT)?,
        price(&row.low_price, unit, ENDPOINT)?,
        price(&row.close_price, unit, ENDPOINT)?,
        NumericInput::try_new(&row.volume)
            .and_then(|input| Quantity::parse(&input))
            .map_err(|_error| rejected_at("candles:parse:2"))?,
    )
    .map_err(|_error| rejected_at("candles:parse:3"))?;
    let time = CandleTime::try_new(at, period).map_err(|_error| rejected_at("candles:parse:4"))?;
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
            domain::regular_session_candle_status(session, parsed.collected_at)
                .map_err(|_error| rejected_at("candles:assemble:1"))?
        }
    };
    Candle::try_new(
        instrument.instrument_id().clone(),
        parsed.time,
        adjustment,
        parsed.values,
        CandleObservation::new(parsed.collected_at, status),
    )
    .map_err(|_error| rejected_at("candles:assemble:2"))
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
                        .map_err(|_error| rejected_at("candles:validate_gaps:1"))?;
                if next
                    .checked_add(span)
                    .map_err(|_error| rejected_at("candles:validate_gaps:2"))?
                    != right.time().timestamp()
                {
                    return Err(rejected_at("candles:validate_gaps:3"));
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
        return Err(rejected_at("candles:session_for:1"));
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
        return Err(rejected_at("candles:validate_daily_gap:1"));
    }
    let mut next = left
        .checked_add(TimeSpan::DAY)
        .map_err(|_error| rejected_at("candles:validate_daily_gap:2"))?;
    while next.offset_calendar_date() < right.offset_calendar_date() {
        if !session_for(reader, instrument, next.offset_calendar_date())?
            .sessions()
            .is_empty()
        {
            return Err(rejected_at("candles:validate_daily_gap:3"));
        }
        next = next
            .checked_add(TimeSpan::DAY)
            .map_err(|_error| rejected_at("candles:validate_daily_gap:4"))?;
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
    mode: CandleMode,
    candles: &[Candle],
) -> Result<(), PortError> {
    match period {
        CandlePeriod::Minute => return Ok(()),
        CandlePeriod::Daily => {}
    }
    match mode.adjustment {
        PriceAdjustment::Raw => return Ok(()),
        PriceAdjustment::Adjusted(_) => {}
    }
    let facts = reader.market_master();
    let master = facts
        .instruments()
        .iter()
        .find(|entry| {
            entry
                .quote_instrument()
                .is_ok_and(|quoted| &quoted == instrument)
        })
        .ok_or_else(|| {
            mode.master_errors.missing(
                ENDPOINT,
                unsupported(application::BrokerCapability::AdjustedHistory),
            )
        })?;
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
        .map_err(|error| match error {
            domain::PriceLimitsError::MissingLimits => {
                unsupported(application::BrokerCapability::AdjustedHistory)
            }
            domain::PriceLimitsError::CurrencyMismatch
            | domain::PriceLimitsError::DifferentInstrument
            | domain::PriceLimitsError::UnexpectedLimits
            | domain::PriceLimitsError::ReversedLimits
            | domain::PriceLimitsError::FutureTimestamp => invalid(ENDPOINT),
        })?;
    match result {
        domain::PriceLimitCheck::Within | domain::PriceLimitCheck::Unlimited => Ok(()),
        domain::PriceLimitCheck::BelowLower | domain::PriceLimitCheck::AboveUpper => {
            Err(invalid(ENDPOINT))
        }
    }
}
