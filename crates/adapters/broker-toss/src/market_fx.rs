use crate::{QueryKey, ReadRequest, ReadRoute, TokenRenewal, TossAccountRead};
use application::{Clock, PortError, ReadEndpoint, SecretStore};
use domain::{
    Currency, FxPair, FxRate, FxRateKind, FxSource, FxValidity, NumericInput, Ratio, TimeInterval,
    Timestamp,
};

pub(crate) async fn fetch<S: SecretStore, T: Clock, R: TokenRenewal>(
    reader: &TossAccountRead<S, T, R>,
    pair: FxPair,
    at: Option<Timestamp>,
    endpoint: ReadEndpoint,
) -> Result<(FxRate, FxRate), PortError> {
    let mut request = ReadRequest::new(ReadRoute::FxRate)
        .with_query(QueryKey::BaseCurrency, currency(pair.base()).to_owned())
        .with_query(QueryKey::QuoteCurrency, currency(pair.quote()).to_owned());
    if let Some(value) = at {
        request = request.with_query(
            QueryKey::DateTime,
            crate::market_read::candles::format_time(value)?,
        );
    }
    let body = reader.market_read(request, endpoint).await?;
    let decoded: broker_toss_schema::types::ExchangeRateResponse =
        serde_json::from_value(body).map_err(|_error| crate::market_read::invalid(endpoint))?;
    if decoded.base_currency.to_string() != currency(pair.base())
        || decoded.quote_currency.to_string() != currency(pair.quote())
    {
        return Err(crate::market_read::invalid(endpoint));
    }
    let start = crate::market_read::timestamp(&decoded.valid_from, endpoint)?;
    let end = crate::market_read::timestamp(&decoded.valid_until, endpoint)?;
    let validity = TimeInterval::try_new(start, end)
        .map_err(|_error| crate::market_read::invalid(endpoint))?;
    let parse = |text: &str, kind| {
        let number =
            NumericInput::try_new(text).map_err(|_error| crate::market_read::invalid(endpoint))?;
        FxRate::parse(&number, pair, kind, validity, FxSource::TossOpenApi)
            .map_err(|_error| crate::market_read::invalid(endpoint))
    };
    let buy = parse(&decoded.rate, FxRateKind::Buy)?;
    let mid = parse(&decoded.mid_rate, FxRateKind::Mid)?;
    NumericInput::try_new(&decoded.basis_point)
        .and_then(|value| Ratio::parse(&value))
        .map_err(|_error| crate::market_read::invalid(endpoint))?;
    if buy.validity_at(at.unwrap_or_else(|| reader.market_now())) != FxValidity::Valid {
        return Err(crate::market_read::invalid(endpoint));
    }
    Ok((buy, mid))
}
fn currency(value: Currency) -> &'static str {
    match value {
        Currency::Krw => "KRW",
        Currency::Usd => "USD",
    }
}
