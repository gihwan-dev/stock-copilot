use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossAccountRead};
use application::{Clock, PortError, RankingQuery, ReadEndpoint, RequestRejection, SecretStore};
use domain::{
    KnownMarket, MasterCode, Money, NumericInput, Quantity, RankingCautionFilter, RankingDuration,
    RankingKind, RankingOrdinal, RankingResponseInput, RankingRow, StockRanking, Symbol,
};
use serde_json::Value;
const ENDPOINT: ReadEndpoint = ReadEndpoint::Rankings;
fn invalid() -> PortError {
    crate::market_read::invalid(ENDPOINT)
}
fn unsupported() -> PortError {
    crate::market_read::rejected(ENDPOINT, RequestRejection::UnsupportedQuery)
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, PortError> {
    crate::market_read::text(value, field, ENDPOINT)
}
fn required<'a>(value: &'a Value, field: &str) -> Result<&'a Value, PortError> {
    value.get(field).ok_or_else(invalid)
}
fn number(value: &Value, field: &str) -> Result<NumericInput, PortError> {
    NumericInput::try_new(text(value, field)?).map_err(|_error| invalid())
}
fn kind(kind: RankingKind) -> Result<&'static str, PortError> {
    match kind {
        RankingKind::MarketTradingAmount => Ok("MARKET_TRADING_AMOUNT"),
        RankingKind::MarketTradingVolume => Ok("MARKET_TRADING_VOLUME"),
        RankingKind::TossTradingAmount => Ok("TOSS_SECURITIES_TRADING_AMOUNT"),
        RankingKind::TossTradingVolume => Ok("TOSS_SECURITIES_TRADING_VOLUME"),
        RankingKind::TopGainers => Ok("TOP_GAINERS"),
        RankingKind::TopLosers => Ok("TOP_LOSERS"),
        RankingKind::Unknown => Err(unsupported()),
    }
}
fn duration(value: RankingDuration) -> Result<&'static str, PortError> {
    match value {
        RankingDuration::Realtime => Ok("realtime"),
        RankingDuration::Day => Ok("1d"),
        RankingDuration::Week => Ok("1w"),
        RankingDuration::Month => Ok("1mo"),
        RankingDuration::ThreeMonths => Ok("3mo"),
        RankingDuration::SixMonths => Ok("6mo"),
        RankingDuration::Year => Ok("1y"),
        RankingDuration::Unknown => Err(unsupported()),
    }
}
pub(crate) async fn rankings<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    query: &RankingQuery,
) -> Result<StockRanking, PortError> {
    let kind_code = kind(query.kind())?;
    let duration_code = duration(query.duration())?;
    let market = match query.market() {
        KnownMarket::Kr => "KR",
        KnownMarket::Us => "US",
    };
    let caution = match query.caution_filter() {
        RankingCautionFilter::Included => "false",
        RankingCautionFilter::Excluded => "true",
    };
    let request = ReadRequest::new(ReadRoute::Rankings)
        .with_query(QueryKey::Type, kind_code.to_owned())
        .with_query(QueryKey::MarketCountry, market.to_owned())
        .with_query(QueryKey::Duration, duration_code.to_owned())
        .with_query(QueryKey::Count, query.limit().as_usize().to_string())
        .with_query(QueryKey::ExcludeInvestmentCaution, caution.to_owned());
    let value = reader.market_read(request, ENDPOINT).await?;
    let records = required(&value, "rankings")?
        .as_array()
        .ok_or_else(invalid)?;
    if records.len() > *query.limit().as_usize() {
        return Err(invalid());
    }
    let raw_at = required(&value, "rankedAt")?;
    let ranked_at = if raw_at.is_null() {
        None
    } else {
        Some(crate::market_read::timestamp(
            raw_at.as_str().ok_or_else(invalid)?,
            ENDPOINT,
        )?)
    };
    let mut previous = 0_usize;
    let mut rows = Vec::new();
    for record in records {
        let row = parse_row(record)?;
        let rank = *row.rank.as_usize();
        if rank <= previous {
            return Err(invalid());
        }
        previous = rank;
        rows.push(row);
    }
    StockRanking::from_response(RankingResponseInput {
        kind: MasterCode::try_new(kind_code).map_err(|_error| invalid())?,
        market: query.market(),
        duration: MasterCode::try_new(duration_code).map_err(|_error| invalid())?,
        limit: query.limit(),
        rows,
        ranked_at,
        collected: reader.market_now(),
        caution_filter: query.caution_filter(),
    })
    .map_err(|_error| invalid())
}

fn parse_row(record: &Value) -> Result<RankingRow, PortError> {
    let rank = usize::try_from(required(record, "rank")?.as_u64().ok_or_else(invalid)?)
        .map_err(|_error| invalid())?;
    let currency = crate::market_read::currency(text(record, "currency")?, ENDPOINT)?;
    let price = required(record, "price")?;
    let last = number(price, "lastPrice")?;
    domain::Price::parse(&last, currency).map_err(|_error| invalid())?;
    let base = number(price, "basePrice")?;
    Money::parse(&base, currency).map_err(|_error| invalid())?;
    let raw_change = required(price, "changeRate")?;
    let change_rate = if raw_change.is_null() {
        None
    } else {
        Some(
            NumericInput::try_new(raw_change.as_str().ok_or_else(invalid)?)
                .map_err(|_error| invalid())?,
        )
    };
    let volume = number(record, "tradingVolume")?;
    let amount = number(record, "tradingAmount")?;
    Quantity::parse(&volume).map_err(|_error| invalid())?;
    Money::parse(&amount, currency).map_err(|_error| invalid())?;
    Ok(RankingRow {
        rank: RankingOrdinal::new(rank),
        symbol: Symbol::try_new(text(record, "symbol")?).map_err(|_error| invalid())?,
        currency,
        volume,
        amount,
        change_rate,
    })
}
