use crate::{Database, sql::market as sql};
use application::{
    CacheWriteOutcome, FxRateQuery, MarketDataRepository, SystemPortError, WriteCompleted,
};
use domain::{
    AdjustmentBasis, Candle, CandleObservation, CandlePeriod, CandleStatus, CandleTime, Currency,
    FxPair, FxRate, FxRateKind, FxSource, InstrumentId, Nanosecond, NumericInput, OffsetSeconds,
    Ohlcv, Price, PriceAdjustment, Quantity, Quote, Symbol, TimeInterval, Timestamp, UnixSeconds,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

#[derive(Debug)]
pub struct MarketStore<'database> {
    database: &'database Database,
}
impl<'database> MarketStore<'database> {
    #[must_use]
    pub const fn new(database: &'database Database) -> Self {
        Self { database }
    }
}
fn invalid<E>(_error: E) -> rusqlite::Error {
    rusqlite::Error::InvalidQuery
}
fn timestamp(row: &Row<'_>, index: usize) -> rusqlite::Result<Timestamp> {
    Timestamp::from_parts(
        UnixSeconds::new(row.get(index)?),
        Nanosecond::new(row.get(index.checked_add(1).ok_or_else(|| invalid(()))?)?),
        OffsetSeconds::new(row.get(index.checked_add(2).ok_or_else(|| invalid(()))?)?),
    )
    .map_err(invalid)
}
fn currency(text: &str) -> rusqlite::Result<Currency> {
    match text {
        "KRW" => Ok(Currency::Krw),
        "USD" => Ok(Currency::Usd),
        _ => Err(invalid(())),
    }
}
fn currency_tag(value: Currency) -> &'static str {
    match value {
        Currency::Krw => "KRW",
        Currency::Usd => "USD",
    }
}
fn pair_tag(value: FxPair) -> &'static str {
    match value {
        FxPair::UsdKrw => "USD/KRW",
        FxPair::KrwUsd => "KRW/USD",
    }
}
fn kind_tag(value: FxRateKind) -> &'static str {
    match value {
        FxRateKind::Mid => "mid",
        FxRateKind::Buy => "buy",
    }
}
fn price(text: &str, unit: Currency) -> rusqlite::Result<Price> {
    Price::parse(&NumericInput::try_new(text).map_err(invalid)?, unit).map_err(invalid)
}
fn text_price(value: Price) -> String {
    value.as_money().amount().as_decimal().to_string()
}
fn optional_timestamp(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<Timestamp>> {
    match (
        row.get::<_, Option<i64>>(index)?,
        row.get::<_, Option<u32>>(index.checked_add(1).ok_or_else(|| invalid(()))?)?,
        row.get::<_, Option<i32>>(index.checked_add(2).ok_or_else(|| invalid(()))?)?,
    ) {
        (Some(seconds), Some(nanos), Some(offset)) => Ok(Some(
            Timestamp::from_parts(
                UnixSeconds::new(seconds),
                Nanosecond::new(nanos),
                OffsetSeconds::new(offset),
            )
            .map_err(invalid)?,
        )),
        (None, None, None) => Ok(None),
        _ => Err(invalid(())),
    }
}
struct CachedQuote {
    quote: Quote,
    last_server: Option<Timestamp>,
}
fn decode_quote(row: &Row<'_>) -> rusqlite::Result<(InstrumentId, CachedQuote)> {
    let unit = currency(&row.get::<_, String>(2)?)?;
    let observed = optional_timestamp(row, 4)?;
    let last_server = optional_timestamp(row, 10)?;
    match (observed, last_server) {
        (Some(actual), Some(last)) if actual == last => {}
        (None, Some(_) | None) => {}
        (Some(_), Some(_) | None) => return Err(invalid(())),
    }
    let quote = Quote::try_new(
        Symbol::try_new(&row.get::<_, String>(1)?).map_err(invalid)?,
        unit,
        row.get::<_, Option<String>>(3)?
            .map(|text| price(&text, unit))
            .transpose()?,
        observed,
        timestamp(row, 7)?,
    )
    .map_err(invalid)?;
    if let Some(last) = last_server {
        Quote::try_new(
            quote.symbol().clone(),
            quote.currency(),
            quote.last_price(),
            Some(last),
            quote.collected_at(),
        )
        .map_err(invalid)?;
    }
    Ok((
        InstrumentId::try_new(&row.get::<_, String>(0)?).map_err(invalid)?,
        CachedQuote { quote, last_server },
    ))
}
fn load_quote(
    connection: &Connection,
    instrument: &InstrumentId,
) -> rusqlite::Result<Option<CachedQuote>> {
    let found = connection
        .query_row(sql::QUOTE, [instrument.as_str()], decode_quote)
        .optional()?;
    match found {
        Some((id, quote)) if id == *instrument => Ok(Some(quote)),
        None => Ok(None),
        Some(_) => Err(invalid(())),
    }
}
fn save_quote(
    tx: &Transaction<'_>,
    id: &InstrumentId,
    quote: &Quote,
) -> rusqlite::Result<CacheWriteOutcome> {
    let observed = quote.timestamp();
    let last_server = if let Some(current) = load_quote(tx, id)? {
        if quote.collected_at() < current.quote.collected_at() {
            return Ok(CacheWriteOutcome::OlderIgnored);
        }
        if let (Some(actual), Some(previous)) = (observed, current.last_server)
            && actual < previous
        {
            return Ok(CacheWriteOutcome::OlderIgnored);
        }
        if *quote == current.quote {
            return Ok(CacheWriteOutcome::Unchanged);
        }
        observed.or(current.last_server)
    } else {
        observed
    };
    let collected = quote.collected_at();
    tx.execute(sql::DELETE_QUOTE, [id.as_str()])?;
    tx.execute(
        sql::INSERT_QUOTE,
        params![
            id.as_str(),
            quote.symbol().as_str(),
            currency_tag(quote.currency()),
            quote.last_price().map(text_price),
            observed.map(|at| *at.utc_seconds().as_i64()),
            observed.map(|at| *at.nanosecond().as_u32()),
            observed.map(|at| *at.offset_seconds().as_i32()),
            collected.utc_seconds().as_i64(),
            collected.nanosecond().as_u32(),
            collected.offset_seconds().as_i32(),
            last_server.map(|at| *at.utc_seconds().as_i64()),
            last_server.map(|at| *at.nanosecond().as_u32()),
            last_server.map(|at| *at.offset_seconds().as_i32()),
        ],
    )?;
    Ok(CacheWriteOutcome::Changed)
}
fn decode_fx(row: &Row<'_>) -> rusqlite::Result<FxRate> {
    let pair = match row.get::<_, String>(0)?.as_str() {
        "USD/KRW" => FxPair::UsdKrw,
        "KRW/USD" => FxPair::KrwUsd,
        _ => return Err(invalid(())),
    };
    let kind = match row.get::<_, String>(1)?.as_str() {
        "mid" => FxRateKind::Mid,
        "buy" => FxRateKind::Buy,
        _ => return Err(invalid(())),
    };
    if currency(&row.get::<_, String>(8)?)? != pair.quote() {
        return Err(invalid(()));
    }
    let source = match row.get::<_, String>(10)?.as_str() {
        "toss" => FxSource::TossOpenApi,
        _ => return Err(invalid(())),
    };
    FxRate::parse(
        &NumericInput::try_new(&row.get::<_, String>(9)?).map_err(invalid)?,
        pair,
        kind,
        TimeInterval::try_new(timestamp(row, 2)?, timestamp(row, 5)?).map_err(invalid)?,
        source,
    )
    .map_err(invalid)
}
fn load_fx(
    connection: &Connection,
    pair: FxPair,
    kind: FxRateKind,
) -> rusqlite::Result<Vec<FxRate>> {
    connection
        .prepare(sql::FX_RATES)?
        .query_map(params![pair_tag(pair), kind_tag(kind)], decode_fx)?
        .collect()
}
fn save_fx(tx: &Transaction<'_>, rate: FxRate) -> rusqlite::Result<CacheWriteOutcome> {
    let existing = load_fx(tx, rate.pair(), rate.kind())?;
    let previous = existing.first().copied();
    let outcome = match previous {
        Some(current) if rate.as_of() < current.as_of() => {
            return Ok(CacheWriteOutcome::OlderIgnored);
        }
        Some(current)
            if rate.as_of() == current.as_of()
                && rate.quote_per_base() == current.quote_per_base()
                && rate.source() == current.source() =>
        {
            CacheWriteOutcome::Unchanged
        }
        Some(_) | None => CacheWriteOutcome::Changed,
    };
    let start = rate.as_of();
    let end = rate.valid_until();
    tx.execute(
        sql::DELETE_FX,
        params![
            pair_tag(rate.pair()),
            kind_tag(rate.kind()),
            start.utc_seconds().as_i64(),
            start.nanosecond().as_u32()
        ],
    )?;
    tx.execute(
        sql::INSERT_FX,
        params![
            pair_tag(rate.pair()),
            kind_tag(rate.kind()),
            start.utc_seconds().as_i64(),
            start.nanosecond().as_u32(),
            start.offset_seconds().as_i32(),
            end.utc_seconds().as_i64(),
            end.nanosecond().as_u32(),
            end.offset_seconds().as_i32(),
            currency_tag(rate.pair().quote()),
            text_price(rate.quote_per_base()),
            match rate.source() {
                FxSource::TossOpenApi => "toss",
            },
        ],
    )?;
    Ok(outcome)
}
fn period_tag(value: CandlePeriod) -> &'static str {
    match value {
        CandlePeriod::Daily => "daily",
        CandlePeriod::Minute => "minute",
    }
}
fn adjustment_parts(value: PriceAdjustment) -> (&'static str, String, Option<i32>) {
    match value {
        PriceAdjustment::Raw => ("raw", String::new(), None),
        PriceAdjustment::Adjusted(basis) => {
            let at = *basis.as_timestamp();
            (
                "adjusted",
                [
                    at.utc_seconds().as_i64().to_string(),
                    at.nanosecond().as_u32().to_string(),
                ]
                .join(":"),
                Some(*at.offset_seconds().as_i32()),
            )
        }
    }
}
fn decode_adjustment(
    kind: &str,
    basis: &str,
    offset: Option<i32>,
) -> rusqlite::Result<PriceAdjustment> {
    match (kind, basis, offset) {
        ("raw", "", None) => Ok(PriceAdjustment::Raw),
        ("adjusted", text, Some(zone)) => {
            let (seconds, nanos) = text.split_once(':').ok_or_else(|| invalid(()))?;
            let at = Timestamp::from_parts(
                UnixSeconds::new(seconds.parse().map_err(invalid)?),
                Nanosecond::new(nanos.parse().map_err(invalid)?),
                OffsetSeconds::new(zone),
            )
            .map_err(invalid)?;
            Ok(PriceAdjustment::Adjusted(AdjustmentBasis::new(at)))
        }
        _ => Err(invalid(())),
    }
}
fn decode_candle(row: &Row<'_>) -> rusqlite::Result<Candle> {
    let id = InstrumentId::try_new(&row.get::<_, String>(0)?).map_err(invalid)?;
    let period = match row.get::<_, String>(1)?.as_str() {
        "daily" => CandlePeriod::Daily,
        "minute" => CandlePeriod::Minute,
        _ => return Err(invalid(())),
    };
    let adjustment = decode_adjustment(
        &row.get::<_, String>(2)?,
        &row.get::<_, String>(3)?,
        row.get(4)?,
    )?;
    let unit = currency(&row.get::<_, String>(8)?)?;
    let values = Ohlcv::try_new(
        price(&row.get::<_, String>(9)?, unit)?,
        price(&row.get::<_, String>(10)?, unit)?,
        price(&row.get::<_, String>(11)?, unit)?,
        price(&row.get::<_, String>(12)?, unit)?,
        Quantity::parse(&NumericInput::try_new(&row.get::<_, String>(13)?).map_err(invalid)?)
            .map_err(invalid)?,
    )
    .map_err(invalid)?;
    let status = match row.get::<_, String>(17)?.as_str() {
        "finalized" => CandleStatus::Finalized,
        "in_progress" => CandleStatus::InProgress,
        _ => return Err(invalid(())),
    };
    Candle::try_new(
        id,
        CandleTime::try_new(timestamp(row, 5)?, period).map_err(invalid)?,
        adjustment,
        values,
        CandleObservation::new(timestamp(row, 14)?, status),
    )
    .map_err(invalid)
}
fn load_candles(connection: &Connection, id: &InstrumentId) -> rusqlite::Result<Vec<Candle>> {
    connection
        .prepare(sql::CANDLES)?
        .query_map([id.as_str()], decode_candle)?
        .collect()
}
fn save_candle(tx: &Transaction<'_>, candle: &Candle) -> rusqlite::Result<()> {
    let time = candle.time();
    let at = time.timestamp();
    let collected = candle.collected_at();
    let values = candle.values();
    let (adjustment, basis, basis_offset) = adjustment_parts(candle.adjustment());
    tx.execute(
        sql::DELETE_CANDLE,
        params![
            candle.instrument().as_str(),
            period_tag(time.period()),
            adjustment,
            basis,
            at.utc_seconds().as_i64(),
            at.nanosecond().as_u32()
        ],
    )?;
    tx.execute(
        sql::INSERT_CANDLE,
        params![
            candle.instrument().as_str(),
            period_tag(time.period()),
            adjustment,
            basis,
            basis_offset,
            at.utc_seconds().as_i64(),
            at.nanosecond().as_u32(),
            at.offset_seconds().as_i32(),
            currency_tag(values.currency()),
            text_price(values.open()),
            text_price(values.high()),
            text_price(values.low()),
            text_price(values.close()),
            values.volume().as_decimal().to_string(),
            collected.utc_seconds().as_i64(),
            collected.nanosecond().as_u32(),
            collected.offset_seconds().as_i32(),
            match candle.status() {
                CandleStatus::Finalized => "finalized",
                CandleStatus::InProgress => "in_progress",
            },
        ],
    )?;
    Ok(())
}
impl MarketDataRepository for MarketStore<'_> {
    async fn store_quote(
        &self,
        instrument: &InstrumentId,
        quote: &Quote,
    ) -> Result<CacheWriteOutcome, SystemPortError> {
        let id = instrument.clone();
        let value = quote.clone();
        self.database
            .write(move |tx| save_quote(tx, &id, &value))
            .await
    }
    async fn quote(&self, instrument: &InstrumentId) -> Result<Option<Quote>, SystemPortError> {
        let id = instrument.clone();
        self.database
            .read(move |connection| Ok(load_quote(connection, &id)?.map(|cached| cached.quote)))
            .await
    }
    async fn store_candles(
        &self,
        instrument: &InstrumentId,
        candles: &[Candle],
    ) -> Result<WriteCompleted, SystemPortError> {
        let id = instrument.clone();
        let values = candles.to_vec();
        self.database
            .write(move |tx| {
                if values.iter().any(|value| value.instrument() != &id) {
                    return Err(invalid(()));
                }
                for value in &values {
                    save_candle(tx, value)?;
                }
                Ok(WriteCompleted)
            })
            .await
    }
    async fn candles(
        &self,
        instrument: &InstrumentId,
        window: TimeInterval,
        period: CandlePeriod,
        adjustment: PriceAdjustment,
    ) -> Result<Vec<Candle>, SystemPortError> {
        let id = instrument.clone();
        self.database
            .read(move |connection| {
                let mut values = load_candles(connection, &id)?;
                values.retain(|value| {
                    value.time().period() == period
                        && value.adjustment() == adjustment
                        && value.time().timestamp() >= window.start()
                        && value.time().timestamp() < window.end()
                });
                values.sort_by_key(|value| value.time().timestamp());
                Ok(values)
            })
            .await
    }
    async fn store_fx_rate(&self, rate: &FxRate) -> Result<CacheWriteOutcome, SystemPortError> {
        let value = *rate;
        self.database.write(move |tx| save_fx(tx, value)).await
    }
    async fn fx_rate(
        &self,
        pair: FxPair,
        kind: FxRateKind,
        at: FxRateQuery,
    ) -> Result<Option<FxRate>, SystemPortError> {
        self.database
            .read(move |connection| {
                Ok(load_fx(connection, pair, kind)?
                    .into_iter()
                    .find(|value| match at {
                        FxRateQuery::Latest => true,
                        FxRateQuery::At(requested) => value.as_of() <= requested,
                    }))
            })
            .await
    }
}

