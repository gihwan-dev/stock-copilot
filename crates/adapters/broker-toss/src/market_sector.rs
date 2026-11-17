use crate::{QueryKey, ReadRequest, ReadRoute, RouteId, TokenRenewal, TossAccountRead};
use application::{Clock, PortError, ReadEndpoint, RequestRejection, SecretStore};
use domain::{
    DisplayInstrumentName, KnownMarket, MasterCode, NumericInput, Quantity, RankingDuration,
    SectorCatalog, SectorComposition, SectorDepth, SectorDetail, SectorEtfRow, SectorEtfs,
    SectorId, SectorMetadataRow, SectorRateRow, SectorStockRow, Symbol,
};
use serde_json::Value;
const MAX_ROWS: usize = 8192;

fn invalid(endpoint: ReadEndpoint) -> PortError {
    crate::market_read::invalid(endpoint)
}
fn required<'a>(
    value: &'a Value,
    key: &str,
    endpoint: ReadEndpoint,
) -> Result<&'a Value, PortError> {
    value.get(key).ok_or_else(|| invalid(endpoint))
}
fn items<'a>(
    value: &'a Value,
    key: &str,
    endpoint: ReadEndpoint,
) -> Result<&'a [Value], PortError> {
    let values = required(value, key, endpoint)?
        .as_array()
        .ok_or_else(|| invalid(endpoint))?;
    if values.len() > MAX_ROWS {
        return Err(invalid(endpoint));
    }
    Ok(values)
}
fn text<'a>(value: &'a Value, key: &str, endpoint: ReadEndpoint) -> Result<&'a str, PortError> {
    crate::market_read::text(value, key, endpoint)
}
fn optional_text<'a>(
    value: &'a Value,
    key: &str,
    endpoint: ReadEndpoint,
) -> Result<Option<&'a str>, PortError> {
    let item = required(value, key, endpoint)?;
    if item.is_null() {
        Ok(None)
    } else {
        item.as_str().map(Some).ok_or_else(|| invalid(endpoint))
    }
}
fn number(
    value: &Value,
    key: &str,
    endpoint: ReadEndpoint,
) -> Result<Option<NumericInput>, PortError> {
    optional_text(value, key, endpoint)?
        .map(|raw| NumericInput::try_new(raw).map_err(|_error| invalid(endpoint)))
        .transpose()
}
fn metadata(value: &Value, endpoint: ReadEndpoint) -> Result<SectorMetadataRow, PortError> {
    let id = response_id(text(value, "sectorId", endpoint)?, endpoint)?;
    let name = DisplayInstrumentName::try_new(text(value, "name", endpoint)?)
        .map_err(|_error| invalid(endpoint))?;
    let depth = match required(value, "depth", endpoint)?.as_i64() {
        Some(0) => SectorDepth::Major,
        Some(1) => SectorDepth::Middle,
        Some(2) => SectorDepth::Minor,
        _ => return Err(invalid(endpoint)),
    };
    let parent = optional_text(value, "parentSectorId", endpoint)?
        .map(|raw| response_id(raw, endpoint))
        .transpose()?;
    Ok(SectorMetadataRow {
        id,
        name,
        depth,
        parent,
    })
}
fn response_id(raw: &str, endpoint: ReadEndpoint) -> Result<SectorId, PortError> {
    let id = SectorId::try_new(raw).map_err(|_error| invalid(endpoint))?;
    route_id(&id, endpoint).map_err(|_error| invalid(endpoint))?;
    Ok(id)
}
fn market(market: KnownMarket) -> &'static str {
    match market {
        KnownMarket::Kr => "KR",
        KnownMarket::Us => "US",
    }
}
fn route_id(value: &SectorId, endpoint: ReadEndpoint) -> Result<RouteId, PortError> {
    if value.as_str().len() > 40
        || !value
            .as_str()
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err(crate::market_read::rejected(
            endpoint,
            RequestRejection::MalformedRequest,
        ));
    }
    RouteId::new(value.as_str()).map_err(|_error| {
        crate::market_read::rejected(endpoint, RequestRejection::MalformedRequest)
    })
}
pub(crate) async fn sectors<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
) -> Result<SectorCatalog, PortError> {
    let endpoint = ReadEndpoint::Sectors;
    let value = reader
        .market_read(ReadRequest::new(ReadRoute::Sectors), endpoint)
        .await?;
    let rows = items(&value, "sectors", endpoint)?
        .iter()
        .map(|row| metadata(row, endpoint))
        .collect::<Result<Vec<_>, _>>()?;
    SectorCatalog::from_response(rows, reader.market_now()).map_err(|_error| invalid(endpoint))
}
pub(crate) async fn detail<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    sector: &SectorId,
    market: KnownMarket,
) -> Result<SectorDetail, PortError> {
    let endpoint = ReadEndpoint::Sector;
    let route = ReadRoute::Sector(route_id(sector, endpoint)?);
    let value = reader
        .market_read(
            ReadRequest::new(route)
                .with_query(QueryKey::MarketCountry, self::market(market).to_owned()),
            endpoint,
        )
        .await?;
    let row = metadata(&value, endpoint)?;
    if &row.id != sector {
        return Err(invalid(endpoint));
    }
    let summary = optional_text(&value, "summary", endpoint)?
        .map(|raw| DisplayInstrumentName::try_new(raw).map_err(|_error| invalid(endpoint)))
        .transpose()?;
    let performance = required(&value, "performance", endpoint)?;
    let rates = items(performance, "changeRates", endpoint)?
        .iter()
        .map(|entry| {
            let duration = RankingDuration::from_code(
                &MasterCode::try_new(text(entry, "duration", endpoint)?)
                    .map_err(|_error| invalid(endpoint))?,
            );
            Ok(SectorRateRow {
                duration,
                change_rate: number(entry, "changeRate", endpoint)?,
            })
        })
        .collect::<Result<Vec<_>, PortError>>()?;
    SectorDetail::from_response(row, market, summary, rates, reader.market_now())
        .map_err(|_error| invalid(endpoint))
}
pub(crate) async fn composition<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    sector: &SectorId,
    market: KnownMarket,
) -> Result<SectorComposition, PortError> {
    let endpoint = ReadEndpoint::SectorComposition;
    let route = ReadRoute::SectorStocks(route_id(sector, endpoint)?);
    let value = reader
        .market_read(
            ReadRequest::new(route)
                .with_query(QueryKey::MarketCountry, self::market(market).to_owned()),
            endpoint,
        )
        .await?;
    let rows = items(&value, "stocks", endpoint)?
        .iter()
        .map(|entry| {
            DisplayInstrumentName::try_new(text(entry, "name", endpoint)?)
                .map_err(|_error| invalid(endpoint))?;
            if let Some(volume) = number(entry, "volume", endpoint)? {
                Quantity::parse(&volume).map_err(|_error| invalid(endpoint))?;
            }
            Ok(SectorStockRow {
                symbol: Symbol::try_new(text(entry, "symbol", endpoint)?)
                    .map_err(|_error| invalid(endpoint))?,
                currency: crate::market_read::currency(
                    text(entry, "currency", endpoint)?,
                    endpoint,
                )?,
                market_capitalization: number(entry, "marketCapitalization", endpoint)?,
                trading_amount: number(entry, "tradingAmount", endpoint)?,
            })
        })
        .collect::<Result<Vec<_>, PortError>>()?;
    SectorComposition::from_stocks_response(sector.clone(), market, rows, reader.market_now())
        .map_err(|_error| invalid(endpoint))
}
pub(crate) async fn etfs<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    sector: &SectorId,
    market: KnownMarket,
) -> Result<SectorEtfs, PortError> {
    let endpoint = ReadEndpoint::SectorEtfs;
    let route = ReadRoute::SectorEtfs(route_id(sector, endpoint)?);
    let value = reader
        .market_read(
            ReadRequest::new(route)
                .with_query(QueryKey::MarketCountry, self::market(market).to_owned()),
            endpoint,
        )
        .await?;
    let rows = items(&value, "etfs", endpoint)?
        .iter()
        .map(|entry| {
            Ok(SectorEtfRow {
                symbol: Symbol::try_new(text(entry, "symbol", endpoint)?)
                    .map_err(|_error| invalid(endpoint))?,
                name: DisplayInstrumentName::try_new(text(entry, "name", endpoint)?)
                    .map_err(|_error| invalid(endpoint))?,
            })
        })
        .collect::<Result<Vec<_>, PortError>>()?;
    for row in &rows {
        if !reader.market_master().instruments().iter().any(|master| {
            master.quote_instrument().is_ok_and(|instrument| {
                instrument.market() == &market && instrument.symbol() == &row.symbol
            }) && matches_etf_type(master)
        }) {
            return Err(invalid(endpoint));
        }
    }
    SectorEtfs::from_response(sector.clone(), market, rows, reader.market_now())
        .map_err(|_error| invalid(endpoint))
}

fn matches_etf_type(master: &domain::MasterInstrument) -> bool {
    master.row().security_type.known() == Some(domain::ExcludedSecurityType::Etf)
        || master.row().security_type.known() == Some(domain::ExcludedSecurityType::ForeignEtf)
}
