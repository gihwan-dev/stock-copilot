use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossAccountRead};
use application::{
    Clock, PortError, ReadEndpoint, RequestRejection, SecretStore, SectorRankingQuery,
};
use domain::{
    DisplayInstrumentName, KnownMarket, NumericInput, RankingDuration, RankingKind, RankingOrdinal,
    SectorDepth, SectorId, SectorRanking, SectorRankingRow,
};
use serde_json::Value;
const ENDPOINT: ReadEndpoint = ReadEndpoint::SectorRankings;
fn invalid() -> PortError {
    crate::market_read::invalid(ENDPOINT)
}
fn unsupported() -> PortError {
    crate::market_read::rejected(ENDPOINT, RequestRejection::UnsupportedQuery)
}
fn required<'a>(value: &'a Value, field: &str) -> Result<&'a Value, PortError> {
    value.get(field).ok_or_else(invalid)
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, PortError> {
    crate::market_read::text(value, field, ENDPOINT)
}
fn optional_number(value: &Value, field: &str) -> Result<Option<NumericInput>, PortError> {
    let raw = required(value, field)?;
    if raw.is_null() {
        Ok(None)
    } else {
        NumericInput::try_new(raw.as_str().ok_or_else(invalid)?)
            .map(Some)
            .map_err(|_error| invalid())
    }
}
fn kind(value: RankingKind) -> Result<&'static str, PortError> {
    match value {
        RankingKind::TopGainers => Ok("TOP_GAINERS"),
        RankingKind::MarketTradingAmount => Ok("MARKET_TRADING_AMOUNT"),
        RankingKind::MarketTradingVolume
        | RankingKind::TossTradingAmount
        | RankingKind::TossTradingVolume
        | RankingKind::TopLosers
        | RankingKind::Unknown => Err(unsupported()),
    }
}
fn duration(value: RankingDuration) -> Result<&'static str, PortError> {
    match value {
        RankingDuration::Day => Ok("1d"),
        RankingDuration::Week => Ok("1w"),
        RankingDuration::Month => Ok("1mo"),
        RankingDuration::ThreeMonths => Ok("3mo"),
        RankingDuration::SixMonths => Ok("6mo"),
        RankingDuration::Year => Ok("1y"),
        RankingDuration::Realtime | RankingDuration::Unknown => Err(unsupported()),
    }
}
pub(crate) async fn fetch<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    query: &SectorRankingQuery,
) -> Result<SectorRanking, PortError> {
    let kind = kind(query.kind())?;
    let period = duration(query.duration())?;
    let market = match query.market() {
        KnownMarket::Kr => "KR",
        KnownMarket::Us => "US",
    };
    let catalog = crate::market_sector::sectors(reader)
        .await
        .map_err(|error| PortError::new(ENDPOINT, error.status(), error.kind()))?;
    let request = ReadRequest::new(ReadRoute::SectorRankings)
        .with_query(QueryKey::Type, kind.to_owned())
        .with_query(QueryKey::MarketCountry, market.to_owned())
        .with_query(QueryKey::Duration, period.to_owned());
    let value = reader.market_read(request, ENDPOINT).await?;
    let records = required(&value, "rankings")?
        .as_array()
        .ok_or_else(invalid)?;
    if records.len() > 100 {
        return Err(invalid());
    }
    let mut rows = Vec::new();
    let mut previous = 0_usize;
    for record in records {
        let rank = usize::try_from(required(record, "rank")?.as_u64().ok_or_else(invalid)?)
            .map_err(|_error| invalid())?;
        if rank <= previous {
            return Err(invalid());
        }
        previous = rank;
        let id = SectorId::try_new(text(record, "sectorId")?).map_err(|_error| invalid())?;
        let name =
            DisplayInstrumentName::try_new(text(record, "name")?).map_err(|_error| invalid())?;
        let entry = catalog
            .sectors()
            .iter()
            .find(|sector| sector.id() == &id)
            .ok_or_else(invalid)?;
        if entry.depth() == SectorDepth::Minor || entry.name() != &name {
            return Err(invalid());
        }
        rows.push(SectorRankingRow {
            rank: RankingOrdinal::new(rank),
            sector: id,
            name,
            change_rate: optional_number(record, "changeRate")?,
            trading_amount: optional_number(record, "tradingAmount")?,
            market_capitalization: optional_number(record, "marketCapitalization")?,
            currency: crate::market_read::currency(text(record, "currency")?, ENDPOINT)?,
        });
    }
    SectorRanking::from_response(
        query.kind(),
        query.market(),
        query.duration(),
        rows,
        reader.market_now(),
    )
    .map_err(|_error| invalid())
}
