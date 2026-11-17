pub(crate) mod candles;
use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossAccountRead, TossTimestamp};
use application::{
    Clock, HttpStatus, PortError, PortErrorKind, QuoteObservation, ReadEndpoint, RequestRejection,
    ResponseMismatch, SecretStore,
};
pub(crate) use candles::candles;
use domain::{Currency, NumericInput, Price, Quote, QuoteInstrument, Timestamp};
use serde_json::Value;

pub(crate) fn invalid(endpoint: ReadEndpoint) -> PortError {
    crate::account_read::invalid(endpoint, ResponseMismatch::Validation)
}
pub(crate) fn rejected(endpoint: ReadEndpoint, reason: RequestRejection) -> PortError {
    PortError::new(
        endpoint,
        HttpStatus::NotReceived,
        PortErrorKind::InvalidRequest(reason),
    )
}
pub(crate) fn timestamp(text: &str, endpoint: ReadEndpoint) -> Result<Timestamp, PortError> {
    TossTimestamp::try_from(text)
        .map(TossTimestamp::into_domain)
        .map_err(|_error| invalid(endpoint))
}
pub(crate) fn currency(text: &str, endpoint: ReadEndpoint) -> Result<Currency, PortError> {
    match text {
        "KRW" => Ok(Currency::Krw),
        "USD" => Ok(Currency::Usd),
        _ => Err(invalid(endpoint)),
    }
}
pub(crate) fn price(
    text: &str,
    unit: Currency,
    endpoint: ReadEndpoint,
) -> Result<Price, PortError> {
    NumericInput::try_new(text)
        .and_then(|value| Price::parse(&value, unit))
        .map_err(|_error| invalid(endpoint))
}
pub(crate) fn text<'a>(
    value: &'a Value,
    key: &str,
    endpoint: ReadEndpoint,
) -> Result<&'a str, PortError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(endpoint))
}
pub(crate) async fn quotes<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    instruments: &[QuoteInstrument],
) -> Result<Vec<QuoteObservation>, PortError> {
    if instruments.is_empty() || instruments.len() > 200 {
        return Err(rejected(
            ReadEndpoint::Quotes,
            RequestRejection::BatchLimitExceeded,
        ));
    }
    let symbols = instruments
        .iter()
        .map(|item| item.symbol().as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(",");
    let value = reader
        .market_read(
            ReadRequest::new(ReadRoute::Quotes).with_query(QueryKey::Symbols, symbols),
            ReadEndpoint::Quotes,
        )
        .await?;
    let rows = value
        .as_array()
        .ok_or_else(|| invalid(ReadEndpoint::Quotes))?;
    let mut identified = std::collections::BTreeMap::new();
    for row in rows {
        let symbol = text(row, "symbol", ReadEndpoint::Quotes)?;
        if !instruments
            .iter()
            .any(|item| item.symbol().as_str() == symbol)
            || identified.insert(symbol, row).is_some()
        {
            return Err(invalid(ReadEndpoint::Quotes));
        }
    }
    let observed = reader.market_now();
    Ok(instruments
        .iter()
        .map(|instrument| QuoteObservation {
            instrument: instrument.clone(),
            result: identified
                .get(instrument.symbol().as_str())
                .ok_or_else(|| invalid(ReadEndpoint::Quotes))
                .and_then(|row| quote(row, instrument, observed)),
        })
        .collect())
}
fn quote(
    value: &Value,
    instrument: &QuoteInstrument,
    observed: Timestamp,
) -> Result<Quote, PortError> {
    let endpoint = ReadEndpoint::Quotes;
    let decoded: broker_toss_schema::types::PriceResponse =
        serde_json::from_value(value.clone()).map_err(|_error| invalid(endpoint))?;
    let unit = currency(&decoded.currency.to_string(), endpoint)?;
    if unit != instrument.currency() {
        return Err(invalid(endpoint));
    }
    let last = price(&decoded.last_price, unit, endpoint)?;
    let stamp = decoded
        .timestamp
        .as_deref()
        .map(|stamp| timestamp(stamp, endpoint))
        .transpose()?;
    Quote::try_new(
        instrument.symbol().clone(),
        unit,
        Some(last),
        stamp,
        observed,
    )
    .map_err(|_error| invalid(endpoint))
}