fn close_evidence(
    connection: &Connection,
    market: &domain::Market,
    symbol: &Symbol,
    date: domain::CalendarDate,
    as_of: Timestamp,
) -> rusqlite::Result<Vec<domain::ObservedCloseEvidence>> {
    let (known, tag, currency) = match market {
        domain::Market::Kr => (domain::KnownMarket::Kr, "KR", Currency::Krw),
        domain::Market::Us => (domain::KnownMarket::Us, "US", Currency::Usd),
        domain::Market::Unknown(_unknown) => return Ok(Vec::new()),
    };
    let mapping = connection
        .query_row(
            sql::ACTIVE_MAPPING_BY_SYMBOL,
            params![tag, symbol.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((raw_id, raw_symbol)) = mapping else {
        return Ok(Vec::new());
    };
    let instrument_id = InstrumentId::try_new(&raw_id).map_err(invalid)?;
    let quote = domain::QuoteInstrument::restore_from_saved_state(domain::SavedQuoteInstrument {
        market: known,
        currency,
        symbol: Symbol::try_new(&raw_symbol).map_err(invalid)?,
        instrument_id: instrument_id.clone(),
    })
    .map_err(invalid)?;
    Ok(load_candles(connection, &instrument_id)?
        .into_iter()
        .filter(|candle| {
            candle.time().period() == CandlePeriod::Daily
                && candle.time().date() == date
                && candle.collected_at() <= as_of
        })
        .map(|candle| domain::ObservedCloseEvidence::new(quote.clone(), candle))
        .collect())
}

impl application::ValuationCloseEvidenceRepository for MarketStore<'_> {
    async fn observed_close_evidence(
        &self,
        instrument: &domain::ResolvedOrderInstrument,
        date: domain::CalendarDate,
        as_of: Timestamp,
    ) -> Result<Vec<domain::ObservedCloseEvidence>, SystemPortError> {
        let market = instrument.market();
        let symbol = instrument.symbol().clone();
        self.database
            .read(move |connection| close_evidence(connection, &market, &symbol, date, as_of))
            .await
    }
}
