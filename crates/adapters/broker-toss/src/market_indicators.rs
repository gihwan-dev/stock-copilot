use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossAccountRead};
use application::{Clock, PortError, ReadEndpoint, RequestRejection, SecretStore};
use domain::{IndicatorCode, MarketIndicatorQuote, MasterCode, NumericInput};
use std::collections::{BTreeMap, BTreeSet};
const ENDPOINT: ReadEndpoint = ReadEndpoint::IndicatorQuotes;

pub(crate) async fn quotes<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    codes: &[IndicatorCode],
) -> Result<Vec<MarketIndicatorQuote>, PortError> {
    if codes.is_empty() || codes.len() > 200 {
        return Err(crate::market_read::rejected(
            ENDPOINT,
            RequestRejection::BatchLimitExceeded,
        ));
    }
    let requested = codes
        .iter()
        .copied()
        .map(code)
        .collect::<Result<Vec<_>, _>>()?;
    let unique = requested.iter().copied().collect::<BTreeSet<_>>();
    let value = reader
        .market_read(
            ReadRequest::new(ReadRoute::IndicatorQuotes).with_query(
                QueryKey::Symbols,
                unique.iter().copied().collect::<Vec<_>>().join(","),
            ),
            ENDPOINT,
        )
        .await?;
    let decoded: Vec<broker_toss_schema::types::MarketIndicatorPriceResponse> =
        serde_json::from_value(value).map_err(|_error| invalid())?;
    if decoded.len() != unique.len() {
        return Err(invalid());
    }
    let mut rows = BTreeMap::new();
    let now = reader.market_now();
    for item in decoded {
        if !unique.contains(item.symbol.as_str()) {
            return Err(invalid());
        }
        let name = item.symbol;
        let input = NumericInput::try_new(&item.last_price).map_err(|_error| invalid())?;
        let timestamp = item
            .timestamp
            .as_deref()
            .map(|stamp| crate::market_read::timestamp(stamp, ENDPOINT))
            .transpose()?;
        let quote = MarketIndicatorQuote::from_response(
            MasterCode::try_new(&name).map_err(|_error| invalid())?,
            &input,
            timestamp,
            now,
        )
        .map_err(|_error| invalid())?;
        if rows.insert(name, quote).is_some() {
            return Err(invalid());
        }
    }
    requested
        .into_iter()
        .map(|name| rows.get(name).cloned().ok_or_else(invalid))
        .collect()
}
fn invalid() -> PortError {
    crate::market_read::invalid(ENDPOINT)
}
pub(crate) fn code(value: IndicatorCode) -> Result<&'static str, PortError> {
    match value {
        IndicatorCode::Kospi => Ok("KOSPI"),
        IndicatorCode::Kosdaq => Ok("KOSDAQ"),
        IndicatorCode::KrBond2Y => Ok("KR_BOND_2Y"),
        IndicatorCode::KrBond3Y => Ok("KR_BOND_3Y"),
        IndicatorCode::KrBond5Y => Ok("KR_BOND_5Y"),
        IndicatorCode::KrBond10Y => Ok("KR_BOND_10Y"),
        IndicatorCode::KrBond20Y => Ok("KR_BOND_20Y"),
        IndicatorCode::KrBond30Y => Ok("KR_BOND_30Y"),
        IndicatorCode::Unknown => Err(crate::market_read::rejected(
            ENDPOINT,
            RequestRejection::UnsupportedQuery,
        )),
    }
}
