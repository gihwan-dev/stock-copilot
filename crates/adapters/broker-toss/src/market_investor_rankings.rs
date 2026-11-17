use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossAccountRead};
use application::{Clock, PortError, ReadEndpoint, SecretStore};
use domain::{
    InvestorGroupRow, InvestorTradingRankings, NetTradingRow, NumericInput, RankingLimit,
    RankingOrdinal, Symbol,
};
use serde_json::Value;
const ENDPOINT: ReadEndpoint = ReadEndpoint::InvestorTradingRankings;
fn invalid() -> PortError {
    crate::market_read::invalid(ENDPOINT)
}
fn required<'a>(value: &'a Value, field: &str) -> Result<&'a Value, PortError> {
    value.get(field).ok_or_else(invalid)
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, PortError> {
    crate::market_read::text(value, field, ENDPOINT)
}
fn list(
    value: &Value,
    name: &str,
    amount_key: &str,
    limit: RankingLimit,
) -> Result<Vec<NetTradingRow>, PortError> {
    let rows = required(value, name)?.as_array().ok_or_else(invalid)?;
    if rows.len() > *limit.as_usize() {
        return Err(invalid());
    }
    let mut result = Vec::new();
    let mut previous = 0_usize;
    for item in rows {
        let rank = usize::try_from(required(item, "rank")?.as_u64().ok_or_else(invalid)?)
            .map_err(|_error| invalid())?;
        if rank <= previous {
            return Err(invalid());
        }
        previous = rank;
        result.push(NetTradingRow {
            rank: RankingOrdinal::new(rank),
            symbol: Symbol::try_new(text(item, "symbol")?).map_err(|_error| invalid())?,
            currency: crate::market_read::currency(text(item, "currency")?, ENDPOINT)?,
            amount: NumericInput::try_new(text(item, amount_key)?).map_err(|_error| invalid())?,
        });
    }
    Ok(result)
}
fn group(value: &Value, limit: RankingLimit) -> Result<InvestorGroupRow, PortError> {
    let stamp = required(value, "rankedAt")?;
    let ranked_at = if stamp.is_null() {
        None
    } else {
        Some(crate::market_read::timestamp(
            stamp.as_str().ok_or_else(invalid)?,
            ENDPOINT,
        )?)
    };
    Ok(InvestorGroupRow {
        ranked_at,
        net_buy: list(value, "netBuyRankings", "netBuyAmount", limit)?,
        net_sell: list(value, "netSellRankings", "netSellAmount", limit)?,
    })
}
pub(crate) async fn fetch<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    limit: RankingLimit,
) -> Result<InvestorTradingRankings, PortError> {
    let value = reader
        .market_read(
            ReadRequest::new(ReadRoute::InvestorRankings)
                .with_query(QueryKey::Count, limit.as_usize().to_string()),
            ENDPOINT,
        )
        .await?;
    InvestorTradingRankings::from_response(
        group(required(&value, "foreigner")?, limit)?,
        group(required(&value, "institution")?, limit)?,
        group(required(&value, "individual")?, limit)?,
        limit,
        reader.market_now(),
    )
    .map_err(|_error| invalid())
}
